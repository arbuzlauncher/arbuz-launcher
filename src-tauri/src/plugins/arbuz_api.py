import json as _json
import _arbuz_native as _n

PLUGIN_ID = _n.current_plugin()
VERSION = _n.launcher_version()
API_VERSION = 1
_MISSING = object()

def _loads(s):
    try:
        return _json.loads(s)
    except Exception:
        return None

def _dumps(v):
    try:
        return _json.dumps(v, ensure_ascii=False)
    except Exception:
        return "null"

class Response:
    def __init__(self, d):
        self.status = int(d.get("status", 0) or 0)
        self.ok = bool(d.get("ok"))
        self.body = d.get("body", "")
        self.headers = d.get("headers", {})
    def json(self):
        return _json.loads(self.body or "null")
    def __repr__(self):
        return f"<Response {self.status}>"

class _HTTP:
    def get(self, url, headers=None, timeout=None):
        return Response(_loads(_n.http_get(PLUGIN_ID, str(url), _dumps(headers or {}))))
    def post(self, url, data="", headers=None):
        return Response(_loads(_n.http_post(PLUGIN_ID, str(url), str(data), _dumps(headers or {}))))
    def get_json(self, url, headers=None, timeout=None):
        return self.get(url, headers, timeout).json()
    def post_json(self, url, data=None, headers=None):
        h = {"Content-Type": "application/json"}
        if headers:
            h.update(headers)
        return self.post(url, _dumps(data if data is not None else {}), h).json()

class _Storage:
    def get(self, key, default=None):
        v = _loads(_n.storage_get(PLUGIN_ID, str(key)))
        return default if v is None else v
    def set(self, key, value):
        _n.storage_set(PLUGIN_ID, str(key), _dumps(value))
    def all(self):
        return _loads(_n.storage_all(PLUGIN_ID)) or {}
    def __getitem__(self, key):
        v = self.get(key, _MISSING)
        if v is _MISSING:
            raise KeyError(key)
        return v
    def __setitem__(self, key, value):
        self.set(key, value)
    def __contains__(self, key):
        return self.get(key, _MISSING) is not _MISSING
    def update(self, values):
        for k, v in dict(values).items():
            self.set(k, v)

class _Config:
    def get(self, key, default=None):
        v = _loads(_n.config_get(PLUGIN_ID, str(key)))
        return default if v is None else v
    def set(self, key, value):
        _n.config_set(PLUGIN_ID, str(key), _dumps(value))

class _Instances:
    def list(self):
        return _loads(_n.instances_list(PLUGIN_ID)) or []
    def get(self, instance_id):
        return _loads(_n.instance_get(PLUGIN_ID, str(instance_id)))
    def version_jar(self, instance_id):
        return _n.version_jar(PLUGIN_ID, str(instance_id))

class _Mc:
    def visual(self, instance_id):
        return _loads(_n.mc_visual(PLUGIN_ID, str(instance_id)))
    def assets_dir(self):
        return _n.assets_dir(PLUGIN_ID)
    def cache_dir(self):
        return _n.cache_dir(PLUGIN_ID)

class _Sources:
    def register(self, desc):
        _n.register_source(PLUGIN_ID, _dumps(desc))

class _Themes:
    def register(self, desc):
        _n.register_theme(PLUGIN_ID, _dumps(desc))

class _Panels:
    def register(self, desc):
        _n.register_panel(PLUGIN_ID, _dumps(desc))

class _Sidebar:
    def register(self, desc):
        _n.register_sidebar(PLUGIN_ID, _dumps(desc))
    def add(self, desc):
        self.register(desc)

class _Commands:
    def register(self, desc):
        _n.register_command(PLUGIN_ID, _dumps(desc))

class _Auth:
    def register(self, desc):
        _n.register_auth(PLUGIN_ID, _dumps(desc))

class _Ui:
    def toast(self, message):
        _n.toast(PLUGIN_ID, str(message))
    def notify(self, event, payload=None):
        _n.notify(PLUGIN_ID, str(event), _dumps(payload))
    def set_visual(self, visual):
        _n.set_visual(PLUGIN_ID, _dumps(visual))
    def open_panel(self, panel):
        self.notify("ui.open_panel", {"panel": str(panel)})
    def emit(self, event, data=None):
        self.notify("panel.event", {"event": str(event), "data": data})

_hooks = {}

def hook(name):
    def deco(fn):
        _hooks.setdefault(str(name), []).append(fn)
        return fn
    return deco

def on(name):
    return hook(name)

def command(name=None, title=None, description=None):
    def deco(fn):
        cid = str(name or fn.__name__)
        commands.register({
            "id": cid,
            "title": str(title or cid),
            "description": str(description or ""),
        })
        hook("command." + cid)(fn)
        return fn
    return deco

def panel(pid=None, title=None, description=None, icon=None, height=None):
    def deco(fn):
        pid2 = str(pid or fn.__name__)
        desc = {
            "id": pid2,
            "title": str(title or pid2),
            "description": str(description or ""),
        }
        if icon:
            desc["icon"] = str(icon)
        if height:
            desc["height"] = int(height)
        panels.register(desc)
        hook("panel." + pid2 + ".render")(fn)
        return fn
    return deco

def on_start(fn):
    return hook("launcher.start")(fn)

def on_ready(fn):
    return hook("launcher.ready")(fn)

def log(*args):
    _n.log(PLUGIN_ID, "info", " ".join(str(a) for a in args))

def warn(*args):
    _n.log(PLUGIN_ID, "warn", " ".join(str(a) for a in args))

def error(*args):
    _n.log(PLUGIN_ID, "error", " ".join(str(a) for a in args))

http = _HTTP()
storage = _Storage()
config = _Config()
instances = _Instances()
mc = _Mc()
sources = _Sources()
themes = _Themes()
panels = _Panels()
sidebar = _Sidebar()
commands = _Commands()
auth = _Auth()
ui = _Ui()

def data_dir():
    return _n.data_dir(PLUGIN_ID)

def plugin_dir():
    return _n.plugin_dir(PLUGIN_ID)

def libs_dir():
    """Общая папка библиотек: <data>/libs (создаётся автоматически)."""
    return _n.libs_dir()

def plugin_lib_dir():
    """Папка lib/ рядом с .aplug: локальные модули этого плагина."""
    return _n.plugin_lib_dir(PLUGIN_ID)

def library(name):
    """Импортирует пользовательскую библиотеку по имени модуля."""
    import importlib
    return importlib.import_module(str(name))

def reload_library(name):
    """Перезагружает уже импортированную библиотеку (удобно при разработке)."""
    import importlib
    return importlib.reload(importlib.import_module(str(name)))

def read_text(name):
    return _n.read_text(PLUGIN_ID, str(name))

def write_text(name, text):
    _n.write_text(PLUGIN_ID, str(name), str(text))

def read_json(name, default=None):
    try:
        return _loads(read_text(name))
    except Exception:
        return default

def write_json(name, value):
    write_text(name, _dumps(value))

def read_bytes(name):
    return _n.read_bytes(PLUGIN_ID, str(name))

def write_bytes(name, data):
    _n.write_bytes(PLUGIN_ID, str(name), bytes(data))

def cache_read_bytes(name):
    return _n.cache_read_bytes(PLUGIN_ID, str(name))

def cache_write_bytes(name, data):
    _n.cache_write_bytes(PLUGIN_ID, str(name), bytes(data))

def list_dir(name="."):
    return _loads(_n.list_dir(PLUGIN_ID, str(name))) or []
