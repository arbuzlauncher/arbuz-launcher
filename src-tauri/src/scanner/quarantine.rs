//! Карантин (параноидальный режим): заблокированные моды переносятся в
//! `<instance>/quarantine/` и не удаляются.

use crate::error::{err, Res};
use std::path::Path;

/// Имя файла в карантине не должно выходить за пределы папки.
fn safe_name(name: &str) -> Res<String> {
    let n = name.trim();
    if n.is_empty()
        || n.len() > 200
        || n.contains(['/', '\\', ':', '\0'])
        || n == "."
        || n == ".."
        || n.starts_with('.')
    {
        return err("Некорректное имя файла");
    }
    Ok(n.to_string())
}

/// Переносит файл в папку карантина. Если имя занято — добавляет суффикс.
/// Возвращает имя файла в карантине.
pub(crate) fn move_file(src: &Path, quarantine_dir: &Path, name: &str) -> Res<String> {
    let name = safe_name(name)?;
    std::fs::create_dir_all(quarantine_dir)?;
    let mut dst = quarantine_dir.join(&name);
    let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("mod");
    let ext = Path::new(&name).extension().and_then(|s| s.to_str()).unwrap_or("jar");
    let mut i = 1u32;
    while dst.exists() {
        if i > 999 {
            return err("В карантине уже есть такой файл");
        }
        dst = quarantine_dir.join(format!("{stem}.{i}.{ext}"));
        i += 1;
    }
    std::fs::rename(src, &dst)?;
    Ok(dst.file_name().and_then(|s| s.to_str()).unwrap_or(&name).to_string())
}

/// Список файлов в карантине.
pub(crate) fn list(quarantine_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(quarantine_dir) else {
        return out;
    };
    for e in rd.flatten() {
        if e.path().is_file() {
            if let Some(n) = e.file_name().to_str() {
                out.push(n.to_string());
            }
        }
    }
    out.sort();
    out
}

/// Возвращает файл из карантина в указанную папку.
pub(crate) fn restore(quarantine_dir: &Path, mods_dir: &Path, name: &str) -> Res<()> {
    let name = safe_name(name)?;
    let src = quarantine_dir.join(&name);
    if !src.is_file() {
        return err("Файла нет в карантине");
    }
    std::fs::create_dir_all(mods_dir)?;
    std::fs::rename(&src, mods_dir.join(&name))?;
    Ok(())
}

/// Окончательно удаляет файл из карантина.
pub(crate) fn remove(quarantine_dir: &Path, name: &str) -> Res<()> {
    let name = safe_name(name)?;
    let src = quarantine_dir.join(&name);
    if !src.is_file() {
        return err("Файла нет в карантине");
    }
    std::fs::remove_file(&src)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arbuz-quar-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn move_and_restore_round_trip() {
        let dir = tmp("rt");
        let mods = dir.join("mods");
        let q = dir.join("quarantine");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("evil.jar"), b"jar").unwrap();

        let name = move_file(&mods.join("evil.jar"), &q, "evil.jar").expect("move");
        assert_eq!(name, "evil.jar");
        assert!(!mods.join("evil.jar").exists());
        assert!(q.join("evil.jar").is_file());
        assert_eq!(list(&q), vec!["evil.jar".to_string()]);

        restore(&q, &mods, "evil.jar").expect("restore");
        assert!(mods.join("evil.jar").is_file());
        assert!(list(&q).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn name_collision_gets_suffix() {
        let dir = tmp("col");
        let q = dir.join("q");
        std::fs::create_dir_all(&q).unwrap();
        std::fs::write(dir.join("a.jar"), b"1").unwrap();
        std::fs::write(dir.join("b.jar"), b"2").unwrap();
        let first = move_file(&dir.join("a.jar"), &q, "a.jar").unwrap();
        let second = move_file(&dir.join("b.jar"), &q, "a.jar").unwrap();
        assert_eq!(first, "a.jar");
        assert_ne!(second, "a.jar");
        assert!(q.join(&second).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn traversal_names_rejected() {
        let dir = tmp("tr");
        assert!(safe_name("../evil.jar").is_err());
        assert!(safe_name("C:\\x").is_err());
        assert!(safe_name("").is_err());
        assert!(safe_name("normal-mod.jar").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
