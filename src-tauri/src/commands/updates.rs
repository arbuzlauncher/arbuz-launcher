use super::*;

// ---------------------------------------------------------------------------
// Обновление лаунчера
// ---------------------------------------------------------------------------

/// Есть ли на GitHub более новый релиз лаунчера.
#[tauri::command]
pub async fn check_launcher_update(
    state: State<'_, AppState>,
) -> Res<Option<crate::updates::LauncherUpdate>> {
    crate::plugins::hook("launcher.update.check.before", serde_json::json!({}));
    let upd = crate::updates::check(&state).await?;
    crate::plugins::hook(
        "launcher.update.check.after",
        serde_json::json!({ "update": &upd }),
    );
    Ok(upd)
}

/// Скачивает обновление лаунчера и возвращает путь к файлу.
#[tauri::command]
pub async fn launcher_update_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Res<String> {
    crate::plugins::hook("launcher.update.download.before", serde_json::json!({}));
    let upd = crate::updates::check(&state)
        .await?
        .ok_or_else(|| AppError::Msg("Обновлений нет".into()))?;
    let path = crate::updates::download(&state, &app, &upd).await?;
    crate::plugins::hook(
        "launcher.update.download.after",
        serde_json::json!({ "path": path, "update": &upd }),
    );
    Ok(path.to_string_lossy().to_string())
}

/// Устанавливает скачанное обновление и перезапускает лаунчер.
#[tauri::command]
pub async fn launcher_update_install(app: AppHandle, path: String) -> Res<()> {
    crate::plugins::hook(
        "launcher.update.install.before",
        serde_json::json!({ "path": path }),
    );
    crate::updates::install_and_restart(&app, std::path::Path::new(&path))?;
    crate::plugins::hook(
        "launcher.update.install.after",
        serde_json::json!({ "path": path }),
    );
    Ok(())
}
