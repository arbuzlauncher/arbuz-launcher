use super::*;

// ---------------------------------------------------------------------------
// Каталог серверов (Б3)
// ---------------------------------------------------------------------------

/// Список серверов из статичного JSON (кэш; refresh — перечитать по URL).
#[tauri::command]
pub async fn servers_catalog(state: State<'_, AppState>, refresh: bool) -> Res<Vec<crate::servers::CatalogServer>> {
    let url = state.config.lock().await.servers_url.clone();
    crate::plugins::hook(
        "servers.fetch.before",
        serde_json::json!({ "url": url, "refresh": refresh }),
    );
    let list = crate::servers::catalog(&state.http, &url, refresh).await?;
    crate::plugins::hook(
        "servers.fetch.after",
        serde_json::json!({ "url": url, "count": list.len() }),
    );
    Ok(list)
}

/// Server List Ping одного сервера.
#[tauri::command]
pub async fn server_ping(ip: String) -> Res<crate::servers::PingInfo> {
    crate::plugins::hook("servers.ping.before", serde_json::json!({ "ip": ip }));
    let info = crate::servers::ping(&ip).await;
    crate::plugins::hook(
        "servers.ping.after",
        serde_json::json!({ "ip": ip, "ping": &info }),
    );
    Ok(info)
}

/// Список серверов из servers.dat профиля (только чтение).
#[tauri::command]
pub async fn servers_dat_list(id: String) -> Res<Vec<crate::servers::ServerEntry>> {
    let list = crate::servers::list_servers(&id).await?;
    crate::plugins::hook(
        "servers.dat.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

/// Добавляет сервер в servers.dat профиля.
#[tauri::command]
pub async fn servers_dat_add(id: String, name: String, ip: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "servers.dat.add.before",
        serde_json::json!({ "id": id, "name": name, "ip": ip }),
    ) {
        return err(&msg);
    }
    crate::servers::add_server(&id, &name, &ip).await?;
    crate::plugins::hook(
        "servers.dat.add.after",
        serde_json::json!({ "id": id, "name": name, "ip": ip }),
    );
    Ok(())
}

/// Закреплённые серверы профиля.
#[tauri::command]
pub async fn pinned_servers(id: String) -> Res<Vec<crate::servers::PinnedServer>> {
    let list = crate::servers::pinned_list(&id).await?;
    crate::plugins::hook(
        "servers.pinned.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

/// Закрепить сервер на профиле (или снять закрепление).
#[tauri::command]
pub async fn pin_server(id: String, name: String, ip: String, pin: bool) -> Res<()> {
    crate::plugins::hook(
        "servers.pin.before",
        serde_json::json!({ "id": id, "name": name, "ip": ip, "pin": pin }),
    );
    crate::servers::pin(&id, &name, &ip, pin).await?;
    crate::plugins::hook(
        "servers.pin.after",
        serde_json::json!({ "id": id, "name": name, "ip": ip, "pin": pin }),
    );
    Ok(())
}
