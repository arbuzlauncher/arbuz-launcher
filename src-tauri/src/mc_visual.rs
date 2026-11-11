//! Динамический визуал Minecraft: панорама и текстуры кнопок из jar версии.
//!
//! Доступен плагинам через `arbuz.mc.visual(...)`, но работает и без Python —
//! как обычная Rust-функция. Все извлечённые файлы кэшируются в
//! `<data>/cache/mc-visual/<version>/`, поэтому повторный выбор версии
//! мгновенный.

use crate::error::Res;
use crate::paths;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

const PANORAMA_FACES: [&str; 6] = [
    "assets/minecraft/textures/gui/title/background/panorama_0.png",
    "assets/minecraft/textures/gui/title/background/panorama_1.png",
    "assets/minecraft/textures/gui/title/background/panorama_2.png",
    "assets/minecraft/textures/gui/title/background/panorama_3.png",
    "assets/minecraft/textures/gui/title/background/panorama_4.png",
    "assets/minecraft/textures/gui/title/background/panorama_5.png",
];

const PANORAMA_SINGLE: &[&str] = &[
    "assets/minecraft/textures/gui/title/background/panorama.png",
    "assets/minecraft/textures/gui/title/background/panorama_0.png",
];

const BUTTONS: &[(&str, &[&str])] = &[
    (
        "normal",
        &[
            "assets/minecraft/textures/gui/sprites/widget/button.png",
            "assets/minecraft/textures/gui/widget/button.png",
            "assets/minecraft/textures/gui/widgets.png",
        ],
    ),
    (
        "hover",
        &[
            "assets/minecraft/textures/gui/sprites/widget/button_highlighted.png",
            "assets/minecraft/textures/gui/widget/button_highlighted.png",
        ],
    ),
    (
        "disabled",
        &[
            "assets/minecraft/textures/gui/sprites/widget/button_disabled.png",
            "assets/minecraft/textures/gui/widget/button_disabled.png",
        ],
    ),
];

/// Читает `instance.json` без async — нужно и Python-плагинам.
fn instance_version(id: &str) -> Res<String> {
    paths::validate_instance_id(id)?;
    let raw = std::fs::read_to_string(paths::instance_json(id))?;
    let v: Value = serde_json::from_str(&raw)?;
    Ok(v.get("mcVersion")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string())
}

fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, dir: &Path) -> Option<String> {
    let mut entry = zip.by_name(name).ok()?;
    let file_name = Path::new(name).file_name()?.to_string_lossy().to_string();
    let out = dir.join(&file_name);
    if !out.is_file() {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).ok()?;
        std::fs::create_dir_all(dir).ok()?;
        std::fs::write(&out, &buf).ok()?;
    }
    Some(out.to_string_lossy().to_string())
}

fn version_accent(version: &str) -> &'static str {
    let v = version.trim_start_matches("1.").split('.').next().unwrap_or("");
    match v {
        "21" | "20" => "#e6a3bf", // вишнёвый сад
        "19" => "#8ab4a0",
        "18" => "#c9a86a",
        "17" => "#79a86b",
        "16" => "#b34a35", // Нижний мир
        "15" | "14" | "13" => "#6aa84f",
        _ => "#5fb339",
    }
}

fn title_for(version: &str) -> String {
    if version.is_empty() {
        "Minecraft".into()
    } else {
        format!("Minecraft {version}")
    }
}

/// Извлекает визуал для профиля. Возвращает JSON, пригодный для фронтенда.
pub fn extract_sync(id: &str) -> Res<Value> {
    let version = instance_version(id)?;
    if version.is_empty() {
        return Ok(json!({
            "version": "",
            "title": "Minecraft",
            "panorama": [],
            "button": {},
            "accent": "#5fb339",
            "error": "У профиля не выбрана версия игры",
        }));
    }
    let jar = paths::version_jar(&version);
    if !jar.is_file() {
        return Ok(json!({
            "version": version,
            "title": title_for(&version),
            "panorama": [],
            "button": {},
            "accent": version_accent(&version),
            "error": "Клиентский jar ещё не скачан — запустите игру один раз",
        }));
    }

    let cache = paths::mc_visual_cache_dir(&version);
    std::fs::create_dir_all(&cache)?;
    let file = std::fs::File::open(&jar)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| crate::error::AppError::Msg(format!("Не удалось открыть jar: {e}")))?;

    let mut panorama = Vec::new();
    for name in PANORAMA_FACES {
        if let Some(p) = extract_entry(&mut zip, name, &cache) {
            panorama.push(p);
        }
    }
    if panorama.is_empty() {
        for name in PANORAMA_SINGLE {
            if let Some(p) = extract_entry(&mut zip, name, &cache) {
                panorama.push(p);
                break;
            }
        }
    }

    let mut button = serde_json::Map::new();
    for (kind, names) in BUTTONS {
        for name in *names {
            if let Some(p) = extract_entry(&mut zip, name, &cache) {
                button.insert((*kind).to_string(), json!(p));
                break;
            }
        }
    }

    // Старый widgets.png — это атлас, а не одна кнопка: отдаём его как
    // «normal», фронтенд растянет по размеру кнопки с image-rendering: pixelated.
    Ok(json!({
        "version": version,
        "title": title_for(&version),
        "panorama": panorama,
        "faces": if panorama.len() == 6 { 6 } else { 1 },
        "button": button,
        "accent": version_accent(&version),
        "source": "jar",
        "cached": true,
    }))
}

/// Абсолютный путь к кэшу версии (для отладки/команд).
pub fn cache_dir(version: &str) -> PathBuf {
    paths::mc_visual_cache_dir(version)
}
