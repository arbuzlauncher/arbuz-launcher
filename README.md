# Arbuz Launcher

A Minecraft launcher for Windows, macOS and Linux. Tauri + React on the inside,
no telemetry, no accounts required for co-op.

## Download

Releases: https://github.com/arbuzlauncher/arbuz-launcher/releases

- **Windows** — `Arbuz-Launcher-<version>-windows-x64-setup.exe` (installer) or
  `...-windows-x64-portable.exe` (single file). The installer puts the launcher in
  `%USERPROFILE%\arbuz-launcher`.
- **macOS** — `...-macos-universal.dmg`, works on both Apple Silicon and Intel.
- **Linux** — `...-linux-x86_64.AppImage`, `...-linux-amd64.deb`,
  `...-linux-x86_64.rpm` or `...-linux-x86_64.tar.xz` (binary + `.desktop` file).

## Build from source

You need Node.js 22+, pnpm 12+ and Rust stable. Tauri also needs the platform
toolchain:

**Windows** — Visual Studio Build Tools with the C++ workload, WebView2
(already present on Windows 10/11) and NSIS for the installer
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
pnpm check       # TypeScript
pnpm test        # Rust tests
```

Builds:

```bash
# Windows: exe + NSIS installer
pnpm run dist

# macOS: universal dmg
pnpm tauri build --target universal-apple-darwin --bundles dmg

# Linux
pnpm tauri build --bundles deb,rpm,appimage
```


## License

GPL-3.0. See [LICENSE](LICENSE).
