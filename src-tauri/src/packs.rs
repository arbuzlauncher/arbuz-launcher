//! Установка сборок: `.mrpack` (Modrinth) и CurseForge `.zip` (ТЗ §11).
//!
//! Тип сборки определяется по содержимому архива: есть `modrinth.index.json`
//! — это mrpack, есть `manifest.json` — CurseForge. Профиль создаётся без
//! скачивания Minecraft: игра докачивается при первом нажатии «Играть».

use crate::catalog::curseforge;
use crate::catalog::PACK_ID;
use crate::content;
use crate::error::{err, AppError, Res};
use crate::http::{download_all, Dl, Progress};
use crate::models::{default_java, Instance, Loader};
use crate::paths;
use crate::state::AppState;
use crate::store;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const BIOMES: [&str; 5] = ["plains", "taiga", "desert", "nether", "snow"];
const API_MR: &str = "https://api.modrinth.com/v2";

/// Лимиты распаковки сборки: защита от zip-bomb (один файл и суммарно).
const MAX_UNPACK_FILE: u64 = 512 * 1024 * 1024;
const MAX_UNPACK_TOTAL: u64 = 8 * 1024 * 1024 * 1024;
/// Предел размера `modrinth.index.json`/`manifest.json` при чтении из архива.
const MAX_INDEX_JSON: u64 = 32 * 1024 * 1024;

/// Число профилей на диске — для выбора биома по кругу.
async fn count_instances() -> usize {
    let mut n = 0usize;
    if let Ok(mut rd) = tokio::fs::read_dir(paths::instances_dir()).await {
        while let Ok(Some(_)) = rd.next_entry().await {
            n += 1;
        }
    }
    n
}

/// Создаёт профиль под сборку и готовит стандартные папки.
async fn create_profile(
    name: &str,
    mc: &str,
    loader: Loader,
    loader_version: Option<String>,
) -> Res<Instance> {
    let count = count_instances().await;
    let id = Uuid::new_v4().simple().to_string();
    let instance = Instance {
        id: id.clone(),
        name: name.to_string(),
        mc_version: mc.to_string(),
        loader,
        loader_version,
        biome: BIOMES[count % BIOMES.len()].to_string(),
        icon: None,
        icon_preset: None,
        wallpaper: None,
        ram_min_mb: 1024,
        ram_max_mb: 4096,
        java_major: default_java(mc),
        java_path: None,
        jvm_args: String::new(),
        width: 1280,
        height: 720,
        fullscreen: false,
        playtime_sec: 0,
        playtime_days: BTreeMap::new(),
        last_played: None,
        scanner_mode: None,
        created: chrono::Utc::now().timestamp(),
        pack: None,
        game_dir: None,
    };
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    let game = paths::instance_minecraft_dir(&id);
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(game.join(sub)).await?;
    }
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

// ---------------------------------------------------------------------------
// Zip-хелперы
// ---------------------------------------------------------------------------

/// Читает JSON-файл из архива; `None`, если файла в архиве нет.
/// Размер ограничен, чтобы запись-бомба не съела память.
fn zip_json(path: &Path, name: &str) -> Res<Option<Value>> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let out = match z.by_name(name) {
        Ok(e) => {
            if e.size() > MAX_INDEX_JSON {
                return err("Индекс сборки слишком большой");
            }
            let mut buf = Vec::new();
            e.take(MAX_INDEX_JSON).read_to_end(&mut buf)?;
            Some(serde_json::from_slice(&buf)?)
        }
        Err(zip::result::ZipError::FileNotFound) => None,
        Err(e) => return Err(e.into()),
    };
    Ok(out)
}

/// Распаковывает содержимое папок `prefixes` архива в `dest`.
/// Пути с `..`, префиксом диска или Windows-нормализуемыми компонентами
/// пропускаются; объём распаковки ограничен (защита от zip-bomb).
fn extract_dirs(path: &Path, prefixes: &[&str], dest: &Path) -> Res<()> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let mut total: u64 = 0;
    for i in 0..z.len() {
        let e = z.by_index(i)?;
        let raw = e.name().replace('\\', "/");

        let mut rest: Option<String> = None;
        for p in prefixes {
            let pre = format!("{p}/");
            if let Some(r) = raw.strip_prefix(pre.as_str()) {
                rest = Some(r.to_string());
                break;
            }
        }
        let Some(rest) = rest else {
            continue;
        };
        let Some(rel) = content::safe_rel(&rest) else {
            continue;
        };
        let out = dest.join(rel);

        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        let declared = e.size();
        if declared > MAX_UNPACK_FILE || total.saturating_add(declared) > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = std::fs::File::create(&out)?;
        let copied = std::io::copy(&mut e.take(MAX_UNPACK_FILE), &mut w)?;
        total = total.saturating_add(copied);
        if total > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
    }
    Ok(())
}

/// Распаковывает архив целиком, сохраняя структуру (импорт MultiMC/Prism).
fn extract_all(path: &Path, dest: &Path) -> Res<()> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let mut total: u64 = 0;
    for i in 0..z.len() {
        let e = z.by_index(i)?;
        let raw = e.name().replace('\\', "/");
        let Some(rel) = content::safe_rel(&raw) else {
            continue;
        };
        let out = dest.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        let declared = e.size();
        if declared > MAX_UNPACK_FILE || total.saturating_add(declared) > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = std::fs::File::create(&out)?;
        let copied = std::io::copy(&mut e.take(MAX_UNPACK_FILE), &mut w)?;
        total = total.saturating_add(copied);
        if total > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
    }
    Ok(())
}

/// Загрузчик сборки из `dependencies` mrpack-индекса.
fn loader_from_deps(deps: &Value) -> (Loader, Option<String>) {
    for (key, loader) in [
        ("fabric-loader", Loader::Fabric),
        ("quilt-loader", Loader::Quilt),
        ("forge", Loader::Forge),
        ("neoforge", Loader::Neoforge),
    ] {
        if let Some(v) = deps.get(key).and_then(|x| x.as_str()) {
            if !v.is_empty() {
                return (loader, Some(v.to_string()));
            }
        }
    }
    (Loader::Vanilla, None)
}

/// Для Forge номер версии записывается как `{mc}-{версия}` (ТЗ §11).
fn forge_version(mc: &str, loader: Loader, v: Option<String>) -> Option<String> {
    match (loader, v) {
        (Loader::Forge, Some(v)) => Some(format!("{mc}-{v}")),
        (_, v) => v,
    }
}

// ---------------------------------------------------------------------------
// Импорт архива сборки
// ---------------------------------------------------------------------------

/// Определяет тип сборки по содержимому архива и импортирует её.
pub async fn import_pack(app: &AppHandle, state: &AppState, path: &str) -> Res<Instance> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return err("Файл сборки не найден");
    }

    let probe = p.clone();
    let index = tokio::task::spawn_blocking(move || zip_json(&probe, "modrinth.index.json")).await??;
    if let Some(index) = index {
        return import_mrpack(app, state, &p, index).await;
    }

    let probe2 = p.clone();
    let manifest = tokio::task::spawn_blocking(move || zip_json(&probe2, "manifest.json")).await??;
    if let Some(m) = manifest {
        return import_curseforge(app, state, &p, m).await;
    }

    // MultiMC / Prism: распаковываем как есть и импортируем папку инстанса.
    let probe3 = p.clone();
    let mmc = tokio::task::spawn_blocking(move || zip_has(&probe3, "mmc-pack.json")).await??;
    if mmc {
        return import_multimc(&p).await;
    }

    err("Это не сборка Modrinth (.mrpack), CurseForge или MultiMC (.zip)")
}

/// Есть ли в архиве запись с таким именем.
fn zip_has(path: &Path, name: &str) -> Res<bool> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let found = z.by_name(name).is_ok();
    Ok(found)
}

/// MultiMC / Prism: архив распаковывается во временную папку и читается
/// тем же кодом, что и обычный импорт инстанса из папки.
async fn import_multimc(zip: &Path) -> Res<Instance> {
    let tmp = paths::tmp_dir().join(format!("mmc-{}", Uuid::new_v4().simple()));
    tokio::fs::create_dir_all(&tmp).await?;
    let z = zip.to_path_buf();
    let dest = tmp.clone();
    let unpacked = tokio::task::spawn_blocking(move || extract_all(&z, &dest)).await?;
    let out = match unpacked {
        Ok(()) => crate::prism::import(&tmp.to_string_lossy()).await,
        Err(e) => Err(e),
    };
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    out
}

/// Импорт Modrinth `.mrpack`.
async fn import_mrpack(app: &AppHandle, state: &AppState, zip: &Path, index: Value) -> Res<Instance> {
    let name = index
        .get("name")
        .and_then(|x| x.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Сборка")
        .to_string();
    let deps = index.get("dependencies").cloned().unwrap_or(Value::Null);
    let mc = deps.get("minecraft").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if mc.is_empty() {
        return err("В сборке не указана версия Minecraft");
    }
    let (loader, lv) = loader_from_deps(&deps);
    let inst = create_profile(&name, &mc, loader, forge_version(&mc, loader, lv)).await?;
    let game = paths::instance_minecraft_dir(&inst.id);

    let (dls, _) = index_files(&game, &index)?;
    if !dls.is_empty() {
        download_all(app, &state.http, PACK_ID, dls, 0.0, 90.0).await?;
    }

    let zip2 = zip.to_path_buf();
    let dest = game.clone();
    tokio::task::spawn_blocking(move || {
        extract_dirs(&zip2, &["overrides", "client-overrides"], &dest)
    })
    .await??;

    let _ = app.emit("progress", Progress { id: PACK_ID.into(), pct: 100.0 });
    Ok(inst)
}

/// Файлы из `modrinth.index.json`: что скачать и какие пути они займут.
fn index_files(game: &Path, index: &Value) -> Res<(Vec<Dl>, Vec<String>)> {    let mut dls: Vec<Dl> = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    if let Some(files) = index.get("files").and_then(|x| x.as_array()) {
        for f in files {
            if f.pointer("/env/client").and_then(|x| x.as_str()) == Some("unsupported") {
                continue;
            }
            let Some(rel) = f.get("path").and_then(|x| x.as_str()) else {
                continue;
            };
            let Some(url) = f.pointer("/downloads/0").and_then(|x| x.as_str()) else {
                continue;
            };
            if url.is_empty() {
                continue;
            }
            let sha1 = f
                .pointer("/hashes/sha1")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let size = f.get("fileSize").and_then(|x| x.as_u64()).unwrap_or(0);
            dls.push(Dl {
                url: url.to_string(),
                path: content::safe_join(game, rel)?,
                sha1,
                sha256: None,
                size,
            });
            paths.push(rel.to_string());
        }
    }
    Ok((dls, paths))
}

/// Только пути файлов из `modrinth.index.json`.
fn index_paths(index: &Value) -> Vec<String> {
    index
        .get("files")
        .and_then(|x| x.as_array())
        .map(|fs| {
            fs.iter()
                .filter(|f| f.pointer("/env/client").and_then(|x| x.as_str()) != Some("unsupported"))
                .filter_map(|f| f.get("path").and_then(|x| x.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Файлы сборки CurseForge по `manifest.json`: что скачать и куда положить.
async fn cf_manifest_files(
    state: &AppState,
    manifest: &Value,
    game: &Path,
) -> Res<(Vec<Dl>, Vec<String>)> {
    let mut ids: Vec<i64> = Vec::new();
    if let Some(files) = manifest.get("files").and_then(|x| x.as_array()) {
        for f in files {
            if let Some(id) = f.get("fileID").and_then(|x| x.as_i64()) {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let (api, key) = curseforge::endpoint(state).await;
    let mut dls: Vec<Dl> = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    // CurseForge принимает не больше 50 id за раз, а в сборках бывает и 300 модов.
    for chunk in ids.chunks(50) {
        let v: Value = curseforge::post(state, &format!("{api}/mods/files"), &key)
            .json(&serde_json::json!({ "fileIds": chunk }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
            for f in arr {
                let fid = f.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                let fname = f.get("fileName").and_then(|x| x.as_str()).unwrap_or("");
                if fname.is_empty() {
                    continue;
                }
                let url = match f.get("downloadUrl").and_then(|x| x.as_str()) {
                    Some(u) if !u.is_empty() && (key.is_some() || curseforge::is_cdn_url(u)) => {
                        u.to_string()
                    }
                    _ => curseforge::forgecdn_url(fid, fname),
                };
                let sha1 = f
                    .get("hashes")
                    .and_then(|x| x.as_array())
                    .and_then(|a| {
                        a.iter()
                            .find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1))
                    })
                    .and_then(|h| h.get("value").and_then(|x| x.as_str()))
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let size = f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);
                let rel = format!("mods/{fname}");
                dls.push(Dl {
                    url,
                    path: content::safe_join(game, &rel)?,
                    sha1,
                    sha256: None,
                    size,
                });
                paths.push(rel);
            }
        }
    }
    Ok((dls, paths))
}

/// Импорт CurseForge `.zip` по `manifest.json`.
async fn import_curseforge(
    app: &AppHandle,
    state: &AppState,
    zip: &Path,
    manifest: Value,
) -> Res<Instance> {
    let name = manifest
        .get("name")
        .and_then(|x| x.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Сборка")
        .to_string();
    let mc = manifest
        .pointer("/minecraft/version")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if mc.is_empty() {
        return err("В сборке не указана версия Minecraft");
    }

    // `id` загрузчика вида "forge-47.2.0": префикс до первого дефиса.
    let mut loader = Loader::Vanilla;
    let mut lv: Option<String> = None;
    if let Some(mls) = manifest.pointer("/minecraft/modLoaders").and_then(|x| x.as_array()) {
        let pick = mls
            .iter()
            .find(|m| m.get("primary").and_then(|x| x.as_bool()).unwrap_or(false))
            .or_else(|| mls.first());
        if let Some(m) = pick {
            if let Some(id) = m.get("id").and_then(|x| x.as_str()) {
                let (pre, ver) = id.split_once('-').unwrap_or((id, ""));
                loader = match pre {
                    "forge" => Loader::Forge,
                    "fabric" => Loader::Fabric,
                    "quilt" => Loader::Quilt,
                    "neoforge" => Loader::Neoforge,
                    _ => Loader::Vanilla,
                };
                lv = if ver.is_empty() { None } else { Some(ver.to_string()) };
            }
        }
    }

    let inst = create_profile(&name, &mc, loader, forge_version(&mc, loader, lv)).await?;
    let game = paths::instance_minecraft_dir(&inst.id);

    let (dls, _) = cf_manifest_files(state, &manifest, &game).await?;
    if !dls.is_empty() {
        download_all(app, &state.http, PACK_ID, dls, 0.0, 90.0).await?;
    }

    let prefix = manifest
        .get("overrides")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("overrides")
        .to_string();
    let zip2 = zip.to_path_buf();
    let dest = game.clone();
    tokio::task::spawn_blocking(move || extract_dirs(&zip2, &[prefix.as_str()], &dest)).await??;

    let _ = app.emit("progress", Progress { id: PACK_ID.into(), pct: 100.0 });
    Ok(inst)
}

/// Версия сборки, доступная к установке (ТЗ §11).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackVersion {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    /// ISO-8601.
    pub date: String,
}

/// Список версий проекта в каталоге.
pub async fn list_versions(state: &AppState, source: &str, project_id: &str) -> Res<Vec<PackVersion>> {
    match source {
        "modrinth" => modrinth_versions(state, project_id).await,
        "curseforge" => curseforge_versions(state, project_id).await,
        _ => err("Неизвестный источник"),
    }
}

/// Есть ли в версии файл `.mrpack`.
fn has_mrpack(v: &Value) -> bool {
    v.get("files")
        .and_then(|x| x.as_array())
        .map(|fs| {
            fs.iter().any(|f| {
                f.get("filename")
                    .and_then(|x| x.as_str())
                    .map(|n| n.ends_with(".mrpack"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Версии Modrinth, у которых есть `.mrpack`.
async fn modrinth_versions(state: &AppState, project_id: &str) -> Res<Vec<PackVersion>> {
    let v: Value = state
        .http
        .get(format!("{API_MR}/project/{project_id}/version"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out = Vec::new();
    if let Some(arr) = v.as_array() {
        for ver in arr {
            if !has_mrpack(ver) {
                continue;
            }
            let first = |key: &str| {
                ver.get(key)
                    .and_then(|x| x.as_array())
                    .and_then(|a| a.first())
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            out.push(PackVersion {
                id: ver.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                name: ver.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                mc_version: first("game_versions"),
                loader: first("loaders"),
                date: ver
                    .get("date_published")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(out)
}

/// Версии CurseForge.
async fn curseforge_versions(state: &AppState, project_id: &str) -> Res<Vec<PackVersion>> {
    let (api, key) = curseforge::endpoint(state).await;
    let v: Value = curseforge::get(state, &format!("{api}/mods/{project_id}/files?pageSize=50"), &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out = Vec::new();
    if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
        for f in arr {
            // `gameVersions` — смесь версий игры и названий загрузчиков.
            let gv = f.get("gameVersions").and_then(|x| x.as_array());
            let names: Vec<&str> = gv
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            let mc = names
                .iter()
                .find(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
                .copied()
                .unwrap_or("");
            let loader = names
                .iter()
                .find(|s| matches!(**s, "Forge" | "Fabric" | "Quilt" | "NeoForge"))
                .copied()
                .unwrap_or("");
            out.push(PackVersion {
                id: f.get("id").and_then(|x| x.as_i64()).map(|x| x.to_string()).unwrap_or_default(),
                name: f.get("displayName").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                mc_version: mc.to_string(),
                loader: loader.to_string(),
                date: f.get("fileDate").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Установка сборки из каталога
// ---------------------------------------------------------------------------

/// Файл сборки, выбранный в каталоге.
struct PackFile {
    url: String,
    sha1: Option<String>,
    size: u64,
    filename: String,
    version_id: String,
    version_name: String,
}

/// Файл `.mrpack` выбранной версии (или последней, если версия не задана).
async fn modrinth_pack(
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
) -> Res<PackFile> {
    let v: Value = state
        .http
        .get(format!("{API_MR}/project/{project_id}/version"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let empty = Vec::new();
    let arr = v.as_array().unwrap_or(&empty);
    for ver in arr {
        if let Some(want) = version_id {
            if ver.get("id").and_then(|x| x.as_str()) != Some(want) {
                continue;
            }
        }
        let Some(files) = ver.get("files").and_then(|x| x.as_array()) else {
            continue;
        };
        for f in files {
            let fname = f.get("filename").and_then(|x| x.as_str()).unwrap_or("");
            if !fname.ends_with(".mrpack") {
                continue;
            }
            let url = f.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if url.is_empty() {
                continue;
            }
            let sha1 = f
                .pointer("/hashes/sha1")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let size = f.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
            let vid = ver.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let vname = ver
                .get("version_number")
                .and_then(|x| x.as_str())
                .unwrap_or(&vid)
                .to_string();
            return Ok(PackFile {
                url,
                sha1,
                size,
                filename: fname.to_string(),
                version_id: vid,
                version_name: vname,
            });
        }
    }
    err("У выбранной версии нет файла .mrpack")
}

/// Файл сборки CurseForge: конкретная версия или последняя.
async fn curseforge_pack(
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
) -> Res<PackFile> {
    let (api, key) = curseforge::endpoint(state).await;
    let url = match version_id {
        Some(id) => format!("{api}/mods/{project_id}/files/{id}"),
        None => format!("{api}/mods/{project_id}/files?pageSize=1"),
    };
    let v: Value = curseforge::get(state, &url, &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let f = match version_id {
        Some(_) => v.get("data").cloned().unwrap_or(Value::Null),
        None => v
            .get("data")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null),
    };
    if f.is_null() {
        return err("У проекта нет файлов");
    }

    let fid = f.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
    let fname = f.get("fileName").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if fname.is_empty() {
        return err("У файла нет имени");
    }
    let url = match f.get("downloadUrl").and_then(|x| x.as_str()) {
        Some(u) if !u.is_empty() && (key.is_some() || curseforge::is_cdn_url(u)) => u.to_string(),
        _ => curseforge::forgecdn_url(fid, &fname),
    };
    let sha1 = f
        .get("hashes")
        .and_then(|x| x.as_array())
        .and_then(|a| a.iter().find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1)))
        .and_then(|h| h.get("value").and_then(|x| x.as_str()))
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let size = f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);
    let vname = f
        .get("displayName")
        .and_then(|x| x.as_str())
        .unwrap_or(&fname)
        .to_string();
    Ok(PackFile {
        url,
        sha1,
        size,
        filename: fname,
        version_id: fid.to_string(),
        version_name: vname,
    })
}

/// Скачивает файл сборки во временную папку.
async fn save_pack_file(app: &AppHandle, state: &AppState, f: &PackFile) -> Res<PathBuf> {
    let tmp = paths::tmp_dir();
    tokio::fs::create_dir_all(&tmp).await?;
    // Имя файла приходит из ответа каталога — берём только basename.
    let fname = Path::new(&f.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if fname.is_empty() || content::bad_component(fname) {
        return err("Недопустимое имя файла сборки");
    }
    let file = tmp.join(fname);
    download_all(
        app,
        &state.http,
        PACK_ID,
        vec![Dl {
            url: f.url.clone(),
            path: file.clone(),
            sha1: f.sha1.clone(),
            sha256: None,
            size: f.size,
        }],
        0.0,
        100.0,
    )
    .await?;
    Ok(file)
}

/// Скачивает выбранную (или последнюю) версию сборки и импортирует её.
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    source: &str,
    project_id: &str,
    version_id: Option<String>,
) -> Res<Instance> {
    let f = match source {
        "modrinth" => modrinth_pack(state, project_id, version_id.as_deref()).await?,
        "curseforge" => curseforge_pack(state, project_id, version_id.as_deref()).await?,
        _ => return err("Неизвестный источник"),
    };

    let file = save_pack_file(app, state, &f).await?;
    // Список файлов версии — чтобы при обновлении убрать то, чего в сборке больше нет.
    let mr_files = if f.filename.ends_with(".mrpack") {
        zip_json(&file, "modrinth.index.json")
            .ok()
            .flatten()
            .map(|index| index_paths(&index))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let manifest = if f.filename.ends_with(".mrpack") {
        None
    } else {
        zip_json(&file, "manifest.json").ok().flatten()
    };

    let result = import_pack(app, state, &file.to_string_lossy()).await;
    let _ = tokio::fs::remove_file(&file).await;
    let mut inst = result?;
    let files = match manifest {
        Some(m) => {
            let game = paths::instance_minecraft_dir(&inst.id);
            cf_manifest_files(state, &m, &game)
                .await
                .map(|(_, p)| p)
                .unwrap_or_default()
        }
        None => mr_files,
    };
    inst.pack = Some(crate::models::PackRef {
        source: source.to_string(),
        project_id: project_id.to_string(),
        version_id: f.version_id,
        version_name: f.version_name,
        files,
    });
    store::write_instance(&inst).await?;
    Ok(inst)
}

/// Доступное обновление сборки.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdate {
    pub version_id: String,
    pub version_name: String,
    pub changelog: String,
    pub date: String,
}

fn loader_tag(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Quilt => "quilt",
    }
}

/// Название загрузчика так, как его пишет CurseForge в `gameVersions`.
fn cf_loader_name(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "",
        Loader::Fabric => "Fabric",
        Loader::Forge => "Forge",
        Loader::Neoforge => "NeoForge",
        Loader::Quilt => "Quilt",
    }
}

/// Список изменений версии CurseForge (HTML → простой текст).
async fn curseforge_changelog(state: &AppState, project_id: &str, file_id: &str) -> String {
    let (api, key) = curseforge::endpoint(state).await;
    let url = format!("{api}/mods/{project_id}/files/{file_id}/changelog");
    let Ok(resp) = curseforge::get(state, &url, &key).send().await else {
        return String::new();
    };
    let Ok(v) = resp.json::<Value>().await else {
        return String::new();
    };
    let raw = v.get("data").and_then(|x| x.as_str()).unwrap_or("");
    html_text(raw)
}

/// Грубо превращает HTML из changelog в текст, понятный Markdown-рендереру.
fn html_text(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut tag = false;
    let mut buf = String::new();
    for ch in src.chars() {
        match ch {
            '<' => tag = true,
            '>' => {
                tag = false;
                let t = buf.trim().to_ascii_lowercase();
                if t.starts_with("br")
                    || t.starts_with("/p")
                    || t.starts_with("/li")
                    || t.starts_with("/h")
                    || t.starts_with("/div")
                {
                    out.push('\n');
                } else if t.starts_with("li") {
                    out.push_str("- ");
                }
                buf.clear();
            }
            _ if tag => buf.push(ch),
            _ => out.push(ch),
        }
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    let mut text = String::new();
    let mut blank = 0;
    for line in decoded.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        text.push_str(l);
        text.push('\n');
    }
    text.trim().to_string()
}

/// Есть ли для профиля более новая версия сборки (та же игра и загрузчик).
pub async fn check_pack(state: &AppState, id: &str) -> Res<Option<PackUpdate>> {
    let inst = store::read_instance(id).await?;
    let Some(pack) = inst.pack.clone() else {
        return Ok(None);
    };
    if pack.project_id.is_empty() {
        return Ok(None);
    }
    if pack.source == "curseforge" {
        let list = curseforge_versions(state, &pack.project_id).await?;
        let want = cf_loader_name(inst.loader);
        let found = list.into_iter().find(|v| {
            !v.id.is_empty()
                && v.mc_version == inst.mc_version
                && (want.is_empty() || v.loader == want)
        });
        let Some(v) = found else {
            return Ok(None);
        };
        if v.id == pack.version_id {
            return Ok(None);
        }
        let changelog = curseforge_changelog(state, &pack.project_id, &v.id).await;
        let version_name = if v.name.trim().is_empty() {
            format!("#{}", v.id)
        } else {
            v.name
        };
        return Ok(Some(PackUpdate {
            version_id: v.id,
            version_name,
            changelog,
            date: v.date,
        }));
    }
    if pack.source != "modrinth" {
        return Ok(None);
    }
    let v: Value = state
        .http
        .get(format!("{API_MR}/project/{}/version", pack.project_id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let empty = Vec::new();
    let arr = v.as_array().unwrap_or(&empty);
    let loader = loader_tag(inst.loader);
    for ver in arr {
        let vid = ver.get("id").and_then(|x| x.as_str()).unwrap_or("");
        if vid.is_empty() || !has_mrpack(ver) {
            continue;
        }
        let games = ver
            .get("game_versions")
            .and_then(|x| x.as_array())
            .unwrap_or(&empty);
        let loaders = ver.get("loaders").and_then(|x| x.as_array()).unwrap_or(&empty);
        let ok_game = games
            .iter()
            .any(|g| g.as_str() == Some(inst.mc_version.as_str()));
        let ok_loader = loaders.iter().any(|l| l.as_str() == Some(loader));
        if !ok_game || !ok_loader {
            continue;
        }
        if vid == pack.version_id {
            return Ok(None);
        }
        return Ok(Some(PackUpdate {
            version_id: vid.to_string(),
            version_name: ver
                .get("version_number")
                .and_then(|x| x.as_str())
                .unwrap_or(vid)
                .to_string(),
            changelog: ver
                .get("changelog")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            date: ver
                .get("date_published")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        }));
    }
    Ok(None)
}

/// Ставит новую версию сборки поверх профиля: докачивает изменённые файлы,
/// распаковывает overrides и убирает то, чего в новой версии больше нет.
pub async fn apply_update(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    version_id: &str,
) -> Res<Instance> {
    let mut inst = store::read_instance(id).await?;
    let Some(pack) = inst.pack.clone() else {
        return err("Профиль создан не из сборки — обновлять нечего");
    };
    if pack.project_id.is_empty() {
        return err("У сборки не записан проект каталога — обновите её вручную");
    }
    let f = match pack.source.as_str() {
        "modrinth" => modrinth_pack(state, &pack.project_id, Some(version_id)).await?,
        "curseforge" => curseforge_pack(state, &pack.project_id, Some(version_id)).await?,
        _ => return err("Неизвестный источник сборки"),
    };
    let file = save_pack_file(app, state, &f).await?;
    let game = paths::instance_minecraft_dir(id);

    // Что скачать из новой версии: mrpack и CurseForge-архив устроены по-разному.
    let prepared: Res<(Vec<Dl>, Vec<String>)> = async {
        if f.filename.ends_with(".mrpack") {
            let index = zip_json(&file, "modrinth.index.json")?
                .ok_or_else(|| AppError::Msg("В сборке нет modrinth.index.json".into()))?;
            index_files(&game, &index)
        } else {
            let manifest = zip_json(&file, "manifest.json")?
                .ok_or_else(|| AppError::Msg("В сборке нет manifest.json".into()))?;
            cf_manifest_files(state, &manifest, &game).await
        }
    }
    .await;

    let (dls, paths) = match prepared {
        Ok(v) => v,
        Err(e) => {
            let _ = tokio::fs::remove_file(&file).await;
            return Err(e);
        }
    };
    if !dls.is_empty() {
        download_all(app, &state.http, PACK_ID, dls, 0.0, 90.0).await?;
    }

    let zip2 = file.clone();
    let dest = game.clone();
    let unpacked = tokio::task::spawn_blocking(move || {
        extract_dirs(&zip2, &["overrides", "client-overrides"], &dest)
    })
    .await?;
    let _ = tokio::fs::remove_file(&file).await;
    unpacked?;

    // Файлы, которые ставила прошлая версия и которых нет в новой, убираем.
    for old in &pack.files {
        if paths.iter().any(|p| p == old) {
            continue;
        }
        if let Ok(p) = content::safe_join(&game, old) {
            let _ = tokio::fs::remove_file(&p).await;
        }
    }

    inst.pack = Some(crate::models::PackRef {
        source: pack.source,
        project_id: pack.project_id,
        version_id: f.version_id,
        version_name: f.version_name,
        files: paths,
    });
    store::write_instance(&inst).await?;
    let _ = app.emit("progress", Progress { id: PACK_ID.into(), pct: 100.0 });
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_html_becomes_text() {
        let s = html_text(
            "<p>Fixed <b>bugs</b></p><ul><li>One</li><li>Two</li></ul><br>Thanks &amp; enjoy",
        );
        assert!(s.contains("Fixed bugs"));
        assert!(s.contains("- One"));
        assert!(s.contains("Thanks & enjoy"));
        assert!(!s.contains('<'));
    }

    #[test]
    fn zip_rel_rejects_escape() {
        assert!(content::safe_rel("../evil.jar").is_none());
        assert!(content::safe_rel("mods/../../evil.jar").is_none());
        assert!(content::safe_rel("C:/Windows/x.dll").is_none());
        assert!(content::safe_rel("a.jar:stream").is_none());
        assert!(content::safe_rel("").is_none());
        assert!(content::safe_rel("./").is_none());
        // Windows схлопывает завершающие пробелы и точки: ".. " == "..".
        assert!(content::safe_rel(".. /evil.jar").is_none());
        assert!(content::safe_rel("mods/.. /evil.jar").is_none());
    }

    #[test]
    fn zip_rel_keeps_normal_path() {
        assert_eq!(
            content::safe_rel("mods/sodium.jar"),
            Some(PathBuf::from("mods").join("sodium.jar"))
        );
        assert_eq!(
            content::safe_rel("./config/a.toml"),
            Some(PathBuf::from("config").join("a.toml"))
        );
    }

    #[test]
    fn loader_deps_mapping() {
        let deps = serde_json::json!({ "minecraft": "1.20.1", "fabric-loader": "0.15.0" });
        let (loader, v) = loader_from_deps(&deps);
        assert_eq!(loader, Loader::Fabric);
        assert_eq!(v.as_deref(), Some("0.15.0"));
        assert_eq!(forge_version("1.20.1", Loader::Fabric, v), Some("0.15.0".into()));
    }

    #[test]
    fn forge_loader_version_gets_mc_prefix() {
        let deps = serde_json::json!({ "minecraft": "1.20.1", "forge": "47.2.0" });
        let (loader, v) = loader_from_deps(&deps);
        assert_eq!(loader, Loader::Forge);
        assert_eq!(forge_version("1.20.1", loader, v).as_deref(), Some("1.20.1-47.2.0"));
    }
}
