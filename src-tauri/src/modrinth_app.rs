//! Импорт профилей из Modrinth App.
//!
//! Modrinth App держит профили в `%APPDATA%\ModrinthApp\profiles\<папка>`.
//! Метаданные (версия игры и загрузчик) в новых версиях лежат в SQLite
//! (`app.db`), но их надёжно даёт и шапка `logs/latest.log`
//! («Loading Minecraft 1.21.11 with Fabric Loader 0.19.3»). Иконка, если
//! есть, лежит файлом `icon.png` в самой папке профиля.

use crate::error::{err, Res};
use crate::models::{default_java, Instance, Loader};
use crate::{paths, store};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use uuid::Uuid;

static RE_FABRIC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Loading Minecraft (\S+) with Fabric Loader (\S+)").unwrap());
static RE_QUILT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Loading Minecraft (\S+) with Quilt Loader (\S+)").unwrap());
static RE_FORGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Loading Minecraft (\S+) with (?:NeoForge|Forge|Minecraft Forge) (\S+)").unwrap()
});
static RE_MC: Lazy<Regex> = Lazy::new(|| Regex::new(r"Loading Minecraft (\S+)").unwrap());
static RE_MCVER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Minecraft Version:\s*([0-9][\w.\-]*)").unwrap());
static RE_FORGEVER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:NeoForge|Forge) version[:\s]+([0-9][\w.\-]*)").unwrap());
static RE_DIRNAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(fabric|forge|neoforge|quilt|vanilla)\s+([0-9][\w.\-]*)$").unwrap());

/// Профиль Modrinth App для окна импорта.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MrProfile {
    /// Отображаемое имя профиля.
    pub name: String,
    /// Папка профиля (полный путь).
    pub path: String,
    /// Версия Minecraft.
    pub mc_version: String,
    /// Загрузчик: `fabric` | `forge` | `neoforge` | `quilt` | `vanilla`.
    pub loader: String,
    /// Версия загрузчика, если известна.
    pub loader_version: Option<String>,
    /// Сколько модов в `mods/`.
    pub mods: usize,
    /// Есть ли своя иконка.
    pub has_icon: bool,
}

/// Каталог с профилями Modrinth App или `None`, если приложение не найдено.
pub fn profiles_root() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("ModrinthApp").join("profiles");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Список профилей Modrinth App.
pub fn list() -> Res<Vec<MrProfile>> {
    let Some(root) = profiles_root() else {
        return Ok(Vec::new());
    };
    let mut out: Vec<MrProfile> = Vec::new();
    for e in std::fs::read_dir(&root)?.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let (mc, loader, loader_ver) = read_info(&dir, &name);
        let mods = count_mods(&dir);
        out.push(MrProfile {
            name,
            path: dir.to_string_lossy().to_string(),
            mc_version: mc,
            loader: loader_key(loader).to_string(),
            loader_version: loader_ver,
            mods,
            has_icon: find_icon(&dir).is_some(),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Импортирует профиль Modrinth App и возвращает созданный профиль лаунчера.
pub async fn import(path: &str) -> Res<Instance> {
    let root = profiles_root().ok_or_else(|| {
        crate::error::AppError::Msg("Modrinth App не найден на этом компьютере".into())
    })?;
    let dir = PathBuf::from(path);
    if !dir.is_dir() {
        return err("Папка профиля не найдена");
    }
    // Разрешаем импортировать только из каталога Modrinth App.
    let dir = dir
        .canonicalize()
        .map_err(|_| crate::error::AppError::Msg("Папка профиля не найдена".into()))?;
    let root = root.canonicalize().unwrap_or(root);
    if !dir.starts_with(&root) {
        return err("Можно импортировать только профили из Modrinth App");
    }

    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Modrinth".into());
    let (mc, loader, loader_ver) = read_info(&dir, &name);

    let id = Uuid::new_v4().simple().to_string();
    let biomes = ["plains", "taiga", "desert", "nether", "snow"];
    let biome = biomes[id.as_bytes()[0] as usize % biomes.len()].to_string();

    let mut instance = Instance {
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
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(dst.join(sub)).await?;
    }

    copy_game(&dir, &dst).await?;

    // Иконка профиля, если она есть, становится иконкой инстанса.
    if let Some(icon) = find_icon(&dir) {
        let target = paths::instance_icon(&id, "icon.png");
        if tokio::fs::copy(&icon, &target).await.is_ok() {
            instance.icon = Some("icon.png".to_string());
        }
    }

    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

/// Иконка профиля (`icon.png` / `icon.jpg`) в самой папке.
fn find_icon(dir: &Path) -> Option<PathBuf> {
    for name in ["icon.png", "icon.jpg", "icon.jpeg"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Число jar-ов в `mods/`.
fn count_mods(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("mods"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("jar"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Версия игры и загрузчик: из `profile.json`, из шапки лога либо из имени папки.
fn read_info(dir: &Path, name: &str) -> (String, Loader, Option<String>) {
    if let Some(v) = read_profile_json(dir) {
        return v;
    }
    if let Some(v) = read_log(dir) {
        return v;
    }
    if let Some(c) = RE_DIRNAME.captures(name) {
        let loader = loader_from(&c[1].to_lowercase());
        return (c[2].to_string(), loader, None);
    }
    ("1.21.1".to_string(), Loader::Vanilla, None)
}

/// Старый формат Modrinth App: `profile.json` рядом с игрой.
fn read_profile_json(dir: &Path) -> Option<(String, Loader, Option<String>)> {
    let text = std::fs::read_to_string(dir.join("profile.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mc = v.get("game_version").and_then(|x| x.as_str())?.to_string();
    let loader = loader_from(v.get("loader").and_then(|x| x.as_str()).unwrap_or("vanilla"));
    let lv = match v.get("loader_version") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(o)) => {
            o.get("id").and_then(|x| x.as_str()).map(|s| s.to_string())
        }
        _ => None,
    };
    Some((mc, loader, lv))
}

/// Новый формат: метаданные достаём из шапки `latest.log`.
fn read_log(dir: &Path) -> Option<(String, Loader, Option<String>)> {
    let path = dir.join("logs").join("latest.log");
    let bytes = std::fs::read(&path).ok()?;
    let slice = &bytes[..bytes.len().min(400_000)];
    let end = std::str::from_utf8(slice)
        .map(|_| slice.len())
        .unwrap_or_else(|e| e.valid_up_to());
    let text = String::from_utf8_lossy(&slice[..end]);

    if let Some(c) = RE_FABRIC.captures(&text) {
        return Some((c[1].to_string(), Loader::Fabric, Some(c[2].to_string())));
    }
    if let Some(c) = RE_QUILT.captures(&text) {
        return Some((c[1].to_string(), Loader::Quilt, Some(c[2].to_string())));
    }
    if let Some(c) = RE_FORGE.captures(&text) {
        let loader = if c[0].to_lowercase().contains("neoforge") {
            Loader::Neoforge
        } else {
            Loader::Forge
        };
        return Some((c[1].to_string(), loader, Some(c[2].to_string())));
    }
    // Fallback: строка «Minecraft Version:» встречается и у Forge.
    if let Some(c) = RE_MCVER.captures(&text) {
        let mc = c[1].to_string();
        if let Some(f) = RE_FORGEVER.captures(&text) {
            let loader = if f[0].to_lowercase().contains("neoforge") {
                Loader::Neoforge
            } else {
                Loader::Forge
            };
            return Some((mc, loader, Some(f[1].to_string())));
        }
        return Some((mc, Loader::Vanilla, None));
    }
    if let Some(c) = RE_MC.captures(&text) {
        return Some((c[1].to_string(), Loader::Vanilla, None));
    }
    None
}

fn loader_from(s: &str) -> Loader {
    match s.to_ascii_lowercase().as_str() {
        "fabric" => Loader::Fabric,
        "forge" => Loader::Forge,
        "neoforge" => Loader::Neoforge,
        "quilt" => Loader::Quilt,
        _ => Loader::Vanilla,
    }
}

/// Строковый ключ загрузчика — как он ходит во фронтенд.
fn loader_key(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Quilt => "quilt",
    }
}

/// Копирует игровой каталог профиля, пропуская логи и служебные файлы.
async fn copy_game(src: &Path, dst: &Path) -> Res<()> {
    const SKIP_DIRS: [&str; 4] = ["logs", "crash-reports", "downloads", "debug"];
    const SKIP_FILES: [&str; 4] = [
        "profile.json",
        "debug-profile.json",
        "command_history.txt",
        "usercache.json",
    ];

    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((s, d)) = stack.pop() {
        tokio::fs::create_dir_all(&d).await?;
        let mut rd = tokio::fs::read_dir(&s).await?;
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().to_string();
            let low = name.to_lowercase();
            let sp = e.path();
            let dp = d.join(&name);
            let ft = e.file_type().await?;
            if ft.is_dir() {
                if SKIP_DIRS.contains(&low.as_str()) {
                    continue;
                }
                stack.push((sp, dp));
            } else if ft.is_file() {
                if SKIP_FILES.contains(&low.as_str())
                    || low.starts_with("hs_err_pid")
                    || low.ends_with(".lock")
                {
                    continue;
                }
                tokio::fs::copy(&sp, &dp).await?;
            }
        }
    }
    Ok(())
}
