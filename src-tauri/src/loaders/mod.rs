//! Установка модовых загрузчиков (ТЗ §7.6).
//!
//! Fabric и Quilt отдают готовый `profile/json` с полем `inheritsFrom`; его
//! достаточно положить в `shared/versions/<id>/<id>.json`, а слияние с vanilla
//! сделает `mojang::versions::merged`.
//!
//! Forge и NeoForge ставятся своим installer'ом с флагом `--installClient`;
//! id итогового профиля определяется по новой папке в `shared/versions/`.

pub mod fabric;
pub mod forge;
pub mod neoforge;
pub mod quilt;

use crate::error::*;
use crate::models::{Instance, Loader};
use crate::state::AppState;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// Версия загрузчика. Поле `stable` есть только у Fabric.
#[derive(Clone)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

/// Итог установки: id профиля в `shared/versions/` и выбранная версия загрузчика.
pub struct Installed {
    pub profile_id: String,
    pub loader_version: String,
}

/// Список версий загрузчика для пары (mc, loader), от новых к старым.
/// Для Vanilla — пустой список.
pub async fn versions(http: &reqwest::Client, loader: Loader, mc: &str) -> Res<Vec<LoaderVersion>> {
    match loader {
        Loader::Vanilla => Ok(Vec::new()),
        Loader::Fabric => fabric::versions(http, mc).await,
        Loader::Quilt => quilt::versions(http, mc).await,
        Loader::Forge => forge::versions(http, mc).await,
        Loader::Neoforge => neoforge::versions(http, mc).await,
    }
}

/// Устанавливает загрузчик профиля и возвращает id для `versions::merged`.
/// Если `loader_version` не задана — берётся новейшая (ТЗ §7.6).
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    lo: f32,
    hi: f32,
) -> Res<Installed> {
    let mc = inst.mc_version.as_str();
    let list = versions(&state.http, inst.loader, mc).await?;
    if list.is_empty() {
        return err(format!("Для Minecraft {mc} нет {}", inst.loader.label()));
    }

    let chosen = match inst
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(v) => v.to_string(),
        None => list
            .iter()
            .find(|v| v.stable)
            .or_else(|| list.first())
            .map(|v| v.version.clone())
            .unwrap_or_default(),
    };

    let profile_id = match inst.loader {
        Loader::Fabric => fabric::install(&state.http, mc, &chosen).await?,
        Loader::Quilt => quilt::install(&state.http, mc, &chosen).await?,
        Loader::Forge => {
            forge::install(app, state, id, inst, &chosen, lo, hi).await?
        }
        Loader::Neoforge => {
            neoforge::install(app, state, id, inst, &chosen, lo, hi).await?
        }
        Loader::Vanilla => return err("У ванильного профиля нет загрузчика"),
    };

    Ok(Installed {
        profile_id,
        loader_version: chosen,
    })
}

/// Сохраняет profile.json загрузчика как `shared/versions/<id>/<id>.json`.
pub(crate) async fn save_profile(text: &str) -> Res<String> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Msg("В профиле загрузчика нет поля id".into()))?
        .to_string();

    let path = crate::paths::version_json(&id);
    if let Some(p) = path.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    tokio::fs::write(&path, text).await?;
    Ok(id)
}

/// Имена подпапок `shared/versions/` — чтобы найти профиль, созданный installer'ом.
pub(crate) async fn version_dir_names() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(crate::paths::versions_dir()).await else {
        return out;
    };
    while let Ok(Some(e)) = rd.next_entry().await {
        if e.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            out.push(e.file_name().to_string_lossy().to_string());
        }
    }
    out
}

/// `launcher_profiles.json` нужен installer'ам Forge и NeoForge.
async fn ensure_launcher_profiles() -> Res<()> {
    let p = crate::paths::launcher_profiles();
    if tokio::fs::metadata(&p).await.is_err() {
        tokio::fs::write(&p, "{\"profiles\":{}}").await?;
    }
    Ok(())
}

/// Vanilla client jar нужен installer'у до начала установки.
async fn ensure_client_jar(http: &reqwest::Client, mc: &str) -> Res<()> {
    let vj = crate::mojang::versions::fetch_json(http, mc).await?;
    let Some(c) = vj.downloads.as_ref().and_then(|d| d.client.as_ref()) else {
        return Ok(());
    };
    crate::http::download_file(
        http,
        &crate::http::Dl {
            url: c.url.clone(),
            path: crate::paths::version_jar(mc),
            sha1: c.sha1.clone(),
            sha256: None,
            size: c.size,
        },
    )
    .await
}

/// Java для запуска installer'а: путь профиля → установленная нужной версии →
/// скачивание Adoptium (как на шаге Java обычного запуска).
async fn installer_java(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    lo: f32,
    hi: f32,
) -> Res<PathBuf> {
    if let Some(p) = inst.java_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    if let Some(j) = crate::java::detect_javas()
        .await?
        .into_iter()
        .find(|j| j.major == inst.java_major)
    {
        return Ok(PathBuf::from(j.path));
    }
    crate::java::ensure_java(app, &state.http, id, inst.java_major, lo, hi).await
}

/// Папка `shared/versions/<dir>` уже содержит установленный профиль этого
/// загрузчика: в имени есть его название и версия. Имена у Forge исторически
/// разные (`1.20.1-forge-47.4.28`, `1.12.2-forge1.12.2-14.23.5.2864`,
/// `1.7.10-Forge10.13.4.1614`), поэтому сверяем по вхождению.
fn matches_tag(dir: &str, tag: &str, word: &str) -> bool {
    let d = dir.to_lowercase();
    if !d.contains(word) {
        return false;
    }
    let t = tag.to_lowercase();
    let Some(i) = d.find(&t) else {
        return false;
    };
    match d[i + t.len()..].chars().next() {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric(),
    }
}

/// Уже установленный профиль загрузчика, если он есть.
pub(crate) async fn find_installed(tag: &str, word: &str) -> Option<String> {
    version_dir_names()
        .await
        .into_iter()
        .find(|d| matches_tag(d, tag, word))
}

/// Общий путь установки Forge и NeoForge: скачать installer, выполнить
/// `--installClient <shared>` и определить id профиля по новой папке.
pub(crate) async fn install_with_installer(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    url: String,
    jar_name: String,
    tag: String,
    label: &str,
    lo: f32,
    hi: f32,
) -> Res<String> {
    let word = label.to_lowercase();

    // Повторная установка не нужна: installer в этом случае ничего не делает.
    if let Some(name) = find_installed(&tag, &word).await {
        return Ok(name);
    }

    ensure_launcher_profiles().await?;
    ensure_client_jar(&state.http, &inst.mc_version).await?;

    let java = installer_java(app, state, id, inst, lo, hi).await?;

    let jar = crate::paths::installers_dir().join(&jar_name);
    // Установщик исполняется JVM — без совпавшего хеша его запускать нельзя.
    let sha1 = match crate::http::fetch_sha1(&state.http, &url).await {
        Some(s) => s,
        None => return err("Не удалось получить контрольную сумму установщика"),
    };
    crate::http::download_file(
        &state.http,
        &crate::http::Dl {
            url,
            path: jar.clone(),
            sha1: Some(sha1),
            sha256: None,
            size: 0,
        },
    )
    .await?;

    let before = version_dir_names().await;
    let mut cmd = tokio::process::Command::new(&java);
    cmd.arg("-jar")
        .arg(&jar)
        .arg("--installClient")
        .arg(crate::paths::shared_dir())
        .current_dir(crate::paths::shared_dir());
    // Без этого на Windows на секунду открывается окно консоли.
    crate::winproc::hide_console(&mut cmd);
    let out = cmd.output().await?;
    let after = version_dir_names().await;

    let _ = app.emit(
        "progress",
        crate::http::Progress {
            id: id.to_string(),
            pct: hi,
        },
    );

    let created: Vec<String> = after.into_iter().filter(|d| !before.contains(d)).collect();
    if let Some(first) = created
        .iter()
        .find(|d| matches_tag(d, &tag, &word))
        .or_else(|| created.first())
    {
        return Ok(first.clone());
    }

    let text = String::from_utf8_lossy(&out.stderr);
    let last = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .next_back()
        .unwrap_or("без вывода");
    err(format!("Установщик {label} не создал профиль: {last}"))
}
