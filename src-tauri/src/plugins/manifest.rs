//! Манифест Python-плагина.
//!
//! `.aplug` — обычный `.py`-файл. Метаданные можно задать двумя способами:
//! 1. строкой-заголовком (безопасно читается без запуска кода):
//!    `# arbuz-plugin: {"id":"my.plugin","name":"My plugin"}`
//! 2. переменной `PLUGIN = {...}` в начале файла (читается через `ast.literal_eval`).
//!
//! Манифест нужен, чтобы показать плагин в интерфейсе до его включения.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Что плагин умеет и какие права просит.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    /// Версия API лаунчера, под которую написан плагин.
    pub api: u32,
    /// События, на которые плагин подписан (для списка в интерфейсе).
    pub hooks: Vec<String>,
    /// `network`, `fs`, `instances`, `mc`, `ui`, `themes`, `sources`, `config`.
    pub permissions: Vec<String>,
    /// Имя файла с кодом внутри папки плагина (по умолчанию `main.aplug`).
    pub entry: String,
    pub icon: Option<String>,
    pub homepage: Option<String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.1.0".into(),
            author: String::new(),
            description: String::new(),
            api: 1,
            hooks: Vec::new(),
            permissions: Vec::new(),
            entry: "main.aplug".into(),
            icon: None,
            homepage: None,
        }
    }
}

impl Manifest {
    /// Приводит манифест к безопасному виду и подставляет id/имя по умолчанию.
    pub fn sanitize(&mut self, fallback_id: &str) {
        let clean = |s: &str, max: usize| -> String {
            s.trim().chars().take(max).collect::<String>()
        };
        // id попадает в имя папки: оставляем только безопасные символы и
        // убираем точки/дефисы по краям, чтобы не получить «..» или скрытую папку.
        let sanitize_id = |s: &str| -> String {
            s.trim()
                .to_ascii_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                .take(80)
                .collect::<String>()
                .trim_matches(|c| matches!(c, '.' | '-' | '_'))
                .to_string()
        };
        self.id = sanitize_id(&self.id);
        if self.id.is_empty() {
            self.id = sanitize_id(fallback_id);
        }
        if self.id.is_empty() {
            self.id = "plugin".into();
        }
        self.name = clean(&self.name, 120);
        if self.name.is_empty() {
            self.name = self.id.clone();
        }
        self.version = clean(&self.version, 40);
        if self.version.is_empty() {
            self.version = "0.1.0".into();
        }
        self.author = clean(&self.author, 120);
        self.description = clean(&self.description, 2000);
        self.api = self.api.clamp(1, 999);
        self.entry = self
            .entry
            .trim()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if self.entry.is_empty()
            || self.entry.contains("..")
            || !self.entry.ends_with(".aplug")
        {
            self.entry = "main.aplug".into();
        }
        let known = [
            "network", "fs", "instances", "mc", "ui", "themes", "sources", "config", "auth",
        ];
        self.permissions = self
            .permissions
            .iter()
            .map(|p| p.trim().to_ascii_lowercase())
            .filter(|p| known.contains(&p.as_str()))
            .collect();
        self.permissions.sort();
        self.permissions.dedup();
        self.hooks = self
            .hooks
            .iter()
            .map(|h| h.trim().chars().take(120).collect::<String>())
            .filter(|h| !h.is_empty())
            .collect();
        self.hooks.sort();
        self.hooks.dedup();
        self.icon = self
            .icon
            .as_ref()
            .map(|s| s.trim().replace('\\', "/"))
            .filter(|s| !s.is_empty() && !s.contains("..") && s.len() <= 200);
        self.homepage = self
            .homepage
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| s.starts_with("https://") && s.len() <= 500);
    }
}

/// Разбирает строку-заголовок `# arbuz-plugin: {...}`.
fn parse_header(source: &str) -> Option<Manifest> {
    for line in source.lines().take(40) {
        let line = line.trim();
        let rest = line
            .strip_prefix("# arbuz-plugin:")
            .or_else(|| line.strip_prefix("#arbuz-plugin:"))
            .or_else(|| line.strip_prefix("# arbuz:"))?;
        return serde_json::from_str::<Manifest>(rest.trim()).ok();
    }
    None
}

/// Достаёт `PLUGIN = {...}` из Python-файла через `ast.literal_eval`.
/// Код плагина при этом не выполняется.
fn parse_plugin_var(source: &str) -> Option<Manifest> {
    let helper = r#"
import ast as _ast, json as _json
_out = ""
try:
    _tree = _ast.parse(src)
    for _n in _tree.body:
        if isinstance(_n, _ast.Assign):
            for _t in _n.targets:
                if isinstance(_t, _ast.Name) and _t.id in ("PLUGIN", "PLUGIN_INFO"):
                    _out = _json.dumps(_ast.literal_eval(_n.value))
                    break
except Exception:
    _out = ""
"#;
    pyo3::Python::with_gil(|py| {
        let locals = pyo3::types::PyDict::new(py);
        locals.set_item("src", source).ok()?;
        py.run(helper, None, Some(locals)).ok()?;
        let raw: String = locals.get_item("_out").ok()??.extract().ok()?;
        if raw.is_empty() {
            return None;
        }
        serde_json::from_str::<Manifest>(&raw).ok()
    })
}

/// Читает манифест из исходника `.aplug`.
pub fn from_source(source: &str, fallback_id: &str) -> Manifest {
    let mut m = parse_header(source)
        .or_else(|| parse_plugin_var(source))
        .unwrap_or_default();
    m.sanitize(fallback_id);
    m
}

/// Читает манифест из файла `plugin.json` (для `.arbz` и папок).
pub fn from_json_file(path: &Path, fallback_id: &str) -> Option<Manifest> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut m: Manifest = serde_json::from_str(&raw).ok()?;
    m.sanitize(fallback_id);
    Some(m)
}

/// Ищет манифест рядом с `.aplug` (в той же папке) и внутри файла.
pub fn discover(source_path: &Path, fallback_id: &str) -> Manifest {
    let dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(m) = from_json_file(&dir.join("plugin.json"), fallback_id) {
        return m;
    }
    let source = std::fs::read_to_string(source_path).unwrap_or_default();
    from_source(&source, fallback_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_parsed() {
        let src = r#"# arbuz-plugin: {"id":"demo.plugin","name":"Demo","permissions":["ui","network"]}
print("hi")
"#;
        let m = from_source(src, "fallback");
        assert_eq!(m.id, "demo.plugin");
        assert_eq!(m.name, "Demo");
        assert!(m.permissions.contains(&"network".to_string()));
    }

    #[test]
    fn id_is_sanitized() {
        let mut m = Manifest::default();
        m.id = "../../Bad ID!!".into();
        m.sanitize("fallback");
        assert_eq!(m.id, "badid");
    }
}
