//! Своя папка игры у профиля (`.minecraft`).
//!
//! Код игры и весь контент (моды, миры, скриншоты) ищут папку по одному пути —
//! `instances/<id>/minecraft`. Чтобы не менять это в десятке мест, при выборе
//! другой папки мы кладём туда ссылку: junction на Windows (не требует прав
//! администратора и работает между дисками), symlink на macOS и Linux.
//! Дальше всё работает как обычно, а данные игрока лежат там, где он выбрал.

use crate::error::{err, Res};
use crate::models::Instance;
use crate::{paths, store};
use std::path::{Path, PathBuf};

/// Привязывает папку игры к профилю.
pub async fn attach(link: &Path, target: &Path) -> Res<()> {
    if let Some(parent) = link.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(["/C", "mklink", "/J"]).arg(link).arg(target);
        crate::winproc::hide_console(&mut cmd);
        let out = cmd.output().await?;
        if !out.status.success() {
            let e = String::from_utf8_lossy(&out.stderr);
            return err(format!("Не удалось привязать папку игры: {}", e.trim()));
        }
    }
    #[cfg(not(windows))]
    {
        tokio::fs::symlink(target, link).await?;
    }
    Ok(())
}

/// Снимает ссылку, не трогая саму папку с игрой.
pub async fn unlink(link: &Path) -> Res<()> {
    let meta = match tokio::fs::symlink_metadata(link).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if meta.file_type().is_symlink() {
        tokio::fs::remove_file(link).await?;
    } else {
        #[cfg(windows)]
        tokio::fs::remove_dir(link).await?;
        #[cfg(not(windows))]
        tokio::fs::remove_dir_all(link).await?;
    }
    Ok(())
}

/// Пустая папка игры профиля: на случай сброса или ручного удаления ссылки.
pub async fn ensure_profile(id: &str) -> Res<()> {
    let dir = paths::instance_minecraft_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(dir.join(sub)).await?;
    }
    Ok(())
}

/// Отвязывает папку перед удалением профиля, чтобы не стереть данные игрока.
pub async fn detach(inst: &Instance) {
    if inst.game_dir.is_some() {
        let _ = unlink(&paths::instance_minecraft_dir(&inst.id)).await;
    }
}

/// Меняет папку игры: `None` — снова папка профиля.
pub async fn switch(inst: &mut Instance, path: Option<&str>) -> Res<()> {
    let link = paths::instance_minecraft_dir(&inst.id);
    let custom = path.map(str::trim).filter(|p| !p.is_empty());

    if is_reset(&link, custom)? {
        if inst.game_dir.is_some() {
            unlink(&link).await?;
        }
        ensure_profile(&inst.id).await?;
        inst.game_dir = None;
        return Ok(());
    }

    let target = PathBuf::from(custom.unwrap_or_default());
    tokio::fs::create_dir_all(&target).await?;
    if is_inside_profile(inst, &target) {
        let _ = ensure_profile(&inst.id).await;
        return err("Выберите папку вне профиля лаунчера");
    }

    if inst.game_dir.is_some() {
        unlink(&link).await?;
    } else if tokio::fs::metadata(&link).await.map(|m| m.is_dir()).unwrap_or(false) {
        take_old_folder(inst, &link, &target).await?;
    }

    if let Err(e) = attach(&link, &target).await {
        // Профиль не должен остаться без папки игры.
        ensure_profile(&inst.id).await?;
        inst.game_dir = None;
        let _ = store::write_instance(inst).await;
        return Err(e);
    }
    inst.game_dir = Some(target.to_string_lossy().to_string());
    Ok(())
}

/// Выбрана папка самого профиля — это не смена, а сброс.
fn is_reset(link: &Path, custom: Option<&str>) -> Res<bool> {
    let Some(raw) = custom else {
        return Ok(true);
    };
    if raw.contains("..") {
        return err("Некорректный путь к папке игры");
    }
    let p = Path::new(raw);
    if !p.is_absolute() {
        return err("Укажите полный путь к папке игры");
    }
    let same = std::fs::canonicalize(p).ok() == std::fs::canonicalize(link).ok();
    Ok(same)
}

/// Ссылку внутрь профиля не кладём: получится рекурсия.
fn is_inside_profile(inst: &Instance, target: &Path) -> bool {
    let dir = paths::instance_dir(&inst.id);
    let target = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    target.starts_with(dir)
}

/// Убирает старую папку профиля: пустую — удаляет, с данными — отодвигает
/// в `minecraft.old-<дата>`, чтобы ничего не потерять.
async fn take_old_folder(inst: &Instance, link: &Path, target: &Path) -> Res<()> {
    let empty = std::fs::read_dir(target)?.next().is_none();
    if empty {
        let (from, to) = (link.to_path_buf(), target.to_path_buf());
        tokio::task::spawn_blocking(move || move_children(&from, &to)).await??;
    }
    if tokio::fs::remove_dir(link).await.is_err() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let backup = paths::instance_dir(&inst.id).join(format!("minecraft.old-{stamp}"));
        tokio::fs::rename(link, &backup).await?;
    }
    Ok(())
}

/// Переносит содержимое старой папки игры в новую.
fn move_children(from: &Path, to: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if dst.exists() {
            continue;
        }
        let src = e.path();
        if std::fs::rename(&src, &dst).is_err() {
            copy_recursive(&src, &dst)?;
            if src.is_dir() {
                std::fs::remove_dir_all(&src)?;
            } else {
                std::fs::remove_file(&src)?;
            }
        }
    }
    Ok(())
}

fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}
