use super::*;

// ---------------------------------------------------------------------------
// Сохранённые списки модов и серверные паки
// ---------------------------------------------------------------------------

/// Все сохранённые списки модов.
#[tauri::command]
pub async fn modlists_list() -> Res<Vec<crate::modlists::SavedModList>> {
    let list = crate::modlists::list().await?;
    crate::plugins::hook(
        "mods.lists.list",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

/// Сохраняет текущий каталоговый контент профиля как список модов.
#[tauri::command]
pub async fn modlist_save(
    id: String,
    name: String,
) -> Res<crate::modlists::SavedModList> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "mods.lists.save.before",
        serde_json::json!({ "id": id, "name": name }),
    );
    let list = crate::modlists::save(&id, &name).await?;
    crate::plugins::hook(
        "mods.lists.save.after",
        serde_json::json!({ "id": id, "list": &list }),
    );
    Ok(list)
}

/// Удаляет сохранённый список.
#[tauri::command]
pub async fn modlist_delete(id: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "mods.lists.delete.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    crate::modlists::remove(&id).await?;
    crate::plugins::hook(
        "mods.lists.delete.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

/// Ставит все моды из сохранённого списка в профиль.
#[tauri::command]
pub async fn modlist_install(
    app: AppHandle,
    state: State<'_, AppState>,
    list_id: String,
    instance_id: String,
) -> Res<u32> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "mods.lists.install.before",
        serde_json::json!({ "listId": list_id, "instanceId": instance_id }),
    ) {
        return err(&msg);
    }
    let n = crate::modlists::install(&app, &state, &list_id, &instance_id).await?;
    crate::plugins::hook(
        "mods.lists.install.after",
        serde_json::json!({ "listId": list_id, "instanceId": instance_id, "count": n }),
    );
    Ok(n)
}

/// Собирает серверный пак (Vanilla/Paper/Purpur/Fabric/Quilt/Forge/NeoForge).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn build_server_pack(
    state: State<'_, AppState>,
    instance_id: Option<String>,
    kind: String,
    mc_version: String,
    loader_version: Option<String>,
    dest: String,
    ram_mb: Option<u32>,
) -> Res<crate::serverpack::ServerPackResult> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "server.pack.build.before",
        serde_json::json!({
            "instanceId": instance_id,
            "kind": kind,
            "mcVersion": mc_version,
            "loaderVersion": loader_version,
            "dest": dest,
        }),
    ) {
        return err(&msg);
    }
    let res = crate::serverpack::build(
        &state,
        instance_id.as_deref(),
        &kind,
        &mc_version,
        loader_version.as_deref(),
        &dest,
        ram_mb.unwrap_or(4096),
    )
    .await?;
    crate::plugins::hook(
        "server.pack.build.after",
        serde_json::json!({ "result": &res }),
    );
    Ok(res)
}

/// Открывает папку `config` профиля.
#[tauri::command]
pub async fn open_pack_configs(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("config");
    tokio::fs::create_dir_all(&dir).await?;
    crate::plugins::hook(
        "pack.configs.open",
        serde_json::json!({ "id": id, "path": dir.to_string_lossy() }),
    );
    open_path(&app, &dir)?;
    Ok(())
}

#[tauri::command]
pub async fn account_skin(state: State<'_, AppState>, id: String) -> Res<Option<crate::skins::Skin>> {
    crate::plugins::hook("auth.skin.before", serde_json::json!({ "id": id }));
    let skin = crate::skins::account_skin(&state, &id).await?;
    crate::plugins::hook(
        "auth.skin.after",
        serde_json::json!({ "id": id, "skin": &skin }),
    );
    Ok(skin)
}
