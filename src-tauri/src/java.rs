use crate::error::*;
use crate::paths;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub major: u8,
    pub version: String,
    pub path: String,
}

#[cfg(windows)]
const JAVA_EXES: &[&str] = &["java.exe", "javaw.exe"];
#[cfg(not(windows))]
const JAVA_EXES: &[&str] = &["java"];

/// Для опроса версии годится только консольный java: `javaw.exe` не пишет в
/// stdout/stderr, а при ошибке запуска показывает модальное окно вместо вывода.
#[cfg(windows)]
const PROBE_EXES: &[&str] = &["java.exe"];
#[cfg(not(windows))]
const PROBE_EXES: &[&str] = &["java"];

/// Настоящая ли это установка Java: рядом с каталогом `bin` лежит `lib` или
/// `release`. Отсекает шимы `Common Files\Oracle\Java\javapath`, которые лишь
/// перенаправляют запуск по записи в реестре и падают, если та устарела.
fn is_java_home(exe: &Path) -> bool {
    let Some(home) = exe.parent().and_then(|p| p.parent()) else {
        return false;
    };
    home.join("lib").is_dir() || home.join("release").is_file()
}

/// Рекурсивный поиск `bin/java` (или `bin/javaw.exe`) внутри каталога-кандидата.
fn scan_root(root: &Path, out: &mut Vec<PathBuf>, exes: &[&str]) {
    if !root.is_dir() {
        return;
    }
    for e in WalkDir::new(root)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !e.file_type().is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !exes.iter().any(|x| *x == name.as_str()) {
            continue;
        }
        let in_bin = e
            .path()
            .parent()
            .and_then(|p| p.file_name())
            .map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("bin"));
        if in_bin && is_java_home(e.path()) {
            out.push(e.path().to_path_buf());
        }
    }
}

/// Разбор major-версии из вывода `java -version` без регулярных выражений.
fn parse_major(text: &str) -> Option<u8> {
    let idx = text.find("version \"")?;
    let rest = &text[idx + "version \"".len()..];
    let mut num = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else {
            break;
        }
    }
    if num.is_empty() {
        return None;
    }
    let parts: Vec<&str> = num.split('.').collect();
    let first: u8 = parts.first()?.parse().ok()?;
    if first == 1 {
        parts.get(1)?.parse().ok()
    } else {
        Some(first)
    }
}

/// Запуск `java -version` и разбор результата. `None` — кандидат невалиден.
async fn probe(path: &Path) -> Option<(u8, String)> {
    let mut cmd = tokio::process::Command::new(path);
    cmd.arg("-version");
    // GUI-процесс без этого флага показывает на Windows мигающее окно консоли.
    crate::winproc::hide_console(&mut cmd);
    let out = tokio::time::timeout(std::time::Duration::from_secs(5), cmd.output())
        .await
        .ok()?
        .ok()?;
    let text = if !out.stderr.is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let version = text
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    let major = parse_major(&text)?;
    Some((major, version))
}

/// Поиск всех доступных установок Java.
pub async fn detect_javas() -> Res<Vec<JavaInstall>> {
    let mut direct: Vec<PathBuf> = Vec::new();
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(h) = std::env::var("JAVA_HOME") {
        let h = h.trim();
        if !h.is_empty() {
            direct.push(PathBuf::from(h).join("bin"));
        }
    }
    if let Ok(p) = std::env::var("PATH") {
        for d in std::env::split_paths(&p) {
            direct.push(d);
        }
    }

    #[cfg(windows)]
    {
        for base in [
            "C:\\Program Files\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\Microsoft",
            "C:\\Program Files\\Zulu",
        ] {
            roots.push(PathBuf::from(base));
        }
    }
    #[cfg(not(windows))]
    {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    }

    let rt = paths::runtimes_dir();
    if let Ok(rd) = std::fs::read_dir(&rt) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n.starts_with("java") {
                roots.push(e.path());
            }
        }
    }

    let mut cands: Vec<PathBuf> = Vec::new();
    for d in &direct {
        for n in PROBE_EXES {
            let p = d.join(n);
            if p.is_file() && is_java_home(&p) {
                cands.push(p);
            }
        }
    }
    for r in &roots {
        scan_root(r, &mut cands, PROBE_EXES);
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<JavaInstall> = Vec::new();
    for cand in cands {
        let canon = std::fs::canonicalize(&cand).unwrap_or(cand);
        let key = canon.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Some((major, version)) = probe(&canon).await {
            out.push(JavaInstall {
                major,
                version,
                path: canon.to_string_lossy().to_string(),
            });
        }
    }

    out.sort_by_key(|j| j.major);
    Ok(out)
}

/// Скачивание и распаковка JRE Adoptium, если нужной версии нет локально.
pub async fn ensure_java(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
    progress_id: &str,
    major: u8,
    lo: f32,
    hi: f32,
) -> Res<PathBuf> {
    if let Some(j) = detect_javas().await?.into_iter().find(|j| j.major == major) {
        return Ok(PathBuf::from(j.path));
    }

    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };

    // Сборку берём через assets API: он отдаёт ссылку вместе с sha256 и размером.
    #[derive(serde::Deserialize)]
    struct Asset {
        binary: AssetBinary,
    }
    #[derive(serde::Deserialize)]
    struct AssetBinary {
        package: AssetPackage,
    }
    #[derive(serde::Deserialize)]
    struct AssetPackage {
        link: String,
        checksum: String,
        size: u64,
    }

    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?os={os}&architecture={arch}&image_type=jre"
    );
    let assets: Vec<Asset> = http
        .get(&api)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(pkg) = assets.into_iter().next().map(|a| a.binary.package) else {
        return err("Adoptium не отдал сборку JRE");
    };
    if pkg.checksum.len() != 64 || !pkg.checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
        return err("Adoptium не отдал контрольную сумму JRE");
    }
    let ext = if pkg.link.ends_with(".tar.gz") { "tar.gz" } else { "zip" };

    let tmp = paths::tmp_dir().join(format!("java{major}.{ext}"));
    crate::http::download_all(
        app,
        http,
        progress_id,
        vec![crate::http::Dl {
            url: pkg.link,
            path: tmp.clone(),
            sha1: None,
            sha256: Some(pkg.checksum.to_ascii_lowercase()),
            size: pkg.size,
        }],
        lo,
        hi,
    )
    .await?;

    let dest = paths::java_dir(major);
    tokio::fs::create_dir_all(&dest).await?;

    let tmp2 = tmp.clone();
    let dest2 = dest.clone();
    let found = tokio::task::spawn_blocking(move || -> Res<Option<PathBuf>> {
        unpack(&tmp2, &dest2)?;
        Ok(find_java(&dest2))
    })
    .await??;

    let _ = tokio::fs::remove_file(&tmp).await;

    match found {
        Some(p) => Ok(p),
        None => err("Не удалось найти java после распаковки"),
    }
}

/// Распаковка архива по расширению: `.zip` или `.tar.gz`.
fn unpack(archive: &Path, dest: &Path) -> Res<()> {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        unpack_zip(archive, dest)
    } else {
        unpack_targz(archive, dest)
    }
}

fn unpack_zip(archive: &Path, dest: &Path) -> Res<()> {
    let f = std::fs::File::open(archive)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        // enclosed_name защищает от выхода за пределы dest (в том числе через `..`).
        let Some(rel) = e.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut e, &mut w)?;
        }
    }
    Ok(())
}

fn unpack_targz(archive: &Path, dest: &Path) -> Res<()> {
    let f = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut ar = tar::Archive::new(gz);
    ar.unpack(dest)?;
    Ok(())
}

/// Первый `bin/java` или `bin/javaw.exe` внутри распакованного JRE.
fn find_java(root: &Path) -> Option<PathBuf> {
    let mut fallback: Option<PathBuf> = None;
    for e in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !e.file_type().is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !JAVA_EXES.iter().any(|x| *x == name.as_str()) {
            continue;
        }
        let in_bin = e
            .path()
            .parent()
            .and_then(|p| p.file_name())
            .map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("bin"));
        if !in_bin {
            continue;
        }
        // Предпочитаем консольный java, javaw — запасной вариант.
        if name != "javaw.exe" {
            return Some(e.path().to_path_buf());
        }
        if fallback.is_none() {
            fallback = Some(e.path().to_path_buf());
        }
    }
    fallback
}
