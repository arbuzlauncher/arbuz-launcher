use super::*;

// ---------------------------------------------------------------------------
// Синхронизация настроек между профилями (Б1)
// ---------------------------------------------------------------------------

/// Ключи и значения options.txt главного профиля (для списка в UI).
/// Настройки синхронизации кладутся в config.json через set_config.
#[tauri::command]
pub async fn sync_master_options(state: State<'_, AppState>) -> Res<std::collections::BTreeMap<String, String>> {
    let master = state
        .config
        .lock()
        .await
        .sync
        .clone()
        .map(|s| s.master)
        .unwrap_or_default();
    if master.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }
    crate::sync::master_keys(&master).await
}
