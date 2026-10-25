use crate::error::{err, Res};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Имя папки данных — она лежит рядом с лаунчером или в профиле пользователя.
const FOLDER: &str = "arbuz-launcher";

/// Корень данных лаунчера.
///
/// Лаунчер портативный: настройки, аккаунты, профили, runtimes и instances
/// лежат в папке `arbuz-launcher` рядом с исполняемым файлом, а не в AppData.
///
/// Писать рядом с собой можно не всегда. Если не выходит (например, лаунчер
/// стоит в `Program Files`) или сам exe распакован в AppData — так делает
/// установщик, ставя лаунчер в `%LOCALAPPDATA%\Programs`, — берём
/// `%USERPROFILE%\arbuz-launcher`. AppData мы не трогаем в любом случае.
/// Свой путь можно задать переменной окружения `ARBUZ_DATA_DIR`.
pub fn root() -> PathBuf {
    ROOT.get_or_init(resolve).clone()
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();

fn resolve() -> PathBuf {
    if let Some(dir) = std::env::var_os("ARBUZ_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(dir) = beside_exe() {
        if !in_appdata(&dir) && writable(&dir) {
            return dir;
        }
    }
    dirs::home_dir()
        .map(|home| home.join(FOLDER))
        .unwrap_or_else(|| PathBuf::from(FOLDER))
}

/// `arbuz-launcher` рядом с exe. На Linux и macOS папка приложения бывает
/// только для чтения (или живёт до закрытия образа), поэтому — лишь Windows.
fn beside_exe() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    // Лаунчер и так лежит в папке со своим именем — так ставит установщик,
    // `%USERPROFILE%\arbuz-launcher\arbuz-launcher.exe`. Тогда данные кладём
    // прямо туда, а не во вложенную папку с тем же названием.
    let named = dir
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case(FOLDER))
        .unwrap_or(false);
    Some(if named { dir } else { dir.join(FOLDER) })
}

/// Установщик ставит лаунчер внутрь AppData — тогда «рядом с ним» и означало
/// бы AppData, от которого мы уходим.
fn in_appdata(dir: &Path) -> bool {
    let s = dir.to_string_lossy().to_ascii_lowercase();
    s.contains("\\appdata\\") || s.contains("/appdata/")
}

/// Проверяем пробным файлом: права на запись не угадать по метаданным.
fn writable(dir: &Path) -> bool {
    if !dir.is_dir() && std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".probe");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn config_file() -> PathBuf {
    root().join("config.json")
}

/// Первый запуск: файла config.json ещё нет — значит показываем визард знакомства.
pub fn first_run() -> bool {
    !config_file().exists()
}

pub fn accounts_file() -> PathBuf {
    root().join("accounts.json")
}

// runtimes/
pub fn runtimes_dir() -> PathBuf {
    root().join("runtimes")
}

pub fn java_dir(major: u8) -> PathBuf {
    runtimes_dir().join(format!("java{major}"))
}

pub fn authlib_jar() -> PathBuf {
    runtimes_dir().join("authlib-injector.jar")
}

/// Пользовательские обои главной.
pub fn wallpapers_dir() -> PathBuf {
    root().join("wallpapers")
}

pub fn installers_dir() -> PathBuf {
    runtimes_dir().join("installers")
}

pub fn tmp_dir() -> PathBuf {
    runtimes_dir().join("tmp")
}

// сканер модов
/// Веса и пороги сканера (правится вручную или продвинутыми настройками).
pub fn scanner_config_file() -> PathBuf {
    root().join("scanner.json")
}

/// Локальная база известной малвари (обновляется отдельно).
pub fn malware_db_file() -> PathBuf {
    root().join("malware_db.json")
}

/// Кэш результатов сканирования по SHA-256.
pub fn scanner_cache_dir() -> PathBuf {
    root().join("cache").join("scanner")
}

/// Ручные исключения сканера для профиля.
pub fn instance_scanner_json(id: &str) -> PathBuf {
    instance_dir(id).join("scanner.json")
}

/// Карантин заблокированных модов (параноидальный режим).
pub fn instance_quarantine_dir(id: &str) -> PathBuf {
    instance_dir(id).join("quarantine")
}

// shared/
pub fn shared_dir() -> PathBuf {
    root().join("shared")
}

pub fn libraries_dir() -> PathBuf {
    shared_dir().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    shared_dir().join("assets")
}

pub fn assets_indexes_dir() -> PathBuf {
    assets_dir().join("indexes")
}

pub fn assets_objects_dir() -> PathBuf {
    assets_dir().join("objects")
}

pub fn versions_dir() -> PathBuf {
    shared_dir().join("versions")
}

pub fn version_dir(id: &str) -> PathBuf {
    versions_dir().join(id)
}

pub fn version_json(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.json"))
}

pub fn version_jar(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.jar"))
}

pub fn launcher_profiles() -> PathBuf {
    shared_dir().join("launcher_profiles.json")
}

// instances/
pub fn instances_dir() -> PathBuf {
    root().join("instances")
}

/// Кэш распакованных из архивов иконок контента.
pub fn icons_cache_dir() -> PathBuf {
    root().join("cache").join("icons")
}

pub fn instance_dir(id: &str) -> PathBuf {
    instances_dir().join(id)
}

/// Проверяет, что `id` — идентификатор профиля, созданный лаунчером
/// (`Uuid::new_v4().simple()`: 32 строчных hex-символа). Не даёт `..` и
/// разделителям вывести путь за пределы `instances/`.
pub fn validate_instance_id(id: &str) -> Res<()> {
    let ok = id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if ok {
        Ok(())
    } else {
        err("Некорректный идентификатор профиля")
    }
}

pub fn instance_json(id: &str) -> PathBuf {
    instance_dir(id).join("instance.json")
}

pub fn instance_content_json(id: &str) -> PathBuf {
    instance_dir(id).join("content.json")
}

/// gameDir профиля.
pub fn instance_minecraft_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}

pub fn instance_natives_dir(id: &str) -> PathBuf {
    instance_dir(id).join("natives")
}

/// Путь к файлу иконки профиля (имя файла из поля `icon`).
pub fn instance_icon(id: &str, file: &str) -> PathBuf {
    instance_dir(id).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appdata_paths_are_recognized() {
        assert!(in_appdata(Path::new(
            r"C:\Users\me\AppData\Local\Programs\arbuz-launcher"
        )));
        assert!(in_appdata(Path::new(
            r"c:\users\me\appdata\roaming\arbuz-launcher"
        )));
        assert!(!in_appdata(Path::new(r"D:\Games\arbuz-launcher")));
        assert!(!in_appdata(Path::new(r"C:\Users\me\arbuz-launcher")));
    }

    #[test]
    fn instance_id_accepts_uuid_simple() {
        assert!(validate_instance_id("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn instance_id_rejects_traversal_and_odd_input() {
        assert!(validate_instance_id("..").is_err());
        assert!(validate_instance_id("../..").is_err());
        assert!(validate_instance_id("0123456789abcdef0123456789abcdef/..").is_err());
        assert!(validate_instance_id("0123456789ABCDEF0123456789abcdef").is_err());
        assert!(validate_instance_id("0123456789abcdef").is_err());
        assert!(validate_instance_id("0123456789abcdef0123456789abcdeg").is_err());
        assert!(validate_instance_id("").is_err());
    }
}
