# Arbuz Launcher

A Minecraft launcher for Windows, macOS and Linux built with Tauri + React.
No telemetry, no account required for offline play, and no forced mod updates.

## Features

- **Profiles** for Vanilla, Fabric, Quilt, Forge and NeoForge with per-profile
  Java, memory, JVM arguments, window size and isolated game directories.
- **Content catalog** for Modrinth and CurseForge: mods, resource packs,
  shaders and datapacks, with dependency hints and update checks.
- **Mod conflict analyzer** — duplicate mods, wrong loader, missing or disabled
  dependencies and known incompatible pairs, with one-click fixes.
- **Log and crash analyzer** — updatable rule set, run journal, automatic fixes
  (memory, Java, loader) and a redacted support report.
- **Saved mod lists** — snapshot the catalog content of a profile and install it
  into another profile in one click.
- **Server pack builder** — Vanilla, Paper, Purpur, Fabric, Quilt, Forge and
  NeoForge, with mods, configs and start scripts packed into a zip.
- **Config editor** — syntax checks for JSON/TOML/properties, version history,
  diff and rollback.
- **Python plugins and libraries** — `.aplug` files and `.arbz` archives with
  hooks for almost every launcher operation, custom panels, catalog sources,
  auth providers and themes. Write shared modules in `<data>/libs/` or local
  modules in `<plugin>/lib/` and import them normally. **ArbuzSDK** adds
  `@arbuz.panel` / `@arbuz.command` decorators and a `window.arbuz` bridge for
  panel HTML/CSS/JS — see [docs/SDK.md](docs/SDK.md).
- **Themes** — built-in Flat, Monet, Catppuccin (Mocha/Latte), Dracula
  (Dracula/Alucard) and AMOLED palettes plus a fully custom theme with color,
  size and CSS controls. The separate theme engine loads `.arbtheme` packages;
  no `.arbtheme` files are preinstalled.
- **AI assistant** — free OpenCode Zen by default; OpenAI, OpenRouter, Groq,
  DeepSeek, Gemini, Anthropic and custom OpenAI-compatible servers are supported.
- **Co-op** — LAN sharing, UPnP port mapping and P2P tunnels.
- **Safety** — Fractureiser guard, optional mod scanner and sandbox modes,
  including an optional clipboard block for isolated game processes.
- **Self-update** for the launcher, with a portable mode.

## Download

Releases: https://github.com/arbuzlauncher/arbuz-launcher/releases

- **Windows** — `Arbuz-Launcher-<version>-windows-x64-setup.exe` (installer) or
  `...-windows-x64-portable.exe` (single file). The per-user installer defaults to
  `%USERPROFILE%\arbuz-launcher`; all-users installation requests elevation only
  when needed.
- **macOS** — `...-macos-universal.dmg`, works on Apple Silicon and Intel.
- **Linux** — `...-linux-x86_64.AppImage`, `...-linux-amd64.deb`,
  `...-linux-x86_64.rpm` or `...-linux-x86_64.tar.xz` (binary + `.desktop` file).

Launcher data (profiles, plugins, themes, configs) is stored in the platform data
directory by default. The portable build keeps everything next to the executable.

## Build from source

Requirements: Node.js 22+, pnpm 12+ and Rust stable. Tauri also needs the
platform toolchain:

**Windows** — Visual Studio Build Tools with the C++ workload, WebView2
(preinstalled on Windows 10/11) and NSIS for the installer
(`choco install nsis`, or let Tauri download it on the first build).

**macOS**

```bash
xcode-select --install
```

**Linux (Ubuntu/Debian)**

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libdbus-1-dev libssl-dev patchelf build-essential
```

Then:

```bash
pnpm install --frozen-lockfile
pnpm tauri dev   # run with hot reload
pnpm check       # TypeScript type check
pnpm build       # frontend production build
pnpm test        # Rust tests
```

Builds:

```bash
# Windows: exe + NSIS installer
pnpm run dist

# macOS: universal dmg
pnpm tauri build --target universal-apple-darwin --bundles dmg

# Linux: AppImage, deb and rpm
pnpm tauri build --bundles deb,rpm,appimage
```

### Python runtime for plugins

Plugins use PyO3. On Windows, stage an embeddable Python runtime before building:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-python.ps1
pnpm tauri build --no-bundle
pnpm installer
```

The script downloads embeddable Python and places `python3.dll`, `python312.dll`
and `python312.zip` into `src-tauri/python-runtime`. The installer copies them
next to `arbuz-launcher.exe`; the launcher sets `PYTHONHOME` automatically.
On Linux and macOS the system Python is used (development headers are required
to compile).

## Documentation

- [Plugins](docs/PLUGINS.md)
- [Themes](docs/THEMES.md)
- [AI assistant](docs/AI.md)

## License

GPL-3.0. See [LICENSE](LICENSE).
