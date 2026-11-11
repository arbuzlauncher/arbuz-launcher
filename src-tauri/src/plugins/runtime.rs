//! Мост между Python-плагинами и лаунчером.
//!
//! Каждый плагин получает свой модуль `arbuz` (Python-обёртка над `_arbuz_native`).
//! Все опасные операции проходят через проверку прав из манифеста: плагин без
//! `network` не сможет сделать HTTP-запрос, без `fs` — читать файлы и т.д.

use crate::paths;
use once_cell::sync::Lazy;use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyModule, PyString};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Права, которые плагин может запросить в манифесте.
pub const KNOWN_PERMISSIONS: &[&str] = &[
    "network", "fs", "instances", "mc", "ui", "themes", "sources", "config", "auth",
];

#[derive(Default)]
struct Registry {
    perms: HashMap<String, HashSet<String>>,
    dirs: HashMap<String, PathBuf>,
    sources: Vec<Value>,
    themes: Vec<Value>,
    panels: Vec<Value>,
    commands: Vec<Value>,
    sidebar: Vec<Value>,
    providers: Vec<Value>,
    visuals: HashMap<String, Value>,
    app: Option<tauri::AppHandle>,
}

static REG: Lazy<RwLock<Registry>> = Lazy::new(|| RwLock::new(Registry::default()));

/// Последние сообщения плагинов: их показывает вкладка «Логи плагинов».
const MAX_LOGS: usize = 500;
static LOGS: Lazy<RwLock<VecDeque<Value>>> = Lazy::new(|| RwLock::new(VecDeque::new()));

thread_local! {
    static CURRENT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub fn set_current(id: Option<String>) {
    CURRENT.with(|c| *c.borrow_mut() = id);
}

pub fn current() -> Option<String> {
    CURRENT.with(|c| c.borrow().clone())
}

pub fn set_app(app: tauri::AppHandle) {
    if let Ok(mut r) = REG.write() {
        r.app = Some(app);
    }
}

pub fn clear_plugins() {
    if let Ok(mut r) = REG.write() {
        r.perms.clear();
        r.dirs.clear();
        r.sources.clear();
        r.themes.clear();
        r.panels.clear();
        r.commands.clear();
        r.sidebar.clear();
        r.providers.clear();
        r.visuals.clear();
    }
}

pub fn register_plugin(id: &str, dir: &Path, perms: &[String]) {
    if let Ok(mut r) = REG.write() {
        r.perms
            .insert(id.to_string(), perms.iter().cloned().collect());
        r.dirs.insert(id.to_string(), dir.to_path_buf());
    }
}

/// Добавляет в `sys.path` общую папку библиотек и `lib/` конкретного плагина.
///
/// Общая папка `<data>/libs` доступна всем плагинам: там можно держать
/// собственные модули и импортировать их обычным `import`. Папка `lib/`
/// рядом с `.aplug` доступна только этому плагину (на практике — пока плагин
/// загружен; имена модулей должны быть уникальными).
pub fn prepare_library_paths(py: Python<'_>, plugin_dir: &Path) -> PyResult<()> {
    let sys = py.import("sys")?;
    let path = sys.getattr("path")?.downcast::<PyList>()?;
    let global = paths::libs_dir();
    let _ = std::fs::create_dir_all(&global);
    let local = plugin_dir.join("lib");
    let _ = std::fs::create_dir_all(&local);
    for dir in [&global, &local] {
        let value = dir.to_string_lossy().to_string();
        let exists = path
            .iter()
            .any(|item| item.extract::<String>().ok().as_deref() == Some(value.as_str()));
        if !exists {
            path.insert(0, value)?;
        }
    }
    Ok(())
}

pub fn unregister_plugin(id: &str) {
    if let Ok(mut r) = REG.write() {
        r.perms.remove(id);
        r.dirs.remove(id);
        r.visuals.remove(id);
    }
}

pub fn sources() -> Vec<Value> {
    REG.read().map(|r| r.sources.clone()).unwrap_or_default()
}

pub fn themes() -> Vec<Value> {
    REG.read().map(|r| r.themes.clone()).unwrap_or_default()
}

pub fn panels() -> Vec<Value> {
    REG.read().map(|r| r.panels.clone()).unwrap_or_default()
}

pub fn commands() -> Vec<Value> {
    REG.read().map(|r| r.commands.clone()).unwrap_or_default()
}

pub fn sidebar() -> Vec<Value> {
    let mut list = REG.read().map(|r| r.sidebar.clone()).unwrap_or_default();
    list.sort_by_key(|v| v.get("order").and_then(|x| x.as_i64()).unwrap_or(100));
    list
}

/// Проверяет право плагина (например, `ui`) без запуска Python-кода.
pub fn has_permission(id: &str, perm: &str) -> bool {
    REG.read()
        .ok()
        .and_then(|r| r.perms.get(id).map(|p| p.contains(perm)))
        .unwrap_or(false)
}

/// Последние строки логов плагинов (старые сверху).
pub fn logs() -> Vec<Value> {
    LOGS.read()
        .map(|l| l.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn clear_logs() {
    if let Ok(mut l) = LOGS.write() {
        l.clear();
    }
}

pub fn auth_providers() -> Vec<Value> {
    REG.read().map(|r| r.providers.clone()).unwrap_or_default()
}

pub fn visual(id: &str) -> Option<Value> {
    REG.read().ok().and_then(|r| r.visuals.get(id).cloned())
}

pub(crate) fn emit(event: &str, payload: Value) {
    if let Ok(r) = REG.read() {
        if let Some(app) = &r.app {
            use tauri::Emitter;
            let _ = app.emit(
                "plugin-event",
                json!({ "event": event, "payload": payload }),
            );
        }
    }
}

fn plugin_dir(plugin: &str) -> PyResult<PathBuf> {
    REG.read()
        .ok()
        .and_then(|r| r.dirs.get(plugin).cloned())
        .ok_or_else(|| PyRuntimeError::new_err("Плагин не загружен"))
}

fn has_perm(plugin: &str, perm: &str) -> bool {
    REG.read()
        .ok()
        .and_then(|r| r.perms.get(plugin).cloned())
        .map(|p| p.contains(perm))
        .unwrap_or(false)
}

fn require(plugin: &str, perm: &str) -> PyResult<()> {
    if has_perm(plugin, perm) {
        Ok(())
    } else {
        Err(PyRuntimeError::new_err(format!(
            "Плагину «{plugin}» нужно право «{perm}» в манифесте"
        )))
    }
}

fn safe_join(base: &Path, rel: &str) -> PyResult<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains(':')
        || rel.split('/').any(|p| p == ".." || p.is_empty())
    {
        return Err(PyRuntimeError::new_err("Некорректный путь"));
    }
    Ok(base.join(rel))
}

fn read_config_value() -> Value {
    std::fs::read_to_string(paths::config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_config_value(v: &Value) -> PyResult<()> {
    let mut cfg: crate::models::Config = serde_json::from_value(v.clone())
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректный конфиг: {e}")))?;
    cfg.sanitize();
    let text = serde_json::to_string_pretty(&cfg)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    std::fs::write(paths::config_file(), text)
        .map_err(|e| PyRuntimeError::new_err(format!("Не удалось сохранить конфиг: {e}")))?;
    Ok(())
}

fn config_get_inner(key: &str) -> Value {
    let mut cur = read_config_value();
    for part in key.split('.').filter(|p| !p.is_empty()) {
        match cur.get(part) {
            Some(v) => cur = v.clone(),
            None => return Value::Null,
        }
    }
    cur
}

fn config_set_inner(key: &str, value: Value) -> PyResult<()> {
    let mut root = read_config_value();
    let parts: Vec<&str> = key.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Err(PyRuntimeError::new_err("Пустой ключ настройки"));
    }
    let mut cur = &mut root;
    for part in &parts[..parts.len() - 1] {
        if !cur.is_object() {
            *cur = json!({});
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry((*part).to_string())
            .or_insert_with(|| json!({}));
    }
    if !cur.is_object() {
        *cur = json!({});
    }
    cur.as_object_mut()
        .unwrap()
        .insert(parts[parts.len() - 1].to_string(), value);
    write_config_value(&root)
}

fn read_storage(dir: &Path) -> Map<String, Value> {
    std::fs::read_to_string(dir.join("storage.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write_storage(dir: &Path, map: &Map<String, Value>) -> PyResult<()> {
    let text = serde_json::to_string_pretty(&Value::Object(map.clone()))
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    std::fs::write(dir.join("storage.json"), text)
        .map_err(|e| PyRuntimeError::new_err(format!("Не удалось сохранить storage.json: {e}")))
}

// ---------------------------------------------------------------------------
// Публичные Rust-функции (нужны менеджеру плагинов и командам)
// ---------------------------------------------------------------------------

/// Преобразует JSON в Python-объект.
pub fn json_to_py(py: Python<'_>, v: &Value) -> PyObject {
    match v {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py(py),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(u) = n.as_u64() {
                u.into_py(py)
            } else {
                n.as_f64().unwrap_or(0.0).into_py(py)
            }
        }
        Value::String(s) => s.into_py(py),
        Value::Array(a) => {
            let list = PyList::empty(py);
            for item in a {
                let _ = list.append(json_to_py(py, item));
            }
            list.into_py(py)
        }
        Value::Object(o) => {
            let dict = PyDict::new(py);
            for (k, v) in o {
                let _ = dict.set_item(k, json_to_py(py, v));
            }
            dict.into_py(py)
        }
    }
}

/// Преобразует Python-объект в JSON (для результатов хуков).
pub fn py_to_json(v: &PyAny) -> Value {
    if v.is_none() {
        return Value::Null;
    }
    if let Ok(b) = v.downcast::<PyBool>() {
        return Value::Bool(b.is_true());
    }
    if let Ok(s) = v.downcast::<PyString>() {
        return Value::String(s.to_string_lossy().to_string());
    }
    if let Ok(i) = v.extract::<i64>() {
        return json!(i);
    }
    if let Ok(f) = v.extract::<f64>() {
        return json!(f);
    }
    if let Ok(list) = v.downcast::<PyList>() {
        return Value::Array(list.iter().map(py_to_json).collect());
    }
    if let Ok(dict) = v.downcast::<PyDict>() {
        let mut out = Map::new();
        for (k, val) in dict.iter() {
            out.insert(k.str().map(|s| s.to_string()).unwrap_or_default(), py_to_json(val));
        }
        return Value::Object(out);
    }
    Value::String(v.str().map(|s| s.to_string()).unwrap_or_default())
}

/// Чтение значения из storage.json плагина (для SDK панелей).
pub fn storage_get(plugin: &str, key: &str) -> Option<Value> {
    let dir = REG.read().ok().and_then(|r| r.dirs.get(plugin).cloned())?;
    read_storage(&dir).get(key).cloned()
}

/// Запись значения в storage.json плагина (для SDK панелей).
pub fn storage_set(plugin: &str, key: &str, value: Value) -> Result<(), String> {
    let dir = REG
        .read()
        .ok()
        .and_then(|r| r.dirs.get(plugin).cloned())
        .ok_or_else(|| "Плагин не загружен".to_string())?;
    let mut map = read_storage(&dir);
    map.insert(key.chars().take(120).collect(), value);
    write_storage(&dir, &map).map_err(|e| e.to_string())
}

/// Весь storage.json плагина (для SDK панелей).
pub fn storage_all(plugin: &str) -> Value {
    REG.read()
        .ok()
        .and_then(|r| r.dirs.get(plugin).cloned())
        .map(|dir| Value::Object(read_storage(&dir)))
        .unwrap_or_else(|| json!({}))
}

/// Чтение настройки лаунчера из SDK панели (нужно право `config`).
pub fn config_get(plugin: &str, key: &str) -> Result<Value, String> {
    if !has_perm(plugin, "config") {
        return Err("Плагину нужно право «config» в манифесте".to_string());
    }
    Ok(config_get_inner(key))
}

/// Запись настройки лаунчера из SDK панели (нужно право `config`).
pub fn config_set(plugin: &str, key: &str, value: Value) -> Result<(), String> {
    if !has_perm(plugin, "config") {
        return Err("Плагину нужно право «config» в манифесте".to_string());
    }
    config_set_inner(key, value).map_err(|e| e.to_string())
}

/// Python-модуль `arbuz` для плагинов (лежит рядом: `arbuz_api.py`).
pub const ARBUZ_API: &str = include_str!("arbuz_api.py");

/// Создаёт нативный модуль `_arbuz_native` и кладёт его в `sys.modules`.
pub fn install_native(py: Python<'_>) -> PyResult<()> {
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    if modules.get_item("_arbuz_native").is_ok() {
        return Ok(());
    }
    let m = PyModule::new(py, "_arbuz_native")?;
    m.add_function(wrap_pyfunction!(py_current_plugin, m)?)?;
    m.add_function(wrap_pyfunction!(py_launcher_version, m)?)?;
    m.add_function(wrap_pyfunction!(py_log, m)?)?;
    m.add_function(wrap_pyfunction!(py_toast, m)?)?;
    m.add_function(wrap_pyfunction!(py_notify, m)?)?;
    m.add_function(wrap_pyfunction!(py_config_get, m)?)?;
    m.add_function(wrap_pyfunction!(py_config_set, m)?)?;
    m.add_function(wrap_pyfunction!(py_storage_get, m)?)?;
    m.add_function(wrap_pyfunction!(py_storage_set, m)?)?;
    m.add_function(wrap_pyfunction!(py_storage_all, m)?)?;
    m.add_function(wrap_pyfunction!(py_http_get, m)?)?;
    m.add_function(wrap_pyfunction!(py_http_post, m)?)?;
    m.add_function(wrap_pyfunction!(py_instances_list, m)?)?;
    m.add_function(wrap_pyfunction!(py_instance_get, m)?)?;
    m.add_function(wrap_pyfunction!(py_version_jar, m)?)?;
    m.add_function(wrap_pyfunction!(py_mc_visual, m)?)?;
    m.add_function(wrap_pyfunction!(py_assets_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_cache_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_data_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_plugin_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_libs_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_plugin_lib_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_read_text, m)?)?;
    m.add_function(wrap_pyfunction!(py_write_text, m)?)?;
    m.add_function(wrap_pyfunction!(py_read_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(py_write_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(py_cache_read_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(py_cache_write_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(py_list_dir, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_source, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_theme, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_panel, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_sidebar, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_command, m)?)?;
    m.add_function(wrap_pyfunction!(py_register_auth, m)?)?;
    m.add_function(wrap_pyfunction!(py_set_visual, m)?)?;
    modules.set_item("_arbuz_native", m)?;
    Ok(())
}

/// Создаёт модуль `arbuz` для плагина `id` и регистрирует его в `sys.modules`.
pub fn make_api_module(py: Python<'_>, id: &str) -> PyResult<Py<PyModule>> {
    install_native(py)?;
    set_current(Some(id.to_string()));
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    let name = format!("arbuz_{id}");
    let m = PyModule::from_code(py, ARBUZ_API, "arbuz.py", &name)?;
    modules.set_item("arbuz", m)?;
    set_current(None);
    Ok(m.into_py(py))
}

// ---------------------------------------------------------------------------
// Нативные функции, доступные Python
// ---------------------------------------------------------------------------

#[pyfunction]
#[pyo3(name = "current_plugin")]
fn py_current_plugin() -> String {
    current().unwrap_or_default()
}

#[pyfunction]
#[pyo3(name = "launcher_version")]
fn py_launcher_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[pyfunction]
#[pyo3(name = "log")]
fn py_log(plugin: &str, level: &str, msg: &str) -> PyResult<()> {
    let msg = msg.chars().take(4000).collect::<String>();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = json!({
        "plugin": plugin,
        "level": level,
        "message": msg,
        "ts": ts,
    });
    eprintln!("[plugin:{plugin}][{level}] {msg}");
    if let Ok(mut logs) = LOGS.write() {
        logs.push_back(entry.clone());
        while logs.len() > MAX_LOGS {
            logs.pop_front();
        }
    }
    emit("log", entry);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "toast")]
fn py_toast(plugin: &str, msg: &str) -> PyResult<()> {
    require(plugin, "ui")?;
    emit(
        "ui.toast",
        json!({ "plugin": plugin, "message": msg.chars().take(500).collect::<String>() }),
    );
    Ok(())
}

#[pyfunction]
#[pyo3(name = "notify")]
fn py_notify(plugin: &str, event: &str, payload: &str) -> PyResult<()> {
    require(plugin, "ui")?;
    let v: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
    emit(
        event,
        json!({ "plugin": plugin, "payload": v }),
    );
    Ok(())
}

#[pyfunction]
#[pyo3(name = "config_get")]
fn py_config_get(plugin: &str, key: &str) -> PyResult<String> {
    require(plugin, "config")?;
    Ok(config_get_inner(key).to_string())
}

#[pyfunction]
#[pyo3(name = "config_set")]
fn py_config_set(plugin: &str, key: &str, value: &str) -> PyResult<()> {
    require(plugin, "config")?;
    let v: Value = serde_json::from_str(value).unwrap_or(Value::Null);
    config_set_inner(key, v)?;
    emit("config.changed", json!({ "plugin": plugin, "key": key }));
    Ok(())
}

#[pyfunction]
#[pyo3(name = "storage_get")]
fn py_storage_get(plugin: &str, key: &str) -> PyResult<String> {
    let dir = plugin_dir(plugin)?;
    let map = read_storage(&dir);
    Ok(map.get(key).cloned().unwrap_or(Value::Null).to_string())
}

#[pyfunction]
#[pyo3(name = "storage_set")]
fn py_storage_set(plugin: &str, key: &str, value: &str) -> PyResult<()> {
    let dir = plugin_dir(plugin)?;
    let mut map = read_storage(&dir);
    let v: Value = serde_json::from_str(value).unwrap_or(Value::Null);
    map.insert(key.chars().take(120).collect(), v);
    write_storage(&dir, &map)
}

#[pyfunction]
#[pyo3(name = "storage_all")]
fn py_storage_all(plugin: &str) -> PyResult<String> {
    let dir = plugin_dir(plugin)?;
    Ok(Value::Object(read_storage(&dir)).to_string())
}

fn blocking_client(plugin: &str) -> PyResult<reqwest::blocking::Client> {
    require(plugin, "network")?;
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("ArbuzLauncher-Plugin/1.0")
        .timeout(std::time::Duration::from_secs(30));
    builder = builder.proxy(reqwest::Proxy::custom(crate::state::manual_proxy_for));
    builder
        .build()
        .map_err(|e| PyRuntimeError::new_err(format!("HTTP-клиент: {e}")))
}

fn parse_headers(raw: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .map(|o| {
            o.into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn http_result(resp: reqwest::blocking::Response) -> PyResult<String> {
    let status = resp.status().as_u16();
    let ok = resp.status().is_success();
    let headers = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
        .collect::<std::collections::HashMap<String, String>>();
    let body = resp.text().unwrap_or_default();
    let body = body.chars().take(5 * 1024 * 1024).collect::<String>();
    Ok(json!({ "status": status, "ok": ok, "body": body, "headers": headers }).to_string())
}

#[pyfunction]
#[pyo3(name = "http_get")]
fn py_http_get(plugin: &str, url: &str, headers: &str) -> PyResult<String> {
    crate::http::validate_url(url).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let client = blocking_client(plugin)?;
    let mut req = client.get(url);
    for (k, v) in parse_headers(headers) {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .map_err(|e| PyRuntimeError::new_err(format!("HTTP: {e}")))?;
    http_result(resp)
}

#[pyfunction]
#[pyo3(name = "http_post")]
fn py_http_post(plugin: &str, url: &str, data: &str, headers: &str) -> PyResult<String> {
    crate::http::validate_url(url).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let client = blocking_client(plugin)?;
    let mut req = client.post(url).body(data.to_string());
    for (k, v) in parse_headers(headers) {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .map_err(|e| PyRuntimeError::new_err(format!("HTTP: {e}")))?;
    http_result(resp)
}

fn instances_json() -> Vec<Value> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(paths::instances_dir()) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path().join("instance.json");
        if let Ok(raw) = std::fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                out.push(v);
            }
        }
    }
    out
}

#[pyfunction]
#[pyo3(name = "instances_list")]
fn py_instances_list(plugin: &str) -> PyResult<String> {
    require(plugin, "instances")?;
    Ok(Value::Array(instances_json()).to_string())
}

#[pyfunction]
#[pyo3(name = "instance_get")]
fn py_instance_get(plugin: &str, id: &str) -> PyResult<String> {
    require(plugin, "instances")?;
    paths::validate_instance_id(id).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let raw = std::fs::read_to_string(paths::instance_json(id))
        .map_err(|e| PyRuntimeError::new_err(format!("Профиль не найден: {e}")))?;
    Ok(raw)
}

#[pyfunction]
#[pyo3(name = "version_jar")]
fn py_version_jar(plugin: &str, id: &str) -> PyResult<String> {
    require(plugin, "mc")?;
    paths::validate_instance_id(id).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let raw = std::fs::read_to_string(paths::instance_json(id))
        .map_err(|e| PyRuntimeError::new_err(format!("Профиль не найден: {e}")))?;
    let v: Value = serde_json::from_str(&raw).unwrap_or_default();
    let mc = v.get("mcVersion").and_then(|x| x.as_str()).unwrap_or("");
    let jar = paths::version_jar(mc);
    if jar.is_file() {
        Ok(jar.to_string_lossy().to_string())
    } else {
        Ok(String::new())
    }
}

#[pyfunction]
#[pyo3(name = "mc_visual")]
fn py_mc_visual(plugin: &str, id: &str) -> PyResult<String> {
    require(plugin, "mc")?;
    crate::mc_visual::extract_sync(id)
        .map(|v| v.to_string())
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
#[pyo3(name = "assets_dir")]
fn py_assets_dir(plugin: &str) -> PyResult<String> {
    require(plugin, "mc")?;
    Ok(paths::assets_dir().to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "cache_dir")]
fn py_cache_dir(plugin: &str) -> PyResult<String> {
    let dir = paths::plugin_cache_dir(plugin);
    let _ = std::fs::create_dir_all(&dir);
    Ok(dir.to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "data_dir")]
fn py_data_dir(_plugin: &str) -> PyResult<String> {
    Ok(paths::root().to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "plugin_dir")]
fn py_plugin_dir(plugin: &str) -> PyResult<String> {
    Ok(plugin_dir(plugin)?.to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "libs_dir")]
fn py_libs_dir() -> PyResult<String> {
    let dir = paths::libs_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Папка библиотек: {e}")))?;
    Ok(dir.to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "plugin_lib_dir")]
fn py_plugin_lib_dir(plugin: &str) -> PyResult<String> {
    let dir = plugin_dir(plugin)?.join("lib");
    std::fs::create_dir_all(&dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Папка библиотек плагина: {e}")))?;
    Ok(dir.to_string_lossy().to_string())
}

#[pyfunction]
#[pyo3(name = "read_text")]
fn py_read_text(plugin: &str, name: &str) -> PyResult<String> {
    require(plugin, "fs")?;
    let p = safe_join(&plugin_dir(plugin)?, name)?;
    std::fs::read_to_string(p)
        .map_err(|e| PyRuntimeError::new_err(format!("Чтение файла: {e}")))
}

#[pyfunction]
#[pyo3(name = "write_text")]
fn py_write_text(plugin: &str, name: &str, text: &str) -> PyResult<()> {
    require(plugin, "fs")?;
    let p = safe_join(&plugin_dir(plugin)?, name)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    }
    std::fs::write(p, text).map_err(|e| PyRuntimeError::new_err(format!("Запись файла: {e}")))
}

#[pyfunction]
#[pyo3(name = "read_bytes")]
fn py_read_bytes(plugin: &str, name: &str) -> PyResult<Vec<u8>> {
    require(plugin, "fs")?;
    let p = safe_join(&plugin_dir(plugin)?, name)?;
    std::fs::read(p).map_err(|e| PyRuntimeError::new_err(format!("Чтение файла: {e}")))
}

#[pyfunction]
#[pyo3(name = "write_bytes")]
fn py_write_bytes(plugin: &str, name: &str, data: Vec<u8>) -> PyResult<()> {
    require(plugin, "fs")?;
    let p = safe_join(&plugin_dir(plugin)?, name)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    }
    std::fs::write(p, data).map_err(|e| PyRuntimeError::new_err(format!("Запись файла: {e}")))
}

fn cache_join(plugin: &str, name: &str) -> PyResult<PathBuf> {
    let dir = paths::plugin_cache_dir(plugin);
    std::fs::create_dir_all(&dir).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    safe_join(&dir, name)
}

#[pyfunction]
#[pyo3(name = "cache_read_bytes")]
fn py_cache_read_bytes(plugin: &str, name: &str) -> PyResult<Vec<u8>> {
    let p = cache_join(plugin, name)?;
    std::fs::read(p).map_err(|e| PyRuntimeError::new_err(format!("Чтение кэша: {e}")))
}

#[pyfunction]
#[pyo3(name = "cache_write_bytes")]
fn py_cache_write_bytes(plugin: &str, name: &str, data: Vec<u8>) -> PyResult<()> {
    let p = cache_join(plugin, name)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    }
    std::fs::write(p, data).map_err(|e| PyRuntimeError::new_err(format!("Запись кэша: {e}")))
}

#[pyfunction]
#[pyo3(name = "list_dir")]
fn py_list_dir(plugin: &str, name: &str) -> PyResult<String> {
    require(plugin, "fs")?;
    let base = plugin_dir(plugin)?;
    let p = if name == "." || name.is_empty() {
        base
    } else {
        safe_join(&base, name)?
    };
    let mut out = Vec::new();
    let rd = std::fs::read_dir(p).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    for e in rd.flatten() {
        out.push(json!({
            "name": e.file_name().to_string_lossy(),
            "dir": e.file_type().map(|t| t.is_dir()).unwrap_or(false),
        }));
    }
    Ok(Value::Array(out).to_string())
}

fn upsert(list: &mut Vec<Value>, item: Value) {
    let id = item.get("id").and_then(|x| x.as_str()).unwrap_or("");
    if let Some(slot) = list
        .iter_mut()
        .find(|x| x.get("id").and_then(|v| v.as_str()) == Some(id))
    {
        *slot = item;
    } else {
        list.push(item);
    }
}

#[pyfunction]
#[pyo3(name = "register_source")]
fn py_register_source(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "sources")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректный источник: {e}")))?;
    if v.get("id").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
        return Err(PyRuntimeError::new_err("У источника должен быть id"));
    }
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        upsert(&mut r.sources, v.clone());
    }
    emit("source.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "register_theme")]
fn py_register_theme(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "themes")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректная тема: {e}")))?;
    if v.get("id").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
        return Err(PyRuntimeError::new_err("У темы должен быть id"));
    }
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        upsert(&mut r.themes, v.clone());
    }
    emit("theme.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "register_panel")]
fn py_register_panel(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "ui")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректная панель: {e}")))?;
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        r.panels.push(v.clone());
    }
    emit("panel.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "register_sidebar")]
fn py_register_sidebar(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "ui")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректный пункт сайдбара: {e}")))?;
    if v.get("id").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
        return Err(PyRuntimeError::new_err("У пункта сайдбара должен быть id"));
    }
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        upsert(&mut r.sidebar, v.clone());
    }
    emit("sidebar.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "register_command")]
fn py_register_command(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "ui")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректная команда: {e}")))?;
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        r.commands.push(v.clone());
    }
    emit("command.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "register_auth")]
fn py_register_auth(plugin: &str, raw: &str) -> PyResult<()> {
    require(plugin, "auth")?;
    let mut v: Value = serde_json::from_str(raw)
        .map_err(|e| PyRuntimeError::new_err(format!("Некорректный провайдер входа: {e}")))?;
    if v.get("id").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
        return Err(PyRuntimeError::new_err("У провайдера входа должен быть id"));
    }
    v["plugin"] = json!(plugin);
    if let Ok(mut r) = REG.write() {
        upsert(&mut r.providers, v.clone());
    }
    emit("auth.registered", v);
    Ok(())
}

#[pyfunction]
#[pyo3(name = "set_visual")]
fn py_set_visual(plugin: &str, raw: &str) -> PyResult<()> {
    let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    if let Ok(mut r) = REG.write() {
        r.visuals.insert(plugin.to_string(), v.clone());
    }
    emit("visual.changed", json!({ "plugin": plugin, "visual": v }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_is_safe() {
        pyo3::Python::with_gil(|py| {
            let v = json!({"a": [1, 2, true, null, "x"], "b": {"c": 3.5}});
            let obj = json_to_py(py, &v);
            let back = py_to_json(obj.as_ref(py));
            assert_eq!(v, back);
        });
    }

    #[test]
    fn safe_join_blocks_traversal() {
        assert!(safe_join(Path::new("C:/base"), "a/b.txt").is_ok());
        assert!(safe_join(Path::new("C:/base"), "../x").is_err());
        assert!(safe_join(Path::new("C:/base"), "C:/x").is_err());
    }

    #[test]
    fn plugin_can_import_local_library() {
        pyo3::Python::with_gil(|py| {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!("arbuz-lib-{stamp}"));
            let lib = root.join("lib");
            std::fs::create_dir_all(&lib).unwrap();
            std::fs::write(lib.join("arbuz_test_library.py"), "VALUE = 42\n").unwrap();

            prepare_library_paths(py, &root).unwrap();
            let module = py.import("arbuz_test_library").unwrap();
            assert_eq!(
                module.getattr("VALUE").unwrap().extract::<i32>().unwrap(),
                42
            );
            let _ = std::fs::remove_dir_all(&root);
        });
    }
}
