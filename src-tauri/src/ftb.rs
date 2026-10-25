//! Каталог сборок Feed The Beast.
//!
//! У FTB нет отдельных модов через публичный API, зато есть поиск сборок,
//! карточка и полный список файлов версии. Этого достаточно, чтобы ставить
//! сборку как обычный профиль и проверять обновления.

use crate::catalog::{
    CatalogItem, ProjectDetails, ProjectLink, SearchArgs, SearchResult,
};
use crate::error::Res;
use crate::models::Loader;
use crate::state::AppState;
use serde_json::Value;

const API: &str = "https://api.feed-the-beast.com/v1/modpacks";

/// Версия сборки FTB в удобном для лаунчера виде.
#[derive(Clone)]
pub struct VersionInfo {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub java_major: Option<u8>,
    /// release | beta | alpha
    pub kind: String,
    /// ISO-8601.
    pub date: String,
    released: i64,
}

pub async fn pack_json(state: &AppState, id: &str) -> Res<Value> {
    let url = format!("{API}/modpack/{id}");
    Ok(state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

pub async fn version_json(state: &AppState, pack_id: &str, version_id: &str) -> Res<Value> {
    let url = format!("{API}/modpack/{pack_id}/{version_id}");
    Ok(state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?)
}

/// Markdown-описание изменений версии; ошибки не критичны для обновления.
pub async fn changelog(state: &AppState, pack_id: &str, version_id: &str) -> String {
    let url = format!("{API}/modpack/{pack_id}/{version_id}/changelog");
    let Ok(resp) = state.http.get(url).send().await else {
        return String::new();
    };
    let Ok(v) = resp.json::<Value>().await else {
        return String::new();
    };
    v.get("content")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

/// Поиск только по сборкам: отдельные моды FTB через этот API не отдаёт.
pub async fn search(state: &AppState, args: &SearchArgs) -> Res<SearchResult> {
    if args.kind != "pk" {
        return Ok(SearchResult {
            hits: Vec::new(),
            total: 0,
        });
    }

    // Пустой `term` API считает ошибкой. Для пустого запроса показываем все
    // сборки FTB — они все так или иначе относятся к Feed The Beast.
    let term = args.query.trim();
    let term = if term.is_empty() { "ftb" } else { term };
    let url = format!(
        "{API}/modpack/search/200/detailed/?term={}&platform=ftb",
        urlencoding::encode(term)
    );
    let v: Value = state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let packs = v
        .get("packs")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let filtered: Vec<Value> = packs
        .into_iter()
        .filter(|p| matches_filters(p, args))
        .collect();
    let total = filtered.len() as u64;
    let start = args.offset as usize;
    let hits = filtered
        .iter()
        .skip(start)
        .take(20)
        .filter_map(|p| hit(p))
        .collect();
    Ok(SearchResult { hits, total })
}

pub async fn project(state: &AppState, id: &str) -> Res<ProjectDetails> {
    let v = pack_json(state, id).await?;
    let title = str_of(&v, "name");
    let synopsis = str_of(&v, "synopsis");
    let description = str_of(&v, "description");
    let slug = str_of(&v, "slug");
    let mut links = Vec::new();
    if let Some(raw) = v.get("links").and_then(|x| x.as_array()) {
        for link in raw {
            let label = str_of(link, "name");
            let url = str_of(link, "link");
            if !url.is_empty() {
                links.push(ProjectLink {
                    label: if label.is_empty() {
                        "Link".to_string()
                    } else {
                        label
                    },
                    url,
                });
            }
        }
    }
    let site = if slug.is_empty() {
        format!("https://feed-the-beast.com/modpacks/{id}")
    } else {
        format!("https://feed-the-beast.com/modpacks/{id}-{slug}")
    };
    links.push(ProjectLink {
        label: "Feed The Beast".into(),
        url: site,
    });

    Ok(ProjectDetails {
        id: id.to_string(),
        source: "ftb".into(),
        title,
        description: if synopsis.is_empty() {
            description.clone()
        } else {
            synopsis
        },
        body: description,
        downloads: count(&v, "plays").max(count(&v, "installs")),
        icon_url: icon(&v),
        links,
    })
}

/// Список версий: от новых к старым, без приватных.
pub async fn versions(state: &AppState, project_id: &str) -> Res<Vec<VersionInfo>> {
    let v = pack_json(state, project_id).await?;
    let mut out: Vec<VersionInfo> = v
        .get("versions")
        .and_then(|x| x.as_array())
        .map(|list| {
            list.iter()
                .filter(|x| !x.get("private").and_then(|b| b.as_bool()).unwrap_or(false))
                .filter_map(version_info)
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.released.cmp(&a.released));
    // Если есть стабильные версии, не предлагаем бету/альфу по умолчанию.
    let stable: Vec<VersionInfo> = out
        .iter()
        .filter(|v| v.kind.eq_ignore_ascii_case("release"))
        .cloned()
        .collect();
    if !stable.is_empty() {
        return Ok(stable);
    }
    Ok(out)
}

fn version_info(v: &Value) -> Option<VersionInfo> {
    let id = num_str(v, "id");
    if id.is_empty() {
        return None;
    }
    let targets = v.get("targets").and_then(|x| x.as_array())?;
    let mc = target(targets, "game", "minecraft")?;
    let mc_version = str_of(mc, "version");
    if mc_version.is_empty() {
        return None;
    }

    let (loader, loader_version) = loader_of(targets, &mc_version);
    let java_major = target(targets, "runtime", "java")
        .or_else(|| target(targets, "java", "java"))
        .and_then(|t| major_of(&str_of(t, "version")));
    let released = v.get("released").and_then(|x| x.as_i64()).unwrap_or(0);
    let date = if released > 0 {
        chrono::DateTime::from_timestamp(released, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default()
    } else {
        String::new()
    };

    Some(VersionInfo {
        id,
        name: str_of(v, "name"),
        mc_version,
        loader,
        loader_version,
        java_major,
        kind: str_of(v, "type"),
        date,
        released,
    })
}

fn loader_of(targets: &[Value], mc: &str) -> (Loader, Option<String>) {
    for t in targets {
        let name = str_of(t, "name").to_ascii_lowercase();
        let kind = str_of(t, "type").to_ascii_lowercase();
        if kind != "modloader" && kind != "loader" && kind != "mod_loader" {
            continue;
        }
        let ver = str_of(t, "version");
        let ver = ver.trim();
        if ver.is_empty() {
            continue;
        }
        let (loader, value) = match name.as_str() {
            "neoforge" => (Loader::Neoforge, forge_value(mc, ver)),
            "forge" => (Loader::Forge, forge_value(mc, ver)),
            "fabric" => (Loader::Fabric, ver.to_string()),
            "quilt" => (Loader::Quilt, ver.to_string()),
            _ => continue,
        };
        return (loader, Some(value));
    }
    (Loader::Vanilla, None)
}

/// Forge в FTB иногда пишет версию как `1.20.1-47.2.0`, а лаунчер ждёт
/// только `47.2.0` и сам добавляет версию Minecraft.
fn forge_value(mc: &str, version: &str) -> String {
    version
        .strip_prefix(&format!("{mc}-"))
        .unwrap_or(version)
        .to_string()
}

fn major_of(version: &str) -> Option<u8> {
    let parts: Vec<u8> = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect();
    if parts.first() == Some(&1) && parts.get(1) == Some(&8) {
        Some(8)
    } else {
        parts.first().copied()
    }
}

fn target<'a>(targets: &'a [Value], kind: &str, name: &str) -> Option<&'a Value> {
    targets.iter().find(|t| {
        let tkind = str_of(t, "type").to_ascii_lowercase();
        let tname = str_of(t, "name").to_ascii_lowercase();
        tkind == kind || tname == name
    })
}

fn matches_filters(v: &Value, args: &SearchArgs) -> bool {
    let tags = tag_names(v);
    if let Some(mc) = args.mc_version.as_deref().filter(|s| !s.is_empty()) {
        if !tags.iter().any(|t| t.eq_ignore_ascii_case(mc)) {
            return false;
        }
    }
    if !args.loaders.is_empty() {
        let ok = args.loaders.iter().any(|l| {
            tags.iter().any(|t| {
                t.eq_ignore_ascii_case(l)
                    || (l.eq_ignore_ascii_case("neoforge") && t.eq_ignore_ascii_case("neo forge"))
            })
        });
        if !ok {
            return false;
        }
    }
    true
}

fn hit(v: &Value) -> Option<CatalogItem> {
    let id = num_str(v, "id");
    if id.is_empty() {
        return None;
    }
    let tags = tag_names(v);
    let loaders = tags
        .iter()
        .filter(|t| is_loader(t))
        .map(|t| loader_label(t))
        .collect();
    let categories = tags
        .iter()
        .filter(|t| !is_loader(t) && !looks_like_version(t))
        .cloned()
        .collect();
    let author = v
        .get("authors")
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .map(|a| str_of(a, "name"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "FTB".into());

    Some(CatalogItem {
        id,
        title: str_of(v, "name"),
        author,
        description: str_of(v, "synopsis"),
        downloads: count(v, "plays").max(count(v, "installs")),
        loaders,
        categories,
        icon_url: icon(v),
        source: "ftb".into(),
        kind: "pk".into(),
        installed: false,
    })
}

fn tag_names(v: &Value) -> Vec<String> {
    v.get("tags")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    t.as_str()
                        .map(str::to_string)
                        .or_else(|| t.get("name").and_then(|x| x.as_str()).map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_loader(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "fabric" | "forge" | "neoforge" | "neo forge" | "quilt"
    )
}

fn loader_label(tag: &str) -> String {
    match tag.to_ascii_lowercase().as_str() {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" | "neo forge" => "NeoForge",
        "quilt" => "Quilt",
        _ => tag,
    }
    .to_string()
}

fn looks_like_version(tag: &str) -> bool {
    tag.chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
        && tag.contains('.')
}

fn icon(v: &Value) -> Option<String> {
    let art = v.get("art").and_then(|x| x.as_array())?;
    art.iter()
        .find(|a| str_of(a, "type").eq_ignore_ascii_case("square"))
        .or_else(|| art.first())
        .map(|a| str_of(a, "url"))
        .filter(|u| !u.is_empty())
}

fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn num_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| {
            x.as_str()
                .map(str::to_string)
                .or_else(|| x.as_i64().map(|n| n.to_string()))
                .or_else(|| x.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_default()
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ftb_targets() {
        let v = serde_json::json!({
            "id": 100,
            "name": "1.0.0",
            "released": 1_700_000_000,
            "targets": [
                { "name": "minecraft", "version": "1.20.1", "type": "game" },
                { "name": "forge", "version": "1.20.1-47.2.0", "type": "modloader" }
            ]
        });
        let info = version_info(&v).unwrap_or_else(|| panic!("version"));
        assert_eq!(info.mc_version, "1.20.1");
        assert_eq!(info.loader, Loader::Forge);
        assert_eq!(info.loader_version.as_deref(), Some("47.2.0"));
        assert!(!info.date.is_empty());
        assert_eq!(major_of("25.0.2+10-LTS"), Some(25));
        assert_eq!(major_of("1.8.0_402"), Some(8));
    }

    #[tokio::test]
    #[ignore = "network"]
    async fn live_ftb_search_and_versions() {
        let state = AppState::new();
        let args = SearchArgs {
            source: "ftb".into(),
            kind: "pk".into(),
            query: "ftb".into(),
            loaders: Vec::new(),
            mc_version: None,
            categories: Vec::new(),
            offset: 0,
            sort: "relevance".into(),
            instance_id: None,
        };
        let found = search(&state, &args).await.unwrap_or_else(|e| panic!("{e}"));
        assert!(!found.hits.is_empty());
        assert!(found.hits.iter().all(|h| h.source == "ftb"));

        let list = versions(&state, "135").await.unwrap_or_else(|e| panic!("{e}"));
        assert!(!list.is_empty());
        assert!(list.iter().all(|v| !v.mc_version.is_empty()));
    }
}
