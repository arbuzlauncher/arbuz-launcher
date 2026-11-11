//! Установка `.aplug` и архивов `.arbz` (обычный zip с несколькими плагинами).

use crate::error::{err, Res};
use crate::paths;
use crate::plugins::manifest::{self, Manifest};
use crate::plugins::manager::{self, PluginInfo};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 5000;
const MAX_PREVIEW_FILE: usize = 140_000;
const MAX_PREVIEW_TOTAL: usize = 420_000;

/// Файл внутри плагина для предпросмотра кода.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    pub name: String,
    pub size: u64,
    pub binary: bool,
    pub code: Option<String>,
}

/// Предупреждение о возможностях/рисках плагина до установки.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWarning {
    pub level: String,
    pub code: String,
    pub message: String,
}

/// Полное превью `.aplug`/`.arbz` до установки: манифест, права, код и риски.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPreview {
    pub path: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub kind: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub api: u32,
    pub permissions: Vec<String>,
    pub hooks: Vec<String>,
    pub files: Vec<PreviewFile>,
    pub warnings: Vec<PreviewWarning>,
    pub network: bool,
    pub obfuscated: bool,
    pub dangerous: bool,
    pub risk: String,
}

fn valid_rel(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.is_empty()
        || name.starts_with('/')
        || name.contains(':')
        || name.split('/').any(|p| p == ".." || p.is_empty())
    {
        return None;
    }
    Some(PathBuf::from(name))
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && !id.starts_with('.')
}

/// Ставит один `.aplug`: создаёт папку плагина и кладёт туда `main.aplug`.
pub fn install_aplug(path: &Path) -> Res<Vec<String>> {
    let source = std::fs::read_to_string(path)?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
    let m = manifest::from_source(&source, stem);
    if !safe_id(&m.id) {
        return err("Некорректный id плагина");
    }
    let dir = paths::plugins_dir().join(&m.id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("main.aplug"), &source)?;
    let manifest_path = dir.join("plugin.json");
    std::fs::write(manifest_path, serde_json::to_string_pretty(&m)?)?;
    Ok(vec![m.id])
}

/// Распаковывает `.arbz` и возвращает id найденных плагинов.
pub fn install_archive(path: &Path) -> Res<Vec<String>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_ARCHIVE_BYTES {
        return err("Архив слишком большой (максимум 256 МБ)");
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin-pack");
    let dest = paths::plugins_dir().join(format!(".arbz-{}", stem));
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&dest)?;

    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| crate::error::AppError::Msg(format!("Не удалось открыть архив: {e}")))?;
    if zip.len() > MAX_FILES {
        return err("В архиве слишком много файлов");
    }
    let mut ids = Vec::new();
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| crate::error::AppError::Msg(format!("Ошибка архива: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = valid_rel(entry.name()) else {
            return err("В архиве небезопасный путь");
        };
        let out = dest.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        entry
            .take(64 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| crate::error::AppError::Msg(format!("Ошибка чтения архива: {e}")))?;
        let mut f = std::fs::File::create(&out)?;
        f.write_all(&buf)?;
        if out.extension().and_then(|x| x.to_str()) == Some("aplug") {
            let source = String::from_utf8_lossy(&buf).to_string();
            let fallback = out.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
            let m = manifest::from_source(&source, fallback);
            if safe_id(&m.id) && !ids.contains(&m.id) {
                ids.push(m.id);
            }
        }
    }
    if ids.is_empty() {
        return err("В архиве нет ни одного файла .aplug");
    }
    // Если внутри один плагин и есть plugin.json, переносим папку под его id.
    if ids.len() == 1 {
        let id = ids[0].clone();
        let final_dir = paths::plugins_dir().join(&id);
        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir)?;
        }
        if std::fs::rename(&dest, &final_dir).is_err() {
            // Между дисками rename может не сработать — копируем.
            std::fs::create_dir_all(&final_dir)?;
            copy_dir(&dest, &final_dir)?;
            std::fs::remove_dir_all(&dest)?;
        }
    }
    Ok(ids)
}

fn copy_dir(from: &Path, to: &Path) -> Res<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Ставит `.aplug` или `.arbz` и возвращает обновлённый список плагинов.
pub fn install(path: &Path) -> Res<Vec<PluginInfo>> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ids = match ext.as_str() {
        "aplug" => install_aplug(path)?,
        "arbz" => install_archive(path)?,
        _ => return err("Поддерживаются только .aplug и .arbz"),
    };
    // Свежеустановленный плагин включаем: в `.disabled.json` мог остаться
    // старый id от предыдущей версии или от удалённого тестового плагина.
    for id in &ids {
        let _ = manager::set_plugin_enabled(id, true);
    }
    manager::reload(manager::global_enabled());
    let all = manager::scan_installed();
    Ok(all.into_iter().filter(|p| ids.contains(&p.id)).collect())
}

/// Удаляет плагин. Если в папке лежит несколько `.aplug`, удаляется только
/// файл этого плагина, чтобы не снести соседей.
pub fn uninstall(id: &str) -> Res<()> {
    if !safe_id(id) {
        return err("Некорректный id плагина");
    }
    let info = manager::scan_installed().into_iter().find(|p| p.id == id);
    let Some(info) = info else {
        return err("Плагин не найден");
    };
    let dir = PathBuf::from(&info.path);
    let plugins_root = paths::plugins_dir();
    let canon_root = plugins_root.canonicalize().unwrap_or(plugins_root);
    let canon_dir = dir.canonicalize().unwrap_or(dir.clone());
    if !canon_dir.starts_with(&canon_root) {
        return err("Плагин находится вне папки плагинов");
    }
    let aplugs: Vec<PathBuf> = std::fs::read_dir(&canon_dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("aplug"))
                .collect()
        })
        .unwrap_or_default();
    if aplugs.len() <= 1 {
        std::fs::remove_dir_all(&canon_dir)?;
    } else {
        // Ищем файл с нужным id по манифесту.
        for p in aplugs {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
            let m = manifest::discover(&p, stem);
            if m.id == id {
                let _ = std::fs::remove_file(&p);
                break;
            }
        }
    }
    manager::reload(manager::global_enabled());
    Ok(())
}

/// Устанавливает тему из `.arbtheme` (папка `themes/<id>`).
pub fn install_theme(path: &Path) -> Res<String> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "arbtheme" {
        let file = std::fs::File::open(path)?;
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|e| crate::error::AppError::Msg(format!("Не удалось открыть тему: {e}")))?;
        let mut id = String::new();
        let mut entries = Vec::new();
        for i in 0..zip.len() {
            let e = zip
                .by_index(i)
                .map_err(|e| crate::error::AppError::Msg(format!("Ошибка темы: {e}")))?;
            if e.is_dir() {
                continue;
            }
            let Some(rel) = valid_rel(e.name()) else {
                return err("В теме небезопасный путь");
            };
            let mut buf = Vec::new();
            e.take(16 * 1024 * 1024).read_to_end(&mut buf)?;
            if rel == Path::new("theme.json") {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) {
                    id = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                }
            }
            entries.push((rel, buf));
        }
        if !safe_id(&id) {
            return err("В архиве темы нет theme.json с id");
        }
        let dir = paths::themes_dir().join(&id);
        std::fs::create_dir_all(&dir)?;
        for (rel, buf) in entries {
            let out = dir.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(out, buf)?;
        }
        return Ok(id);
    }
    err("Поддерживаются только .arbtheme")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

static B64_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"['"][A-Za-z0-9+/]{120,}={0,2}['"]"#).expect("base64 regex"));
static HEX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:\\x[0-9a-fA-F]{2}){8,}").expect("hex regex"));

fn push_warning(warnings: &mut Vec<PreviewWarning>, level: &str, code: &str, message: &str) {
    if warnings.iter().any(|w| w.code == code) {
        return;
    }
    warnings.push(PreviewWarning {
        level: level.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    });
}

/// Эвристики по исходнику: сеть, обфускация, опасные вызовы.
fn analyze_source(src: &str, warnings: &mut Vec<PreviewWarning>) -> (bool, bool, bool) {
    let lower = src.to_ascii_lowercase();
    let network = [
        "arbuz.http", "http.get", "http.post", "requests", "urllib", "socket", "aiohttp",
        "httpx", "websocket", "smtplib", "ftplib", "paramiko", "telnetlib",
    ]
    .iter()
    .any(|p| lower.contains(p));
    let dangerous = [
        "subprocess",
        "os.system",
        "os.popen",
        "ctypes",
        "winreg",
        "powershell",
        "cmd.exe",
        "shutil.rmtree",
    ]
    .iter()
    .any(|p| lower.contains(p));
    let dynamic = [
        "exec(",
        "eval(",
        "compile(",
        "marshal.loads",
        "pickle.loads",
        "zlib.decompress",
        "codecs.decode",
        "bytes.fromhex",
    ]
    .iter()
    .any(|p| lower.contains(p));
    let base64 = B64_RE.is_match(src)
        || lower.contains("b64decode")
        || lower.contains("b64encode")
        || lower.contains("base64.");
    let hexesc = HEX_RE.is_match(src);
    let long_lines = src.lines().any(|l| l.len() > 2000);
    let high_entropy = src.len() > 4000 && entropy(src.as_bytes()) > 6.0;
    let obfuscated = dynamic || base64 || hexesc || long_lines || high_entropy;

    if network {
        push_warning(
            warnings,
            "warn",
            "network",
            "Код обращается к внешним серверам.",
        );
    }
    if dangerous {
        push_warning(
            warnings,
            "danger",
            "dangerous",
            "Код может запускать программы или обращаться к системным API.",
        );
    }
    if dynamic {
        push_warning(
            warnings,
            "danger",
            "exec",
            "Найден динамический код: exec/eval/compile.",
        );
    }
    if base64 {
        push_warning(
            warnings,
            "warn",
            "base64",
            "Найден длинный base64-блок — код может быть скрыт.",
        );
    }
    if hexesc {
        push_warning(
            warnings,
            "warn",
            "hex",
            "Найдены длинные hex-последовательности — возможна обфускация.",
        );
    }
    if long_lines {
        push_warning(
            warnings,
            "warn",
            "long_lines",
            "Очень длинные строки — код может быть минифицирован.",
        );
    }
    if high_entropy {
        push_warning(
            warnings,
            "warn",
            "entropy",
            "Высокая энтропия кода — возможна обфускация.",
        );
    }
    (network, obfuscated, dangerous)
}

fn is_text_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".aplug", ".py", ".json", ".txt", ".md", ".css", ".js", ".ts", ".toml", ".yml", ".yaml",
        ".ini", ".cfg",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn truncate_code(src: &str, max: usize) -> String {
    if src.len() <= max {
        return src.to_string();
    }
    let mut out: String = src.chars().take(max).collect();
    out.push_str("\n\n# … (код обрезан для предпросмотра)");
    out
}

#[allow(clippy::too_many_arguments)]
fn build_preview(
    path: &Path,
    bytes: &[u8],
    kind: &str,
    mut id: String,
    mut name: String,
    mut version: String,
    author: String,
    description: String,
    api: u32,
    mut permissions: Vec<String>,
    mut hooks: Vec<String>,
    files: Vec<PreviewFile>,
    mut warnings: Vec<PreviewWarning>,
    mut network: bool,
    obfuscated: bool,
    dangerous: bool,
) -> PluginPreview {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
    if id.is_empty() {
        id = stem.to_string();
    }
    if name.is_empty() {
        name = id.clone();
    }
    if version.is_empty() {
        version = "0.1.0".into();
    }
    permissions.sort();
    permissions.dedup();
    hooks.sort();
    hooks.dedup();
    if permissions.iter().any(|p| p == "network") {
        network = true;
        push_warning(
            &mut warnings,
            "warn",
            "network_permission",
            "Запрошено право network — плагин может ходить в сеть.",
        );
    }
    if permissions.iter().any(|p| p == "fs") {
        push_warning(
            &mut warnings,
            "info",
            "fs_permission",
            "Запрошено право fs — плагин может читать и писать файлы.",
        );
    }
    if !permissions.is_empty() {
        push_warning(
            &mut warnings,
            "info",
            "permissions",
            &format!("Плагин запрашивает права: {}.", permissions.join(", ")),
        );
    }
    if !hooks.is_empty() {
        push_warning(
            &mut warnings,
            "info",
            "hooks",
            &format!("Плагин подписывается на хуки: {}.", hooks.join(", ")),
        );
    }
    if author.trim().is_empty() {
        push_warning(
            &mut warnings,
            "warn",
            "author_missing",
            "Автор плагина не указан.",
        );
    }
    if api > 1 {
        push_warning(
            &mut warnings,
            "info",
            "api_version",
            &format!("Плагин написан для API {api}, лаунчер поддерживает API 1."),
        );
    }
    if files.iter().any(|f| f.binary) {
        push_warning(
            &mut warnings,
            "warn",
            "binary_files",
            "В плагине есть бинарные файлы — их содержимое не показано.",
        );
    }
    let risk = if dangerous || obfuscated {
        "high"
    } else if network || permissions.iter().any(|p| p == "fs") {
        "medium"
    } else {
        "low"
    };
    PluginPreview {
        path: path.to_string_lossy().to_string(),
        file_name: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin")
            .to_string(),
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        kind: kind.to_string(),
        id,
        name,
        version,
        author,
        description,
        api,
        permissions,
        hooks,
        files,
        warnings,
        network,
        obfuscated,
        dangerous,
        risk: risk.to_string(),
    }
}

/// Превью `.aplug`: манифест, права, исходник и эвристики.
pub fn inspect_aplug(path: &Path) -> Res<PluginPreview> {
    let bytes = std::fs::read(path)?;
    let source = String::from_utf8_lossy(&bytes).to_string();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin");
    let m = manifest::from_source(&source, stem);
    let mut warnings = Vec::new();
    let (network, obfuscated, dangerous) = analyze_source(&source, &mut warnings);
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("main.aplug")
        .to_string();
    let files = vec![PreviewFile {
        name: file_name,
        size: bytes.len() as u64,
        binary: false,
        code: Some(truncate_code(&source, MAX_PREVIEW_FILE)),
    }];
    Ok(build_preview(
        path,
        &bytes,
        "aplug",
        m.id,
        m.name,
        m.version,
        m.author,
        m.description,
        m.api,
        m.permissions,
        m.hooks,
        files,
        warnings,
        network,
        obfuscated,
        dangerous,
    ))
}

/// Превью `.arbz`: все `.aplug`, манифесты и текстовые файлы.
pub fn inspect_archive(path: &Path) -> Res<PluginPreview> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_ARCHIVE_BYTES {
        return err("Архив слишком большой (максимум 256 МБ)");
    }
    let bytes = std::fs::read(path)?;
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| crate::error::AppError::Msg(format!("Не удалось открыть архив: {e}")))?;
    if zip.len() > MAX_FILES {
        return err("В архиве слишком много файлов");
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin-pack");
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut permissions = Vec::new();
    let mut hooks = Vec::new();
    let mut id = String::new();
    let mut name = String::new();
    let mut version = String::new();
    let mut author = String::new();
    let mut description = String::new();
    let mut api = 1u32;
    let mut network = false;
    let mut obfuscated = false;
    let mut dangerous = false;
    let mut total_code = 0usize;
    let mut aplugs = 0usize;

    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| crate::error::AppError::Msg(format!("Ошибка архива: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_string();
        let size = entry.size();
        let mut buf = Vec::new();
        entry
            .take(2 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| crate::error::AppError::Msg(format!("Ошибка чтения архива: {e}")))?;
        let text = is_text_name(&entry_name);
        if text {
            let src = String::from_utf8_lossy(&buf).to_string();
            let (n, o, d) = analyze_source(&src, &mut warnings);
            network |= n;
            obfuscated |= o;
            dangerous |= d;
            if entry_name.ends_with(".aplug") {
                aplugs += 1;
                let m = manifest::from_source(&src, stem);
                if id.is_empty() {
                    id = m.id;
                }
                if name.is_empty() {
                    name = m.name;
                }
                if version.is_empty() {
                    version = m.version;
                }
                if author.is_empty() {
                    author = m.author;
                }
                if description.is_empty() {
                    description = m.description;
                }
                api = api.max(m.api);
                permissions.extend(m.permissions);
                hooks.extend(m.hooks);
            } else if entry_name.ends_with("plugin.json") {
                if let Ok(m) = serde_json::from_slice::<Manifest>(&buf) {
                    if id.is_empty() {
                        id = m.id;
                    }
                    if name.is_empty() {
                        name = m.name;
                    }
                    if version.is_empty() {
                        version = m.version;
                    }
                    if author.is_empty() {
                        author = m.author;
                    }
                    if description.is_empty() {
                        description = m.description;
                    }
                    api = api.max(m.api);
                    permissions.extend(m.permissions);
                    hooks.extend(m.hooks);
                }
            }
            let remaining = MAX_PREVIEW_TOTAL.saturating_sub(total_code);
            let code = if remaining > 0 {
                let part = truncate_code(&src, MAX_PREVIEW_FILE.min(remaining));
                total_code += part.len();
                Some(part)
            } else {
                None
            };
            files.push(PreviewFile {
                name: entry_name,
                size,
                binary: false,
                code,
            });
        } else {
            files.push(PreviewFile {
                name: entry_name,
                size,
                binary: true,
                code: None,
            });
        }
    }
    if aplugs == 0 {
        dangerous = true;
        push_warning(
            &mut warnings,
            "danger",
            "no_plugins",
            "В архиве нет ни одного файла .aplug.",
        );
    }
    Ok(build_preview(
        path,
        &bytes,
        "arbz",
        id,
        name,
        version,
        author,
        description,
        api,
        permissions,
        hooks,
        files,
        warnings,
        network,
        obfuscated,
        dangerous,
    ))
}

/// Превью плагина до установки.
pub fn inspect(path: &Path) -> Res<PluginPreview> {
    if !path.is_file() {
        return err("Файл плагина не найден");
    }
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "aplug" => inspect_aplug(path),
        "arbz" => inspect_archive(path),
        _ => err("Поддерживаются только .aplug и .arbz"),
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn inspects_inline_aplug() {
        let src = r#"# arbuz-plugin: {"id":"test.inline","name":"Inline","version":"1.0.0","author":"Test","description":"Inline test","api":1,"permissions":["ui"],"hooks":["panel.test.render"]}

PANEL_ID = "test"

def render(payload):
    return {"html": "<p>hi</p>"}
"#;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("arbuz-inline-{stamp}.aplug"));
        std::fs::write(&path, src).unwrap();
        let p = inspect(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(p.id, "test.inline");
        assert!(p.permissions.contains(&"ui".to_string()));
        assert!(p.files.iter().any(|f| {
            f.code
                .as_deref()
                .map(|c| c.contains("PANEL_ID"))
                .unwrap_or(false)
        }));
        assert_eq!(p.sha256.len(), 64);
        assert!(!p.dangerous);
        assert_eq!(p.risk, "low");
    }
}
