//! Автоматическая защита от Fractureiser.
//!
//! Проверка работает всегда, даже если «Сканер модов» выключен:
//! * при запуске лаунчера — фоновая проверка всех профилей;
//! * при установке/скачивании мода — проверка нового файла;
//! * перед запуском игры — гейт, который не пускает заражённый профиль.
//!
//! Ловим не только хеши из `malware_db.json`, но и характерные строки
//! (домены, имена файлов, классы), чтобы поймать пересобранные варианты.

use crate::error::{AppError, Res};
use crate::paths;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Характерные строки Fractureiser (в нижнем регистре).
const INDICATORS: &[&str] = &[
    "fractureiser",
    "files.gamewise.co",
    "85.217.144.130",
    "libwebgl64.jar",
    "libwebgl64",
    "stage0.class",
    "stage1.class",
    "stage2.class",
    "stage3.class",
    "venE",
    "fractureiser.xyz",
    "https://files.gamewise.co",
    "https://85.217.144.130",
    "client.jar",
    "lib.jar",
];

/// Известные имена файлов, которые Fractureiser создаёт вне mods/.
const KNOWN_PATHS: &[&str] = &[
    ".config/.data/lib.jar",
    ".local/share/.data/lib.jar",
    "AppData/Local/Microsoft/EdgeUpdate/MicrosoftEdgeUpdate.exe",
    "AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup",
    "AppData/Local/Temp/libwebgl64.jar",
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FsHit {
    pub file: String,
    pub sha256: String,
    pub reason: String,
    /// Профиль, в котором найден файл (если это файл профиля).
    pub instance: Option<String>,
    /// Файл лежит внутри профиля лаунчера — его можно отправить в карантин.
    pub quarantined: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn lower_contains(haystack: &[u8], needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let n = needle.as_bytes();
    if n.len() > haystack.len() {
        return false;
    }
    let first = n[0].to_ascii_lowercase();
    haystack
        .windows(n.len())
        .any(|w| w[0].to_ascii_lowercase() == first && w.eq_ignore_ascii_case(n))
}

/// Ищет индикаторы в сырых байтах (в том числе внутри class-файлов).
fn scan_bytes(bytes: &[u8], label: &str) -> Option<String> {
    for ind in INDICATORS {
        if lower_contains(bytes, ind) {
            return Some(format!("найден индикатор «{ind}» в {label}"));
        }
    }
    None
}

/// Проверяет jar/zip по записям, а обычный файл — целиком.
fn scan_archive(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    if zip.len() > 20_000 {
        return Some("слишком много файлов внутри архива".into());
    }
    for i in 0..zip.len() {
        let e = zip.by_index(i).ok()?;
        if e.is_dir() || e.size() > 64 * 1024 * 1024 {
            continue;
        }
        let name = e.name().to_string();
        let mut buf = Vec::new();
        if e.take(8 * 1024 * 1024).read_to_end(&mut buf).is_err() {
            continue;
        }
        if let Some(r) = scan_bytes(&buf, &name) {
            return Some(r);
        }
    }
    None
}

fn known_hash(bytes: &[u8]) -> Option<String> {
    let sha = sha256_hex(bytes);
    let db = crate::scanner::malware_db::load(&paths::malware_db_file());
    if db.hashes.iter().any(|h| h.eq_ignore_ascii_case(&sha)) {
        return Some(format!("хеш {sha} есть в базе известной малвари"));
    }
    None
}

/// Проверяет один файл. `None` — чисто.
pub fn check_file(path: &Path) -> Option<FsHit> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 512 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let sha = sha256_hex(&bytes);
    let reason = known_hash(&bytes)
        .or_else(|| scan_bytes(&bytes, &path.display().to_string()))
        .or_else(|| {
            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "jar" | "zip") {
                scan_archive(path)
            } else {
                None
            }
        })?;
    Some(FsHit {
        file: path.to_string_lossy().to_string(),
        sha256: sha,
        reason,
        instance: None,
        quarantined: false,
    })
}

/// Все jar-файлы в `mods/` профиля.
pub fn scan_instance_sync(id: &str) -> Vec<FsHit> {
    let mut out = Vec::new();
    let dir = paths::instance_minecraft_dir(id).join("mods");
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() {
            if let Some(mut hit) = check_file(&p) {
                hit.file = p
                    .strip_prefix(paths::instance_minecraft_dir(id))
                    .map(|x| crate::content::rel_slash(x))
                    .unwrap_or_else(|_| hit.file);
                hit.instance = Some(id.to_string());
                out.push(hit);
            }
        }
    }
    out
}

/// Проверяет все профили лаунчера.
pub fn scan_all_sync() -> Vec<FsHit> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(paths::instances_dir()) else {
        return out;
    };
    for e in rd.flatten() {
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let id = e.file_name().to_string_lossy().to_string();
            out.extend(scan_instance_sync(&id));
        }
    }
    // Известные пути вне профилей: только сообщаем, не трогаем.
    if let Some(home) = dirs::home_dir() {
        for rel in KNOWN_PATHS {
            let p = home.join(rel);
            if p.exists() {
                if let Some(mut hit) = check_file(&p) {
                    hit.quarantined = false;
                    out.push(hit);
                }
            }
        }
    }
    out
}

fn quarantine_hit(id: &str, hit: &FsHit) -> bool {
    let game = paths::instance_minecraft_dir(id);
    let src = game.join(&hit.file);
    if !src.is_file() {
        return false;
    }
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("malware.jar");
    crate::scanner::quarantine::move_file(&src, &paths::instance_quarantine_dir(id), name).is_ok()
}

/// Гейт перед запуском игры: заражённые файлы уходят в карантин.
pub async fn enforce_instance(id: &str) -> Res<()> {
    paths::validate_instance_id(id)?;
    let id2 = id.to_string();
    let mut hits = tokio::task::spawn_blocking(move || scan_instance_sync(&id2)).await?;
    if hits.is_empty() {
        return Ok(());
    }
    for hit in &mut hits {
        hit.quarantined = quarantine_hit(id, hit);
    }
    let list: Vec<String> = hits
        .iter()
        .map(|h| format!("{} — {}", h.file, h.reason))
        .collect();
    Err(AppError::Msg(format!(
        "Запуск остановлен защитой Fractureiser: {}. Файлы перенесены в quarantine профиля.",
        list.join("; ")
    )))
}

/// Проверяет профиль после установки модов. Заражённые файлы удаляются.
pub async fn guard_profile(id: &str) -> Res<()> {
    paths::validate_instance_id(id)?;
    let id2 = id.to_string();
    let hits = tokio::task::spawn_blocking(move || scan_instance_sync(&id2)).await?;
    if hits.is_empty() {
        return Ok(());
    }
    let mut names = Vec::new();
    for hit in &hits {
        let p = paths::instance_minecraft_dir(id).join(&hit.file);
        if p.is_file() {
            let _ = std::fs::remove_file(&p);
        }
        names.push(format!("{} — {}", hit.file, hit.reason));
    }
    Err(AppError::Msg(format!(
        "Fractureiser: установка остановлена, опасные файлы удалены: {}",
        names.join("; ")
    )))
}

/// Фоновая проверка при старте лаунчера. Результаты уходят событием
/// `security-alert`, заражённые файлы в профилях отправляются в карантин.
pub fn startup_scan(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let hits = tokio::task::spawn_blocking(scan_all_sync)
            .await
            .unwrap_or_default();
        if hits.is_empty() {
            return;
        }
        let mut report = Vec::new();
        for hit in &hits {
            let mut q = hit.clone();
            if let Some(id) = &hit.instance {
                q.quarantined = quarantine_hit(id, &hit);
            }
            report.push(q);
        }
        use tauri::Emitter;
        let _ = app.emit("security-alert", json!({ "source": "fractureiser", "hits": report }));
    });
}

/// Синхронная проверка одного файла для команды/тестов.
pub fn check_one(path: &Path) -> Option<FsHit> {
    check_file(path)
}

/// Список путей, которые проверяются при старте (для документации/UI).
pub fn protected_paths() -> Vec<PathBuf> {
    KNOWN_PATHS.iter().map(PathBuf::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicators_are_found() {
        let data = b"some class ... https://files.gamewise.co/stage0 ...";
        assert!(scan_bytes(data, "test").is_some());
        assert!(scan_bytes(b"clean code", "test").is_none());
    }

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
