use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 9: новости
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_news(state: State<'_, AppState>) -> Res<Vec<crate::news::NewsItem>> {
    crate::plugins::hook("news.fetch.before", serde_json::json!({}));
    let list = crate::news::get(&state.http).await?;
    crate::plugins::hook(
        "news.fetch.after",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

#[tauri::command]
pub fn data_dir() -> String {
    paths::root().to_string_lossy().to_string()
}

#[tauri::command]
pub async fn open_data_folder(app: AppHandle) -> Res<()> {
    let dir = paths::root();
    tokio::fs::create_dir_all(&dir).await?;
    open_path(&app, &dir)?;
    Ok(())
}

/// Копирует выбранную картинку в папку обоев и возвращает имя файла.
#[tauri::command]
pub async fn set_home_wallpaper(source_path: String) -> Res<String> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "ui.wallpaper.before",
        serde_json::json!({ "path": source_path }),
    ) {
        return err(&msg);
    }
    let src = std::path::Path::new(&source_path);
    if source_path.contains("..") || !src.is_absolute() {
        return err("Некорректный путь к обоям");
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return err("Поддерживаются PNG, JPG и WEBP");
    }
    match tokio::fs::metadata(src).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл обоев не найден"),
    }

    let dir = paths::wallpapers_dir();
    tokio::fs::create_dir_all(&dir).await?;
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
    }
    let name = {
        // Уникальное имя: webview кэширует картинку по URL, поэтому при смене
        // обоев путь обязан меняться, иначе показывается предыдущая картинка.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("home-{stamp}.{ext}")
    };
    tokio::fs::copy(src, dir.join(&name)).await?;
    crate::plugins::hook(
        "ui.wallpaper.after",
        serde_json::json!({ "name": name }),
    );
    Ok(name)
}

#[tauri::command]
pub async fn clear_home_wallpaper() -> Res<()> {
    crate::plugins::hook("ui.wallpaper.clear.before", serde_json::json!({}));
    let dir = paths::wallpapers_dir();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
    }
    crate::plugins::hook("ui.wallpaper.clear.after", serde_json::json!({}));
    Ok(())
}
