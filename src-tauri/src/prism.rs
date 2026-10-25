//! Импорт инстансов MultiMC / Prism Launcher.
//!
//! Инстанс — это папка с `mmc-pack.json` (компоненты: версия Minecraft и
//! загрузчик) и игровым каталогом `minecraft/` или `.minecraft/`.

use crate::error::{err, AppError, Res};
use crate::models::{default_java, Instance, Loader};
use crate::{paths, store};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Читает `mmc-pack.json`: версию Minecraft и загрузчик.
fn read_pack(root: &Path) -> Res<(String, Loader, Option<String>)> {
    let text = std::fs::read_to_string(root.join("mmc-pack.json")).map_err(|_| {
        AppError::Msg("Не найден mmc-pack.json — это не инстанс MultiMC/Prism".into())
    })?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let comps = v
        .get("components")
        .and_then(|c| c.as_array())
        .ok_or_else(|| AppError::Msg("В mmc-pack.json нет components".into()))?;

    let mut mc: Option<String> = None;
    let mut loader = Loader::Vanilla;
    let mut loader_ver: Option<String> = None;
    for c in comps {
        let uid = c.get("uid").and_then(|x| x.as_str()).unwrap_or("");
        let ver = c.get("version").and_then(|x| x.as_str()).unwrap_or("");
        match uid {
            "net.minecraft" => mc = Some(ver.to_string()),
            "net.fabricmc.fabric-loader" => {
                loader = Loader::Fabric;
                loader_ver = Some(ver.into());
            }
            "net.minecraftforge" => {
                loader = Loader::Forge;
                loader_ver = Some(ver.into());
            }
            "net.neoforged" => {
                loader = Loader::Neoforge;
                loader_ver = Some(ver.into());
            }
            "org.quiltmc.quilt-loader" => {
                loader = Loader::Quilt;
                loader_ver = Some(ver.into());
            }
            _ => {}
        }
    }
    let mc = mc.ok_or_else(|| AppError::Msg("В mmc-pack.json нет net.minecraft".into()))?;
    Ok((mc, loader, loader_ver))
}

/// Имя из `instance.cfg` (строка `name=...`) или имя папки.
fn read_name(root: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(root.join("instance.cfg")) {
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("name=") {
                let v = v.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "MultiMC".into())
}

/// Рекурсивно копирует каталог (без символических ссылок).
async fn copy_dir(src: &Path, dst: &Path) -> Res<()> {
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((s, d)) = stack.pop() {
        tokio::fs::create_dir_all(&d).await?;
        let mut rd = tokio::fs::read_dir(&s).await?;
        while let Some(e) = rd.next_entry().await? {
            let ft = e.file_type().await?;
            let sp = e.path();
            let dp = d.join(e.file_name());
            if ft.is_dir() {
                stack.push((sp, dp));
            } else if ft.is_file() {
                tokio::fs::copy(&sp, &dp).await?;
            }
        }
    }
    Ok(())
}

/// Импортирует инстанс и возвращает созданный профиль.
pub async fn import(dir: &str) -> Res<Instance> {
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        return err("Папка не найдена");
    }
    let (mc, loader, loader_ver) = read_pack(&root)?;
    let name = read_name(&root);

    let id = Uuid::new_v4().simple().to_string();
    let biomes = ["plains", "taiga", "desert", "nether", "snow"];
    let biome = biomes[id.as_bytes()[0] as usize % biomes.len()].to_string();

    let instance = Instance {
        id: id.clone(),
        name,
        mc_version: mc.clone(),
        loader,
        loader_version: loader_ver,
        biome,
        icon: None,
        icon_preset: None,
        wallpaper: None,
        ram_min_mb: 1024,
        ram_max_mb: 4096,
        java_major: default_java(&mc),
        java_path: None,
        jvm_args: String::new(),
        width: 1280,
        height: 720,
        fullscreen: false,
        playtime_sec: 0,
        playtime_days: std::collections::BTreeMap::new(),
        last_played: None,
        scanner_mode: None,
        created: chrono::Utc::now().timestamp(),
        pack: None,
        game_dir: None,
    };

    let dst = paths::instance_minecraft_dir(&id);
    tokio::fs::create_dir_all(&dst).await?;
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(dst.join(sub)).await?;
    }

    // У Prism игровая папка `minecraft/`, у старых MultiMC — `.minecraft/`.
    let game = if root.join("minecraft").is_dir() {
        root.join("minecraft")
    } else {
        root.join(".minecraft")
    };
    if game.is_dir() {
        copy_dir(&game, &dst).await?;
    }

    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}
