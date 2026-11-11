//! Движок тем: внешние `theme.json` + CSS и темы плагинов. Встроенных тем нет.
//!
//! Тема задаёт палитру (те же роли, что у «своей» темы), радиусы, шрифт,
//! layout-флаги и произвольный CSS. CSS применяется только к окну лаунчера
//! (`#win`), поэтому тема не может сломать другие приложения.

use crate::error::{err, Res};
use crate::models::CustomTheme;
use crate::paths;
use crate::plugins::runtime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemePack {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub dark: bool,
    /// Роли: bg, sbg, s1, s2, s3, on, onv, ln, pri, sec, ter.
    pub colors: BTreeMap<String, String>,
    pub radius: f32,
    pub font: String,
    /// CSS-файл рядом с theme.json (например, `theme.css`).
    pub css: String,
    /// Свободный токен раскладки. Пишется в `data-theme-layout`, чтобы CSS
    /// темы мог выбрать свои элементы через `#win[data-theme-layout="..."]`.
    pub layout: String,
    /// Мягкие формы и скругления.
    pub soft: bool,
    /// Тема зарегистрирована плагином.
    pub plugin: Option<String>,
}

impl Default for ThemePack {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            author: String::new(),
            description: String::new(),
            dark: true,
            colors: BTreeMap::new(),
            radius: 8.0,
            font: String::new(),
            css: String::new(),
            layout: "default".into(),
            soft: false,
            plugin: None,
        }
    }
}

const ROLES: [&str; 11] = [
    "bg", "sbg", "s1", "s2", "s3", "on", "onv", "ln", "pri", "sec", "ter",
];

fn hex_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

impl ThemePack {
    pub fn sanitize(&mut self, fallback_id: &str) {
        self.id = self
            .id
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .take(80)
            .collect();
        if self.id.is_empty() {
            self.id = fallback_id
                .trim()
                .to_ascii_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                .take(80)
                .collect();
        }
        if self.id.is_empty() {
            self.id = "theme".into();
        }
        self.name = self.name.trim().chars().take(120).collect();
        if self.name.is_empty() {
            self.name = self.id.clone();
        }
        self.author = self.author.trim().chars().take(120).collect();
        self.description = self.description.trim().chars().take(1000).collect();
        self.colors.retain(|k, v| {
            ROLES.contains(&k.as_str()) && hex_ok(v) && {
                let lower = v.to_ascii_lowercase();
                *v = lower;
                true
            }
        });
        if self.colors.is_empty() {
            self.colors = fallback_colors(self.dark);
        }
        if !self.radius.is_finite() {
            self.radius = 8.0;
        }
        self.radius = self.radius.clamp(0.0, 26.0);
        self.font = self
            .font
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | ',' | '\'' | '"' | '-' | '_'))
            .take(80)
            .collect::<String>()
            .trim()
            .to_string();
        self.css = self.css.chars().take(100_000).collect();
        let trimmed = self.css.trim().to_string();
        if trimmed.contains('{') || trimmed.contains('\n') {
            // CSS прямо в theme.json.
            self.css = trimmed;
        } else {
            // Имя файла рядом с theme.json: только относительный путь.
            self.css = trimmed
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string();
            if self.css.contains("..") || self.css.contains(':') {
                self.css.clear();
            }
        }
        self.layout = self
            .layout
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .take(32)
            .collect();
        if self.layout.is_empty() {
            self.layout = "default".into();
        }
    }

    /// Превращает палитру темы в `CustomTheme` для существующего движка.
    pub fn custom(&self) -> CustomTheme {
        let d = CustomTheme::default();
        let get = |k: &str, def: &str| {
            self.colors
                .get(k)
                .cloned()
                .unwrap_or_else(|| def.to_string())
        };
        let mut c = CustomTheme::default();
        c.bg = get("bg", &d.bg);
        c.sbg = get("sbg", &d.sbg);
        c.s1 = get("s1", &d.s1);
        c.s2 = get("s2", &d.s2);
        c.s3 = get("s3", &d.s3);
        c.on = get("on", &d.on);
        c.onv = get("onv", &d.onv);
        c.ln = get("ln", &d.ln);
        c.pri = get("pri", &d.pri);
        c.sec = get("sec", &d.sec);
        c.ter = get("ter", &d.ter);
        c.font = self.font.clone();
        c.radius = self.radius;
        c.soft = self.soft;
        c
    }
}

/// Резервная палитра для `theme.json` без цветов. Это не отдельная тема:
/// список тем заполняется только из `<data>/themes` и плагинов.
fn fallback_colors(dark: bool) -> BTreeMap<String, String> {
    let mut c = BTreeMap::new();
    let mut add = |k: &str, v: &str| {
        c.insert(k.to_string(), v.to_string());
    };
    if dark {
        add("bg", "#1a1b1e");
        add("sbg", "#141517");
        add("s1", "#212225");
        add("s2", "#2a2c30");
        add("s3", "#34363b");
        add("on", "#e8e9eb");
        add("onv", "#8f949c");
        add("ln", "#2c2e33");
        add("pri", "#5fb339");
        add("sec", "#2a2c30");
        add("ter", "#5fb339");
    } else {
        add("bg", "#f3f3f3");
        add("sbg", "#ebebeb");
        add("s1", "#ffffff");
        add("s2", "#f7f7f7");
        add("s3", "#ececec");
        add("on", "#1a1a1a");
        add("onv", "#5f5f5f");
        add("ln", "#dedede");
        add("pri", "#4a8f2c");
        add("sec", "#eaeaea");
        add("ter", "#4a8f2c");
    }
    c
}

fn read_theme_file(path: &Path) -> Option<ThemePack> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut t: ThemePack = serde_json::from_str(&raw).ok()?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("theme");
    t.sanitize(stem);
    Some(t)
}

/// Все темы: пользовательские и зарегистрированные плагинами.
pub fn list() -> Vec<ThemePack> {
    let mut out: Vec<ThemePack> = Vec::new();
    let dir = paths::themes_dir();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(t) = read_theme_file(&p.join("theme.json")) {
                    out.push(t);
                }
            }
        }
    }
    for v in runtime::themes() {
        if let Ok(mut t) = serde_json::from_value::<ThemePack>(v) {
            let id = t.id.clone();
            t.sanitize(&id);
            t.plugin = t.plugin.or_else(|| Some("plugin".into()));
            out.push(t);
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|t| seen.insert(t.id.clone()));
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Тема + её CSS. CSS из файла рядом с `theme.json` имеет приоритет над полем.
pub fn load(id: &str) -> Res<(ThemePack, String)> {
    let id = id.trim();
    if id.is_empty() {
        return err("Не указан id темы");
    }
    let mut theme = list()
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| crate::error::AppError::Msg("Тема не найдена".into()))?;
    let mut css = theme.css.clone();
    let dir = paths::themes_dir().join(&theme.id);
    if !theme.css.is_empty() {
        let file = dir.join(&theme.css);
        if let Ok(text) = std::fs::read_to_string(&file) {
            css = text;
        }
    }
    if css.len() > 100_000 {
        css.truncate(100_000);
    }
    theme.css.clear();
    Ok((theme, css))
}

/// Удаляет пользовательскую тему. Плагинные не трогаем.
pub fn remove(id: &str) -> Res<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
        return err("Некорректный id темы");
    }
    if list().into_iter().any(|t| t.id == id && t.plugin.is_some()) {
        return err("Плагинную тему удалить нельзя");
    }
    let dir = paths::themes_dir().join(id);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub fn dir() -> PathBuf {
    paths::themes_dir()
}

/// Публичный список для плагинов/команд: только безопасные поля.
pub fn list_json() -> Value {
    Value::Array(
        list()
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "author": t.author,
                    "description": t.description,
                    "dark": t.dark,
                    "plugin": t.plugin,
                    "layout": t.layout,
                    "soft": t.soft,
                    "radius": t.radius,
                    "font": t.font,
                    "colors": t.colors,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_colors_have_all_roles() {
        for dark in [true, false] {
            let c = fallback_colors(dark);
            for r in ROLES {
                assert!(c.contains_key(r), "нет роли {r}");
            }
        }
    }

    #[test]
    fn sanitize_rejects_bad_colors() {
        let mut t = ThemePack {
            dark: true,
            ..ThemePack::default()
        };
        t.colors.insert("bg".into(), "javascript:alert(1)".into());
        t.sanitize("x");
        assert_ne!(t.colors.get("bg").unwrap(), "javascript:alert(1)");
    }
}
