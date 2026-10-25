//! Каталог контента: Modrinth и CurseForge (ТЗ §10).

pub mod curseforge;
pub mod modrinth;

use crate::content::{self, ContentMeta};
use crate::error::{err, Res};
use crate::models::{Instance, Loader};
use crate::state::AppState;
use crate::store;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::AppHandle;

/// Прогресс установки из каталога идёт под этим id, пока профиля ещё нет.
pub const PACK_ID: &str = "pack";

/// Id прогресса скачивания контента в профиль.
///
/// Отдельный от id профиля: иначе фронтенд принимает установку мода за запуск
/// игры и кнопка «Играть» навсегда застревает на «Запуск Java… 100%».
pub(crate) fn content_progress_id(instance_id: &str) -> String {
    format!("content:{instance_id}")
}

/// Ресурс-паки и шейдеры не привязаны к версии игры: их можно ставить на любой
/// профиль (текстуры для 1.12.2 работают и на 1.20.1).
pub fn any_version(kind: &str) -> bool {
    kind == "rp" || kind == "sh"
}

/// Есть ли у проекта файл под профиль (версия игры + загрузчик).
///
/// `None` — проверить не удалось (сеть, чужой ответ): вызывающий решает сам.
pub async fn has_file(
    state: &AppState,
    source: &str,
    project_id: &str,
    kind: &str,
    loader: &str,
    mc: &str,
) -> Option<bool> {
    let res = match source {
        "modrinth" => modrinth::has_file(&state.http, project_id, kind, loader, mc).await,
        "curseforge" => curseforge::has_file(state, project_id, kind, loader, mc).await,
        _ => return None,
    };
    res.ok()
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchArgs {
    pub source: String,
    pub kind: String,
    pub query: String,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub mc_version: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub offset: u32,
    /// downloads | newest | updated
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub loaders: Vec<String>,
    pub categories: Vec<String>,
    pub icon_url: Option<String>,
    pub source: String,
    pub kind: String,
    pub installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<CatalogItem>,
    pub total: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub file: String,
    pub name: String,
    pub new_version: String,
}

/// Ссылка проекта: исходники, баг-трекер, Discord, пожертвования.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLink {
    pub label: String,
    pub url: String,
}

/// Подробности проекта для страницы мода.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub id: String,
    pub source: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub links: Vec<ProjectLink>,
}

/// Добавляет ссылку, если адрес непустой.
pub(crate) fn push_link(links: &mut Vec<ProjectLink>, label: &str, url: Option<&str>) {
    if let Some(u) = url.map(str::trim).filter(|u| !u.is_empty()) {
        links.push(ProjectLink {
            label: label.to_string(),
            url: u.to_string(),
        });
    }
}

/// Грубо вырезает теги: описания CurseForge приходят HTML-ом. Блочные теги
/// превращаем в переводы строк, чтобы абзацы не слипались (описание потом
/// рендерится как Markdown).
pub(crate) fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                let raw = tag.trim();
                let closing = raw.starts_with('/');
                let name = raw
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let block = matches!(
                    name.as_str(),
                    "p" | "div"
                        | "li"
                        | "tr"
                        | "ul"
                        | "ol"
                        | "table"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                );
                if block || (!closing && matches!(name.as_str(), "br" | "hr")) {
                    out.push('\n');
                } else if closing && name == "td" {
                    out.push(' ');
                }
            }
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    // Схлопываем повторяющиеся пустые строки.
    let mut res = String::with_capacity(decoded.len());
    let mut blank = false;
    for line in decoded.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            if blank {
                continue;
            }
            blank = true;
        } else {
            blank = false;
        }
        res.push_str(l);
        res.push('\n');
    }
    res.trim().to_string()
}

/// Страница проекта (мод, сборка, шейдер…) для выбранного источника.
pub async fn project(state: &AppState, source: &str, id: &str) -> Res<ProjectDetails> {
    match source {
        "curseforge" => curseforge::project(state, id).await,
        "ftb" => crate::ftb::project(state, id).await,
        _ => modrinth::project(&state.http, id).await,
    }
}

/// Обновление, найденное для одного файла профиля.
pub(crate) struct NewFile {
    pub version_id: String,
    pub version_number: String,
    pub url: String,
    pub filename: String,
    pub sha1: String,
    pub size: u64,
}

/// Slug загрузчика в терминах Modrinth. У Vanilla его нет.
pub(crate) fn loader_slug(l: Loader) -> Option<&'static str> {
    match l {
        Loader::Fabric => Some("fabric"),
        Loader::Forge => Some("forge"),
        Loader::Neoforge => Some("neoforge"),
        Loader::Quilt => Some("quilt"),
        Loader::Vanilla => None,
    }
}

/// Папка назначения внутри `minecraft/` для вида контента.
pub(crate) fn dest_dir(kind: &str, world: Option<&str>) -> Res<String> {
    Ok(match kind {
        "mod" => "mods".to_string(),
        "rp" => "resourcepacks".to_string(),
        "sh" => "shaderpacks".to_string(),
        "dp" => {
            let w = match world {
                Some(w) if !w.trim().is_empty() => w,
                _ => return err("Выберите мир"),
            };
            format!("saves/{w}/datapacks")
        }
        _ => return err("Неизвестный вид контента"),
    })
}

pub(crate) fn str_of(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub(crate) fn str_arr(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Поиск по каталогу выбранного источника.
pub async fn search(state: &AppState, args: SearchArgs) -> Res<SearchResult> {
    let mut res = match args.source.as_str() {
        "modrinth" => modrinth::search(&state.http, &args).await?,
        "curseforge" => curseforge::search(state, &args).await?,
        "ftb" => crate::ftb::search(state, &args).await?,
        _ => return err("Неизвестный источник"),
    };

    // `installed` — есть ли такой project_id в content.json профиля (для pk всегда false).
    if let Some(id) = &args.instance_id {
        if args.kind != "pk" {
            let meta = content::load_meta(id).await;
            let set: HashSet<&str> = meta.files.values().map(|e| e.project_id.as_str()).collect();
            for h in &mut res.hits {
                h.installed = set.contains(h.id.as_str());
            }
        }
    }
    Ok(res)
}

/// Дополняет контент иконками проектов Modrinth, если своей в архиве нет.
pub async fn fill_icons(c: &reqwest::Client, items: &mut [content::ContentItem]) {
    let ids: Vec<String> = items
        .iter()
        .filter(|i| i.icon.is_none() && i.source.as_deref() == Some("modrinth"))
        .filter_map(|i| i.project_id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    let icons = modrinth::project_icons(c, &ids).await;
    for i in items.iter_mut() {
        if i.icon.is_some() || i.source.as_deref() != Some("modrinth") {
            continue;
        }
        if let Some(url) = i.project_id.as_ref().and_then(|p| icons.get(p)) {
            i.icon_url = Some(url.clone());
        }
    }
}

/// Установка элемента каталога в профиль.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    source: &str,
    project_id: &str,
    kind: &str,
    world: Option<String>,
) -> Res<()> {
    let inst: Instance = store::read_instance(instance_id).await?;

    // Мод без загрузчика не заработает: в ванильном профиле его некуда ставить.
    if kind == "mod" && loader_slug(inst.loader).is_none() {
        return err(
            "В ванильном профиле нет загрузчика модов. Создайте профиль с Fabric, Forge, Quilt или NeoForge",
        );
    }

    let dir = dest_dir(kind, world.as_deref())?;
    let mut meta: ContentMeta = content::load_meta(instance_id).await;

    match source {
        "modrinth" => modrinth::install(app, state, &inst, project_id, kind, &dir, &mut meta).await?,
        "curseforge" => curseforge::install(app, state, &inst, project_id, kind, &dir, &mut meta).await?,
        _ => return err("Неизвестный источник"),
    }

    content::save_meta(instance_id, &meta).await
}

/// Обновления для файлов, установленных через Modrinth.
pub async fn check_updates(state: &AppState, id: &str) -> Res<Vec<UpdateInfo>> {
    let inst = store::read_instance(id).await?;
    let meta = content::load_meta(id).await;
    let found = modrinth::resolve_updates(state, &inst, &meta).await?;
    Ok(found
        .into_iter()
        .map(|(file, new)| UpdateInfo {
            file,
            name: new.filename,
            new_version: new.version_number,
        })
        .collect())
}

/// Скачивает найденные обновления, удаляет старые файлы, правит content.json.
/// `only` — какие файлы обновлять; пустой список значит «все найденные».
pub async fn apply_updates(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    only: Vec<String>,
) -> Res<u32> {
    let inst: Instance = store::read_instance(id).await?;
    let mut meta = content::load_meta(id).await;
    let found = modrinth::resolve_updates(state, &inst, &meta).await?;
    if found.is_empty() {
        return Ok(0);
    }

    let game = crate::paths::instance_minecraft_dir(id);
    let mut n = 0u32;
    for (file, new) in found {
        if !only.is_empty() && !only.iter().any(|f| *f == file) {
            continue;
        }
        let Some(entry) = meta.files.get(&file).cloned() else {
            continue;
        };
        // Имя файла могло измениться вместе с версией — кладём рядом со старым.
        let dir = match file.rsplit_once('/') {
            Some((d, _)) => d.to_string(),
            None => String::new(),
        };
        let rel = if dir.is_empty() {
            new.filename.clone()
        } else {
            format!("{dir}/{}", new.filename)
        };
        let path = content::safe_join(&game, &rel)?;
        let dl = crate::http::Dl {
            url: new.url.clone(),
            path,
            sha1: if new.sha1.is_empty() {
                None
            } else {
                Some(new.sha1.clone())
            },
            sha256: None,
            size: new.size,
        };
        crate::http::download_all(
            app,
            &state.http,
            &content_progress_id(id),
            vec![dl],
            0.0,
            100.0,
        )
        .await?;

        // Старый файл убираем, если имя изменилось.
        if rel != file {
            if let Ok(old) = content::safe_join(&game, &file) {
                let _ = tokio::fs::remove_file(&old).await;
            }
            meta.files.remove(&file);
        }
        meta.files.insert(
            rel,
            content::Entry {
                source: entry.source,
                project_id: entry.project_id,
                version_id: new.version_id,
                sha1: new.sha1,
            },
        );
        n += 1;
    }

    content::save_meta(id, &meta).await?;
    Ok(n)
}
