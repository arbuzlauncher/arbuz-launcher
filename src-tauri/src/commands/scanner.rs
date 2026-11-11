use super::*;

// ---------------------------------------------------------------------------
// Сканер модов
// ---------------------------------------------------------------------------

/// Сканирует контент профиля и возвращает отчёты по каждому файлу.
#[tauri::command]
pub async fn scan_mods(state: State<'_, AppState>, id: String) -> Res<Vec<crate::scanner::ModReport>> {
    crate::plugins::hook("scanner.scan.before", serde_json::json!({ "id": id }));
    let list = crate::scanner::scan_instance(&state, &id).await?;
    crate::plugins::hook(
        "scanner.scan.after",
        serde_json::json!({ "id": id, "reports": &list }),
    );
    Ok(list)
}

/// Ручное исключение файла из блокировки сканера (или его снятие).
#[tauri::command]
pub async fn scanner_allow_mod(id: String, file: String, allow: bool) -> Res<()> {
    crate::plugins::hook(
        "scanner.allow.before",
        serde_json::json!({ "id": id, "file": file, "allow": allow }),
    );
    crate::scanner::allow_mod(&id, &file, allow).await?;
    crate::plugins::hook(
        "scanner.allow.after",
        serde_json::json!({ "id": id, "file": file, "allow": allow }),
    );
    Ok(())
}

/// Файлы в карантине профиля (параноидальный режим).
#[tauri::command]
pub async fn scanner_quarantine(id: String) -> Res<Vec<String>> {
    let list = crate::scanner::quarantine_list(&id).await?;
    crate::plugins::hook(
        "scanner.quarantine.list",
        serde_json::json!({ "id": id, "files": &list }),
    );
    Ok(list)
}

/// Возвращает файл из карантина в mods/.
#[tauri::command]
pub async fn scanner_quarantine_restore(id: String, name: String) -> Res<()> {
    crate::plugins::hook(
        "scanner.quarantine.restore.before",
        serde_json::json!({ "id": id, "file": name }),
    );
    crate::scanner::quarantine_restore(&id, &name).await?;
    crate::plugins::hook(
        "scanner.quarantine.restore.after",
        serde_json::json!({ "id": id, "file": name }),
    );
    Ok(())
}

/// ИИ-заметка по одному подозрительному моду (через free-модели Zen).
/// Только пояснение для пользователя: вердикт сканера не меняет.
#[tauri::command]
pub async fn ai_scan_note(
    state: State<'_, AppState>,
    id: String,
    file: String,
    lang: String,
) -> Res<crate::scanner::ai_note::AiNote> {
    crate::plugins::hook(
        "ai.scan.before",
        serde_json::json!({ "id": id, "file": file }),
    );
    let note = crate::scanner::ai_note_for(&state, &id, &file, &lang).await?;
    crate::plugins::hook(
        "ai.scan.after",
        serde_json::json!({ "id": id, "file": file, "note": &note }),
    );
    Ok(note)
}
