use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 6: контент и миры
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_content(
    state: State<'_, AppState>,
    id: String,
) -> Res<Vec<crate::content::ContentItem>> {
    paths::validate_instance_id(&id)?;
    let mut items = crate::content::list(&id).await?;
    crate::catalog::fill_icons(&state.http, &mut items).await;
    crate::plugins::hook(
        "content.list",
        serde_json::json!({ "id": id, "count": items.len() }),
    );
    Ok(items)
}

#[tauri::command]
pub async fn toggle_content(id: String, file: String, enabled: bool) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "content.toggle.before",
        serde_json::json!({ "id": id, "file": file, "enabled": enabled }),
    );
    crate::content::toggle(&id, &file, enabled).await?;
    crate::plugins::hook(
        "content.toggle.after",
        serde_json::json!({ "id": id, "file": file, "enabled": enabled }),
    );
    Ok(())
}

#[tauri::command]
pub async fn remove_content(id: String, file: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "content.remove.before",
        serde_json::json!({ "id": id, "file": file }),
    ) {
        return err(&msg);
    }
    crate::content::remove(&id, &file).await?;
    crate::plugins::hook(
        "content.remove.after",
        serde_json::json!({ "id": id, "file": file }),
    );
    Ok(())
}

/// Помечает файл контента как изолируемый (или снимает пометку).
#[tauri::command]
pub async fn toggle_isolation(id: String, file: String, isolated: bool) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "content.isolation.before",
        serde_json::json!({ "id": id, "file": file, "isolated": isolated }),
    );
    crate::content::set_isolated(&id, &file, isolated).await?;
    crate::plugins::hook(
        "content.isolation.after",
        serde_json::json!({ "id": id, "file": file, "isolated": isolated }),
    );
    Ok(())
}

#[tauri::command]
pub async fn list_worlds(id: String) -> Res<Vec<crate::content::World>> {
    paths::validate_instance_id(&id)?;
    let list = crate::content::worlds(&id).await?;
    crate::plugins::hook(
        "world.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

#[tauri::command]
pub async fn list_screenshots(id: String) -> Res<Vec<crate::content::Shot>> {
    paths::validate_instance_id(&id)?;
    let list = crate::content::screenshots(&id).await?;
    crate::plugins::hook(
        "screenshot.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

/// Открывает папку `screenshots` профиля (создаёт её при необходимости).
#[tauri::command]
pub async fn open_screenshots_folder(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("screenshots");
    tokio::fs::create_dir_all(&dir).await?;
    crate::plugins::hook(
        "screenshot.folder.open",
        serde_json::json!({ "id": id, "path": dir.to_string_lossy() }),
    );
    open_path(&app, &dir)?;
    Ok(())
}

/// Открывает один скриншот системным просмотрщиком.
#[tauri::command]
pub async fn open_screenshot(app: AppHandle, id: String, file: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    if file.is_empty() || file.contains(['/', '\\', ':']) || file.contains("..") {
        return err("Некорректное имя файла");
    }
    let path = paths::instance_minecraft_dir(&id).join("screenshots").join(&file);
    if tokio::fs::metadata(&path).await.is_err() {
        return err("Скриншот не найден");
    }
    crate::plugins::hook(
        "screenshot.open",
        serde_json::json!({ "id": id, "file": file }),
    );
    open_path(&app, &path)?;
    Ok(())
}
