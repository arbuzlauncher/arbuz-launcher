//! Проверка обновлений самого лаунчера по релизам GitHub.
//!
//! Своего сервера у проекта нет: новая версия — это релиз в репозитории,
//! а файл установщика лежит в его assets.

use crate::error::Res;
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Репозиторий проекта.
const REPO: &str = "arbuzlauncher/arbuz-launcher";
/// Старое имя: если в новом репозитории релизов ещё нет, смотрим здесь.
const REPO_OLD: &str = "arbuz-top/arbuz-launcher";

/// Найденное обновление.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdate {
    /// Версия из тега релиза, без «v».
    pub version: String,
    /// Версия запущенного лаунчера.
    pub current: String,
    /// Страница релиза — если файла установщика нет.
    pub page: String,
    /// Прямая ссылка на установщик (или страница релиза).
    pub download: String,
    /// Что нового (Markdown из описания релиза).
    pub notes: String,
}

/// Текущая версия лаунчера.
pub fn current() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// `true`, если версия `a` новее `b`. Сравниваем числовые части: 1.0.10 > 1.0.9.
fn newer(a: &str, b: &str) -> bool {
    let nums = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    let (a, b) = (nums(a), nums(b));
    if a.is_empty() {
        return false;
    }
    for i in 0..a.len().max(b.len()) {
        let x = *a.get(i).unwrap_or(&0);
        let y = *b.get(i).unwrap_or(&0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Насколько файл релиза подходит текущей системе: меньше — лучше, `None` — не наш.
///
/// Имена файлов задаёт CI (.github/workflows/build.yml), но старые релизы
/// (только `*-setup.exe`) тоже должны находиться, поэтому Windows не требует
/// слова «windows» в имени.
fn rank(name: &str, os: &str, arch: &str) -> Option<u8> {
    let n = name.to_lowercase();
    if n.ends_with(".sig") || n.ends_with(".sha256") || n.ends_with(".txt") {
        return None;
    }
    match os {
        "windows" => {
            if !n.ends_with(".exe") && !n.ends_with(".msi") {
                return None;
            }
            Some(if n.contains("setup") {
                0
            } else if n.contains("portable") {
                3
            } else {
                2
            })
        }
        "macos" => {
            if !n.ends_with(".dmg") {
                return None;
            }
            let arm = ["aarch64", "arm64"];
            let intel = ["x86_64", "x64", "intel"];
            Some(if n.contains("universal") {
                0
            } else if arch == "aarch64" && arm.iter().any(|s| n.contains(s)) {
                1
            } else if arch != "aarch64" && intel.iter().any(|s| n.contains(s)) {
                1
            } else if arm.iter().any(|s| n.contains(s)) || intel.iter().any(|s| n.contains(s)) {
                4
            } else {
                2
            })
        }
        "linux" => {
            let base = if n.ends_with(".appimage") {
                0
            } else if n.ends_with(".deb") {
                1
            } else if n.ends_with(".rpm") {
                2
            } else {
                return None;
            };
            let mine = ["aarch64", "arm64", "x86_64", "amd64"];
            let other = if arch == "aarch64" {
                ["x86_64", "amd64"]
            } else {
                ["aarch64", "arm64"]
            };
            let bonus = if arch == "aarch64" && mine[..2].iter().any(|s| n.contains(s)) {
                0
            } else if arch != "aarch64" && mine[2..].iter().any(|s| n.contains(s)) {
                0
            } else if other.iter().any(|s| n.contains(s)) {
                6
            } else {
                1
            };
            Some(base + bonus)
        }
        _ => None,
    }
}

/// Индекс самого подходящего файла релиза.
fn best_asset(names: &[&str], os: &str, arch: &str) -> Option<usize> {
    names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| rank(n, os, arch).map(|r| (r, i)))
        .min()
        .map(|(_, i)| i)
}

fn pick_download(rel: &serde_json::Value) -> Option<String> {
    let assets = rel.get("assets")?.as_array()?;
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };

    let (names, urls): (Vec<&str>, Vec<&str>) = assets
        .iter()
        .map(|a| {
            (
                a.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                a.get("browser_download_url")
                    .and_then(|x| x.as_str())
                    .unwrap_or(""),
            )
        })
        .unzip();

    let i = best_asset(&names, os, arch)?;
    let url = urls.get(i).copied().unwrap_or("");
    if url.is_empty() {
        None
    } else {
        Some(url.to_string())
    }
}


async fn from_repo(state: &AppState, repo: &str, me: &str) -> Option<LauncherUpdate> {
    let resp = state
        .http
        .get(format!(
            "https://api.github.com/repos/{repo}/releases?per_page=10"
        ))
        .header("User-Agent", "arbuz-launcher")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let list: serde_json::Value = resp.json().await.ok()?;
    let arr = list.as_array()?;
    for rel in arr {
        if rel.get("draft").and_then(|x| x.as_bool()).unwrap_or(false) {
            continue;
        }
        if rel.get("prerelease").and_then(|x| x.as_bool()).unwrap_or(false) {
            continue;
        }
        let tag = rel.get("tag_name").and_then(|x| x.as_str()).unwrap_or("");
        let version = tag.trim_start_matches('v').trim().to_string();
        if version.is_empty() || !newer(&version, me) {
            continue;
        }
        let page = rel
            .get("html_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let download = pick_download(rel).unwrap_or_else(|| page.clone());
        let notes = rel
            .get("body")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        return Some(LauncherUpdate {
            version,
            current: me.to_string(),
            page,
            download,
            notes,
        });
    }
    None
}

/// Ищет более новый релиз. `None` — обновлений нет или сеть недоступна.
pub async fn check(state: &AppState) -> Res<Option<LauncherUpdate>> {
    let me = current();
    for repo in [REPO, REPO_OLD] {
        if let Some(upd) = from_repo(state, repo, &me).await {
            return Ok(Some(upd));
        }
    }
    Ok(None)
}

/// Скачивает обновление в `<data>/updates/<version>/` и отдаёт путь к файлу.
/// Прогресс идёт событием `progress` с id `launcher-update`.
pub async fn download(
    state: &AppState,
    app: &tauri::AppHandle,
    upd: &LauncherUpdate,
) -> Res<PathBuf> {
    crate::http::validate_url(&upd.download)?;
    let name = upd
        .download
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && s.len() <= 160)
        .unwrap_or("arbuz-update.exe")
        .to_string();
    let safe_version: String = upd
        .version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(40)
        .collect();
    let dir = crate::paths::updates_dir().join(if safe_version.is_empty() {
        "latest".to_string()
    } else {
        safe_version
    });
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(name);
    // Размер заранее неизвестен: прогресс будет «дёргаться» по мере чтения.
    let dl = crate::http::Dl {
        url: upd.download.clone(),
        path: dest.clone(),
        sha1: None,
        sha256: None,
        size: 0,
    };
    crate::http::download_all(app, &state.http, "launcher-update", vec![dl], 0.0, 100.0).await?;
    Ok(dest)
}

/// Запускает установку скачанного обновления и завершает лаунчер.
/// На Windows создаётся cmd-скрипт, который ждёт выхода процесса, чтобы
/// заменить запущенный exe или тихо прогнать установщик.
pub fn install_and_restart(app: &tauri::AppHandle, update: &Path) -> Res<()> {
    if !update.is_file() {
        return crate::error::err("Файл обновления не найден");
    }
    let current = std::env::current_exe()?;
    let dir = crate::paths::updates_dir();
    std::fs::create_dir_all(&dir)?;
    let name = update
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("update.exe")
        .to_ascii_lowercase();

    #[cfg(windows)]
    {
        let script = dir.join("apply-update.cmd");
        let is_setup = name.contains("setup") || name.contains("installer");
        let current_s = current.to_string_lossy().replace('%', "%%");
        let update_s = update.to_string_lossy().replace('%', "%%");
        let body = if is_setup {
            format!(
                "@echo off\r\n\
                 chcp 65001 >nul\r\n\
                 timeout /t 2 /nobreak >nul\r\n\
                 :wait\r\n\
                 tasklist /FI \"PID eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
                 if not errorlevel 1 (timeout /t 1 /nobreak >nul & goto wait)\r\n\
                 start \"\" /wait \"{update}\" /S\r\n\
                 start \"\" \"{current}\"\r\n\
                 del \"%~f0\"\r\n",
                pid = std::process::id(),
                update = update_s,
                current = current_s,
            )
        } else {
            format!(
                "@echo off\r\n\
                 chcp 65001 >nul\r\n\
                 timeout /t 2 /nobreak >nul\r\n\
                 :wait\r\n\
                 tasklist /FI \"PID eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
                 if not errorlevel 1 (timeout /t 1 /nobreak >nul & goto wait)\r\n\
                 copy /Y \"{update}\" \"{current}\" >nul\r\n\
                 start \"\" \"{current}\"\r\n\
                 del \"%~f0\"\r\n",
                pid = std::process::id(),
                update = update_s,
                current = current_s,
            )
        };
        std::fs::write(&script, body)?;
        let mut cmd = std::process::Command::new("cmd");
        let script_s = script.to_string_lossy().to_string();
        cmd.args(["/C", script_s.as_str()]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.spawn()?;
        app.exit(0);
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
        if let Some(target) = appimage {
            let script = dir.join("apply-update.sh");
            let body = format!(
                "#!/bin/sh\nsleep 2\nwhile kill -0 {pid} 2>/dev/null; do sleep 1; done\ncp -f '{update}' '{target}'\nchmod +x '{target}'\n'{target}' &\nrm -f \"$0\"\n",
                pid = std::process::id(),
                update = update.to_string_lossy().replace('\'', "'\\''"),
                target = target.to_string_lossy().replace('\'', "'\\''"),
            );
            std::fs::write(&script, body)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
            }
            std::process::Command::new("sh").arg(&script).spawn()?;
            app.exit(0);
            return Ok(());
        }
        return crate::error::err(
            "Обновление скачано. Для deb/rpm установите его вручную — лаунчер не может заменить системный пакет.",
        );
    }

    #[cfg(target_os = "macos")]
    {
        if name.ends_with(".dmg") {
            let script = dir.join("apply-update.sh");
            let exe = current.to_string_lossy().to_string();
            let app_bundle = exe
                .find(".app/")
                .map(|i| exe[..i + 4].to_string())
                .unwrap_or_else(|| "/Applications/Arbuz Launcher.app".to_string());
            let body = format!(
                "#!/bin/sh\nsleep 2\nwhile kill -0 {pid} 2>/dev/null; do sleep 1; done\nMNT=$(mktemp -d)\nhdiutil attach '{update}' -nobrowse -mountpoint \"$MNT\" >/dev/null\nAPP=$(find \"$MNT\" -maxdepth 1 -name '*.app' | head -n1)\nrm -rf '{app}'\ncp -R \"$APP\" '{app}'\nhdiutil detach \"$MNT\" >/dev/null\nopen '{app}'\nrm -f \"$0\"\n",
                pid = std::process::id(),
                update = update.to_string_lossy().replace('\'', "'\\''"),
                app = app_bundle.replace('\'', "'\\''"),
            );
            std::fs::write(&script, body)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
            }
            std::process::Command::new("sh").arg(&script).spawn()?;
            app.exit(0);
            return Ok(());
        }
        return crate::error::err("Обновление скачано, откройте его вручную.");
    }

    #[allow(unreachable_code)]
    {
        let _ = app;
        crate::error::err("Автообновление не поддерживается на этой платформе")
    }
}

#[cfg(test)]
mod tests {
    use super::{best_asset, newer};

    #[test]
    fn version_comparison() {
        assert!(newer("1.0.1", "1.0.0"));
        assert!(newer("v1.1.0", "1.0.9"));
        assert!(newer("1.0.10", "1.0.9"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("0.9.9", "1.0.0"));
        assert!(!newer("", "1.0.0"));
        assert!(newer("2", "1.9.9"));
    }

    #[test]
    fn picks_asset_for_each_os() {
        let names = [
            "Arbuz-Launcher-1.0.0-windows-x64-setup.exe",
            "Arbuz-Launcher-1.0.0-windows-x64-portable.exe",
            "Arbuz-Launcher-1.0.0-macos-universal.dmg",
            "Arbuz-Launcher-1.0.0-linux-x86_64.AppImage",
            "Arbuz-Launcher-1.0.0-linux-amd64.deb",
            "Arbuz-Launcher-1.0.0-linux-x86_64.rpm",
            "SHA256SUMS.txt",
        ];
        let pick = |os: &str, arch: &str| best_asset(&names, os, arch).map(|i| names[i]);
        assert_eq!(pick("windows", "x86_64"), Some(names[0]));
        assert_eq!(pick("macos", "aarch64"), Some(names[2]));
        assert_eq!(pick("macos", "x86_64"), Some(names[2]));
        assert_eq!(pick("linux", "x86_64"), Some(names[3]));
    }

    #[test]
    fn old_windows_release_still_matches() {
        assert_eq!(best_asset(&["Arbuz-Launcher-1.0.0-setup.exe"], "windows", "x86_64"), Some(0));
        assert_eq!(best_asset(&["Arbuz-Launcher-1.0.0.exe"], "windows", "x86_64"), Some(0));
    }

    #[test]
    fn linux_prefers_own_arch_and_appimage() {
        let names = ["x-linux-aarch64.deb", "x-linux-x86_64.rpm", "x-linux-x86_64.AppImage"];
        assert_eq!(best_asset(&names, "linux", "x86_64"), Some(2));
        let names = ["x-linux-aarch64.deb", "x-linux-x86_64.AppImage"];
        assert_eq!(best_asset(&names, "linux", "aarch64"), Some(0));
    }

    #[test]
    fn other_os_assets_are_not_offered() {
        assert_eq!(best_asset(&["x-windows-setup.exe"], "macos", "aarch64"), None);
        assert_eq!(best_asset(&["x-macos.dmg"], "linux", "x86_64"), None);
    }
}
