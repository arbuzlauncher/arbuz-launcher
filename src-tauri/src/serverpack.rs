//! Сборщик серверных паков: Vanilla, Paper, Purpur, Fabric, Quilt, Forge,
//! NeoForge. Скачивает серверное ядро, копирует моды и конфиги профиля и
//! упаковывает всё в zip со скриптами запуска.

use crate::error::{err, Res};
use crate::paths;
use crate::state::AppState;
use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackResult {
    pub path: String,
    pub files: u32,
    pub bytes: u64,
    pub loader: String,
    pub mc_version: String,
}

async fn get_json(state: &AppState, url: &str) -> Res<Value> {
    Ok(state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

async fn download(state: &AppState, url: &str, dest: &Path) -> Res<u64> {
    let bytes = state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(dest, &bytes).await?;
    Ok(bytes.len() as u64)
}

fn loader_key(kind: &str) -> &'static str {
    match kind {
        "paper" => "Paper",
        "purpur" => "Purpur",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "Vanilla",
    }
}

async fn vanilla_jar(state: &AppState, mc: &str) -> Res<String> {
    let manifest = get_json(
        state,
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
    )
    .await?;
    let url = manifest["versions"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["id"].as_str() == Some(mc)))
        .and_then(|v| v["url"].as_str())
        .ok_or_else(|| crate::error::AppError::Msg(format!("Версия {mc} не найдена")))?;
    let vj = get_json(state, url).await?;
    vj["downloads"]["server"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| crate::error::AppError::Msg("У этой версии нет серверного jar".into()))
}

async fn paper_jar(state: &AppState, mc: &str) -> Res<String> {
    let builds = get_json(
        state,
        &format!("https://api.papermc.io/v2/projects/paper/versions/{mc}/builds"),
    )
    .await?;
    let build = builds["builds"]
        .as_array()
        .and_then(|a| a.last())
        .and_then(|b| b["build"].as_i64())
        .ok_or_else(|| crate::error::AppError::Msg(format!("Paper не поддерживает {mc}")))?;
    Ok(format!(
        "https://api.papermc.io/v2/projects/paper/versions/{mc}/builds/{build}/downloads/paper-{mc}-{build}.jar"
    ))
}

async fn purpur_jar(mc: &str) -> String {
    format!("https://api.purpurmc.org/v2/purpur/{mc}/latest/download")
}

async fn fabric_loader(state: &AppState, mc: &str, want: Option<&str>) -> Res<String> {
    if let Some(v) = want.filter(|s| !s.trim().is_empty()) {
        return Ok(v.trim().to_string());
    }
    let list = get_json(
        state,
        &format!("https://meta.fabricmc.net/v2/versions/loader/{mc}"),
    )
    .await?;
    list.as_array()
        .and_then(|a| a.first())
        .and_then(|v| v["loader"]["version"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| crate::error::AppError::Msg(format!("Fabric не поддерживает {mc}")))
}

async fn quilt_loader(state: &AppState, mc: &str, want: Option<&str>) -> Res<String> {
    if let Some(v) = want.filter(|s| !s.trim().is_empty()) {
        return Ok(v.trim().to_string());
    }
    let list = get_json(
        state,
        &format!("https://meta.quiltmc.org/v3/versions/loader/{mc}"),
    )
    .await?;
    list.as_array()
        .and_then(|a| a.first())
        .and_then(|v| v["loader"]["version"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| crate::error::AppError::Msg(format!("Quilt не поддерживает {mc}")))
}

async fn forge_version(state: &AppState, mc: &str, want: Option<&str>) -> Res<String> {
    if let Some(v) = want.filter(|s| !s.trim().is_empty()) {
        return Ok(v.trim().to_string());
    }
    let promos = get_json(
        state,
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
    )
    .await?;
    let p = promos["promos"]
        .as_object()
        .ok_or_else(|| crate::error::AppError::Msg("Не удалось получить версии Forge".into()))?;
    p.get(&format!("{mc}-recommended"))
        .or_else(|| p.get(&format!("{mc}-latest")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| crate::error::AppError::Msg(format!("Forge не поддерживает {mc}")))
}

async fn neoforge_version(state: &AppState, mc: &str, want: Option<&str>) -> Res<String> {
    if let Some(v) = want.filter(|s| !s.trim().is_empty()) {
        return Ok(v.trim().to_string());
    }
    let xml = state
        .http
        .get("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // 1.21.1 → 21.1.x; 1.20.4 → 20.4.x.
    let prefix = mc
        .strip_prefix("1.")
        .map(|s| format!("{s}."))
        .unwrap_or_else(|| format!("{mc}."));
    let mut best: Option<String> = None;
    for part in xml.split("<version>").skip(1) {
        let Some(v) = part.split("</version>").next() else {
            continue;
        };
        if v.starts_with(&prefix) {
            best = Some(v.trim().to_string());
        }
    }
    best.ok_or_else(|| crate::error::AppError::Msg(format!("NeoForge не поддерживает {mc}")))
}

fn write_scripts(dir: &Path, ram_mb: u32, jar: &str, installer: bool, mc: &str, lv: &str) {
    let ram = ram_mb.clamp(1024, 65536);
    let bat = if installer {
        format!(
            "@echo off\r\njava -jar {jar} --installServer\r\nif exist run.bat (echo Установка завершена. Запускайте start.bat) else (echo Установка не удалась — проверьте Java)\r\npause\r\n"
        )
    } else {
        String::new()
    };
    if installer {
        std::fs::write(dir.join("install.bat"), bat).ok();
        std::fs::write(
            dir.join("install.sh"),
            format!("#!/bin/sh\njava -jar {jar} --installServer\n"),
        )
        .ok();
        std::fs::write(
            dir.join("start.bat"),
            "@echo off\r\nif not exist run.bat (\r\n  echo Сначала запустите install.bat\r\n  pause\r\n  exit /b 1\r\n)\r\ncall run.bat nogui\r\npause\r\n",
        )
        .ok();
        std::fs::write(
            dir.join("start.sh"),
            "#!/bin/sh\nif [ ! -f run.sh ]; then echo 'Сначала запустите install.sh'; exit 1; fi\n./run.sh nogui\n",
        )
        .ok();
    } else {
        std::fs::write(
            dir.join("start.bat"),
            format!("@echo off\r\njava -Xmx{ram}M -jar {jar} nogui\r\npause\r\n"),
        )
        .ok();
        std::fs::write(
            dir.join("start.sh"),
            format!("#!/bin/sh\njava -Xmx{ram}M -jar {jar} nogui\n"),
        )
        .ok();
    }
    let readme = format!(
        "Серверный пак Arbuz Launcher\nВерсия игры: {mc}\nЗагрузчик: {lv}\n\nWindows: start.bat\nLinux/macOS: sh start.sh\n\nПеред первым запуском примите EULA в eula.txt (eula=true).\nПамять по умолчанию: {ram} МБ — измените в скрипте запуска.\n"
    );
    std::fs::write(dir.join("README.txt"), readme).ok();
    std::fs::write(dir.join("eula.txt"), "eula=false\n").ok();
}

fn copy_dir(src: &Path, dst: &Path, out: &mut u32) {
    let Ok(rd) = std::fs::read_dir(src) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() {
            let name = match p.file_name() {
                Some(n) => n,
                None => continue,
            };
            if p.extension().and_then(|x| x.to_str()) == Some("disabled") {
                continue;
            }
            if std::fs::copy(&p, dst.join(name)).is_ok() {
                *out += 1;
            }
        } else if p.is_dir() {
            let name = match p.file_name() {
                Some(n) => n,
                None => continue,
            };
            let sub = dst.join(name);
            if std::fs::create_dir_all(&sub).is_ok() {
                copy_dir(&p, &sub, out);
            }
        }
    }
}

fn zip_dir(root: &Path, dest: &Path) -> Res<(u32, u64)> {
    let file = std::fs::File::create(dest)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0u32;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let name = rel.to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(data) = std::fs::read(&p) else {
                continue;
            };
            zw.start_file(name, opts)?;
            zw.write_all(&data)?;
            count += 1;
            bytes += data.len() as u64;
        }
    }
    zw.finish()?;
    Ok((count, bytes))
}

/// Собирает серверный пак в `dest` (zip). Если задан профиль — копирует его
/// моды и конфиги.
pub async fn build(
    state: &AppState,
    instance_id: Option<&str>,
    kind: &str,
    mc_version: &str,
    loader_version: Option<&str>,
    dest: &str,
    ram_mb: u32,
) -> Res<ServerPackResult> {
    let kind = kind.trim().to_ascii_lowercase();
    let mc = mc_version.trim();
    if mc.is_empty() {
        return err("Не указана версия Minecraft");
    }
    let dest = PathBuf::from(dest.trim());
    if dest.as_os_str().is_empty() {
        return err("Не указан файл серверного пака");
    }
    if dest.extension().and_then(|x| x.to_str()) != Some("zip") {
        return err("Серверный пак сохраняется в .zip");
    }

    let work = paths::root()
        .join("cache")
        .join(format!("serverpack-{}", uuid::Uuid::new_v4().simple()));
    tokio::fs::create_dir_all(&work).await?;

    let (jar_name, installer, resolved_loader) = match kind.as_str() {
        "paper" => ("server.jar".to_string(), false, loader_version.unwrap_or("latest").to_string()),
        "purpur" => ("server.jar".to_string(), false, loader_version.unwrap_or("latest").to_string()),
        "fabric" => {
            let lv = fabric_loader(state, mc, loader_version).await?;
            ("server.jar".to_string(), false, lv)
        }
        "quilt" => {
            let lv = quilt_loader(state, mc, loader_version).await?;
            ("server.jar".to_string(), false, lv)
        }
        "forge" => {
            let lv = forge_version(state, mc, loader_version).await?;
            (format!("forge-{mc}-{lv}-installer.jar"), true, lv)
        }
        "neoforge" => {
            let lv = neoforge_version(state, mc, loader_version).await?;
            (format!("neoforge-{lv}-installer.jar"), true, lv)
        }
        _ => ("server.jar".to_string(), false, loader_version.unwrap_or("").to_string()),
    };

    let jar_path = work.join(&jar_name);
    match kind.as_str() {
        "paper" => {
            let url = paper_jar(state, mc).await?;
            download(state, &url, &jar_path).await?;
        }
        "purpur" => {
            download(state, &purpur_jar(mc).await, &jar_path).await?;
        }
        "fabric" => {
            let url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{mc}/{}/server/jar",
                resolved_loader
            );
            download(state, &url, &jar_path).await?;
        }
        "quilt" => {
            let url = format!(
                "https://meta.quiltmc.org/v3/versions/loader/{mc}/{}/server/jar",
                resolved_loader
            );
            download(state, &url, &jar_path).await?;
        }
        "forge" => {
            let url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{}/forge-{mc}-{}-installer.jar",
                resolved_loader, resolved_loader
            );
            download(state, &url, &jar_path).await?;
        }
        "neoforge" => {
            let url = format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
                resolved_loader, resolved_loader
            );
            download(state, &url, &jar_path).await?;
        }
        _ => {
            let url = vanilla_jar(state, mc).await?;
            download(state, &url, &jar_path).await?;
        }
    }

    write_scripts(&work, ram_mb, &jar_name, installer, mc, &resolved_loader);

    let mut copied = 0u32;
    if let Some(id) = instance_id.filter(|s| !s.is_empty()) {
        paths::validate_instance_id(id)?;
        let game = paths::instance_minecraft_dir(id);
        for (name, sub) in [
            ("mods", "mods"),
            ("config", "config"),
            ("defaultconfigs", "defaultconfigs"),
        ] {
            let src = game.join(name);
            if src.is_dir() {
                let dst = work.join(sub);
                if std::fs::create_dir_all(&dst).is_ok() {
                    copy_dir(&src, &dst, &mut copied);
                }
            }
        }
        let props = game.join("server.properties");
        if props.is_file() {
            let _ = std::fs::copy(props, work.join("server.properties"));
            copied += 1;
        }
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let zip_dest = dest.clone();
    let zip_root = work.clone();
    let (files, bytes) = tokio::task::spawn_blocking(move || zip_dir(&zip_root, &zip_dest))
        .await
        .map_err(|e| crate::error::AppError::Msg(e.to_string()))??;
    let _ = copied;
    let _ = tokio::fs::remove_dir_all(&work).await;

    Ok(ServerPackResult {
        path: dest.to_string_lossy().to_string(),
        files,
        bytes,
        loader: loader_key(&kind).to_string(),
        mc_version: mc.to_string(),
    })
}
