# Theme engine

The theme engine changes the palette, typography, corner radii and custom CSS
without rebuilding the launcher. Themes live in `<data>/themes/` and are managed
from **Experimental → Theme engine**.

## Theme structure

```
<data>/themes/
  my-theme/
    theme.json
    theme.css      (optional)
```

Only directories with a `theme.json` are loaded. Standalone `.json` theme files
are not supported.

Minimal `theme.json`:

```json
{
  "id": "my-theme",
  "name": "My Theme",
  "author": "me",
  "description": "My theme",
  "dark": true,
  "layout": "default",
  "radius": 10,
  "font": "Segoe UI",
  "colors": {
    "bg": "#101014",
    "sbg": "#0b0b0e",
    "s1": "#17171c",
    "s2": "#202027",
    "s3": "#2a2a33",
    "on": "#f2f2f5",
    "onv": "#9a9aa5",
    "ln": "#303038",
    "pri": "#ff8800",
    "sec": "#202027",
    "ter": "#ffb066"
  },
  "css": "theme.css"
}
```

## Fields

| Field | Type | Description |
| --- | --- | --- |
| `id` | string | unique id: letters, digits, `.`, `-`, `_` |
| `name` | string | display name |
| `author` | string | author |
| `description` | string | description |
| `dark` | bool | dark or light theme |
| `layout` | string | free token: letters, digits, `-`, `_`; written to `data-theme-layout` |
| `soft` | bool | rounded, tonal surfaces (the theme editor's “soft shapes”) |
| `radius` | number | corner radius, 0–26 px |
| `font` | string | font family if installed on the system |
| `colors` | object | palette roles, all values as `#rrggbb` |
| `css` | string | CSS file name next to `theme.json` **or** inline CSS |

## Palette roles

| Role | Used for |
| --- | --- |
| `bg` | window background |
| `sbg` | sidebar and titlebar background |
| `s1` | panels and cards |
| `s2` | nested panels, hover states |
| `s3` | borders, inactive elements |
| `on` | primary text |
| `onv` | secondary text |
| `ln` | lines and separators |
| `pri` | accent color, Play button |
| `sec` | secondary surfaces |
| `ter` | tertiary accent |

`onpri`, `onpric`, `onsec`, `onterc` and `outl` are derived automatically.

## CSS

CSS is scoped to the launcher window. Use the engine CSS variables:

```css
#win[data-theme-layout="my-layout"] .panel {
  border-radius: 14px;
  border: 1px solid var(--ln);
  background: color-mix(in srgb, var(--s1) 85%, transparent);
  backdrop-filter: blur(20px);
}
#win[data-theme-layout="my-layout"] .btn.pr {
  background: linear-gradient(180deg, color-mix(in srgb, var(--pri) 90%, #fff), var(--pri));
}
```

Available variables: `--bg`, `--sbg`, `--s1`, `--s2`, `--s3`, `--on`, `--onv`,
`--ln`, `--pri`, `--onpri`, `--sec`, `--ter`, `--r`, `--rb`, `--rp`, `--sbw`,
`--fs`, `--ctl`, `--den`, `--font`.

The `data-theme-layout` attribute is set from the `layout` field. It is a free
token, not a fixed list: name it `winui`, `fluent`, `compact` or anything else
and style that name in your own CSS. The launcher does not ship a WinUI 3 theme
or any other layout — it only provides the engine and CSS variables.

### Recreating a WinUI 3 look

There is no built-in WinUI 3 style, but you can build one entirely in your
theme. Set `"layout": "winui"` in `theme.json` and put this in `theme.css`:

```css
#win[data-theme-layout="winui"] .panel,
#win[data-theme-layout="winui"] .card,
#win[data-theme-layout="winui"] .mod {
  background: color-mix(in srgb, var(--s1) 82%, transparent);
  border: 1px solid color-mix(in srgb, var(--ln) 80%, transparent);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, .16);
  backdrop-filter: blur(28px) saturate(1.15);
}
#win[data-theme-layout="winui"] .btn {
  border: 1px solid color-mix(in srgb, var(--ln) 70%, transparent);
  background: color-mix(in srgb, var(--s2) 76%, transparent);
  border-radius: 5px;
}
#win[data-theme-layout="winui"] .btn.pr {
  background: var(--pri);
  color: var(--onpri);
  border-color: color-mix(in srgb, var(--pri) 65%, #000);
}
```

The same theme can be installed as `.arbtheme` or registered by a plugin.

## Built-in themes and the theme engine

The launcher's own picker (**Settings → Theme**, shown when
**Experimental → Enable themes** is on) ships six palettes: Flat, Monet,
Catppuccin (Mocha/Latte), Dracula (Dracula/Alucard), AMOLED and a fully custom
theme with color, size and CSS controls. They are compiled into the frontend
and are not files on disk.

The theme engine described on this page is separate. It loads `.arbtheme`
packages from `<data>/themes/` and from plugins with the `themes` permission.
No `.arbtheme` packages are preinstalled: the theme list is populated only from
those two sources.

## Installation

1. Open **Experimental → Theme engine**.
2. Click **Install theme** and choose an `.arbtheme` file.
3. Click the theme card to apply it.

`.arbtheme` is a regular zip archive:

```
my-theme.arbtheme
  theme.json
  theme.css
  preview.png
```

Only `.arbtheme` is accepted for theme installation. `.arbz` is a plugin archive
format and cannot be installed as a theme.

## Engine API

Tauri commands available to the frontend:

| Command | Purpose |
| --- | --- |
| `themes_list` | list all themes |
| `theme_load(id)` | theme + CSS |
| `theme_install(path)` | install an `.arbtheme` package |
| `theme_remove(id)` | remove a user theme |
| `theme_open_folder` | open the themes folder |
| `theme_export(id)` | theme + CSS as JSON |

From Rust:

```rust
let (theme, css) = themes::load("my-theme")?;
let custom = theme.custom();       // CustomTheme for the legacy engine
```

## Theme from a plugin

A plugin with the `themes` permission can register a theme:

```python
arbuz.themes.register({
    "id": "plugin-theme",
    "name": "Plugin Theme",
    "dark": True,
    "layout": "my-layout",
    "soft": True,
    "radius": 8,
    "font": "Segoe UI Variable",
    "colors": {
        "bg": "#202020", "sbg": "#1c1c1c", "s1": "#2b2b2b",
        "s2": "#323232", "s3": "#3a3a3a", "on": "#f3f3f3",
        "onv": "#a9a9a9", "ln": "#3d3d3d", "pri": "#60cdff",
        "sec": "#2f2f2f", "ter": "#9ad8ff",
    },
})
```

The theme appears in the list after the plugin is loaded.

## Limitations

- CSS cannot escape `#win`; global application styles are unaffected.
- `theme.css` cannot be an absolute path or contain `..`.
- Plugin themes cannot be deleted.
- If a theme is broken, the launcher falls back to the default theme.
