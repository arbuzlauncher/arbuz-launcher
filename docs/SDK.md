# ArbuzSDK

ArbuzSDK is the plugin API of Arbuz Launcher. It has two sides:

- **Python** — the `arbuz` module available to every `.aplug`. It registers
  panels, commands, sidebar items, sources, themes, auth providers, and talks to
  the launcher (storage, config, HTTP, files, instances, Minecraft helpers).
- **JavaScript** — `window.arbuz` inside every plugin panel. It calls plugin
  commands, reads/writes plugin storage, shows toasts, resizes or closes the
  panel, opens other panels, and receives events from Python.

The SDK is versioned with the launcher. `arbuz.VERSION` / `arbuz.version` is the
launcher version; `arbuz.API_VERSION` / `arbuz.apiVersion` is the plugin API
version (currently `1`).

## Quick start

A minimal plugin:

```python
# arbuz-plugin: {"id":"my.sdk","name":"My SDK plugin","version":"1.0.0","author":"You","api":1,"permissions":["ui"],"hooks":["panel.sdk-demo.render","command.sdk.ping"]}

import arbuz

@arbuz.panel("sdk-demo", title="SDK demo", height=560)
def render(payload):
    return {
        "html": "<button id='ping'>Ping</button><pre id='out'></pre>",
        "css": "button{padding:8px 14px}pre{margin-top:12px}",
        "js": """
          arbuz.ready(function (theme) {
            document.getElementById('out').textContent = 'theme: ' + JSON.stringify(theme);
          });
          document.getElementById('ping').onclick = async function () {
            const result = await arbuz.command('sdk.ping', { at: Date.now() });
            document.getElementById('out').textContent = JSON.stringify(result);
          };
        """,
        "height": 560,
    }

@arbuz.command("sdk.ping", title="Ping")
def ping(payload):
    return {"pong": True, "args": payload.get("args")}

def setup():
    arbuz.sidebar.add({
        "id": "sdk-demo",
        "title": "SDK demo",
        "icon": "modpacks",
        "panel": "sdk-demo",
        "order": 20,
    })
```

Save it as `<name>.aplug` and install it from **Experimental → Install plugin**,
or copy it to `<data>/plugins/<name>.aplug`.

## Python API

### Panels and commands

```python
@arbuz.panel(
    "my-page",                 # panel id, defaults to the function name
    title="My page",
    description="Opened from the sidebar",
    icon="modpacks",           # optional pixel icon name
    height=600,
)
def render(payload):
    # payload: {"plugin": "...", "panel": "my-page", "args": ...}
    return {"html": "...", "css": "...", "js": "...", "height": 600}
```

`@arbuz.panel` registers the panel and subscribes the function to
`panel.<id>.render`. The launcher also calls the generic `panel.render` hook, so
both styles work.

```python
@arbuz.command("my.command", title="Do something", description="...")
def run(payload):
    # payload: {"plugin": "...", "command": "my.command", "args": ...}
    return {"ok": True}
```

`@arbuz.command` registers the command in the plugin list and subscribes the
function to `command.<id>`. Panels call it with `arbuz.command("my.command")`.

### Sidebar

```python
arbuz.sidebar.add({
    "id": "my-page",
    "title": "My page",
    "description": "Tooltip",
    "icon": "modpacks",
    "panel": "my-page",   # panel opened on click, defaults to id
    "order": 10,          # lower first
    "badge": "NEW",       # optional
})
```

`arbuz.sidebar.register(...)` is an alias. Requires the `ui` permission.

### Lifecycle and hooks

```python
@arbuz.on_start
def started(payload):
    # payload: {"version": "...", "dataDir": "..."}
    arbuz.log("launcher started")

@arbuz.on_ready
def ready(payload):
    pass

@arbuz.hook("launcher.start")
def same_as_on_start(payload):
    pass
```

`arbuz.hook(name)` and `arbuz.on(name)` are aliases. Hooks are listed in
[`PLUGINS.md`](PLUGINS.md#hooks). The SDK adds two decorators:

- `arbuz.on_start` → `launcher.start`
- `arbuz.on_ready` → `launcher.ready`

### Storage

Plugin-private JSON storage in `<plugin>/storage.json`:

```python
arbuz.storage.set("best", 42)
best = arbuz.storage.get("best", 0)
everything = arbuz.storage.all()

# dict-like
arbuz.storage["best"] = 42
"best" in arbuz.storage
arbuz.storage.update({"a": 1, "b": 2})
```

The panel has the same storage through `arbuz.storage` in JavaScript.

### Config

Reads/writes launcher settings (requires the `config` permission):

```python
arbuz.config.get("lang", "en")
arbuz.config.set("lang", "ru")
```

### HTTP

Requires the `network` permission. Requests respect the launcher proxy.

```python
r = arbuz.http.get("https://example.com/api")
if r.ok:
    data = r.json()

data = arbuz.http.get_json("https://example.com/api")
r = arbuz.http.post_json("https://example.com/api", {"name": "Arbuz"})
```

### Files and paths

```python
arbuz.data_dir()          # <data> (launcher data root)
arbuz.plugin_dir()        # installed plugin folder
arbuz.read_text("file.txt")
arbuz.write_text("file.txt", "hello")
arbuz.read_json("state.json", {})
arbuz.write_json("state.json", {"a": 1})
arbuz.read_bytes("image.png")
arbuz.write_bytes("image.png", b"...")
arbuz.list_dir(".")
```

File helpers are restricted to the plugin's data/plugin folders.

### Libraries

You can write your own Python libraries for the launcher. Two folders are added
to `sys.path` automatically:

- `<data>/libs/` — shared by every plugin; create it with `arbuz.libs_dir()` or
  the **Libraries folder** button in the plugins panel.
- `<plugin>/lib/` — local to one plugin; create it with
  `arbuz.plugin_lib_dir()`.

A library is an ordinary Python module:

```python
# <data>/libs/arbuz_helpers.py
def greet(name):
    return f"Hello, {name}!"
```

```python
# inside any .aplug
import arbuz_helpers

arbuz.log(arbuz_helpers.greet("Arbuz"))

# or load it explicitly
helpers = arbuz.library("arbuz_helpers")
arbuz.reload_library("arbuz_helpers")   # during development
```

`arbuz.library(name)` is a thin wrapper over `importlib.import_module`, and
`arbuz.reload_library(name)` reloads an already imported module. Module names
must be unique: all plugins share the same Python process.

Panels can use JavaScript libraries too. Keep the library file in the plugin
folder, read it as text, and append it to the panel `js`:

```python
def render(payload):
    lib = arbuz.read_text("lib/ui.js")
    return {"html": "...", "js": lib + "\n" + MY_PANEL_JS}
```

### Minecraft and instances

```python
arbuz.instances.list()
arbuz.instances.get("instance-id")
arbuz.instances.version_jar("instance-id")
arbuz.mc.assets_dir()
arbuz.mc.cache_dir()
arbuz.mc.visual("instance-id")
```

### UI events

```python
arbuz.ui.toast("Hello")
arbuz.ui.notify("my.event", {"x": 1})
arbuz.ui.open_panel("my-page")
arbuz.ui.emit("tick", {"from": "python"})   # delivered to the open panel
```

`arbuz.ui.emit` sends an event to the panel; JavaScript receives it with
`arbuz.on("tick", handler)`.

### Registration APIs

```python
arbuz.sources.register({...})     # catalog source
arbuz.themes.register({...})      # theme
arbuz.panels.register({...})      # low-level panel registration
arbuz.commands.register({...})    # low-level command registration
arbuz.auth.register({...})        # sign-in provider
```

## JavaScript API (panel)

Every panel runs in an `iframe sandbox="allow-scripts"` with no access to the
Tauri API. `window.arbuz` is injected automatically.

```js
await arbuz.ready();                 // resolves with theme info
arbuz.on("tick", (data) => { ... }); // events from Python
```

| Method | Description |
| --- | --- |
| `arbuz.ready(fn?)` | Resolves when the SDK bridge is ready; returns theme info. |
| `arbuz.command(name, args?)` | Runs a plugin command (`command.<name>`), returns its result. |
| `arbuz.call(name, args?)` | Alias of `arbuz.command`. |
| `arbuz.storage.get(key, default?)` | Reads plugin storage. |
| `arbuz.storage.set(key, value)` | Writes plugin storage. |
| `arbuz.storage.all()` | Reads all plugin storage. |
| `arbuz.config.get(key, default?)` | Reads a launcher setting (needs `config`). |
| `arbuz.config.set(key, value)` | Writes a launcher setting (needs `config`). |
| `arbuz.toast(message)` | Shows a launcher toast. |
| `arbuz.resize(height)` | Changes the panel height (200–4000 px). |
| `arbuz.open(panelId)` | Opens another panel by id. |
| `arbuz.close()` | Closes the panel and returns to settings. |
| `arbuz.on(event, fn)` | Subscribes to events sent by `arbuz.ui.emit`. |
| `arbuz.emit(event, data)` | Sends an event to Python: `panel.event.<event>` hook. |
| `arbuz.theme` | `{ dark, accent, style, radius }`. |
| `arbuz.plugin`, `arbuz.panel` | Current plugin/panel ids. |
| `arbuz.version`, `arbuz.apiVersion` | Launcher and SDK versions. |

### Panel → Python events

```js
await arbuz.emit("save", { value: 1 });
```

```python
@arbuz.hook("panel.event.save")
def save(payload):
    return {"ok": True, "value": payload["data"]["value"]}
```

### Python → panel events

```python
arbuz.ui.emit("refresh", {"count": 3})
```

```js
arbuz.on("refresh", (data) => console.log(data.count));
```

## Permissions

The manifest controls what the plugin can do. Missing permissions produce a
clear error in the panel or the plugin log.

| Permission | Allows |
| --- | --- |
| `ui` | panels, sidebar, commands, toasts, UI hooks, panel events |
| `network` | `arbuz.http` |
| `fs` | file helpers (`read_text`, `write_text`, ...) |
| `config` | `arbuz.config` and panel `arbuz.config` |
| `instances` | instance helpers |
| `mc` | Minecraft assets/visual helpers |
| `sources` | catalog sources |
| `themes` | theme registration |
| `auth` | sign-in providers |

Storage is private to the plugin and does not expose other plugins' data.

## Debugging

- **Experimental → Plugin logs** shows the last 500 lines with level filters,
  copy and clear.
- `arbuz.log`, `arbuz.warn`, `arbuz.error` write there and to stderr.
- **Reload** re-runs `setup()` and refreshes the sidebar, panels and UI slots.
- A panel can show errors with `arbuz.toast` or by rendering text; hook errors
  are returned to the panel as an error message.

## CSP and assets

Panels are self-contained: inline HTML, CSS and JavaScript are allowed, but the
launcher CSP blocks external CDN scripts. Bundle library code into the `js`
field. Images may use `data:` URLs. Use `arbuz.command` for anything that needs
the launcher or the network; the iframe itself has no Tauri access.
