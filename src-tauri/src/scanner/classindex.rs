//! Индекс «класс → jar мода» для умной консоли (Б2).
//!
//! Клик по строке стектрейса должен показывать, какому модулю принадлежит
//! класс. Имя .class-записи в jar совпадает с бинарным именем класса, поэтому
//! полноценный разбор не нужен: смотрим центральный каталог каждого jar из
//! mods/ (данные классов повторно использует и сканер через кэш).

use crate::{content, paths};
use serde::Serialize;
use std::path::PathBuf;

/// Владелец класса.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClassOwner {
    /// Имя мода из метаданных.
    pub name: String,
    /// Относительный путь jar: `mods/foo.jar`.
    pub file: String,
}

/// Ищет, в каком jar профиля лежит класс `class` (включая внутренние
/// классы `Foo$1`). `None` — класс игры/библиотеки из общих папок или
/// в mods/ его нет.
pub async fn find_owner(id: &str, class: &str) -> crate::error::Res<Option<ClassOwner>> {
    paths::validate_instance_id(id)?;
    // Бинарное имя класса: без мусора, который мог притащиться из стека.
    let class: String = class
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '$' | '_'))
        .collect();
    if class.len() < 3 || class.len() > 400 {
        return Ok(None);
    }
    let mods_dir = paths::instance_minecraft_dir(id).join("mods");
    let exact = format!("{class}.class");
    let inner = format!("{class}$");

    let class2 = class.clone();
    let found = tokio::task::spawn_blocking(move || -> Option<(PathBuf, String)> {
        let rd = std::fs::read_dir(&mods_dir).ok()?;
        let mut jars: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.to_string_lossy().to_ascii_lowercase().ends_with(".jar")
            })
            .collect();
        // Детерминированный порядок: сначала точное совпадение в любом jar,
        // перебор идёт по алфавиту имён файлов.
        jars.sort();
        for jar in jars {
            let Ok(f) = std::fs::File::open(&jar) else {
                continue;
            };
            let Ok(mut z) = zip::ZipArchive::new(f) else {
                continue;
            };
            if z.by_name(&exact).is_ok() {
                let name = jar.file_name().and_then(|n| n.to_str()).unwrap_or("mod.jar").to_string();
                return Some((jar, name));
            }
            // Внутренние классы: перебор имён дешевле только при малом jar,
            // но центральный каталог и так уже прочитан.
            for i in 0..z.len() {
                if let Ok(e) = z.by_index(i) {
                    let n = e.name();
                    if n.ends_with(".class") && n.starts_with(&inner) {
                        let name = jar.file_name().and_then(|x| x.to_str()).unwrap_or("mod.jar").to_string();
                        return Some((jar, name));
                    }
                }
            }
        }
        None
    })
    .await?;
    let Some((jar, file_name)) = found else {
        return Ok(None);
    };
    let rel = content::rel_slash(
        jar.strip_prefix(paths::instance_minecraft_dir(id)).unwrap_or(&jar),
    );
    let _ = class2;
    // Имя мода — из метаданных jar; без них — имя файла.
    let meta = tokio::task::spawn_blocking(move || content::read_meta(&jar)).await?;
    let name = if meta.0.trim().is_empty() {
        file_name.trim_end_matches(".jar").to_string()
    } else {
        meta.0
    };
    Ok(Some(ClassOwner { name, file: rel }))
}
