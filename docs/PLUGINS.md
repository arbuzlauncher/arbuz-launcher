# Plugins

Plugins are ordinary Python files with the `.aplug` extension. A single file can
add a screen, a catalog source, a sign-in provider, a theme, change game launch
behavior, settings and almost any other launcher action.

> Plugins run inside the launcher process and receive only the permissions
> declared in their manifest. Install only plugins you trust.

> **ArbuzSDK:** the high-level Python decorators (`@arbuz.panel`,
> `@arbuz.command`, `arbuz.sidebar.add`) and the panel JavaScript API
> (`window.arbuz`) are documented in [SDK.md](SDK.md).

## Where plugins live

```
<launcher data>/
  plugins/
    my-plugin/
      main.aplug
      plugin.json        (optional)
      lib/               (local Python libraries)
      assets/...
    other.aplug
    pack.arbz            (archive with several .aplug files)
  libs/                  (shared user libraries)
```

The data folder is opened from **Settings → Data folder**; the plugins folder is
opened from **Experimental → Plugins folder**.

Supported formats:

| File | Meaning |
| --- | --- |
| `.aplug` | a plain Python file with a manifest |
| `.arbz` | a zip archive with one or more `.aplug` files and assets |

`.zip` is not accepted. `.arbz` is the plugin archive format; themes use
`.arbtheme` and cannot be installed from `.arbz`.

Before installation the launcher inspects the file and shows the manifest
(author, description, version), requested permissions, hooks, a SHA-256 hash,
network/obfuscation warnings, and a read-only code preview. Installation only
happens after you confirm. A newly installed plugin is enabled automatically;
installing it again clears an old "disabled" flag. The sidebar, UI slots and
panel list refresh after install, reload or uninstall without restarting the
launcher.

## Manifest

A manifest can be declared in two ways.

### 1. Header comment (recommended)

```python
# arbuz-plugin: {"id":"my.plugin","name":"My Plugin","version":"1.0.0","author":"me","description":"...","api":1,"permissions":["ui","network"],"hooks":["panel.render"]}
```

The header is read without executing code, so the plugin can be listed before it
is enabled.

### 2. `PLUGIN` variable

```python
PLUGIN = {
    "id": "my.plugin",
    "name": "My Plugin",
    "version": "1.0.0",
    "author": "me",
    "description": "Example",
    "api": 1,
    "permissions": ["ui", "network"],
    "hooks": ["panel.render"],
}
```

`PLUGIN` is read through `ast.literal_eval`; the code is not executed.

### Fields

| Field | Required | Description |
| --- | --- | --- |
| `id` | yes | unique id: letters, digits, `.`, `-`, `_` |
| `name` | yes | display name |
| `version` | no | plugin version |
| `author` | no | author |
| `description` | no | description |
| `api` | no | API version (currently `1`) |
| `permissions` | no | requested permissions |
| `hooks` | no | hooks shown in the UI |
| `entry` | no | code file inside the folder, defaults to `main.aplug` |
| `icon` | no | relative icon path |
| `homepage` | no | `https://...` link |

### Permissions

| Permission | Allows |
| --- | --- |
| `network` | `arbuz.http.get/post` |
| `fs` | read/write files inside the plugin folder |
| `instances` | list and read profiles |
| `mc` | access to the version jar, assets and Minecraft visuals |
| `ui` | toasts, notifications, panels, commands |
| `themes` | theme registration |
| `sources` | catalog source registration |
| `config` | read/write `config.json` |
| `auth` | sign-in provider registration |

Calling an API without the corresponding permission raises `RuntimeError`.

## First plugin

`hello.aplug`:

```python
# arbuz-plugin: {"id":"demo.hello","name":"Hello","version":"1.0.0","api":1,"permissions":["ui"],"hooks":["launcher.start","panel.render"]}
import arbuz

@arbuz.hook("launcher.start")
def start(payload):
    arbuz.ui.toast("Hello from the plugin!")
    arbuz.log("launcher", payload.get("version"))

@arbuz.hook("panel.render")
def panel(payload):
    if payload.get("panel") != "hello":
        return None
    return {
        "html": "<h1>Hello!</h1><button id='b'>Click</button>",
        "css": "body{font-family:Segoe UI;color:#fff;background:#202020;padding:24px}button{padding:10px 16px}",
        "js": "document.getElementById('b').onclick=()=>parent.postMessage({type:'arbuz-panel',command:'hello.click',args:{}}, '*')",
        "height": 300,
    }

@arbuz.hook("command.hello.click")
def clicked(payload):
    arbuz.ui.toast("Button clicked")
    return {"ok": True}

def setup():
    arbuz.panels.register({
        "id": "hello",
        "title": "Hello",
        "description": "Example panel",
        "icon": "star",
        "height": 300,
    })
```

Install the file from **Experimental → Install plugin**.

## API

The `arbuz` module is available to every plugin.

### Logs and events

```python
arbuz.log(*args)
arbuz.warn(*args)
arbuz.error(*args)
arbuz.ui.toast("text")
arbuz.ui.notify("my.event", {"x": 1})
arbuz.ui.set_visual({...})     # visual consumed by the frontend
```

### Hooks

```python
@arbuz.hook("game.start")
def on_game_start(payload):
    ...
```

`arbuz.on("event")` is an alias for `arbuz.hook`.

A hook may return `None` or a JSON-compatible object. Some hooks understand
special fields:

| Field | Hooks | Meaning |
| --- | --- | --- |
| `cancel: true` | hooks documented as cancellable | cancel the action |
| `message` | together with `cancel` | error text shown to the user |
| `config` | `config.before_save` | replace the saved config |
| `instance` | `instance.update.before` | replace the profile |
| `token` | `auth.token` | token used to launch the game |
| `url` | `auth.skin` | skin URL |
| `note` | `ai.dependencies.plan` | note appended to the dependency plan |

### Profiles

```python
arbuz.instances.list()            # list of profiles (JSON objects)
arbuz.instances.get(id)           # one profile
arbuz.instances.version_jar(id)   # path to the client jar
```

### Minecraft

```python
arbuz.mc.visual(instance_id)  # panorama, buttons and accent from the jar
arbuz.mc.assets_dir()
arbuz.mc.cache_dir()
```

### HTTP

```python
r = arbuz.http.get("https://example.com/api", headers={"Accept": "application/json"})
print(r.status, r.ok, r.body)
print(r.json())

r = arbuz.http.post("https://example.com/login", data='{"user":"x"}', headers={"Content-Type": "application/json"})
```

Only `https://` is allowed; local addresses are rejected. The proxy configured in
settings is used automatically.

### Files

All paths are relative to the plugin folder and cannot escape it.

```python
arbuz.read_text("data.json")
arbuz.write_text("data.json", "{}")
arbuz.read_bytes("icon.png")
arbuz.write_bytes("icon.png", data)
arbuz.list_dir("assets")
```

Cache (not removed when the plugin is updated):

```python
arbuz.cache_read_bytes("preview.png")
arbuz.cache_write_bytes("preview.png", data)
```

### Libraries

Shared user libraries live in `<data>/libs/` and are added to `sys.path`
automatically. Plugin-local modules can live in `<plugin>/lib/`. Both folders
are created on demand:

```python
arbuz.libs_dir()          # <data>/libs
arbuz.plugin_lib_dir()    # <plugin>/lib
```

```python
import my_shared_library
from my_shared_library import helper

arbuz.library("my_shared_library")          # explicit import
arbuz.reload_library("my_shared_library")   # reload while developing
```

The same modules are available to every plugin in the process, so give them
unique names. See [SDK.md](SDK.md#libraries) for a full example.

### Storage and settings

```python
arbuz.storage.get("key", default=None)
arbuz.storage.set("key", {"any": "json"})
arbuz.storage.all()

arbuz.config.get("accent")
arbuz.config.set("accent", "#ff8800")
```

`config.set` writes `config.json` and emits `config.changed`, so the UI refreshes
automatically.

### Panels (custom screens)

```python
arbuz.panels.register({
    "id": "my-page",
    "title": "My page",
    "description": "...",
    "icon": "star",
    "height": 600,
})
```

The `panel.render` hook receives:

```json
{ "plugin": "my.plugin", "panel": "my-page", "args": null }
```

and returns:

```json
{
  "html": "<div>...</div>",
  "css": "body{...}",
  "js": "console.log('hi')",
  "height": 600
}
```

HTML is rendered in an `iframe sandbox="allow-scripts"` with no access to the
Tauri API. Inline `<script>` blocks are allowed. Keep panels self-contained:
the launcher CSP blocks external CDN scripts, so bundle any library code in the
`js` field. Use `postMessage` to talk to the plugin:

```js
parent.postMessage({ type: 'arbuz-panel', command: 'my.command', args: { x: 1 } }, '*')
```

The plugin receives it through the `command.my.command` hook.

Panels can render anything from a settings page to a small game. Save the
plugin as `<name>.aplug` and install it from **Experimental → Install plugin**,
or copy it to `<data>/plugins/<name>.aplug`.

### Sidebar items

```python
arbuz.sidebar.register({
    "id": "my-page",
    "title": "My page",
    "description": "Opens the panel below",
    "icon": "modpacks",
    "panel": "my-page",
    "order": 10,
    "badge": "NEW",
})
```

The item appears in the sidebar under **Plugins**; clicking it opens the
registered panel as a full screen. `id` is unique per plugin, `panel` defaults
to `id`, and `order` sorts items (lower first). Requires the `ui` permission.

### UI hooks and slots

Plugins can add controls to fixed slots or patch existing elements:

```python
@arbuz.hook("ui.slot.bottom.actions")
def bottom(payload):
    return {"items": [{"id": "open", "label": "My panel", "panel": "my-page"}]}

@arbuz.hook("ui.element.sidebar")
def sidebar(payload):
    return {"style": {"--plugin-accent": "#ff9800"}}
```

Slots include `sidebar.extra` and `bottom.actions`. An item may contain
`command` + `args` to run a plugin command, or `panel` to open a panel. The
return value may also include `html` (trusted HTML) and `patch` (same fields as
`ui.element`).

Every element with `data-plugin-el="<id>"` is passed to `ui.element.<id>` and
the generic `ui.element` hook. The launcher tags the sidebar, title bar, bottom
bar and screen containers; a plugin can return `{"hidden": true}`,
`{"text": "..."}`, `{"html": "..."}`, `{"className": "..."}`,
`{"style": {"color": "#fff"}}`, `{"before": "..."}` or `{"after": "..."}`.
UI hooks require the `ui` permission.

### Logs

`arbuz.log(...)`, `arbuz.warn(...)` and `arbuz.error(...)` write to the plugin
log. The last 500 lines are available in **Experimental → Plugin logs**, with
level filtering, copy and clear buttons. The same entries are emitted as the
`log` plugin event.

### Sign-in providers

```python
arbuz.auth.register({
    "id": "my-auth",
    "name": "My server",
    "description": "Login with username and password",
    "hint": "Accounts are issued at mc.example.com",
    "fields": [
        {"key": "login", "label": "Login", "type": "text"},
        {"key": "password", "label": "Password", "type": "password"},
    ],
})

@arbuz.hook("auth.login")
def login(payload):
    if payload.get("provider") != "my-auth":
        return None
    c = payload.get("credentials", {})
    r = arbuz.http.post("https://mc.example.com/api/login",
                        data=json.dumps({"login": c.get("login"), "password": c.get("password")}),
                        headers={"Content-Type": "application/json"})
    if not r.ok:
        return {"error": "Invalid username or password"}
    data = r.json()
    return {
        "name": data["username"],
        "uuid": data.get("uuid"),
        "token": data["accessToken"],
        "note": "mc.example.com",
    }

@arbuz.hook("auth.token")
def token(payload):
    if payload.get("provider") == "my-auth":
        return {"token": arbuz.storage.get("token")}
    return None

@arbuz.hook("auth.skin")
def skin(payload):
    if payload.get("provider") == "my-auth":
        return {"url": "https://mc.example.com/skin/" + payload["name"] + ".png"}
    return None
```

`auth.login` must return `name` and `token`; `uuid` is optional (an offline UUID
is generated when omitted). The token is stored in the system keyring, not in
`config.json`.

### Catalog sources

```python
arbuz.sources.register({
    "id": "my-repo",
    "name": "My Repo",
    "kind": "json",
    "description": "Internal catalog",
})

@arbuz.hook("source.search")
def search(payload):
    if payload.get("source") != "my-repo":
        return None
    a = payload.get("args", {})
    r = arbuz.http.get("https://repo.example.com/search?q=" + a.get("query", ""))
    return r.json()["items"]   # list of CatalogItem objects
```

Each item must contain `id`, `title`, `author`, `description`, `downloads`,
`loaders`, `categories`, `iconUrl`, `source`, `kind` and `installed`.

### Plugin themes

```python
arbuz.themes.register({
    "id": "my-theme",
    "name": "My Theme",
    "dark": True,
    "layout": "default",
    "radius": 10,
    "font": "Segoe UI",
    "colors": {
        "bg": "#101010", "sbg": "#0c0c0c", "s1": "#181818", "s2": "#202020",
        "s3": "#282828", "on": "#f0f0f0", "onv": "#909090", "ln": "#303030",
        "pri": "#ff8800", "sec": "#202020", "ter": "#ffb066",
    },
})
```

The theme appears in **Experimental → Theme engine** immediately.

### Commands

```python
arbuz.commands.register({
    "id": "my-command",
    "title": "Do something",
    "description": "...",
})

@arbuz.hook("command.my-command")
def run(payload):
    ...
```

The frontend can invoke a command through `plugin_run_command(plugin, command, args)`.

## Hook catalog

Hooks are called in plugin load order. Return `{"cancel": true}` from any
cancellable hook to stop the action.

### Lifecycle

| Hook | When | Payload |
| --- | --- | --- |
| `launcher.start` | launcher started | `version`, `dataDir` |
| `launcher.ready` | UI is ready | — |
| `launcher.exit` | launcher is exiting | — |

### Configuration

| Hook | When | Payload |
| --- | --- | --- |
| `config.load` | config loaded | `config` |
| `config.before_save` | before writing settings | full config |
| `config.after_save` | after writing | full config |
| `config.changed` | config changed by a plugin | config |
| `config.export.before/after` | settings export | `path` |
| `config.import.before/after` | settings import | `path`, config |

### Profiles and instances

| Hook | When | Payload |
| --- | --- | --- |
| `instance.list` | profile list requested | `count`, `instances` |
| `instance.create.before` | before profile creation | `id`, `name`, `mcVersion`, `loader` |
| `instance.create.after` | after creation | `instance` |
| `instance.update.before` | before saving a profile | profile |
| `instance.update.after` | after saving | profile |
| `instance.delete.before` | before deletion | `id` |
| `instance.delete.after` | after deletion | `id` |
| `instance.game_dir.before` | before changing the game directory | `id`, `path` |
| `instance.game_dir.after` | after changing | `id`, `instance` |
| `instance.icon.after` | profile icon changed | `id`, `instance` |
| `instance.folder.open` | game folder opened | `id`, `path` |
| `instance.launch.before` | before game launch | `instanceId`, `name`, `mcVersion`, `loader` |
| `instance.launch.request` | launch command received | `id`, `world`, `server`, `scanOverride` |
| `instance.launch.after` | launch command completed | `id` |
| `instance.import.before/after` | pack, Prism or Modrinth import | `kind`, `path` |
| `instance.import.modrinth.list` | Modrinth profiles listed | `count` |

### Game process

| Hook | When | Payload |
| --- | --- | --- |
| `game.start` | game process started | `instanceId`, `name`, `mcVersion` |
| `game.exit` | game finished | `instanceId`, `code` |
| `game.exit.after` | after exit handling | `instanceId`, `code` |
| `game.crash` | non-zero exit code | `instanceId`, `code` |
| `game.stop.before` | before stopping the game | `id` |
| `game.stop.after` | after stopping | `id` |
| `game.log.read` | game log read | `id`, `lines` |
| `game.logs.open` | logs folder opened | `id`, `path` |
| `game.lan_port` | LAN port detected | `id`, `port` |

### Content and catalog

| Hook | When | Payload |
| --- | --- | --- |
| `content.list` | content listed | `id`, `count` |
| `content.toggle.before/after` | content enabled or disabled | `id`, `file`, `enabled` |
| `content.remove.before/after` | content removed | `id`, `file` |
| `content.isolation.before/after` | isolation toggled | `id`, `file`, `isolated` |
| `mod.download.before` | before catalog install | `instanceId`, `source`, `projectId`, `kind` |
| `mod.download.after` | after catalog install | same |
| `catalog.project.before/after` | project details requested | `source`, `id` |
| `catalog.search.before/after` | catalog search | search arguments / results |
| `catalog.install.before/after` | catalog item installed | `instanceId`, `source`, `projectId`, `kind` |
| `catalog.updates.check.before/after` | update check | `id` / `updates` |
| `catalog.updates.apply.before/after` | updates applied | `id`, `files` |
| `catalog.modpack.install.before/after` | modpack installed | `source`, `projectId` |
| `catalog.modpack.versions.before/after` | modpack versions listed | `source`, `projectId` |
| `world.list` | worlds listed | `id`, `count` |
| `screenshot.list` | screenshots listed | `id`, `count` |
| `screenshot.folder.open` | screenshots folder opened | `id`, `path` |
| `screenshot.open` | one screenshot opened | `id`, `file` |

### Pack editor

| Hook | When | Payload |
| --- | --- | --- |
| `pack.info.before/after` | pack manifest read | `id` / `info` |
| `pack.info.save.before/after` | pack manifest saved | `id`, `manifest` |
| `pack.configs.list` | config list | `id`, `count` |
| `pack.configs.open` | configs folder opened | `id`, `path` |
| `pack.config.read.before/after` | config read | `id`, `path` / `doc` |
| `pack.config.save.before/after` | config saved | `id`, `path`, `content` |
| `pack.config.restore.before/after` | config restored from backup | `id`, `path` |
| `pack.config.history.before/after` | config history listed | `id`, `path`, `count` |
| `pack.validate.before/after` | pack validation | `id`, `issues` |
| `pack.export.before/after` | pack export | `id`, `dest`, `format`, `result` |
| `pack.update.check` | pack update check | `id`, `update` |
| `pack.update.apply.before/after` | pack update applied | `id`, `versionId` |

### Mods, logs and scanning

| Hook | When | Payload |
| --- | --- | --- |
| `mods.conflicts.before/after` | conflict analysis | `id`, `conflicts` |
| `mods.lists.list` | saved mod lists requested | `count` |
| `mods.lists.save.before/after` | current content saved as a list | `id`, `name` / `list` |
| `mods.lists.delete.before/after` | saved list deleted | `id` |
| `mods.lists.install.before/after` | saved list installed | `listId`, `instanceId`, `count` |
| `logs.analyze.before/after` | log analysis | `id`, `analysis` |
| `logs.journal` | run journal requested | `id`, `count` |
| `logs.report.before/after` | support report generated | `id`, `bytes` |
| `logs.report.save.before/after` | support report saved | `id`, `path` |
| `logs.rules.update.before/after` | analysis rules updated | `url`, `count` |
| `logs.fix.before/after` | automatic fix applied | `id`, `kind` |
| `scanner.scan.before/after` | mod scan | `id`, `reports` |
| `scanner.allow.before/after` | file allow-listed | `id`, `file`, `allow` |
| `scanner.quarantine.list` | quarantine listed | `id`, `files` |
| `scanner.quarantine.restore.before/after` | file restored from quarantine | `id`, `file` |
| `scanner.quarantine.delete.before/after` | quarantined file deleted | `id`, `file` |

### Servers, benchmark and news

| Hook | When | Payload |
| --- | --- | --- |
| `servers.fetch.before/after` | server catalog fetched | `url`, `refresh` / `count` |
| `servers.ping.before/after` | server ping | `ip`, `ping` |
| `servers.dat.list` | servers.dat listed | `id`, `count` |
| `servers.dat.add.before/after` | server added to servers.dat | `id`, `name`, `ip` |
| `servers.pinned.list` | pinned servers listed | `id`, `count` |
| `servers.pin.before/after` | server pinned or unpinned | `id`, `name`, `ip`, `pin` |
| `benchmark.start.before/after` | benchmark started | `id`, `world` |
| `benchmark.cancel` | benchmark cancelled | — |
| `benchmark.list` | benchmark results listed | `id`, `count` |
| `news.fetch.before/after` | news fetched | `count` |

### Java and Minecraft versions

| Hook | When | Payload |
| --- | --- | --- |
| `java.detect.before/after` | Java installations detected | `count` |
| `java.install.before/after` | Java installed | `major`, `path` |
| `minecraft.versions.list` | game versions listed | `count` |
| `minecraft.version.add.before/after` | custom version added | `id`, `base` |
| `minecraft.version.remove.before/after` | custom version removed | `id` |
| `minecraft.loader.versions` | loader versions listed | `loader`, `mcVersion`, `count` |

### Authentication

| Hook | When | Payload |
| --- | --- | --- |
| `auth.accounts.list` | accounts listed | `count` |
| `auth.account.add.before/after` | offline account added | `name`, `account` |
| `auth.account.switch.before/after` | active account changed | `id` |
| `auth.account.remove.before/after` | account removed | `id` |
| `auth.login.before/after` | password login | `provider`, `login` / `account` |
| `auth.login` | plugin sign-in provider | `provider`, `credentials` |
| `auth.token` | launch token requested | `accountId`, `provider`, `name` |
| `auth.skin.before/after` | skin requested | `id`, `skin` |
| `auth.microsoft.begin` | Microsoft device flow started | — |
| `auth.microsoft.begin.after` | device code received | `code` |
| `auth.microsoft.poll` | device flow polling | `interval` |
| `auth.microsoft.cancel` | device flow cancelled | — |

### AI, UI, themes and plugins

| Hook | When | Payload |
| --- | --- | --- |
| `ai.dependencies.before` | dependency plan started | `instanceId` |
| `ai.dependencies.plan` | plugin additions to the plan | `instanceId`, `updates`, `issues` |
| `ai.dependencies.after` | dependency plan finished | `instanceId`, `updates`, `issues`, `note` |
| `ai.diagnose.before/after` | AI crash diagnosis | `id`, `diagnosis` |
| `ai.pack.plan.before/after` | AI pack plan | `prompt`, `plan` |
| `ai.models` | model list requested | `count` |
| `ai.test.before/after` | AI connection test | `model`, `reply` |
| `ai.scan.before/after` | AI note for a scanned mod | `id`, `file`, `note` |
| `visual.before/after` | Minecraft visual requested | `instanceId`, `visual` |
| `visual.instance` | plugin supplies Minecraft visual | `instanceId` |
| `ui.panel.render.before/after` | plugin panel rendered | `plugin`, `panel`, `args` |
| `ui.command.before/after` | plugin command executed | `plugin`, `command`, `args` |
| `ui.wallpaper.before/after` | wallpaper changed | `path`, `name` |
| `ui.wallpaper.clear.before/after` | wallpaper cleared | — |
| `theme.list` | themes listed | `count` |
| `theme.load.before/after` | theme loaded | `id`, `theme` |
| `theme.install.before/after` | theme installed | `path`, `id` |
| `theme.remove.before/after` | theme removed | `id` |
| `theme.folder.open` | themes folder opened | `path` |
| `theme.export.before/after` | theme exported | `id`, `theme` |
| `plugin.list` | plugins listed | `count` |
| `plugin.install.before/after` | plugin installed | `path`, `plugins` |
| `plugin.uninstall.before/after` | plugin removed | `id` |
| `plugin.toggle.before/after` | plugin enabled or disabled | `id`, `enabled` |
| `plugin.reload.before/after` | plugins reloaded | — |
| `plugin.folder.open` | plugins folder opened | `path` |
| `plugin.libs.open` | shared libraries folder opened | `path` |
| `plugin.panels.list` | panels listed | `count` |
| `plugin.sources.list` | catalog sources listed | `count` |
| `plugin.commands.list` | commands listed | `count` |
| `plugin.auth.list` | auth providers listed | `count` |
| `source.search` | plugin catalog source search | `source`, `args` |
| `panel.render` | panel render | `plugin`, `panel`, `args` |
| `command.<id>` | plugin command | `plugin`, `command`, `args` |

### Networking and updates

| Hook | When | Payload |
| --- | --- | --- |
| `net.share.before/after` | UPnP port mapping | `port`, `share` |
| `net.unshare.before/after` | port mapping closed | — |
| `p2p.host.before/after` | P2P host started | `port`, `name`, `p2p` |
| `p2p.room.before/after` | P2P room created | `name`, `p2p` |
| `p2p.join.before/after` | P2P join | `code`, `name`, `p2p` |
| `p2p.stop.before/after` | P2P stopped | — |
| `p2p.kick.before/after` | player kicked | `slot` |
| `p2p.admin.before/after` | ownership transferred | `slot` |
| `p2p.new_code.before/after` | P2P room code regenerated | `p2p` |
| `server.pack.build.before/after` | server pack built | `instanceId`, `kind`, `mcVersion`, `dest`, `result` |
| `launcher.update.check.before/after` | launcher update check | `update` |
| `launcher.update.download.before/after` | update downloaded | `path` |
| `launcher.update.install.before/after` | update installed | `path` |

## `.arbz` archive

```
my-pack.arbz
  plugin.json          (shared manifest, optional)
  main.aplug
  assets/logo.png
```

An archive may contain several `.aplug` files; each becomes a separate plugin.
Assets are placed next to the `.aplug` files; plugin paths are relative to the
plugin folder.

## Security

- Plugins are trusted code. There is no Python sandbox: a plugin with `fs` can
  write inside its folder, and one with `network` can access the internet.
- Sign-in tokens are stored in the system keyring (Windows Credential Manager,
  macOS Keychain, libsecret), not in `config.json`.
- Panel HTML is isolated in `iframe sandbox="allow-scripts"`.
- Before installation the manifest, permissions, network/obfuscation warnings
  and a read-only code preview are shown; after installation a plugin can be
  disabled without removing it.

## Debugging

- Plugin logs are available in **Experimental → Plugin logs** (level filter,
  copy, clear) and are also emitted as `plugin-event` (`event: "log"`).
- Load errors are shown on the plugin card.
- **Reload** restarts all plugins without restarting the launcher.
- If the Python engine is missing, plugins are listed but not loaded; the error
  is shown on the card.

## Building with Python

PyO3 is linked against Python 3.12. For a Windows build:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-python.ps1
pnpm tauri build --no-bundle
pnpm installer
```

`fetch-python.ps1` downloads embeddable Python and places `python3.dll`,
`python312.dll` and `python312.zip` into `src-tauri/python-runtime`. The installer
copies them next to `arbuz-launcher.exe`, and the launcher sets `PYTHONHOME`.

On Linux and macOS the system Python is used (development headers are required
to compile).
