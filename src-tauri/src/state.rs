use crate::models::Config;
use crate::paths;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};

pub struct AppState {
    pub http: reqwest::Client,
    pub config: Mutex<Config>,
    /// instance id -> сигнал kill
    pub running: Mutex<HashMap<String, oneshot::Sender<()>>>,
    /// instance id -> PID процесса игры (для монитора RAM/CPU).
    pub game_pids: Mutex<HashMap<String, u32>>,
    /// Живой монитор: один sysinfo::System на процесс, чтобы cpu_usage
    /// считался между опросами, а не с нуля.
    pub monitor: std::sync::Mutex<sysinfo::System>,
    pub ms_cancel: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        // Общий таймаут запроса здесь недопустим: client.jar весит ~40 МБ и
        // на медленном канале качается дольше 30 секунд. Ограничиваем только
        // установку соединения и паузу между чтениями тела ответа.
        let http = reqwest::Client::builder()
            .user_agent("ArbuzLauncher/1.0")
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(16)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http,
            config: Mutex::new(load_config()),
            running: Mutex::new(HashMap::new()),
            game_pids: Mutex::new(HashMap::new()),
            monitor: std::sync::Mutex::new(sysinfo::System::new()),
            ms_cancel: AtomicBool::new(false),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Синхронная загрузка config.json (вызывается до старта рантайма).
fn load_config() -> Config {
    migrate_legacy_dir();
    // Нет config.json — это первый запуск, показываем визард знакомства.
    // У существующего файла поля `onboarded` может не быть: тогда значение
    // берётся из Config::default() (true), чтобы визард не всплывал заново.
    let fresh = paths::first_run();
    let mut cfg: Config = std::fs::read_to_string(paths::config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if fresh {
        cfg.onboarded = false;
    }
    cfg.sanitize();
    cfg
}

/// Один раз переносим данные из старых мест (`%APPDATA%\Quarry` после
/// переименования лаунчера и `%APPDATA%\Arbuz Launcher` — до того, как данные
/// переехали в свою папку рядом с лаунчером), чтобы не потерять профили,
/// аккаунты и настройки.
fn migrate_legacy_dir() {
    let Some(base) = dirs::data_dir() else {
        return;
    };
    let new = paths::root();
    for old in [base.join("Quarry"), base.join("Arbuz Launcher")] {
        if new.exists() || !old.is_dir() || old == new {
            continue;
        }
        move_dir(&old, &new);
    }
}

/// Переносим папку целиком: `rename` мгновенный, но между дисками не работает —
/// тогда копируем.
fn move_dir(old: &std::path::Path, new: &std::path::Path) {
    if std::fs::rename(old, new).is_ok() {
        return;
    }
    let _ = copy_dir(old, new);
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
