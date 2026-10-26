use crate::models::Config;
use crate::paths;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};

/// Ручной прокси из настроек: URL с логином и паролем либо `None`.
/// Живёт отдельно от `AppState`, чтобы его видели все HTTP-клиенты лаунчера
/// и чтобы смена настроек применялась без перезапуска.
static MANUAL_PROXY: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

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
    /// Включён ли «Ускоритель лаунчера» (локальный прокси с фрагментацией).
    pub launcher_accel: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        // Общий таймаут запроса здесь недопустим: client.jar весит ~40 МБ и
        // на медленном канале качается дольше 30 секунд. Ограничиваем только
        // установку соединения и паузу между чтениями тела ответа.
        let cfg = load_config();
        let accel = crate::accelerator::start(cfg.launcher_accel);
        let launcher_accel = accel.enabled.clone();
        set_manual_proxy(&cfg);

        let mut builder = reqwest::Client::builder()
            .user_agent("ArbuzLauncher/1.0")
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(16);

        // Один прокси-слой на все запросы: сначала ручной прокси из настроек,
        // иначе — локальный ускоритель (если он включён).
        let accel_url = if accel.port != 0 {
            // Логин и пароль ускорителя — прямо в URL: так reqwest сам
            // подставит заголовок Proxy-Authorization.
            Some(format!(
                "http://arbuz:{}@127.0.0.1:{}",
                accel.token, accel.port
            ))
        } else {
            None
        };
        let enabled = accel.enabled.clone();
        let proxy = reqwest::Proxy::custom(move |target| {
            if let Some(url) = manual_proxy_for(target) {
                return Some(url);
            }
            let accel_url = accel_url.as_ref()?;
            if !enabled.load(Ordering::Relaxed) {
                return None;
            }
            let scheme = target.scheme();
            let port = target.port_or_known_default();
            let host = target.host_str().unwrap_or("");
            let proxied = (scheme == "https" && port == Some(443))
                || (scheme == "http" && port == Some(80) && !is_local_host(host));
            if proxied {
                Some(accel_url.clone())
            } else {
                None
            }
        });
        builder = builder.proxy(proxy);

        let http = builder.build().unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http,
            config: Mutex::new(cfg),
            running: Mutex::new(HashMap::new()),
            game_pids: Mutex::new(HashMap::new()),
            monitor: std::sync::Mutex::new(sysinfo::System::new()),
            ms_cancel: AtomicBool::new(false),
            launcher_accel,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Локальные адреса не гоняем через ускоритель: там DPI не мешает, а
/// UPnP/SSDP и свои сервисы должны работать напрямую.
fn is_local_host(host: &str) -> bool {
    let h = host
        .trim_matches(|c| c == '[' || c == ']')
        .to_ascii_lowercase();
    if h == "localhost" || h == "::1" {
        return true;
    }
    match h.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(std::net::IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => false,
    }
}

/// URL ручного прокси из настроек: HTTP, SOCKS5 или SOCKS5 с удалённым DNS.
fn manual_proxy_url(cfg: &Config) -> Option<String> {
    if !cfg.proxy_enabled {
        return None;
    }
    let host = cfg.proxy_host.trim();
    if host.is_empty() {
        return None;
    }
    let scheme = match cfg.proxy_kind.as_str() {
        "socks5" => "socks5",
        "socks5h" => "socks5h",
        _ => "http",
    };
    // IPv6 в URL должен быть в квадратных скобках.
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let auth = if cfg.proxy_user.trim().is_empty() {
        String::new()
    } else {
        format!(
            "{}:{}@",
            urlencoding::encode(cfg.proxy_user.trim()),
            urlencoding::encode(&cfg.proxy_pass)
        )
    };
    Some(format!("{scheme}://{auth}{host}:{}", cfg.proxy_port))
}

/// Запоминает ручной прокси для всех HTTP-клиентов лаунчера.
/// Вызывается при старте и при каждом сохранении настроек.
pub fn set_manual_proxy(cfg: &Config) {
    let url = manual_proxy_url(cfg);
    if let Ok(mut guard) = MANUAL_PROXY.write() {
        *guard = url;
    }
}

/// URL ручного прокси для конкретного запроса. Локальные адреса и не-HTTP
/// схемы идут напрямую: через прокси им делать нечего.
pub fn manual_proxy_for(target: &reqwest::Url) -> Option<String> {
    if !matches!(target.scheme(), "http" | "https") {
        return None;
    }
    if is_local_host(target.host_str().unwrap_or("")) {
        return None;
    }
    MANUAL_PROXY.read().ok().and_then(|guard| guard.clone())
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

#[cfg(test)]
mod tests {
    use super::{is_local_host, manual_proxy_url};
    use crate::models::Config;

    #[test]
    fn manual_proxy_url_builds_http_and_socks() {
        let mut cfg = Config::default();
        assert_eq!(manual_proxy_url(&cfg), None, "по умолчанию прокси выключен");

        cfg.proxy_enabled = true;
        cfg.proxy_host = " 127.0.0.1 ".into();
        cfg.proxy_port = 8080;
        cfg.proxy_user = "user".into();
        cfg.proxy_pass = "p@ss word".into();
        assert_eq!(
            manual_proxy_url(&cfg).as_deref(),
            Some("http://user:p%40ss%20word@127.0.0.1:8080")
        );

        cfg.proxy_user.clear();
        cfg.proxy_pass.clear();
        cfg.proxy_kind = "socks5h".into();
        cfg.proxy_host = "::1".into();
        cfg.proxy_port = 1080;
        assert_eq!(
            manual_proxy_url(&cfg).as_deref(),
            Some("socks5h://[::1]:1080")
        );

        cfg.proxy_host = "   ".into();
        assert_eq!(
            manual_proxy_url(&cfg),
            None,
            "пустой хост — прокси не нужен"
        );
    }

    #[test]
    fn local_hosts_bypass_proxy() {
        assert!(is_local_host("localhost"));
        assert!(is_local_host("127.0.0.1"));
        assert!(is_local_host("[::1]"));
        assert!(is_local_host("192.168.0.1"));
        assert!(is_local_host("10.1.2.3"));
        assert!(!is_local_host("api.modrinth.com"));
    }

    #[test]
    fn manual_proxy_applies_only_to_remote_http() {
        let mut cfg = Config::default();
        cfg.proxy_enabled = true;
        cfg.proxy_host = "127.0.0.1".into();
        cfg.proxy_port = 9999;
        super::set_manual_proxy(&cfg);

        let remote = reqwest::Url::parse("https://api.modrinth.com/v2/search").unwrap();
        assert_eq!(
            super::manual_proxy_for(&remote).as_deref(),
            Some("http://127.0.0.1:9999")
        );
        let local = reqwest::Url::parse("http://localhost:8080/x").unwrap();
        assert_eq!(super::manual_proxy_for(&local), None);

        // Не оставляем глобальное состояние включённым для других тестов.
        super::set_manual_proxy(&Config::default());
        assert_eq!(super::manual_proxy_for(&remote), None);
    }

    /// Живая проверка: ручной прокси с логином и паролем действительно
    /// используется клиентом. В роли прокси — локальный ускоритель.
    #[tokio::test]
    #[ignore = "network"]
    async fn live_manual_proxy_routes_requests() {
        let accel = crate::accelerator::start(true);
        if accel.port == 0 {
            return;
        }
        let mut cfg = Config::default();
        cfg.proxy_enabled = true;
        cfg.proxy_host = "127.0.0.1".into();
        cfg.proxy_port = accel.port as u32;
        cfg.proxy_user = "arbuz".into();
        cfg.proxy_pass = accel.token.clone();
        super::set_manual_proxy(&cfg);

        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::custom(super::manual_proxy_for))
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        let res = client
            .get("https://api.modrinth.com/v2/search?query=sodium&limit=1")
            .header("User-Agent", "ArbuzLauncher/1.0")
            .send()
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(res.status().is_success(), "status {}", res.status());

        super::set_manual_proxy(&Config::default());
    }
}
