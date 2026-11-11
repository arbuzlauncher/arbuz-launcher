//! Tauri-команды для плагинов, движка тем и динамического визуала.

use crate::error::{err, Res};
use crate::plugins::{install, manager, runtime};
use crate::state::AppState;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLoad {
    pub theme: crate::themes::ThemePack,
    pub css: String,
}

// ---------------------------------------------------------------------------
// Плагины
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn plugins_list() -> Vec<manager::PluginInfo> {
    let list = manager::scan_installed();
    crate::plugins::hook(
        "plugin.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub async fn plugin_inspect(path: String) -> Res<install::PluginPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        install::inspect(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| crate::error::AppError::Msg(format!("Ошибка проверки плагина: {e}")))?
}

#[tauri::command]
pub async fn plugin_install(
    state: State<'_, AppState>,
    path: String,
) -> Res<Vec<manager::PluginInfo>> {
    let enabled = state.config.lock().await.plugins_enabled;
    let p = std::path::PathBuf::from(path);
    if !p.is_file() {
        return err("Файл плагина не найден");
    }
    crate::plugins::hook(
        "plugin.install.before",
        serde_json::json!({ "path": p.to_string_lossy() }),
    );
    let list = install::install(&p)?;
    manager::reload(enabled);
    runtime::emit("plugins.changed", serde_json::json!({ "action": "install" }));
    crate::plugins::hook(
        "plugin.install.after",
        serde_json::json!({ "plugins": &list }),
    );
    Ok(list)
}

#[tauri::command]
pub async fn plugin_uninstall(state: State<'_, AppState>, id: String) -> Res<()> {
    let enabled = state.config.lock().await.plugins_enabled;
    if let Some(msg) = crate::plugins::hook_cancel(
        "plugin.uninstall.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    install::uninstall(&id)?;
    manager::reload(enabled);
    runtime::emit("plugins.changed", serde_json::json!({ "action": "uninstall", "id": &id }));
    crate::plugins::hook(
        "plugin.uninstall.after",
        serde_json::json!({ "id": id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn plugin_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Res<()> {
    crate::plugins::hook(
        "plugin.toggle.before",
        serde_json::json!({ "id": id, "enabled": enabled }),
    );
    manager::set_plugin_enabled(&id, enabled)?;
    let global = state.config.lock().await.plugins_enabled;
    manager::reload(global);
    runtime::emit(
        "plugins.changed",
        serde_json::json!({ "action": "toggle", "id": &id, "enabled": enabled }),
    );
    crate::plugins::hook(
        "plugin.toggle.after",
        serde_json::json!({ "id": id, "enabled": enabled }),
    );
    Ok(())
}

#[tauri::command]
pub async fn plugin_reload(state: State<'_, AppState>) -> Res<()> {
    let enabled = state.config.lock().await.plugins_enabled;
    crate::plugins::hook("plugin.reload.before", serde_json::json!({}));
    manager::reload(enabled);
    runtime::emit("plugins.changed", serde_json::json!({ "action": "reload" }));
    crate::plugins::hook("plugin.reload.after", serde_json::json!({}));
    Ok(())
}

#[tauri::command]
pub fn plugin_open_folder(app: AppHandle) -> Res<()> {
    let dir = crate::paths::plugins_dir();
    std::fs::create_dir_all(&dir)?;
    crate::plugins::hook(
        "plugin.folder.open",
        serde_json::json!({ "path": dir.to_string_lossy() }),
    );
    crate::commands::open_path(&app, &dir)?;
    Ok(())
}

/// Открывает общую папку пользовательских библиотек `<data>/libs`.
#[tauri::command]
pub fn plugin_open_libs(app: AppHandle) -> Res<()> {
    let dir = crate::paths::libs_dir();
    std::fs::create_dir_all(&dir)?;
    crate::plugins::hook(
        "plugin.libs.open",
        serde_json::json!({ "path": dir.to_string_lossy() }),
    );
    crate::commands::open_path(&app, &dir)?;
    Ok(())
}

#[tauri::command]
pub fn plugin_panels() -> Vec<Value> {
    let list = runtime::panels();
    crate::plugins::hook(
        "plugin.panels.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub fn plugin_sidebar() -> Vec<Value> {
    let list = runtime::sidebar();
    crate::plugins::hook(
        "plugin.sidebar.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub fn plugin_logs() -> Vec<Value> {
    runtime::logs()
}

#[tauri::command]
pub fn plugin_logs_clear() {
    runtime::clear_logs();
    crate::plugins::hook("plugin.logs.clear", serde_json::json!({}));
}

/// SDK панели: чтение значения из storage.json плагина.
#[tauri::command]
pub fn plugin_storage_get(plugin: String, key: String) -> Value {
    if !manager::is_loaded(&plugin) {
        return json!({ "error": "Плагин не загружен или отключён" });
    }
    runtime::storage_get(&plugin, &key).unwrap_or(Value::Null)
}

/// SDK панели: запись значения в storage.json плагина.
#[tauri::command]
pub fn plugin_storage_set(plugin: String, key: String, value: Value) -> Res<()> {
    if !manager::is_loaded(&plugin) {
        return err("Плагин не загружен или отключён");
    }
    runtime::storage_set(&plugin, &key, value).map_err(crate::error::AppError::Msg)
}

/// SDK панели: весь storage.json плагина.
#[tauri::command]
pub fn plugin_storage_all(plugin: String) -> Value {
    if !manager::is_loaded(&plugin) {
        return json!({ "error": "Плагин не загружен или отключён" });
    }
    runtime::storage_all(&plugin)
}

/// SDK панели: чтение настройки лаунчера (право `config`).
#[tauri::command]
pub fn plugin_config_get(plugin: String, key: String) -> Value {
    match runtime::config_get(&plugin, &key) {
        Ok(v) => v,
        Err(e) => json!({ "error": e }),
    }
}

/// SDK панели: запись настройки лаунчера (право `config`).
#[tauri::command]
pub fn plugin_config_set(plugin: String, key: String, value: Value) -> Res<()> {
    runtime::config_set(&plugin, &key, value).map_err(crate::error::AppError::Msg)
}

/// Универсальный слот интерфейса: плагин возвращает пункты, HTML или патч.
#[tauri::command]
pub fn plugin_ui_slot(slot: String, props: Option<Value>) -> Value {
    let payload = serde_json::json!({
        "slot": slot,
        "props": props.unwrap_or(Value::Null),
    });
    crate::plugins::hook("ui.slot.before", payload.clone());
    let event = format!("ui.slot.{slot}");
    let mut items: Vec<Value> = Vec::new();
    let mut html = String::new();
    let mut patch = serde_json::Map::new();
    for r in manager::call_hook(&event, &payload) {
        if r.error.is_some() || !runtime::has_permission(&r.plugin, "ui") {
            continue;
        }
        match r.result {
            Value::Array(list) => {
                for mut item in list {
                    if item.get("plugin").is_none() {
                        item["plugin"] = serde_json::json!(r.plugin);
                    }
                    items.push(item);
                }
            }
            Value::Object(mut o) => {
                if let Some(Value::Array(list)) = o.remove("items") {
                    for mut item in list {
                        if item.get("plugin").is_none() {
                            item["plugin"] = serde_json::json!(r.plugin);
                        }
                        items.push(item);
                    }
                }
                if let Some(Value::String(h)) = o.remove("html") {
                    html.push_str(&h);
                }
                if let Some(Value::Object(p)) = o.remove("patch") {
                    for (k, v) in p {
                        patch.insert(k, v);
                    }
                }
            }
            _ => {}
        }
    }
    let out = serde_json::json!({ "items": items, "html": html, "patch": patch });
    crate::plugins::hook(
        "ui.slot.after",
        serde_json::json!({ "slot": slot, "result": &out }),
    );
    out
}

/// Хук на конкретный элемент интерфейса (`data-plugin-el="..."`).
#[tauri::command]
pub fn plugin_ui_element(id: String, props: Option<Value>) -> Value {
    let payload = serde_json::json!({
        "id": id,
        "props": props.unwrap_or(Value::Null),
    });
    crate::plugins::hook("ui.element.before", payload.clone());
    let mut patch = serde_json::Map::new();
    for event in [format!("ui.element.{id}"), "ui.element".to_string()] {
        for r in manager::call_hook(&event, &payload) {
            if r.error.is_some() || !runtime::has_permission(&r.plugin, "ui") {
                continue;
            }
            if let Value::Object(o) = r.result {
                for (k, v) in o {
                    patch.insert(k, v);
                }
            }
        }
    }
    let out = Value::Object(patch);
    crate::plugins::hook(
        "ui.element.after",
        serde_json::json!({ "id": id, "patch": &out }),
    );
    out
}

#[tauri::command]
pub fn plugin_sources() -> Vec<Value> {
    let list = runtime::sources();
    crate::plugins::hook(
        "plugin.sources.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub fn plugin_commands() -> Vec<Value> {
    let list = runtime::commands();
    crate::plugins::hook(
        "plugin.commands.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub fn plugin_auth_providers() -> Vec<Value> {
    let list = runtime::auth_providers();
    crate::plugins::hook(
        "plugin.auth.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

/// Вход через провайдер, зарегистрированный плагином. Плагин возвращает
/// `{name, uuid?, token, note?}`; токен кладём в системное хранилище.
#[tauri::command]
pub async fn plugin_auth_login(
    state: State<'_, AppState>,
    provider: String,
    credentials: Value,
) -> Res<crate::models::Account> {
    let desc = runtime::auth_providers()
        .into_iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(provider.as_str()))
        .ok_or_else(|| crate::error::AppError::Msg("Провайдер входа не найден".into()))?;
    let plugin = desc
        .get("plugin")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if plugin.is_empty() {
        return err("У провайдера нет плагина");
    }
    let results = manager::call_hook(
        "auth.login",
        &json!({ "provider": provider.clone(), "credentials": credentials, "plugin": plugin }),
    );
    let Some(result) = results
        .into_iter()
        .rev()
        .find(|r| r.plugin == plugin && r.error.is_none() && r.result.is_object())
        .map(|r| r.result)
    else {
        return err("Плагин не вернул данные аккаунта");
    };
    if let Some(e) = result.get("error").and_then(|v| v.as_str()) {
        return err(e);
    }
    let name = result
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .chars()
        .take(16)
        .collect::<String>();
    if name.is_empty() {
        return err("Плагин не вернул имя аккаунта");
    }
    let token = result
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return err("Плагин не вернул токен");
    }
    let id = result
        .get("uuid")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>())
        .filter(|s| s.len() == 32)
        .unwrap_or_else(|| crate::accounts::offline::offline_uuid(&name));
    let acc = crate::models::Account {
        id: id.clone(),
        kind: crate::models::AccountKind::Plugin,
        name,
        note: result
            .get("note")
            .and_then(|v| v.as_str())
            .map(|s| s.chars().take(120).collect())
            .unwrap_or_else(|| {
                desc.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&provider)
                    .to_string()
            }),
    };
    crate::accounts::secret_set(
        &id,
        &json!({ "provider": provider, "token": token, "plugin": plugin }),
    )?;
    crate::accounts::add(acc.clone()).await?;
    crate::accounts::set_active(&id).await?;
    {
        let mut cfg = state.config.lock().await;
        cfg.active_account = Some(id);
    }
    let _ = manager::call_hook(
        "auth.login.after",
        &json!({ "provider": provider, "account": acc.clone() }),
    );
    Ok(acc)
}

/// Рендер панели плагина: возвращает HTML/CSS/JS или произвольный JSON.
#[tauri::command]
pub fn plugin_panel_render(
    plugin: String,
    panel: String,
    args: Option<Value>,
) -> Value {
    let payload = json!({
        "plugin": plugin,
        "panel": panel,
        "args": args.unwrap_or(Value::Null),
    });
    if !manager::is_loaded(&plugin) {
        return json!({ "error": "Плагин не загружен или отключён" });
    }
    crate::plugins::hook("ui.panel.render.before", payload.clone());
    let mut result = Value::Null;
    let mut errors: Vec<String> = Vec::new();
    for event in [format!("panel.{panel}.render"), "panel.render".to_string()] {
        for r in manager::call_hook(&event, &payload) {
            if r.plugin != plugin {
                continue;
            }
            if let Some(e) = r.error {
                errors.push(e);
                continue;
            }
            if !r.result.is_null() {
                result = r.result;
            }
        }
    }
    if result.is_null() {
        if errors.is_empty() {
            json!({ "error": "Плагин не вернул содержимое панели" })
        } else {
            json!({ "error": errors.join("; ") })
        }
    } else {
        crate::plugins::hook(
            "ui.panel.render.after",
            json!({ "plugin": plugin, "panel": panel, "result": &result }),
        );
        result
    }
}

/// Событие из SDK панели в плагин: хук `panel.event.<event>`.
#[tauri::command]
pub fn plugin_panel_event(plugin: String, event: String, data: Option<Value>) -> Value {
    if !manager::is_loaded(&plugin) {
        return json!({ "error": "Плагин не загружен или отключён" });
    }
    let payload = json!({
        "plugin": &plugin,
        "event": &event,
        "data": data.unwrap_or(Value::Null),
    });
    let results = manager::call_hook(&format!("panel.event.{event}"), &payload);
    results
        .into_iter()
        .rev()
        .find(|r| r.plugin == plugin && r.error.is_none() && !r.result.is_null())
        .map(|r| r.result)
        .unwrap_or(Value::Null)
}

#[tauri::command]
pub fn plugin_run_command(plugin: String, command: String, args: Option<Value>) -> Value {    let args = args.unwrap_or(Value::Null);
    crate::plugins::hook(
        "ui.command.before",
        json!({ "plugin": plugin, "command": command, "args": &args }),
    );
    let results = manager::run_command(&plugin, &command, &args);
    let result = results
        .into_iter()
        .rev()
        .find(|r| r.error.is_none() && !r.result.is_null())
        .map(|r| r.result)
        .unwrap_or(Value::Null);
    crate::plugins::hook(
        "ui.command.after",
        json!({ "plugin": plugin, "command": command, "result": &result }),
    );
    result
}

/// Поиск по источнику, зарегистрированному плагином.
#[tauri::command]
pub fn plugin_catalog_search(source: String, args: Value) -> Vec<Value> {
    crate::plugins::hook(
        "catalog.search.before",
        json!({ "source": source, "args": &args, "pluginSource": true }),
    );
    let list = manager::catalog_search(&source, &args);
    crate::plugins::hook(
        "catalog.search.after",
        json!({ "source": source, "result": &list, "pluginSource": true }),
    );
    list
}

/// Динамический визуал Minecraft для профиля (панорама + текстуры кнопок).
#[tauri::command]
pub fn plugin_visual(instance_id: String) -> Value {
    crate::plugins::hook(
        "visual.before",
        json!({ "instanceId": instance_id }),
    );
    let visual = manager::visual(&instance_id);
    crate::plugins::hook(
        "visual.after",
        json!({ "instanceId": instance_id, "visual": &visual }),
    );
    visual
}

// ---------------------------------------------------------------------------
// Темы
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn themes_list() -> Vec<crate::themes::ThemePack> {
    let list = crate::themes::list();
    crate::plugins::hook(
        "theme.list",
        serde_json::json!({ "count": list.len() }),
    );
    list
}

#[tauri::command]
pub fn theme_load(id: String) -> Res<ThemeLoad> {
    crate::plugins::hook("theme.load.before", serde_json::json!({ "id": id }));
    let (theme, css) = crate::themes::load(&id)?;
    crate::plugins::hook(
        "theme.load.after",
        serde_json::json!({ "id": id, "theme": &theme }),
    );
    Ok(ThemeLoad { theme, css })
}

#[tauri::command]
pub async fn theme_install(state: State<'_, AppState>, path: String) -> Res<String> {
    crate::plugins::hook(
        "theme.install.before",
        serde_json::json!({ "path": path }),
    );
    let p = std::path::PathBuf::from(&path);
    if !p.is_file() {
        return err("Файл темы не найден");
    }
    let id = install::install_theme(&p)?;
    // Тема из плагина могла зарегистрироваться только после перезагрузки.
    let enabled = state.config.lock().await.plugins_enabled;
    manager::reload(enabled);
    crate::plugins::hook(
        "theme.install.after",
        serde_json::json!({ "id": id, "path": path }),
    );
    Ok(id)
}

#[tauri::command]
pub fn theme_remove(id: String) -> Res<()> {
    if let Some(msg) = crate::plugins::hook_cancel(
        "theme.remove.before",
        serde_json::json!({ "id": id }),
    ) {
        return err(&msg);
    }
    crate::themes::remove(&id)?;
    crate::plugins::hook("theme.remove.after", serde_json::json!({ "id": id }));
    Ok(())
}

#[tauri::command]
pub fn theme_open_folder(app: AppHandle) -> Res<()> {
    let dir = crate::themes::dir();
    std::fs::create_dir_all(&dir)?;
    crate::plugins::hook(
        "theme.folder.open",
        serde_json::json!({ "path": dir.to_string_lossy() }),
    );
    crate::commands::open_path(&app, &dir)?;
    Ok(())
}

/// Тема в виде JSON (для плагинов и экспорта).
#[tauri::command]
pub fn theme_export(id: String) -> Res<Value> {
    crate::plugins::hook("theme.export.before", serde_json::json!({ "id": id }));
    let (theme, css) = crate::themes::load(&id)?;
    let out = json!({ "theme": theme, "css": css });
    crate::plugins::hook(
        "theme.export.after",
        serde_json::json!({ "id": id, "theme": &out }),
    );
    Ok(out)
}
