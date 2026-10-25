//! Сбор «улик» из jar: список записей, разобранные class-файлы, манифест
//! и метаданные мода. Читаются только маленькие записи (class, манифест,
//! метаданные) — данные ресурсы остаются нетронутыми, лимиты защищают от
//! zip-бомб и архивов с десятками тысяч записей.

use super::classfile::{self, ClassData};
use super::config::Limits;
use std::collections::BTreeMap;
use std::io::Read;

/// Всё, что правила сканера могут посмотреть в одном jar.
#[derive(Clone, Debug, Default)]
pub(crate) struct JarEvidence {
    /// Имена записей архива (прямые слэши, без каталогов).
    pub entries: Vec<String>,
    /// Разобранные class-файлы.
    pub classes: Vec<ClassData>,
    /// Записи `.class`, которые не удалось разобрать.
    pub broken_classes: usize,
    /// `META-INF/MANIFEST.MF`: заголовок -> значение.
    pub manifest: BTreeMap<String, String>,
    /// `fabric.mod.json` / `quilt.mod.json`.
    pub mod_json: Option<serde_json::Value>,
    /// `META-INF/mods.toml` | `META-INF/neoforge.mods.toml`.
    pub mods_toml: Option<toml::Value>,
    /// Суммарный распакованный размер по центральному каталогу.
    pub total_uncompressed: u64,
    /// Превышен лимит записей или суммарного размера (zip-бомба).
    pub over_limits: bool,
    /// Архив не открылся или битый.
    pub zip_error: bool,
}

/// Читает запись целиком, если её заявленный размер в лимите.
fn read_entry(entry: &mut zip::read::ZipFile<'_>, max: u64) -> Option<Vec<u8>> {
    if entry.size() > max {
        return None;
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    match entry.read_to_end(&mut buf) {
        Ok(_) if buf.len() as u64 <= max => Some(buf),
        _ => None,
    }
}

/// Разбирает `MANIFEST.MF` в карту «заголовок -> значение».
fn parse_manifest(raw: &[u8]) -> BTreeMap<String, String> {
    let text = String::from_utf8_lossy(raw).replace("\r\n", "\n");
    let mut out = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            // Продолжение прошлого заголовка (переносы длинных строк).
            if let Some((_, v)) = current.as_mut() {
                v.push_str(rest);
            }
            continue;
        }
        if let Some((k, v)) = current.take() {
            out.insert(k, v);
        }
        if let Some((name, value)) = line.split_once(':') {
            current = Some((name.trim().to_string(), value.trim().to_string()));
        }
    }
    if let Some((k, v)) = current.take() {
        out.insert(k, v);
    }
    out
}

/// Собирает улики из байтов jar.
pub(crate) fn evidence_from_bytes(bytes: &[u8], limits: &Limits) -> JarEvidence {
    let mut ev = JarEvidence::default();
    if bytes.len() as u64 > limits.max_jar_bytes {
        ev.over_limits = true;
        return ev;
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut z = match zip::ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(_) => {
            ev.zip_error = true;
            return ev;
        }
    };
    if z.len() as usize > limits.max_entries {
        ev.over_limits = true;
        return ev;
    }

    for i in 0..z.len() {
        let mut entry = match z.by_index(i) {
            Ok(e) => e,
            Err(_) => {
                ev.zip_error = true;
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        ev.total_uncompressed += entry.size();
        if ev.total_uncompressed > limits.max_uncompressed_bytes {
            ev.over_limits = true;
            return ev;
        }
        ev.entries.push(name.clone());

        if name.ends_with(".class") {
            if ev.classes.len() >= limits.max_classes {
                ev.over_limits = true;
                continue;
            }
            match read_entry(&mut entry, limits.max_class_bytes) {
                Some(data) => match classfile::parse(&data) {
                    Some(cd) => ev.classes.push(cd),
                    None => ev.broken_classes += 1,
                },
                None => ev.broken_classes += 1,
            }
            continue;
        }
        match name.as_str() {
            "META-INF/MANIFEST.MF" => {
                if let Some(data) = read_entry(&mut entry, 1024 * 1024) {
                    ev.manifest = parse_manifest(&data);
                }
            }
            "fabric.mod.json" | "quilt.mod.json" => {
                if let Some(data) = read_entry(&mut entry, 1024 * 1024) {
                    ev.mod_json = serde_json::from_slice(&data).ok();
                }
            }
            "META-INF/mods.toml" | "META-INF/neoforge.mods.toml" => {
                if let Some(data) = read_entry(&mut entry, 1024 * 1024) {
                    let text = String::from_utf8_lossy(&data);
                    ev.mods_toml = toml::from_str(&text).ok();
                }
            }
            _ => {}
        }
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::super::synthetic::{jar, ClassBuilder};
    use super::*;

    fn limits() -> Limits {
        Limits::default()
    }

    #[test]
    fn collects_classes_manifest_and_metadata() {
        let cls = ClassBuilder::new("com/example/foo/Foo")
            .string("hello")
            .build();
        let manifest = b"Manifest-Version: 1.0\r\nPremain-Class: evil.Agent\r\n\r\n";
        let meta = br#"{"id":"foo","version":"1.0","description":"d","contact":{"repo":"https://github.com/a/foo"}}"#;
        let bytes = jar(&[
            ("com/example/foo/Foo.class".into(), cls),
            ("META-INF/MANIFEST.MF".into(), manifest.to_vec()),
            ("fabric.mod.json".into(), meta.to_vec()),
        ]);
        let ev = evidence_from_bytes(&bytes, &limits());
        assert_eq!(ev.classes.len(), 1);
        assert_eq!(ev.classes[0].strings, vec!["hello"]);
        assert_eq!(
            ev.manifest.get("Premain-Class").map(String::as_str),
            Some("evil.Agent")
        );
        assert!(ev.mod_json.is_some());
        assert!(!ev.over_limits);
        assert!(!ev.zip_error);
    }

    #[test]
    fn broken_class_is_counted() {
        let bytes = jar(&[("bad/Broken.class".into(), b"garbage not class".to_vec())]);
        let ev = evidence_from_bytes(&bytes, &limits());
        assert_eq!(ev.broken_classes, 1);
        assert!(ev.classes.is_empty());
    }

    #[test]
    fn zip_bomb_by_entries_is_rejected() {
        let cls = ClassBuilder::new("a/A").build();
        let entries: Vec<(String, Vec<u8>)> = (0..20)
            .map(|i| (format!("pkg/C{i}.class"), cls.clone()))
            .collect();
        let bytes = jar(&entries);
        let lim = Limits {
            max_entries: 10,
            ..Limits::default()
        };
        let ev = evidence_from_bytes(&bytes, &lim);
        assert!(ev.over_limits);
        assert!(ev.entries.len() <= 10);
    }

    #[test]
    fn zip_bomb_by_total_size_is_rejected() {
        let big = vec![0u8; 4096];
        let entries: Vec<(String, Vec<u8>)> = (0..8)
            .map(|i| (format!("data/f{i}.bin"), big.clone()))
            .collect();
        let bytes = jar(&entries);
        let lim = Limits {
            max_uncompressed_bytes: 1024,
            ..Limits::default()
        };
        let ev = evidence_from_bytes(&bytes, &lim);
        assert!(ev.over_limits);
    }

    #[test]
    fn bad_archive_sets_zip_error() {
        let ev = evidence_from_bytes(b"definitely not a zip", &limits());
        assert!(ev.zip_error);
        assert!(ev.entries.is_empty());
    }

    #[test]
    fn mods_toml_metadata_is_parsed() {
        let toml = b"[[mods]]\nmodId=\"foo\"\nversion=\"1.0\"\ndisplayName=\"Foo\"\n";
        let bytes = jar(&[("META-INF/mods.toml".into(), toml.to_vec())]);
        let ev = evidence_from_bytes(&bytes, &limits());
        assert!(ev.mods_toml.is_some());
    }
}
