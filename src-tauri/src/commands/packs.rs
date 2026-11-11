use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 8: сборки
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    version_id: Option<String>,
) -> Res<Instance> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "catalog.modpack.install.before",
        serde_json::json!({ "source": source, "projectId": project_id, "versionId": version_id }),
    ) {
        return err(&msg);
    }
    let inst = crate::packs::install_modpack(&app, &state, &source, &project_id, version_id).await?;
    crate::plugins::hook(
        "catalog.modpack.install.after",
        serde_json::json!({ "source": source, "projectId": project_id, "instance": &inst }),
    );
    Ok(inst)
}

#[tauri::command]
pub async fn list_modpack_versions(
    state: State<'_, AppState>,
    source: String,
    project_id: String,
) -> Res<Vec<crate::packs::PackVersion>> {
    crate::plugins::hook(
        "catalog.modpack.versions.before",
        serde_json::json!({ "source": source, "projectId": project_id }),
    );
    let list = crate::packs::list_versions(&state, &source, &project_id).await?;
    crate::plugins::hook(
        "catalog.modpack.versions.after",
        serde_json::json!({ "source": source, "projectId": project_id, "count": list.len() }),
    );
    Ok(list)
}

/// Есть ли для профиля более новая версия сборки.
#[tauri::command]
pub async fn pack_update(
    state: State<'_, AppState>,
    id: String,
) -> Res<Option<crate::packs::PackUpdate>> {
    paths::validate_instance_id(&id)?;
    let upd = crate::packs::check_pack(&state, &id).await?;
    crate::plugins::hook(
        "pack.update.check",
        serde_json::json!({ "id": id, "update": &upd }),
    );
    Ok(upd)
}

/// Ставит новую версию сборки поверх профиля.
#[tauri::command]
pub async fn apply_pack_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version_id: String,
) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    if version_id.trim().is_empty() {
        return err("Не выбрана версия сборки");
    }
    if let Some(msg) = crate::plugins::hook_cancel(
        "pack.update.apply.before",
        serde_json::json!({ "id": id, "versionId": version_id }),
    ) {
        return err(&msg);
    }
    let inst = crate::packs::apply_update(&app, &state, &id, &version_id).await?;
    crate::plugins::hook(
        "pack.update.apply.after",
        serde_json::json!({ "id": id, "versionId": version_id, "instance": &inst }),
    );
    Ok(inst)
}

#[tauri::command]
pub async fn import_pack(app: AppHandle, state: State<'_, AppState>, path: String) -> Res<Instance> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mrpack" | "zip") {
        return err("Поддерживаются файлы .mrpack и .zip");
    }
    match tokio::fs::metadata(p).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл сборки не найден"),
    }
    if let Some(msg) = crate::plugins::hook_cancel(
        "instance.import.before",
        serde_json::json!({ "kind": "pack", "path": path }),
    ) {
        return err(&msg);
    }
    let inst = crate::packs::import_pack(&app, &state, &path).await?;
    crate::plugins::hook(
        "instance.import.after",
        serde_json::json!({ "kind": "pack", "path": path, "instance": &inst }),
    );
    Ok(inst)
}

/// Импорт инстанса MultiMC / Prism Launcher.
#[tauri::command]
pub async fn import_prism_instance(path: String) -> Res<Instance> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "instance.import.before",
        serde_json::json!({ "kind": "prism", "path": path }),
    ) {
        return err(&msg);
    }
    let inst = crate::prism::import(&path).await?;
    crate::plugins::hook(
        "instance.import.after",
        serde_json::json!({ "kind": "prism", "path": path, "instance": &inst }),
    );
    Ok(inst)
}

/// Список профилей Modrinth App, доступных для импорта.
#[tauri::command]
pub async fn list_modrinth_profiles() -> Res<Vec<crate::modrinth_app::MrProfile>> {
    let list = crate::modrinth_app::list()?;
    crate::plugins::hook(
        "instance.import.modrinth.list",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

/// Импорт профиля из Modrinth App.
#[tauri::command]
pub async fn import_modrinth_profile(path: String) -> Res<Instance> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "instance.import.before",
        serde_json::json!({ "kind": "modrinth", "path": path }),
    ) {
        return err(&msg);
    }
    let inst = crate::modrinth_app::import(&path).await?;
    crate::plugins::hook(
        "instance.import.after",
        serde_json::json!({ "kind": "modrinth", "path": path, "instance": &inst }),
    );
    Ok(inst)
}

/// Разбор последнего падения профиля: причина и виновный мод.
#[tauri::command]
pub async fn diagnose_crash(id: String) -> Res<crate::crash::Diagnosis> {
    crate::plugins::hook("crash.diagnose.before", serde_json::json!({ "id": id }));
    let diag = crate::crash::diagnose(&id).await?;
    crate::plugins::hook(
        "crash.diagnose.after",
        serde_json::json!({ "id": id, "diagnosis": &diag }),
    );
    Ok(diag)
}

/// ИИ-разбор падения через free-модели OpenCode Zen.
///
/// `lang` — язык интерфейса, на нём модель пишет ответ; `console` — хвост
/// консоли лаунчера (stdout/stderr), который может отличаться от `latest.log`.
#[tauri::command]
pub async fn ai_diagnose_crash(
    id: String,
    lang: String,
    console: String,
) -> Res<crate::ai::AiDiagnosis> {
    crate::plugins::hook(
        "ai.diagnose.before",
        serde_json::json!({ "id": id, "lang": lang }),
    );
    let diag = crate::ai::diagnose(&id, &lang, &console).await?;
    crate::plugins::hook(
        "ai.diagnose.after",
        serde_json::json!({ "id": id, "diagnosis": &diag }),
    );
    Ok(diag)
}

/// ИИ-ассистент сборок: по промпту составляет план и подбирает проекты каталога.
///
/// Скачивание и установка выполняются обычными командами каталога —
/// здесь только подбор состава под версию, загрузчик и язык интерфейса.
/// `limit` — предел числа модов (0 или None — без ограничения),
/// `source` — приоритетный каталог (пусто — сразу Modrinth и CurseForge),
/// `model` — выбранная модель (пусто — первая доступная бесплатная).
#[tauri::command]
pub async fn ai_pack_plan(
    state: State<'_, AppState>,
    prompt: String,
    lang: String,
    mc_version: Option<String>,
    loader: Option<String>,
    limit: Option<u32>,
    source: Option<String>,
    model: Option<String>,
) -> Res<crate::builder::PackPlan> {
    if !state.config.lock().await.ai_builder {
        return err("ИИ-ассистент сборок выключен в настройках");
    }
    crate::plugins::hook(
        "ai.pack.plan.before",
        serde_json::json!({ "prompt": prompt, "mcVersion": mc_version, "loader": loader }),
    );
    let plan = crate::builder::plan(
        &state, &prompt, &lang, mc_version, loader, limit, source, model,
    )
    .await?;
    crate::plugins::hook(
        "ai.pack.plan.after",
        serde_json::json!({ "plan": &plan }),
    );
    Ok(plan)
}

/// Бесплатные модели, которыми может собирать ИИ-ассистент.
#[tauri::command]
pub async fn ai_models() -> Res<Vec<String>> {
    let list = crate::ai::models().await?;
    crate::plugins::hook(
        "ai.models",
        serde_json::json!({ "count": list.len() }),
    );
    Ok(list)
}

/// Проверка выбранного ИИ-провайдера: короткий запрос и ответ модели.
#[tauri::command]
pub async fn ai_test() -> Res<String> {
    crate::plugins::hook("ai.test.before", serde_json::json!({}));
    let (model, text) = crate::ai::ask_text(
        "Ты — проверка соединения. Ответь одним словом.",
        "Ответь словом «ok».",
        24,
        0.0,
        None,
    )
    .await?;
    let short: String = text.trim().chars().take(120).collect();
    let out = format!("{model}: {short}");
    crate::plugins::hook(
        "ai.test.after",
        serde_json::json!({ "model": model, "reply": short }),
    );
    Ok(out)
}

/// Открывает папку `crash-reports` профиля (создаёт её при необходимости).
#[tauri::command]
pub async fn open_crash_reports(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("crash-reports");
    tokio::fs::create_dir_all(&dir).await?;
    crate::plugins::hook(
        "crash.reports.open",
        serde_json::json!({ "id": id, "path": dir.to_string_lossy() }),
    );
    open_path(&app, &dir)?;
    Ok(())
}
