//! Кэш результатов сканирования по SHA-256: `<data>/cache/scanner/<sha>.json`.
//!
//! Ключ — хеш содержимого файла, поэтому изменившийся jar автоматически
//! сканируется заново. В кэше лежат находки правил, список классов
//! (для cross-jar правил 21), результат проверки хеша в каталогах и sha1
//! (нужен CurseForge). Уровень и score каждый раз считаются заново по
//! текущему режиму.

use serde::{Deserialize, Serialize};
// sha1 и sha2 переэкспортируют один и тот же трейт digest::Digest.
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::path::{Path, PathBuf};

/// Сериализуемая находка (для кэша и IPC).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindingDto {
    pub rule: String,
    /// Машиночитаемый код: «r6_native», «r12_agent»… — для блок-списков режимов.
    pub code: String,
    pub weight: i32,
    pub hard: bool,
    pub place: String,
    pub what: String,
}

/// Запись кэша сканера.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CacheEntry {
    /// Имя файла на момент сканирования (для сообщений).
    pub file: String,
    pub sha256: String,
    pub sha1: String,
    pub findings: Vec<FindingDto>,
    /// Список классов для правил 21 (дубли/перекрытия между модами).
    pub classes: Vec<String>,
    /// Хеш найден на Modrinth.
    pub modrinth_found: bool,
    /// id проекта Modrinth, если найден.
    pub modrinth_project: Option<String>,
    /// Хеш найден на CurseForge (через sha1).
    pub curseforge_found: bool,
}

impl CacheEntry {
    pub(crate) fn from_findings(
        file: &str,
        sha256: &str,
        sha1: &str,
        findings: Vec<FindingDto>,
        classes: Vec<String>,
    ) -> Self {
        Self {
            file: file.to_string(),
            sha256: sha256.to_string(),
            sha1: sha1.to_string(),
            findings,
            classes,
            modrinth_found: false,
            modrinth_project: None,
            curseforge_found: false,
        }
    }
}

/// Хеши содержимого файла: (sha256 hex, sha1 hex).
pub(crate) fn hashes(bytes: &[u8]) -> (String, String) {
    let sha256 = hex::encode(Sha256::digest(bytes));
    let sha1 = hex::encode(Sha1::digest(bytes));
    (sha256, sha1)
}

fn entry_path(cache_dir: &Path, sha256: &str) -> PathBuf {
    cache_dir.join(format!("{sha256}.json"))
}

/// Читает запись кэша. `None` — нет записи или она битая.
pub(crate) fn load(cache_dir: &Path, sha256: &str) -> Option<CacheEntry> {
    let raw = std::fs::read(entry_path(cache_dir, sha256)).ok()?;
    serde_json::from_slice(&raw).ok()
}

/// Сохраняет запись кэша (ошибки тихо игнорируются: кэш не критичен).
pub(crate) fn save(cache_dir: &Path, entry: &CacheEntry) {
    if std::fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    let path = entry_path(cache_dir, &entry.sha256);
    let tmp = path.with_extension("tmp");
    if serde_json::to_vec(entry)
        .and_then(|data| std::fs::write(&tmp, &data).map_err(serde_json::Error::io))
        .is_ok()
    {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> CacheEntry {
        CacheEntry::from_findings(
            "mod.jar",
            "aa",
            "bb",
            vec![FindingDto {
                rule: "R1".into(),
                code: "r1_exfil".into(),
                weight: 45,
                hard: true,
                place: "com/x/Y".into(),
                what: "тест".into(),
            }],
            vec!["com/x/Y".into()],
        )
    }

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("arbuz-scan-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let e = entry();
        save(&dir, &e);
        let back = load(&dir, "aa").expect("cache entry");
        assert_eq!(back.file, "mod.jar");
        assert_eq!(back.findings, e.findings);
        assert_eq!(back.classes, vec!["com/x/Y".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_broken_is_none() {
        let dir = std::env::temp_dir().join(format!("arbuz-scan-cache2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load(&dir, "nope").is_none());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.json"), "not json").unwrap();
        assert!(load(&dir, "bad").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_starts_with_hex() {
        let (s256, s1) = hashes(b"data");
        assert_eq!(s256.len(), 64);
        assert_eq!(s1.len(), 40);
        assert!(s256.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
