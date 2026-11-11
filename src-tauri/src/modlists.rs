//! Сохранённые списки модов: снимок содержимого профиля и установка в другой
//! профиль через обычный каталог (Modrinth/CurseForge).

use crate::error::{err, Res};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedMod {
    pub source: String,
    pub project_id: String,
    pub kind: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedModList {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub mc_version: String,
    pub loader: String,
    pub mods: Vec<SavedMod>,
}

fn dir() -> PathBuf {
    paths::root().join("modlists")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub async fn list() -> Res<Vec<SavedModList>> {
    let dir = dir();
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(&dir).await else {
        return Ok(out);
    };
    while let Ok(Some(e)) = rd.next_entry().await {
        if e.path().extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        if let Ok(raw) = tokio::fs::read_to_string(e.path()).await {
            if let Ok(l) = serde_json::from_str::<SavedModList>(&raw) {
                out.push(l);
            }
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(out)
}

pub async fn save(instance_id: &str, name: &str) -> Res<SavedModList> {
    let inst = crate::store::read_instance(instance_id).await?;
    let items = crate::content::list(instance_id).await?;
    let mut mods = Vec::new();
    for it in items {
        if !matches!(it.kind.as_str(), "mod" | "rp" | "sh") {
            continue;
        }
        let (Some(source), Some(project_id)) = (it.source.clone(), it.project_id.clone()) else {
            continue;
        };
        mods.push(SavedMod {
            source,
            project_id,
            kind: it.kind,
            name: it.name,
            version: it.version,
        });
    }
    if mods.is_empty() {
        return err("В профиле нет контента из каталога — сохранять нечего");
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    let list = SavedModList {
        id: id.clone(),
        name: if name.trim().is_empty() {
            format!("Список {}", now())
        } else {
            name.trim().chars().take(64).collect()
        },
        created: now(),
        mc_version: inst.mc_version,
        loader: inst.loader.label().to_string(),
        mods,
    };
    tokio::fs::create_dir_all(dir()).await?;
    tokio::fs::write(
        dir().join(format!("{id}.json")),
        serde_json::to_string_pretty(&list)?,
    )
    .await?;
    Ok(list)
}

pub async fn remove(id: &str) -> Res<()> {
    if !safe_id(id) {
        return err("Некорректный id списка");
    }
    let p = dir().join(format!("{id}.json"));
    if !p.is_file() {
        return err("Список не найден");
    }
    tokio::fs::remove_file(p).await?;
    Ok(())
}

/// Ставит все моды списка в профиль. Возвращает число установленных.
pub async fn install(
    app: &tauri::AppHandle,
    state: &crate::state::AppState,
    list_id: &str,
    instance_id: &str,
) -> Res<u32> {
    if !safe_id(list_id) {
        return err("Некорректный id списка");
    }
    paths::validate_instance_id(instance_id)?;
    let raw = tokio::fs::read_to_string(dir().join(format!("{list_id}.json"))).await?;
    let list: SavedModList = serde_json::from_str(&raw)?;
    let mut n = 0u32;
    for m in &list.mods {
        crate::catalog::install(
            app,
            state,
            instance_id,
            &m.source,
            &m.project_id,
            &m.kind,
            None,
        )
        .await?;
        n += 1;
    }
    Ok(n)
}
