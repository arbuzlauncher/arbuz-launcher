//! Менеджер плагинов: установка, загрузка, хуки и реестр расширений.

use crate::plugins::{manifest::Manifest, runtime};
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub api: u32,
    pub hooks: Vec<String>,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub error: Option<String>,
    pub path: String,
    pub icon: Option<String>,
    pub homepage: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    pub plugin: String,
    pub event: String,
    pub result: Value,
    pub error: Option<String>,
}

struct LoadedPlugin {
    manifest: Manifest,
    module: Py<PyModule>,
    api: Py<PyModule>,
    hooks: Py<PyDict>,
    dir: PathBuf,
}

pub struct PluginManager {
    loaded: Mutex<Vec<LoadedPlugin>>,
    errors: Mutex<HashMap<String, String>>,
    enabled: AtomicBool,
}

static MANAGER: Lazy<PluginManager> = Lazy::new(|| PluginManager {
    loaded: Mutex::new(Vec::new()),
    errors: Mutex::new(HashMap::new()),
    enabled: AtomicBool::new(false),
});

pub fn manager() -> &'static PluginManager {
    &MANAGER
}

fn disabled_file() -> PathBuf {
    crate::paths::plugins_dir().join(".disabled.json")
}

fn load_disabled() -> HashSet<String> {
    std::fs::read_to_string(disabled_file())
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn save_disabled(set: &HashSet<String>) -> crate::error::Res<()> {
    let dir = crate::paths::plugins_dir();
    std::fs::create_dir_all(&dir)?;
    let mut list: Vec<String> = set.iter().cloned().collect();
    list.sort();
    std::fs::write(disabled_file(), serde_json::to_string_pretty(&list)?)?;
    Ok(())
}

fn collect_aplug(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "assets" || name == "__pycache__" || name == "node_modules" {
                continue;
            }
            collect_aplug(&p, depth - 1, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("aplug") {
            out.push(p);
        }
    }
}

/// Все установленные плагины (включая выключенные и с ошибками).
pub fn scan_installed() -> Vec<PluginInfo> {
    let disabled = load_disabled();
    let mut files = Vec::new();
    collect_aplug(&crate::paths::plugins_dir(), 4, &mut files);
    let errors = manager()
        .errors
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let mut map: HashMap<String, (Manifest, PathBuf)> = HashMap::new();
    for file in files {
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin");
        let m = crate::plugins::manifest::discover(&file, stem);
        map.entry(m.id.clone()).or_insert((m, file));
    }
    let mut out: Vec<PluginInfo> = map
        .into_iter()
        .map(|(id, (m, path))| PluginInfo {
            enabled: manager().enabled.load(Ordering::Relaxed) && !disabled.contains(&id),
            error: errors.get(&id).cloned(),
            id: m.id.clone(),
            name: m.name,
            version: m.version,
            author: m.author,
            description: m.description,
            api: m.api,
            hooks: m.hooks,
            permissions: m.permissions,
            path: path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            icon: m.icon,
            homepage: m.homepage,
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn module_name(id: &str) -> String {
    let clean: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("arbuz_plugin_{clean}")
}

fn load_one(
    manifest: &Manifest,
    dir: &Path,
    source_path: &Path,
) -> Result<LoadedPlugin, String> {
    let source = std::fs::read_to_string(source_path)
        .map_err(|e| format!("Не удалось прочитать {}: {e}", source_path.display()))?;
    let file_name = source_path.to_string_lossy().to_string();
    let mod_name = module_name(&manifest.id);

    Python::with_gil(|py| -> Result<LoadedPlugin, String> {
        runtime::register_plugin(&manifest.id, dir, &manifest.permissions);
        runtime::prepare_library_paths(py, dir)
            .map_err(|e| format!("Библиотеки плагина: {e}"))?;
        let api = runtime::make_api_module(py, &manifest.id)
            .map_err(|e| format!("API плагина: {e}"))?;
        let module = PyModule::from_code(py, &source, &file_name, &mod_name)
            .map_err(|e| format!("Ошибка выполнения: {e}"))?;
        let sys = py.import("sys").map_err(|e| e.to_string())?;
        let modules = sys.getattr("modules").map_err(|e| e.to_string())?;
        modules
            .set_item(module_name(&manifest.id), module)
            .map_err(|e| e.to_string())?;
        modules
            .set_item("arbuz", api.clone_ref(py))
            .map_err(|e| e.to_string())?;

        // Необязательный setup() вызывается сразу после загрузки.
        if let Ok(setup) = module.getattr("setup") {
            if setup.is_callable() {
                if let Err(e) = setup.call0() {
                    eprintln!("[plugin:{}] setup: {e}", manifest.id);
                }
            }
        }

        let hooks = api
            .as_ref(py)
            .getattr("_hooks")
            .ok()
            .and_then(|v| v.downcast::<PyDict>().ok())
            .map(|d| d.into_py(py))
            .unwrap_or_else(|| PyDict::new(py).into_py(py));

        Ok(LoadedPlugin {
            manifest: manifest.clone(),
            module: module.into_py(py),
            api,
            hooks,
            dir: dir.to_path_buf(),
        })
    })
}

/// Перезагружает все плагины. `enabled = false` полностью выгружает Python-код.
pub fn reload(enabled: bool) {
    manager().enabled.store(enabled, Ordering::Relaxed);
    unload_all();
    if !enabled {
        return;
    }
    let disabled = load_disabled();
    let mut files = Vec::new();
    collect_aplug(&crate::paths::plugins_dir(), 4, &mut files);
    files.sort();

    let mut seen: HashSet<String> = HashSet::new();
    let mut loaded = Vec::new();
    let mut errors = HashMap::new();
    for file in files {
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin");
        let m = crate::plugins::manifest::discover(&file, stem);
        if disabled.contains(&m.id) || !seen.insert(m.id.clone()) {
            continue;
        }
        let dir = file.parent().unwrap_or_else(|| Path::new("."));
        match load_one(&m, dir, &file) {
            Ok(p) => loaded.push(p),
            Err(e) => {
                eprintln!("[plugin:{}] {e}", m.id);
                runtime::unregister_plugin(&m.id);
                errors.insert(m.id.clone(), e);
            }
        }
    }
    if let Ok(mut g) = manager().loaded.lock() {
        *g = loaded;
    }
    if let Ok(mut g) = manager().errors.lock() {
        *g = errors;
    }
}

fn unload_all() {
    let plugins = manager()
        .loaded
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default();
    if plugins.is_empty() {
        runtime::clear_plugins();
        return;
    }
    Python::with_gil(|py| {
        for p in &plugins {
            runtime::set_current(Some(p.manifest.id.clone()));
            let sys = match py.import("sys") {
                Ok(s) => s,
                Err(_) => continue,
            };
            if let Ok(modules) = sys.getattr("modules") {
                let _ = modules.set_item("arbuz", p.api.as_ref(py));
            }
            if let Ok(teardown) = p.module.as_ref(py).getattr("teardown") {
                if teardown.is_callable() {
                    if let Err(e) = teardown.call0() {
                        eprintln!("[plugin:{}] teardown: {e}", p.manifest.id);
                    }
                }
            }
        }
        runtime::set_current(None);
    });
    runtime::clear_plugins();
}

/// Вызывает событие у всех подписанных плагинов.
pub fn call_hook(event: &str, payload: &Value) -> Vec<HookResult> {
    if !manager().enabled.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let loaded = manager()
        .loaded
        .lock()
        .map(|g| g.iter().map(|p| p.manifest.id.clone()).collect::<Vec<_>>())
        .unwrap_or_default();
    if loaded.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for id in loaded {
        let (api, module, hooks) = {
            let g = match manager().loaded.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            let Some(p) = g.iter().find(|p| p.manifest.id == id) else {
                continue;
            };
            (p.api.clone(), p.module.clone(), p.hooks.clone())
        };
        let results = Python::with_gil(|py| -> Vec<(Value, Option<String>)> {
            runtime::set_current(Some(id.clone()));
            let sys = match py.import("sys") {
                Ok(s) => s,
                Err(e) => return vec![(Value::Null, Some(e.to_string()))],
            };
            if let Ok(modules) = sys.getattr("modules") {
                let _ = modules.set_item("arbuz", api.as_ref(py));
            }
            let hooks = hooks.as_ref(py);
            let Some(list) = hooks.get_item(event).ok().flatten() else {
                return Vec::new();
            };
            let Ok(list) = list.downcast::<pyo3::types::PyList>() else {
                return Vec::new();
            };
            let py_payload = runtime::json_to_py(py, payload);
            let mut out = Vec::new();
            for cb in list.iter() {
                if !cb.is_callable() {
                    continue;
                }
                match cb.call1((py_payload.clone_ref(py),)) {
                    Ok(v) => out.push((runtime::py_to_json(v), None)),
                    Err(e) => out.push((Value::Null, Some(e.to_string()))),
                }
            }
            out
        });
        runtime::set_current(None);
        for (result, error) in results {
            out.push(HookResult {
                plugin: id.clone(),
                event: event.to_string(),
                result,
                error,
            });
        }
        let _ = module;
    }
    out
}

/// Результат визуального хука: последний непустой объект.
pub fn visual(instance_id: &str) -> Value {
    let results = call_hook("visual.instance", &json!({ "instanceId": instance_id }));
    for r in results.into_iter().rev() {
        if r.error.is_none() && r.result.is_object() {
            return r.result;
        }
    }
    crate::mc_visual::extract_sync(instance_id)
        .unwrap_or_else(|e| json!({ "error": e.to_string(), "instanceId": instance_id }))
}

/// Поиск по пользовательскому источнику: плагин возвращает список элементов.
pub fn catalog_search(source: &str, args: &Value) -> Vec<Value> {
    let results = call_hook(
        "source.search",
        &json!({ "source": source, "args": args }),
    );
    let mut out = Vec::new();
    for r in results {
        if r.error.is_some() {
            continue;
        }
        match r.result {
            Value::Array(items) => out.extend(items),
            Value::Object(mut o) => {
                if let Some(Value::Array(items)) = o.remove("hits") {
                    out.extend(items);
                }
            }
            _ => {}
        }
    }
    out
}

/// Выполняет команду, зарегистрированную плагином (`command.<name>`).
pub fn run_command(plugin: &str, command: &str, args: &Value) -> Vec<HookResult> {
    call_hook(
        &format!("command.{command}"),
        &json!({ "plugin": plugin, "command": command, "args": args }),
    )
}

pub fn set_plugin_enabled(id: &str, enabled: bool) -> crate::error::Res<()> {
    let mut set = load_disabled();
    if enabled {
        set.remove(id);
    } else {
        set.insert(id.to_string());
    }
    save_disabled(&set)
}

pub fn is_enabled(id: &str) -> bool {
    manager().enabled.load(Ordering::Relaxed) && !load_disabled().contains(id)
}

/// Загружен ли плагин прямо сейчас (включён глобально и не отключён).
pub fn is_loaded(id: &str) -> bool {
    manager()
        .loaded
        .lock()
        .map(|g| g.iter().any(|p| p.manifest.id == id))
        .unwrap_or(false)
}

/// Глобальный переключатель плагинов (для команд установки/перезагрузки).
pub fn global_enabled() -> bool {
    manager().enabled.load(Ordering::Relaxed)
}

/// Убирает старые тестовые артефакты из data-папки (не встроенные плагины).
pub fn cleanup_legacy_artifacts() {
    let plugins = crate::paths::plugins_dir();
    for name in ["mc-visual.aplug", "2048.aplug"] {
        let _ = std::fs::remove_file(plugins.join(name));
    }
    let theme = crate::paths::themes_dir().join("winui3-fluent");
    if theme.is_dir() {
        let _ = std::fs::remove_dir_all(theme);
    }
}

/// Папка конкретного плагина (для удаления и установки).
pub fn plugin_dir_for(id: &str) -> Option<PathBuf> {
    if let Ok(g) = manager().loaded.lock() {
        if let Some(p) = g.iter().find(|p| p.manifest.id == id) {
            return Some(p.dir.clone());
        }
    }
    scan_installed()
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| PathBuf::from(p.path))
}
