use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 2
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_mc_versions(
    state: State<'_, AppState>,
) -> Res<Vec<crate::mojang::versions::McVersion>> {
    let list = crate::mojang::versions::list(&state.http).await?;
    crate::plugins::hook(
        "minecraft.versions.list",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

/// Добавляет пользовательскую версию (свой version.json и/или jar).
#[tauri::command]
pub async fn add_custom_version(
    id: String,
    base: String,
    json_path: Option<String>,
    jar_path: Option<String>,
) -> Res<String> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "minecraft.version.add.before",
        serde_json::json!({ "id": id, "base": base }),
    ) {
        return err(&msg);
    }
    let id = crate::mojang::versions::add_custom(
        &id,
        &base,
        json_path.as_deref(),
        jar_path.as_deref(),
    )
    .await?;
    crate::plugins::hook(
        "minecraft.version.add.after",
        serde_json::json!({ "id": id }),
    );
    Ok(id)
}

#[tauri::command]
pub async fn remove_custom_version(id: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "minecraft.version.remove.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    crate::mojang::versions::remove_custom(&id).await?;
    crate::plugins::hook(
        "minecraft.version.remove.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: Loader,
    mc_version: String,
) -> Res<Vec<String>> {
    let list = crate::loaders::versions(&state.http, loader, &mc_version).await?;
    let out: Vec<String> = list.into_iter().map(|v| v.version).collect();
    crate::plugins::hook(
        "minecraft.loader.versions",
        serde_json::json!({ "loader": loader.label(), "mcVersion": mc_version, "count": out.len() }),
    );
    Ok(out)
}

/// Подробности проекта каталога для страницы мода.
#[tauri::command]
pub async fn catalog_project(
    state: State<'_, AppState>,
    source: String,
    id: String,
) -> Res<crate::catalog::ProjectDetails> {
    crate::plugins::hook(
        "catalog.project.before",
        serde_json::json!({ "source": source, "id": id }),
    );
    let p = crate::catalog::project(&state, &source, &id).await?;
    crate::plugins::hook(
        "catalog.project.after",
        serde_json::json!({ "source": source, "id": id, "project": &p }),
    );
    Ok(p)
}

/// Состояние сессии аккаунта: "ok", "expired" или "offline".
/// Для Microsoft/Ely.by пробуем получить токен — если обновление не прошло,
/// сессия действительно истекла и нужен повторный вход.
#[tauri::command]
pub async fn account_status(state: State<'_, AppState>, id: String) -> Res<String> {
    let acc = crate::accounts::list()
        .await?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::Msg("Аккаунт не найден".into()))?;
    let ok = match acc.kind {
        crate::models::AccountKind::Offline => return Ok("offline".into()),
        crate::models::AccountKind::Microsoft => {
            crate::accounts::microsoft::token(&state.http, &acc).await.is_ok()
        }
        crate::models::AccountKind::Elyby => {
            crate::accounts::elyby::token(&state.http, &acc).await.is_ok()
        }
        crate::models::AccountKind::Littleskin => {
            crate::accounts::littleskin::token(&state.http, &acc).await.is_ok()
        }
        crate::models::AccountKind::Plugin => {
            crate::accounts::plugin_token(&acc.id).is_some()
        }
    };
    Ok(if ok { "ok".into() } else { "expired".into() })
}

#[tauri::command]
pub async fn detect_javas() -> Res<Vec<crate::java::JavaInstall>> {
    crate::plugins::hook("java.detect.before", serde_json::json!({}));
    let list = crate::java::detect_javas().await?;
    crate::plugins::hook(
        "java.detect.after",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

/// Скачивает JRE нужной версии с Adoptium прямо из лаунчера.
#[tauri::command]
pub async fn install_java(
    app: AppHandle,
    state: State<'_, AppState>,
    major: u8,
) -> Res<String> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "java.install.before",
        serde_json::json!({ "major": major }),
    ) {
        return err(&msg);
    }
    let path = crate::java::ensure_java(&app, &state.http, "java", major, 0.0, 100.0).await?;
    let path = path.to_string_lossy().to_string();
    crate::plugins::hook(
        "java.install.after",
        serde_json::json!({ "major": major, "path": path }),
    );
    Ok(path)
}

#[tauri::command]
pub async fn list_accounts() -> Res<Vec<Account>> {
    let list = crate::accounts::list().await?;
    crate::plugins::hook(
        "auth.accounts.list",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

#[tauri::command]
pub async fn offline_add(state: State<'_, AppState>, name: String) -> Res<Account> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "auth.account.add.before",
        serde_json::json!({ "name": name, "kind": "offline" }),
    ) {
        return err(&msg);
    }
    let acc = crate::accounts::offline::add(&name).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.account.add.after",
        serde_json::json!({ "account": &acc }),
    );
    Ok(acc)
}

#[tauri::command]
pub async fn set_active_account(state: State<'_, AppState>, id: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "auth.account.switch.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    crate::accounts::set_active(&id).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.account.switch.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "auth.account.remove.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    crate::accounts::remove(&id).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.account.remove.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn little_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Res<Account> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "auth.login.before",
        serde_json::json!({ "provider": "littleskin", "login": login }),
    ) {
        return err(&msg);
    }
    let acc = crate::accounts::littleskin::login(&state.http, login.trim(), &password).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.login.after",
        serde_json::json!({ "provider": "littleskin", "account": &acc }),
    );
    Ok(acc)
}

#[tauri::command]
pub async fn ely_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Res<Account> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "auth.login.before",
        serde_json::json!({ "provider": "elyby", "login": login }),
    ) {
        return err(&msg);
    }
    let acc = crate::accounts::elyby::login(&state.http, login.trim(), &password).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.login.after",
        serde_json::json!({ "provider": "elyby", "account": &acc }),
    );
    Ok(acc)
}

#[tauri::command]
pub async fn ms_begin(state: State<'_, AppState>) -> Res<crate::accounts::microsoft::DeviceCode> {
    crate::plugins::hook("auth.microsoft.begin", serde_json::json!({}));
    let code = crate::accounts::microsoft::begin(&state).await?;
    crate::plugins::hook(
        "auth.microsoft.begin.after",
        serde_json::json!({ "code": &code }),
    );
    Ok(code)
}

#[tauri::command]
pub async fn ms_poll(
    app: AppHandle,
    state: State<'_, AppState>,
    device_code: String,
    interval: u64,
) -> Res<Account> {
    crate::plugins::hook(
        "auth.microsoft.poll",
        serde_json::json!({ "interval": interval }),
    );
    let acc = crate::accounts::microsoft::poll(&app, &state, &device_code, interval).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    crate::plugins::hook(
        "auth.login.after",
        serde_json::json!({ "provider": "microsoft", "account": &acc }),
    );
    Ok(acc)
}

#[tauri::command]
pub async fn ms_cancel(state: State<'_, AppState>) -> Res<()> {
    crate::plugins::hook("auth.microsoft.cancel", serde_json::json!({}));
    crate::accounts::microsoft::cancel(&state);
    Ok(())
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    world: Option<String>,
    server: Option<String>,
    // Пользователь подтвердил «запустить всё равно» — пропустить гейт сканера.
    scan_override: Option<bool>,
) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "instance.launch.request",
        serde_json::json!({ "id": id, "world": world, "server": server, "scanOverride": scan_override }),
    );
    crate::mojang::launch::launch(app, state.inner(), id.clone(), world, server, scan_override.unwrap_or(false)).await?;
    crate::plugins::hook(
        "instance.launch.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "game.stop.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    let mut running = state.running.lock().await;
    if let Some(tx) = running.remove(&id) {
        let _ = tx.send(());
    }
    drop(running);
    crate::plugins::hook(
        "game.stop.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}
