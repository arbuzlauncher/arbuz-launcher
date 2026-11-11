use super::*;

// ---------------------------------------------------------------------------
// Редактор сборки: манифест, конфиги, проверка, экспорт
// ---------------------------------------------------------------------------

/// Манифест и статистика сборки.
#[tauri::command]
pub async fn pack_info(id: String) -> Res<crate::studio::PackInfo> {
    crate::plugins::hook("pack.info.before", serde_json::json!({ "id": id }));
    let info = crate::studio::info(&id).await?;
    crate::plugins::hook(
        "pack.info.after",
        serde_json::json!({ "id": id, "info": &info }),
    );
    Ok(info)
}

/// Сохраняет имя, версию, автора и описание сборки.
#[tauri::command]
pub async fn save_pack_info(
    id: String,
    manifest: crate::studio::PackManifest,
) -> Res<crate::studio::PackInfo> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "pack.info.save.before",
        serde_json::json!({ "id": id, "manifest": &manifest }),
    ) {
        return err(&msg);
    }
    let info = crate::studio::save(&id, manifest).await?;
    crate::plugins::hook(
        "pack.info.save.after",
        serde_json::json!({ "id": id, "info": &info }),
    );
    Ok(info)
}

/// Все файлы конфигурации профиля, с привязкой к модам.
#[tauri::command]
pub async fn pack_configs(id: String) -> Res<Vec<crate::studio::ConfigEntry>> {
    let list = crate::studio::configs(&id).await?;
    crate::plugins::hook(
        "pack.configs.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

/// Читает конфиг для редактора.
#[tauri::command]
pub async fn read_pack_config(id: String, path: String) -> Res<crate::studio::ConfigDoc> {
    crate::plugins::hook(
        "pack.config.read.before",
        serde_json::json!({ "id": id, "path": path }),
    );
    let doc = crate::studio::read_config(&id, &path).await?;
    crate::plugins::hook(
        "pack.config.read.after",
        serde_json::json!({ "id": id, "path": path, "doc": &doc }),
    );
    Ok(doc)
}

/// История сохранений конфига (до 50 версий на файл).
#[tauri::command]
pub async fn pack_config_history(
    id: String,
    path: String,
) -> Res<Vec<crate::studio::ConfigVersion>> {
    crate::plugins::hook(
        "pack.config.history.before",
        serde_json::json!({ "id": id, "path": path }),
    );
    let list = crate::studio::config_history(&id, &path).await?;
    crate::plugins::hook(
        "pack.config.history.after",
        serde_json::json!({ "id": id, "path": path, "count": list.len() }),
    );
    Ok(list)
}

/// Содержимое сохранённой версии конфига.
#[tauri::command]
pub async fn pack_config_version(
    id: String,
    path: String,
    stamp: u64,
) -> Res<String> {
    crate::studio::config_version(&id, &path, stamp).await
}

/// Diff между сохранённой версией и текущим файлом.
#[tauri::command]
pub async fn pack_config_diff(id: String, path: String, stamp: u64) -> Res<String> {
    crate::studio::config_diff(&id, &path, stamp).await
}

/// Проверка синтаксиса конфига до сохранения.
#[tauri::command]
pub fn validate_pack_config(path: String, content: String) -> crate::studio::ConfigCheck {
    crate::studio::validate_config(&path, &content)
}

/// Сохраняет конфиг (старая версия уходит в `.bak`).
#[tauri::command]
pub async fn save_pack_config(
    id: String,
    path: String,
    content: String,
) -> Res<crate::studio::ConfigDoc> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "pack.config.save.before",
        serde_json::json!({ "id": id, "path": path, "content": content }),
    ) {
        return err(&msg);
    }
    let doc = crate::studio::save_config(&id, &path, &content).await?;
    crate::plugins::hook(
        "pack.config.save.after",
        serde_json::json!({ "id": id, "path": path, "doc": &doc }),
    );
    Ok(doc)
}

/// Откатывает конфиг из `.bak`.
#[tauri::command]
pub async fn restore_pack_config(id: String, path: String) -> Res<crate::studio::ConfigDoc> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "pack.config.restore.before",
        serde_json::json!({ "id": id, "path": path }),
    ) {
        return err(&msg);
    }
    let doc = crate::studio::restore_config(&id, &path).await?;
    crate::plugins::hook(
        "pack.config.restore.after",
        serde_json::json!({ "id": id, "path": path, "doc": &doc }),
    );
    Ok(doc)
}

/// Анализатор конфликтов: дубликаты, чужой загрузчик, зависимости и известные
/// несовместимые пары. Для каждой проблемы возвращаются готовые действия.
#[tauri::command]
pub async fn analyze_conflicts(id: String) -> Res<Vec<crate::studio::ModConflict>> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook("mods.conflicts.before", serde_json::json!({ "id": id }));
    let list = crate::studio::conflicts(&id).await?;
    crate::plugins::hook(
        "mods.conflicts.after",
        serde_json::json!({ "id": id, "conflicts": &list }),
    );
    Ok(list)
}

/// Анализ свежего лога/краш-репорта по обновляемым правилам.
#[tauri::command]
pub async fn analyze_logs(id: String) -> Res<crate::logs::LogAnalysis> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook("logs.analyze.before", serde_json::json!({ "id": id }));
    let res = crate::logs::analyze(&id).await?;
    crate::plugins::hook(
        "logs.analyze.after",
        serde_json::json!({ "id": id, "analysis": &res }),
    );
    Ok(res)
}

/// Журнал запусков профиля (время, код выхода, лог и краш).
#[tauri::command]
pub async fn log_journal(id: String) -> Res<Vec<crate::logs::RunRecord>> {
    paths::validate_instance_id(&id)?;
    let list = crate::logs::journal(&id).await?;
    crate::plugins::hook(
        "logs.journal",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

/// Отчёт для поддержки: версия, найденные проблемы и обезличенный хвост лога.
#[tauri::command]
pub async fn log_support_report(id: String) -> Res<String> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook("logs.report.before", serde_json::json!({ "id": id }));
    let text = crate::logs::support_report(&id).await?;
    crate::plugins::hook(
        "logs.report.after",
        serde_json::json!({ "id": id, "bytes": text.len() }),
    );
    Ok(text)
}

/// Сохраняет отчёт для поддержки в выбранный файл.
#[tauri::command]
pub async fn log_save_report(id: String, dest: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let p = std::path::PathBuf::from(dest.trim());
    if p.as_os_str().is_empty() {
        return err("Не указан файл отчёта");
    }
    crate::plugins::hook(
        "logs.report.save.before",
        serde_json::json!({ "id": id, "path": p.to_string_lossy() }),
    );
    let text = crate::logs::support_report(&id).await?;
    tokio::fs::write(&p, text).await?;
    crate::plugins::hook(
        "logs.report.save.after",
        serde_json::json!({ "id": id, "path": p.to_string_lossy() }),
    );
    Ok(())
}

/// Текущий набор правил анализа логов.
#[tauri::command]
pub fn log_rules() -> serde_json::Value {
    crate::logs::rules_json()
}

/// Обновляет правила анализа логов по https-ссылке.
#[tauri::command]
pub async fn log_update_rules(state: State<'_, AppState>, url: String) -> Res<usize> {
    crate::plugins::hook(
        "logs.rules.update.before",
        serde_json::json!({ "url": url }),
    );
    let n = crate::logs::update_rules(&state, &url).await?;
    crate::plugins::hook(
        "logs.rules.update.after",
        serde_json::json!({ "url": url, "count": n }),
    );
    Ok(n)
}

/// Применяет исправление из анализа лога (память, Java, загрузчик, папки).
#[tauri::command]
pub async fn log_apply_fix(
    app: AppHandle,
    id: String,
    kind: String,
    value: Option<String>,
) -> Res<()> {
    paths::validate_instance_id(&id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "logs.fix.before",
        serde_json::json!({ "id": id, "kind": kind, "value": value }),
    ) {
        return err(&msg);
    }
    crate::logs::apply_fix(&app, &id, &kind, value.as_deref()).await?;
    crate::plugins::hook(
        "logs.fix.after",
        serde_json::json!({ "id": id, "kind": kind }),
    );
    Ok(())
}

/// Проверка сборки: дубликаты, зависимости, Java.
#[tauri::command]
pub async fn validate_pack(id: String) -> Res<Vec<crate::studio::PackIssue>> {
    crate::plugins::hook("pack.validate.before", serde_json::json!({ "id": id }));
    let issues = crate::studio::validate(&id).await?;
    crate::plugins::hook(
        "pack.validate.after",
        serde_json::json!({ "id": id, "issues": &issues }),
    );
    Ok(issues)
}

/// Экспорт сборки в `.mrpack` или `.zip`.
#[tauri::command]
pub async fn export_pack(
    id: String,
    dest: String,
    format: String,
) -> Res<crate::studio::ExportResult> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "pack.export.before",
        serde_json::json!({ "id": id, "dest": dest, "format": format }),
    ) {
        return err(&msg);
    }
    let res = crate::studio::export(&id, &dest, &format).await?;
    crate::plugins::hook(
        "pack.export.after",
        serde_json::json!({ "id": id, "dest": dest, "format": format, "result": &res }),
    );
    Ok(res)
}
