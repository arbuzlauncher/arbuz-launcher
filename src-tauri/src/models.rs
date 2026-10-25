use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    Neoforge,
    Quilt,
}

impl Loader {
    /// Отображаемое имя загрузчика — для сообщений об ошибках.
    pub fn label(&self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Forge => "Forge",
            Loader::Neoforge => "NeoForge",
            Loader::Quilt => "Quilt",
        }
    }
}

/// Откуда поставлена сборка — чтобы искать её обновления в каталоге.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackRef {
    /// modrinth | curseforge
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    #[serde(default)]
    pub version_name: String,
    /// Файлы, скачанные сборкой (пути от gameDir) — чтобы убрать лишние при обновлении.
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Instance {    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub biome: String,        // plains | taiga | desert | nether | snow
    pub icon: Option<String>, // имя файла в папке профиля
    /// id встроенной иконки профиля (см. public/builtin/profiles).
    #[serde(default)]
    pub icon_preset: Option<String>,
    /// id встроенных обоев профиля (см. public/builtin/wallpapers).
    #[serde(default)]
    pub wallpaper: Option<String>,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub java_major: u8, // 8 | 11 | 17 | 21
    pub java_path: Option<String>,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub playtime_sec: u64,
    pub playtime_days: BTreeMap<String, u64>, // "2026-10-14" -> секунды
    pub last_played: Option<i64>,             // unix sec
    pub created: i64,
    /// Сборка, из которой создан профиль (для обновлений). У старых профилей нет.
    #[serde(default)]
    pub pack: Option<PackRef>,
    /// Своя папка игры (`.minecraft`); `None` — `instances/<id>/minecraft`.
    #[serde(default)]
    pub game_dir: Option<String>,
    /// Режим сканера модов для этого профиля: "normal" | "strict" | "paranoid".
    /// `None`/пусто — использовать глобальный режим.
    #[serde(default)]
    pub scanner_mode: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft,
    Elyby,
    Littleskin,
    Offline,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String, // uuid профиля без дефисов
    pub kind: AccountKind,
    pub name: String,
    pub note: String, // e-mail / "ely.by" / "без интернета"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimConfig {
    pub enabled: bool,
    pub preset: String,    // "system" | "off" | "soft" | "smooth" | "fast" | "spring" | "custom"
    pub speed: f32,        // множитель скорости: 0.25..3
    pub amplitude: f32,    // амплитуда сдвига/масштаба, проценты: 0..140
    pub stagger: f32,      // задержка каскада, мс: 0..60
    pub easing: String,    // "ease-out" | "ease-in-out" | "linear" | "spring" | "custom"
    pub curve: String,     // cubic-bezier(...) для старой своей кривой
    #[serde(default, deserialize_with = "de_points")]
    pub points: Vec<[f32; 2]>, // точки своей кривой: [[x, y], ...], x 0..1
    pub dir: String,       // "up" | "left" | "right" | "scale"
    pub screens: bool,     // экраны и вкладки
    pub lists: bool,       // списки, карточки, строки
    pub modals: bool,      // модальные окна и оверлеи
    pub press: bool,       // наведение и нажатия
    pub launch: bool,      // запуск игры и прогресс
}

impl Default for AnimConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preset: "system".into(),
            speed: 1.0,
            amplitude: 100.0,
            stagger: 24.0,
            easing: "ease-out".into(),
            curve: "cubic-bezier(.22,.9,.28,1)".into(),
            points: Vec::new(),
            dir: "up".into(),
            screens: true,
            lists: true,
            modals: true,
            press: true,
            launch: true,
        }
    }
}

impl AnimConfig {
    /// Приводит значения из IPC/файла к безопасным диапазонам.
    pub fn sanitize(&mut self) {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.speed = f(self.speed, 0.25, 3.0, 1.0);
        self.amplitude = f(self.amplitude, 0.0, 140.0, 100.0);
        self.stagger = f(self.stagger, 0.0, 60.0, 24.0);
        if !matches!(
            self.preset.as_str(),
            "system" | "off" | "soft" | "smooth" | "fast" | "spring" | "custom"
        ) {
            self.preset = "system".into();
        }
        if !matches!(
            self.easing.as_str(),
            "ease-out" | "ease-in-out" | "linear" | "spring" | "custom"
        ) {
            self.easing = "ease-out".into();
        }
        if !matches!(self.dir.as_str(), "up" | "left" | "right" | "scale") {
            self.dir = "up".into();
        }
        if self.preset == "off" {
            self.enabled = false;
        }
        let curve = self.curve.trim();
        if curve.is_empty() || curve.len() > 128 {
            self.curve = "cubic-bezier(.22,.9,.28,1)".into();
        } else {
            self.curve = curve.to_string();
        }
        // Своя кривая: не больше 16 точек, x по возрастанию, y с запасом
        // на отскок (точки можно утащить выше 1 или ниже 0).
        self.points.retain(|p| p[0].is_finite() && p[1].is_finite());
        for p in &mut self.points {
            p[0] = p[0].clamp(0.0, 1.0);
            p[1] = p[1].clamp(0.0, 1.35);
        }
        self.points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        self.points.dedup_by(|a, b| (a[0] - b[0]).abs() < 0.02);
        self.points.truncate(16);
    }
}

/// Терпит `"anim": null` и объект с полями неверного типа: настройки
/// анимаций не должны мешать загрузке остального config.json.
fn de_anim<'de, D>(d: D) -> Result<AnimConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(v) => serde_json::from_value::<AnimConfig>(v).unwrap_or_default(),
        None => AnimConfig::default(),
    })
}

/// Терпит `"points": null` и битые пары, сохраняя остальные настройки.
fn de_points<'de, D>(d: D) -> Result<Vec<[f32; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    let mut out = Vec::new();
    if let Some(serde_json::Value::Array(items)) = v {
        for item in items {
            let arr = match item.as_array() {
                Some(a) if a.len() == 2 => a,
                _ => continue,
            };
            let (x, y) = match (arr[0].as_f64(), arr[1].as_f64()) {
                (Some(x), Some(y)) => (x as f32, y as f32),
                _ => continue,
            };
            if x.is_finite() && y.is_finite() {
                out.push([x, y]);
            }
        }
    }
    Ok(out)
}

/// Своя тема: цвета, размеры текста и кнопок, ширина сайдбара.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomTheme {
    pub bg: String,
    pub sbg: String,
    pub s1: String,
    pub s2: String,
    pub s3: String,
    pub on: String,
    pub onv: String,
    pub ln: String,
    pub pri: String,
    pub sec: String,
    pub ter: String,
    pub font: String, // пусто — стандартный шрифт темы
    pub font_scale: f32, // 0.85..1.3
    pub radius: f32,     // 0..26 px
    pub control: f32,    // 0.8..1.3
    pub sidebar: f32,    // 190..320 px
    pub density: f32,    // 0.85..1.3
    pub soft: bool,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            bg: "#1a1b1e".into(),
            sbg: "#141517".into(),
            s1: "#212225".into(),
            s2: "#2a2c30".into(),
            s3: "#34363b".into(),
            on: "#e8e9eb".into(),
            onv: "#8f949c".into(),
            ln: "#2c2e33".into(),
            pri: "#5fb339".into(),
            sec: "#2a2c30".into(),
            ter: "#5fb339".into(),
            font: String::new(),
            font_scale: 1.0,
            radius: 8.0,
            control: 1.0,
            sidebar: 230.0,
            density: 1.0,
            soft: false,
        }
    }
}

fn valid_hex(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

impl CustomTheme {
    /// Приводит цвета и размеры из IPC/файла к безопасным значениям.
    pub fn sanitize(&mut self) {
        let d = CustomTheme::default();
        let colors = [
            (&mut self.bg, &d.bg),
            (&mut self.sbg, &d.sbg),
            (&mut self.s1, &d.s1),
            (&mut self.s2, &d.s2),
            (&mut self.s3, &d.s3),
            (&mut self.on, &d.on),
            (&mut self.onv, &d.onv),
            (&mut self.ln, &d.ln),
            (&mut self.pri, &d.pri),
            (&mut self.sec, &d.sec),
            (&mut self.ter, &d.ter),
        ];
        for (v, def) in colors {
            let t = v.trim().to_ascii_lowercase();
            *v = if valid_hex(&t) { t } else { def.clone() };
        }
        let f = |v: f32, lo: f32, hi: f32, def: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                def
            }
        };
        self.font_scale = f(self.font_scale, 0.85, 1.3, 1.0);
        self.radius = f(self.radius, 0.0, 26.0, 8.0);
        self.control = f(self.control, 0.8, 1.3, 1.0);
        self.sidebar = f(self.sidebar, 190.0, 320.0, 230.0);
        self.density = f(self.density, 0.85, 1.3, 1.0);
        self.font = self
            .font
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | ',' | '\'' | '"' | '-' | '_'))
            .take(80)
            .collect::<String>()
            .trim()
            .to_string();
    }
}

/// Терпит `"custom": null` и битые поля: тема не должна ломать config.json.
fn de_custom<'de, D>(d: D) -> Result<CustomTheme, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(v) => serde_json::from_value::<CustomTheme>(v).unwrap_or_default(),
        None => CustomTheme::default(),
    })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub accent: String, // "#5fb339"
    pub dark: bool,
    pub style: String, // "flat" | "monet" | "amoled" | "catppuccin" | "dracula" | "custom"
    pub lang: String, // "ru" | "en" | "pl" | "uk" | "eo"
    /// Показывать выбор тем (экспериментально). По умолчанию только flat.
    pub experimental_themes: bool,
    /// Свои обои главной: имя файла в `<data>/wallpapers`.
    pub home_wallpaper: Option<String>,
    /// Визард первого запуска пройден. В старых конфигах поля нет —
    /// поэтому значение по умолчанию `true`, а «первый запуск» определяется
    /// отсутствием файла config.json (см. state::load_config).
    pub onboarded: bool,
    pub active_account: Option<String>,
    pub default_java: u8,
    pub close_on_launch: bool, // сворачивать лаунчер при запуске игры
    pub show_console: bool,
    /// Проверять обновления самого лаунчера на GitHub.
    pub check_launcher_updates: bool,
    /// Закрытие окна прячет лаунчер в трей, а не выходит.
    pub close_to_tray: bool,
    pub hide_empty: bool,
    /// Экспериментально: защита при запуске игры.
    pub mod_isolation_enabled: bool,
    /// Уровень защиты: "min" | "std" | "strict".
    pub mod_isolation_level: String,
    /// Сканер модов перед запуском игры.
    pub scanner_enabled: bool,
    /// Режим сканера: "normal" | "strict" | "paranoid".
    pub scanner_mode: String,
    /// ИИ-этап сканера: заметки по подозрительным модам (фрагменты
    /// уходят на сторонний сервис — предупреждение в настройках).
    pub ai_scanner: bool,
    /// URL статичного JSON со списком серверов (формат — в README).
    pub servers_url: String,
    /// Показывать статистику времени игры по профилям.
    pub show_playtime_stats: bool,
    pub curseforge_api_key: String,
    /// Экспериментально: ИИ-ассистент сборок по промпту.
    pub ai_builder: bool,
    /// Лимит модов для ИИ-сборки: 0 — без ограничения.
    pub ai_pack_limit: u32,
    /// Приоритетный каталог ИИ-сборки: пусто — оба, "modrinth" | "curseforge".
    pub ai_pack_source: String,
    /// Модель ИИ-сборки: пусто — первая доступная бесплатная.
    pub ai_pack_model: String,
    #[serde(default, deserialize_with = "de_anim")]
    pub anim: AnimConfig,
    #[serde(default, deserialize_with = "de_custom")]
    pub custom: CustomTheme,
    /// Сдвиги кнопок в режиме расстановки: ключ → [dx, dy].
    #[serde(default)]
    pub moves: BTreeMap<String, [f32; 2]>,
    /// Синхронизация настроек между профилями (главный профиль + ключи).
    #[serde(default)]
    pub sync: Option<crate::sync::SyncSettings>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            accent: "#5fb339".into(),
            dark: true,
            style: "flat".into(),
            lang: "ru".into(),
            experimental_themes: false,
            home_wallpaper: None,
            onboarded: true,
            active_account: None,
            default_java: 21,
            close_on_launch: false,
            show_console: true,
            check_launcher_updates: true,
            close_to_tray: true,
            hide_empty: false,
            mod_isolation_enabled: false,
            mod_isolation_level: "min".into(),
            scanner_enabled: false,
            scanner_mode: "normal".into(),
            ai_scanner: false,
            servers_url: String::new(),
            show_playtime_stats: true,
            curseforge_api_key: String::new(),
            ai_builder: true,
            ai_pack_limit: 0,
            ai_pack_source: String::new(),
            ai_pack_model: String::new(),
            anim: AnimConfig::default(),
            custom: CustomTheme::default(),
            moves: BTreeMap::new(),
            sync: None,
        }
    }
}

impl Config {
    /// Приводит всё, что пришло из файла или webview, к безопасным значениям.
    pub fn sanitize(&mut self) {
        self.anim.sanitize();
        self.custom.sanitize();
        if !matches!(
            self.lang.as_str(),
            "ru" | "en" | "pl" | "uk" | "eo" | "zh" | "et" | "fi" | "fr" | "de" | "be"
                | "sr" | "sk" | "ja" | "tg" | "uz" | "ky" | "lt" | "lv" | "iu" | "kk" | "olb" | "ka"
        ) {
            self.lang = "ru".into();
        }
        if !matches!(self.mod_isolation_level.as_str(), "min" | "std" | "strict") {
            self.mod_isolation_level = "min".into();
        }
        self.ai_pack_limit = self.ai_pack_limit.min(200);
        if !matches!(self.ai_pack_source.as_str(), "" | "modrinth" | "curseforge") {
            self.ai_pack_source = String::new();
        }
        self.ai_pack_model = self.ai_pack_model.trim().to_string();
        if self.ai_pack_model.len() > 80
            || !self
                .ai_pack_model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
        {
            self.ai_pack_model.clear();
        }
        if !matches!(self.scanner_mode.as_str(), "normal" | "strict" | "paranoid") {
            self.scanner_mode = "normal".into();
        }
        self.moves.retain(|k, v| {
            !k.is_empty() && k.len() <= 300 && v[0].is_finite() && v[1].is_finite()
        });
        for v in self.moves.values_mut() {
            v[0] = v[0].clamp(-4000.0, 4000.0);
            v[1] = v[1].clamp(-4000.0, 4000.0);
        }
        while self.moves.len() > 400 {
            let Some(k) = self.moves.keys().next().cloned() else {
                break;
            };
            self.moves.remove(&k);
        }
        self.servers_url = self.servers_url.trim().chars().take(500).collect();
        if !self.servers_url.is_empty()
            && !(self.servers_url.starts_with("https://") || self.servers_url.starts_with("http://"))
        {
            self.servers_url = String::new();
        }
        if let Some(sync) = &mut self.sync {
            if sync.master.len() != 32
                || !sync.master.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                sync.master = String::new();
            }
            sync.keys.retain(|k| !k.is_empty() && k.len() <= 100 && !k.contains(':'));
            sync.keys.sort();
            sync.keys.dedup();
            sync.keys.truncate(500);
        } else {
            self.sync = None;
        }
    }
}

pub fn default_java(mc: &str) -> u8 {
    // mc вида "1.21.1", "1.20" или "26.3" (нумерация по годам).
    let p: Vec<u32> = mc.split('.').filter_map(|x| x.parse().ok()).collect();
    let (major, minor, patch) = (
        p.first().copied().unwrap_or(0),
        p.get(1).copied().unwrap_or(0),
        p.get(2).copied().unwrap_or(0),
    );
    if major >= 25 {
        25
    } else if minor > 20 || (minor == 20 && patch >= 5) {
        21
    } else if minor >= 17 {
        17
    } else {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_config_without_anim_uses_defaults() {
        // config.json старых версий не содержит anim — он должен получить дефолт.
        let raw = r##"{"accent":"#5fb339","dark":true}"##;
        let cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config should deserialize: {e}"),
        };
        assert!(cfg.anim.enabled);
        assert_eq!(cfg.anim.preset, "system");
        assert_eq!(cfg.anim.speed, 1.0);
    }

    #[test]
    fn null_anim_falls_back_to_defaults() {
        // Повреждённый config.json мог записать "anim": null — это не повод
        // ронять загрузку настроек.
        let raw = r##"{"accent":"#5fb339","anim":null}"##;
        let cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config with null anim should deserialize: {e}"),
        };
        assert!(cfg.anim.enabled);
        assert_eq!(cfg.anim.preset, "system");
    }

    #[test]
    fn bad_points_keep_other_anim_settings() {
        let raw = r##"{"anim":{"enabled":false,"speed":1.5,"points":[[0,0],[1,"x"],"bad",[0.5,2]]}}"##;
        let cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config with bad points should deserialize: {e}"),
        };
        assert!(!cfg.anim.enabled);
        assert_eq!(cfg.anim.speed, 1.5);
        assert_eq!(cfg.anim.points, vec![[0.0, 0.0], [0.5, 2.0]]);
    }

    #[test]
    fn anim_sanitize_clamps_values() {
        let mut a = AnimConfig {
            speed: 99.0,
            amplitude: -20.0,
            stagger: 500.0,
            preset: "wat".into(),
            easing: "bounce".into(),
            dir: "down".into(),
            curve: "  ".into(),
            points: vec![[1.2, 2.0], [0.5, -0.4], [0.5, 0.5], [0.5, 0.9], [f32::NAN, 0.0]],
            ..AnimConfig::default()
        };
        a.sanitize();
        assert_eq!(a.speed, 3.0);
        assert_eq!(a.amplitude, 0.0);
        assert_eq!(a.stagger, 60.0);
        assert_eq!(a.preset, "system");
        assert_eq!(a.easing, "ease-out");
        assert_eq!(a.dir, "up");
        assert!(!a.curve.is_empty());
        // NaN выброшен, одинаковые x схлопнуты, значения зажаты.
        assert_eq!(a.points, vec![[0.5, 0.0], [1.0, 1.35]]);
    }

    #[test]
    fn anim_sanitize_disables_off_preset() {
        let mut a = AnimConfig {
            preset: "off".into(),
            enabled: true,
            ..AnimConfig::default()
        };
        a.sanitize();
        assert!(!a.enabled);
    }

    #[test]
    fn anim_config_round_trips_camel_case() {
        let a = AnimConfig {
            points: vec![[0.0, 0.0], [0.4, 1.2], [1.0, 1.0]],
            ..AnimConfig::default()
        };
        let raw = match serde_json::to_string(&a) {
            Ok(s) => s,
            Err(e) => panic!("serialize failed: {e}"),
        };
        assert!(raw.contains("\"stagger\""));
        assert!(raw.contains("\"points\""));
        let back: AnimConfig = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => panic!("deserialize failed: {e}"),
        };
        assert_eq!(back.dir, "up");
        assert!(back.modals);
        assert_eq!(back.points, a.points);
    }

    #[test]
    fn old_config_without_custom_uses_defaults() {
        let raw = r##"{"accent":"#5fb339","style":"custom"}"##;
        let cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config should deserialize: {e}"),
        };
        assert_eq!(cfg.custom.bg, "#1a1b1e");
        assert_eq!(cfg.custom.sidebar, 230.0);
        assert!(!cfg.custom.soft);
    }

    #[test]
    fn null_custom_falls_back_to_defaults() {
        let raw = r##"{"accent":"#5fb339","custom":null}"##;
        let cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config with null custom should deserialize: {e}"),
        };
        assert_eq!(cfg.custom.pri, "#5fb339");
    }

    #[test]
    fn custom_sanitize_fixes_colors_and_clamps() {
        let mut c = CustomTheme {
            bg: "red".into(),
            on: "#ABCDEF".into(),
            font_scale: 9.0,
            radius: -4.0,
            control: 0.1,
            sidebar: 900.0,
            density: f32::NAN,
            ..CustomTheme::default()
        };
        c.sanitize();
        assert_eq!(c.bg, "#1a1b1e");
        assert_eq!(c.on, "#abcdef");
        assert_eq!(c.font_scale, 1.3);
        assert_eq!(c.radius, 0.0);
        assert_eq!(c.control, 0.8);
        assert_eq!(c.sidebar, 320.0);
        assert_eq!(c.density, 1.0);
    }

    #[test]
    fn custom_round_trips_camel_case() {
        let c = CustomTheme::default();
        let raw = match serde_json::to_string(&c) {
            Ok(s) => s,
            Err(e) => panic!("serialize failed: {e}"),
        };
        assert!(raw.contains("\"fontScale\""));
        assert!(raw.contains("\"sidebar\""));
        assert!(raw.contains("\"font\""));
        let back: CustomTheme = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => panic!("deserialize failed: {e}"),
        };
        assert_eq!(back.pri, c.pri);
        assert_eq!(back.sidebar, c.sidebar);
    }

    #[test]
    fn config_sanitizes_moves() {
        let raw = r##"{"moves":{"a":[9999,-9999],"":[1,2],"b":[0.5,0.5]}}"##;
        let mut cfg: Config = match serde_json::from_str(raw) {
            Ok(c) => c,
            Err(e) => panic!("config should deserialize: {e}"),
        };
        cfg.sanitize();
        assert_eq!(cfg.moves.get("a"), Some(&[4000.0, -4000.0]));
        assert!(!cfg.moves.contains_key(""));
        assert_eq!(cfg.moves.get("b"), Some(&[0.5, 0.5]));
    }
}
