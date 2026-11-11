use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 7: каталог
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn search_catalog(
    state: State<'_, AppState>,
    args: crate::catalog::SearchArgs,
) -> Res<crate::catalog::SearchResult> {
    let payload = serde_json::json!({
        "source": args.source,
        "kind": args.kind,
        "query": args.query,
        "loaders": args.loaders,
        "mcVersion": args.mc_version,
        "categories": args.categories,
        "offset": args.offset,
        "sort": args.sort,
        "instanceId": args.instance_id,
    });
    if let Some(msg) = crate::plugins::hook_cancel("catalog.search.before", payload.clone()) {
        return err(&msg);
    }
    let res = crate::catalog::search(&state, args).await?;
    crate::plugins::hook(
        "catalog.search.after",
        serde_json::json!({ "result": &res, "args": payload }),
    );
    Ok(res)
}

#[tauri::command]
pub async fn install_catalog_item(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source: String,
    project_id: String,
    kind: String,
    world: Option<String>,
) -> Res<()> {
    paths::validate_instance_id(&instance_id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "catalog.install.before",
        serde_json::json!({
            "instanceId": instance_id,
            "source": source,
            "projectId": project_id,
            "kind": kind,
            "world": world,
        }),
    ) {
        return err(&msg);
    }
    crate::catalog::install(&app, &state, &instance_id, &source, &project_id, &kind, world).await?;
    crate::plugins::hook(
        "catalog.install.after",
        serde_json::json!({
            "instanceId": instance_id,
            "source": source,
            "projectId": project_id,
            "kind": kind,
        }),
    );
    Ok(())
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>, id: String) -> Res<Vec<crate::catalog::UpdateInfo>> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "catalog.updates.check.before",
        serde_json::json!({ "id": id }),
    );
    let list = crate::catalog::check_updates(&state, &id).await?;
    crate::plugins::hook(
        "catalog.updates.check.after",
        serde_json::json!({ "id": id, "updates": &list }),
    );
    Ok(list)
}

/// План исправления зависимостей: совместимые обновления + замечания сборки.
/// Плагины могут дополнить план через хук `ai.dependencies.plan`.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyPlan {
    pub updates: Vec<crate::catalog::UpdateInfo>,
    pub issues: Vec<crate::studio::PackIssue>,
    pub note: String,
}

#[tauri::command]
pub async fn ai_dependency_plan(
    state: State<'_, AppState>,
    id: String,
) -> Res<DependencyPlan> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "ai.dependencies.before",
        serde_json::json!({ "instanceId": id }),
    );
    let inst = store::read_instance(&id).await?;
    let updates = crate::catalog::check_updates(&state, &id)
        .await
        .unwrap_or_default();
    let issues = crate::studio::validate(&id).await.unwrap_or_default();
    let hook = crate::plugins::call_hook(
        "ai.dependencies.plan",
        &serde_json::json!({
            "instanceId": id,
            "mcVersion": inst.mc_version,
            "loader": inst.loader.label(),
            "updates": updates.clone(),
            "issues": issues.clone(),
        }),
    );
    let note = hook
        .into_iter()
        .rev()
        .find_map(|r| {
            r.result
                .get("note")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| {
            if updates.is_empty() {
                "Совместимых обновлений нет. Проверьте замечания сборки.".to_string()
            } else {
                "Найдены совместимые версии модов. Установить их автоматически?".to_string()
            }
        });
    crate::plugins::hook(
        "ai.dependencies.after",
        serde_json::json!({
            "instanceId": id,
            "updates": updates.len(),
            "issues": issues.len(),
            "note": note,
        }),
    );
    Ok(DependencyPlan {
        updates,
        issues,
        note,
    })
}

#[tauri::command]
pub async fn apply_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    files: Vec<String>,
) -> Res<u32> {
    paths::validate_instance_id(&id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "catalog.updates.apply.before",
        serde_json::json!({ "id": id, "files": files }),
    ) {
        return err(&msg);
    }
    let n = crate::catalog::apply_updates(&app, &state, &id, files.clone()).await?;
    crate::plugins::hook(
        "catalog.updates.apply.after",
        serde_json::json!({ "id": id, "files": files, "updated": n }),
    );
    Ok(n)
}
