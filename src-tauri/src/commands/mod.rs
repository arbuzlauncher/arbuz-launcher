use crate::error::{err, AppError, Res};
use crate::models::{default_java, Account, Config, Instance, Loader};
use crate::state::AppState;
use crate::{paths, store};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

const BIOMES: [&str; 5] = ["plains", "taiga", "desert", "nether", "snow"];

/// Маска, которой ключ CurseForge подменяется при выдаче в webview.
const CF_KEY_MASK: &str = "************";
/// Маска пароля прокси: в webview сам пароль не уходит.
const PROXY_PASS_MASK: &str = "************";
/// Маска API-ключа ИИ-провайдера: в webview сам ключ не уходит.
const AI_KEY_MASK: &str = "************";

/// Читает config.json и приводит поля к безопасным значениям.
async fn read_config() -> Res<Config> {
    // Нет config.json — первый запуск: показываем визард знакомства.
    // (У существующего файла поля `onboarded` может не быть, тогда serde
    // возьмёт значение из `Config::default()` — true, и визард не всплывёт.)
    let fresh = paths::first_run();
    let mut cfg: Config = store::read_json(&paths::config_file()).await?;
    if fresh {
        cfg.onboarded = false;
    }
    cfg.sanitize();
    Ok(cfg)
}

/// Проверяет путь к Java, пришедший из IPC: абсолютный, без `..`, обычный файл
/// с именем `java`/`java.exe`/`javaw.exe`. Не даёт подсунуть произвольный exe.
async fn validate_java_path(raw: &str) -> Res<()> {
    let p = raw.trim();
    if p.is_empty() {
        return Ok(());
    }
    if p.contains("..") {
        return err("Некорректный путь к Java");
    }
    let path = std::path::Path::new(p);
    if !path.is_absolute() {
        return err("Путь к Java должен быть абсолютным");
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(name.as_str(), "java" | "java.exe" | "javaw.exe") {
        return err("Укажите путь к java или java.exe");
    }
    match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() => Ok(()),
        _ => err("Файл Java не найден"),
    }
}

/// Читает все `instances/*/instance.json`, сортировка по `created`.
async fn load_instances() -> Res<Vec<Instance>> {
    let mut out: Vec<Instance> = Vec::new();
    let mut rd = match tokio::fs::read_dir(paths::instances_dir()).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    while let Some(entry) = rd.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let path = entry.path().join("instance.json");
        if tokio::fs::metadata(&path).await.is_err() {
            continue;
        }
        if let Ok(raw) = tokio::fs::read_to_string(&path).await {
            if let Ok(inst) = serde_json::from_str::<Instance>(&raw) {
                out.push(inst);
            }
        }
    }
    out.sort_by_key(|i| i.created);
    Ok(out)
}

/// Последний release из манифеста Mojang.
async fn latest_release(http: &reqwest::Client) -> Res<String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        versions: Vec<ManifestVersion>,
    }
    #[derive(serde::Deserialize)]
    struct ManifestVersion {
        id: String,
        #[serde(rename = "type")]
        kind: String,
    }

    let manifest: Manifest = http
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    match manifest.versions.into_iter().find(|v| v.kind == "release") {
        Some(v) => Ok(v.id),
        None => err("Не удалось определить последнюю версию Minecraft"),
    }
}

/// Открывает путь в системном файловом менеджере.
pub(crate) fn open_path(app: &AppHandle, path: &std::path::Path) -> Res<()> {
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))
}

mod settings;
pub use settings::*;
mod game;
pub use game::*;
mod content;
pub use content::*;
mod catalog;
pub use catalog::*;
mod packs;
pub use packs::*;
mod studio;
pub use studio::*;
mod modlists;
pub use modlists::*;
mod scanner;
pub use scanner::*;
mod sync;
pub use sync::*;
mod console;
pub use console::*;
mod servers;
pub use servers::*;
mod monitor;
pub use monitor::*;
mod benchmark;
pub use benchmark::*;
mod news;
pub use news::*;
mod network;
pub use network::*;
mod updates;
pub use updates::*;
