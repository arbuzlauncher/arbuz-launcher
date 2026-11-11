//! Анализатор логов и крашей: правила, журнал запусков и отчёт для поддержки.
//!
//! Правила лежат в `log-rules.json` рядом с данными лаунчера. Если файла нет,
//! используется встроенный набор. Это позволяет обновлять правила без выпуска
//! новой версии лаунчера.

use crate::error::{err, Res};
use crate::paths;
use crate::store;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogFix {
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    /// error | warn | info
    pub severity: String,
    /// Подстроки, хотя бы одна из которых включает правило (регистр не важен).
    pub patterns: Vec<String>,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub fixes: Vec<LogFix>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogFinding {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub fixes: Vec<LogFix>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub instance_id: String,
    pub started: u64,
    pub ended: u64,
    pub exit_code: Option<i32>,
    pub log: Option<String>,
    pub crash: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogAnalysis {
    pub found: bool,
    pub file: Option<String>,
    pub findings: Vec<LogFinding>,
    pub journal: Vec<RunRecord>,
}

static STARTS: Lazy<Mutex<HashMap<String, u64>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn rules_file() -> PathBuf {
    paths::root().join("log-rules.json")
}

fn journal_dir(id: &str) -> PathBuf {
    paths::root().join("logs").join("journal").join(id)
}

fn fix(kind: &str, label: &str) -> LogFix {
    LogFix {
        kind: kind.into(),
        label: label.into(),
        value: None,
    }
}

/// Встроенные правила. Их можно заменить файлом `log-rules.json`.
fn default_rules() -> Vec<Rule> {
    fn r(
        id: &str,
        severity: &str,
        patterns: &[&str],
        title: &str,
        detail: &str,
        fixes: &[(&str, &str)],
    ) -> Rule {
        Rule {
            id: id.into(),
            severity: severity.into(),
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
            title: title.into(),
            detail: detail.into(),
            fixes: fixes.iter().map(|(k, l)| fix(k, l)).collect(),
        }
    }
    vec![
        r("oom", "error", &["outofmemoryerror", "java heap space", "gc overhead limit exceeded"], "Не хватает памяти", "Игра упёрлась в лимит ОЗУ. Увеличьте максимальную память профиля или уменьшите количество модов.", &[("increase_ram", "Увеличить память на 2 ГБ")]),
        r("direct-buffer", "warn", &["direct buffer memory", "unable to allocate"], "Не хватает прямой памяти", "Много модов с буферами. Увеличьте память и проверьте аргументы JVM.", &[("increase_ram", "Увеличить память")]),
        r("java-version", "error", &["unsupportedclassversionerror", "class file version", "requires java"], "Неподходящая версия Java", "Мод или загрузчик собран под другую версию Java. Сбросьте Java на рекомендованную для этой версии игры.", &[("reset_java", "Сбросить Java")]),
        r("java-missing", "error", &["could not find java", "java is not recognized", "no such file or directory: java"], "Java не найдена", "Лаунчер не смог запустить Java. Установите JRE или укажите путь вручную.", &[("reset_java", "Сбросить Java")]),
        r("mixin-apply", "error", &["mixin apply failed", "mixinapplyerror", "mixin transformer error"], "Ошибка Mixin", "Один из модов не смог применить Mixin. Отключите мод из стека или проверьте его версию.", &[("open_conflicts", "Проверить конфликты")]),
        r("mixin-invalid", "error", &["invalidmixin", "mixin config", "mixin.invalid"], "Некорректный Mixin-конфиг", "Мод содержит битый Mixin-конфиг. Переустановите мод или отключите его.", &[("open_conflicts", "Проверить конфликты")]),
        r("mod-resolution", "error", &["incompatible mod set", "mod resolution failed", "mod resolution encountered an incompatible"], "Несовместимый набор модов", "Загрузчик не смог собрать набор модов: конфликт версий или зависимостей.", &[("open_conflicts", "Открыть анализатор конфликтов")]),
        r("missing-dependency", "error", &["missing dependency", "requires any version of", "requires version", "depends on"], "Не хватает зависимости", "Одному из модов нужна библиотека или другой мод, которых нет в сборке.", &[("open_conflicts", "Найти зависимость")]),
        r("duplicate-mod", "warn", &["duplicate mod", "found duplicate", "multiple versions"], "Дубликаты модов", "В папке mods лежит несколько версий одного мода. Оставьте одну.", &[("open_conflicts", "Открыть конфликты")]),
        r("glfw", "error", &["glfw error", "failed to create window", "failed to initialize glfw"], "Ошибка создания окна", "Проблема с GLFW/OpenGL. Обновите драйвер видеокарты и проверьте, что игра не запущена в несовместимом режиме.", &[("update_driver", "Как обновить драйвер")]),
        r("opengl-context", "error", &["opengl context", "could not create opengl", "failed to create opengl", "wglcreatecontext"], "Не создаётся OpenGL-контекст", "Драйвер видеокарты не даёт игре создать OpenGL-контекст. Обновите драйвер или переключитесь на дискретную видеокарту.", &[("update_driver", "Обновить драйвер")]),
        r("pixel-format", "error", &["pixel format not accelerated", "does not support opengl", "unsupported opengl"], "Нет аппаратного OpenGL", "Система не поддерживает нужный OpenGL. Установите драйвер видеокарты, а не универсальный.", &[("update_driver", "Обновить драйвер")]),
        r("access-violation", "error", &["exception_access_violation", "sigsegv", "fatal error has been detected by the java runtime"], "Сбой нативной части", "JVM или драйвер упал на нативном уровне. Часто это драйвер видеокарты или несовместимый мод с нативными библиотеками.", &[("open_crash_reports", "Открыть crash-reports")]),
        r("hs-err", "error", &["hs_err_pid", "problematic frame"], "Аварийный отчёт JVM", "JVM сохранила hs_err-файл. В нём указан problematic frame — драйвер или нативный мод.", &[("open_crash_reports", "Открыть отчёт")]),
        r("corrupt-jar", "error", &["invalid loc header", "zipException", "corrupt", "unexpected end of zip"], "Битый файл мода", "Один из jar-файлов повреждён. Удалите его и скачайте заново.", &[("open_conflicts", "Найти битый мод")]),
        r("file-not-found", "warn", &["java.io.filenotfoundexception", "no such file or directory"], "Файл не найден", "Мод или конфиг ссылается на отсутствующий файл. Проверьте папку config и переустановите мод.", &[("open_configs", "Открыть конфиги")]),
        r("access-denied", "error", &["accessdeniedexception", "permission denied", "access is denied"], "Нет доступа к файлу", "Антивирус или права Windows мешают чтению файла. Добавьте папку игры в исключения.", &[("open_instance", "Открыть папку игры")]),
        r("disk-full", "error", &["no space left on device", "not enough space", "disk full"], "Мало места на диске", "Освободите место или перенесите папку игры на другой диск.", &[("open_instance", "Открыть папку игры")]),
        r("reserve-memory", "error", &["could not reserve enough space", "unable to create new native thread"], "Не удалось выделить память", "Слишком большой -Xmx или мало свободной памяти. Уменьшите максимальную память профиля.", &[("increase_ram", "Настроить память")]),
        r("metaspace", "warn", &["metaspace", "permgen space"], "Переполнение Metaspace", "Увеличьте память и проверьте аргументы JVM: слишком много классов от модов.", &[("increase_ram", "Увеличить память")]),
        r("watchdog", "warn", &["watchdog", "server thread stuck", "timed out"], "Зависание потока", "Игра или сервер завис. Проверьте моды на утечки и уменьшите количество сущностей/чанков.", &[("open_conflicts", "Проверить моды")]),
        r("network", "warn", &["connection timed out", "connect timed out", "failed to download", "sockettimeoutexception"], "Проблема с сетью", "Не удалось скачать файл или подключиться. Проверьте интернет и прокси в настройках.", &[("check_network", "Проверить сеть")]),
        r("ssl", "warn", &["sslhandshakeexception", "certificate", "pkix"], "Ошибка TLS", "Проблема с сертификатом или системным временем. Проверьте дату и прокси.", &[("check_network", "Проверить сеть")]),
        r("unknown-host", "warn", &["unknownhostexception", "no such host", "name or service not known"], "Неизвестный хост", "DNS не смог разрешить адрес. Проверьте интернет или смените DNS.", &[("check_network", "Проверить сеть")]),
        r("auth-down", "warn", &["authentication servers down", "authserver", "sessionserver"], "Серверы входа недоступны", "Mojang/Microsoft временно недоступны. Попробуйте позже или войдите заново.", &[("relogin", "Войти заново")]),
        r("invalid-session", "warn", &["invalid session", "invalid token", "401 unauthorized"], "Сессия истекла", "Токен входа больше не действует. Войдите в аккаунт заново.", &[("relogin", "Войти заново")]),
        r("world-corrupt", "error", &["corrupted chunk", "chunk load", "failed to load world", "datafixerupper"], "Повреждённый мир", "Мир или чанк повреждён. Сделайте резервную копию и попробуйте восстановить из backup.", &[("backup_world", "Открыть папку миров")]),
        r("registry", "error", &["registry remapping", "missing registry", "unknown registry"], "Ошибка реестра", "Мод удалил или переименовал registry-элемент. Не удаляйте моды из существующего мира без необходимости.", &[("open_conflicts", "Проверить моды")]),
        r("optifine-sodium", "warn", &["optifine", "sodium"], "OptiFine и Sodium вместе", "Эти моды конфликтуют. Оставьте один оптимизатор.", &[("open_conflicts", "Открыть конфликты")]),
        r("stack-overflow", "error", &["stackoverflowerror"], "Переполнение стека", "Рекурсия в моде или несовместимая версия. Отключите подозрительный мод.", &[("open_conflicts", "Проверить моды")]),
        r("loader-version", "error", &["fabric loader", "forge version", "neoforge version", "incompatible loader"], "Неподходящий загрузчик", "Версия загрузчика не подходит к версии игры или модам. Переустановите загрузчик.", &[("reinstall_loader", "Переустановить загрузчик")]),
        r("mod-loading", "error", &["mod loading error", "failed to load mod", "error loading mod"], "Ошибка загрузки мода", "Один из модов не загрузился. Смотрите строку Caused by и отключите виновника.", &[("open_conflicts", "Найти мод")]),
        r("exception-thread", "warn", &["exception in thread"], "Исключение в потоке", "В логе есть исключение. Откройте полный отчёт и проверьте стек.", &[("open_crash_reports", "Открыть отчёт")]),
        r("fatal", "error", &["a fatal error", "fatal error"], "Критическая ошибка", "Игра завершилась с критической ошибкой. Проверьте crash-report.", &[("open_crash_reports", "Открыть отчёт")]),
    ]
}

fn load_rules() -> Vec<Rule> {
    if let Ok(raw) = std::fs::read_to_string(rules_file()) {
        if let Ok(mut list) = serde_json::from_str::<Vec<Rule>>(&raw) {
            list.retain(|r| !r.id.is_empty() && !r.patterns.is_empty());
            if !list.is_empty() {
                return list;
            }
        }
    }
    default_rules()
}

fn newest_log(id: &str) -> Option<(PathBuf, String)> {
    let game = paths::instance_minecraft_dir(id);
    let crash_dir = game.join("crash-reports");
    let mut best: Option<(SystemTime, PathBuf, String)> = None;
    if let Ok(rd) = std::fs::read_dir(&crash_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("txt") {
                continue;
            }
            let mt = e.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
            if best.as_ref().map(|(b, _, _)| mt > *b).unwrap_or(true) {
                best = Some((mt, p, "crash-report".into()));
            }
        }
    }
    if best.is_some() {
        return best.map(|(_, p, s)| (p, s));
    }
    for (name, source) in [("latest.log", "latest-log"), ("debug.log", "debug-log")] {
        let p = game.join("logs").join(name);
        if p.is_file() {
            return Some((p, source.into()));
        }
    }
    None
}

fn read_capped(path: &Path, max: usize) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    let slice = if bytes.len() > max { &bytes[..max] } else { &bytes[..] };
    let end = std::str::from_utf8(slice)
        .map(|_| slice.len())
        .unwrap_or_else(|e| e.valid_up_to());
    String::from_utf8_lossy(&slice[..end]).to_string()
}

/// Применяет правила к тексту лога.
pub fn analyze_text(text: &str) -> Vec<LogFinding> {
    let low = text.to_lowercase();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for rule in load_rules() {
        if !seen.insert(rule.id.clone()) {
            continue;
        }
        if rule
            .patterns
            .iter()
            .any(|p| !p.is_empty() && low.contains(&p.to_lowercase()))
        {
            out.push(LogFinding {
                id: rule.id,
                severity: rule.severity,
                title: rule.title,
                detail: rule.detail,
                fixes: rule.fixes,
            });
        }
    }
    out.sort_by_key(|f| match f.severity.as_str() {
        "error" => 0,
        "warn" => 1,
        _ => 2,
    });
    out
}

pub async fn analyze(id: &str) -> Res<LogAnalysis> {
    paths::validate_instance_id(id)?;
    let Some((path, _source)) = newest_log(id) else {
        return Ok(LogAnalysis {
            found: false,
            file: None,
            findings: Vec::new(),
            journal: journal(id).await?,
        });
    };
    let text = read_capped(&path, 2_000_000);
    Ok(LogAnalysis {
        found: true,
        file: path.file_name().map(|s| s.to_string_lossy().to_string()),
        findings: analyze_text(&text),
        journal: journal(id).await?,
    })
}

pub async fn journal(id: &str) -> Res<Vec<RunRecord>> {
    let dir = journal_dir(id);
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(&dir).await else {
        return Ok(out);
    };
    while let Ok(Some(e)) = rd.next_entry().await {
        if let Ok(raw) = tokio::fs::read_to_string(e.path()).await {
            if let Ok(r) = serde_json::from_str::<RunRecord>(&raw) {
                out.push(r);
            }
        }
    }
    out.sort_by(|a, b| b.started.cmp(&a.started));
    out.truncate(50);
    Ok(out)
}

pub fn journal_start(id: &str) {
    if let Ok(mut g) = STARTS.lock() {
        g.insert(id.to_string(), now());
    }
}

pub async fn journal_finish(id: &str, exit_code: Option<i32>) -> Res<()> {
    let started = STARTS
        .lock()
        .ok()
        .and_then(|mut g| g.remove(id))
        .unwrap_or_else(now);
    let dir = journal_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{started}-{}.json", now()));
    let record = RunRecord {
        instance_id: id.to_string(),
        started,
        ended: now(),
        exit_code,
        log: newest_log(id).map(|(p, _)| p.to_string_lossy().to_string()),
        crash: if exit_code.unwrap_or(0) != 0 {
            Some("Игра завершилась с ненулевым кодом".into())
        } else {
            None
        },
    };
    tokio::fs::write(&file, serde_json::to_string_pretty(&record)?).await?;
    Ok(())
}

fn redact(text: &str) -> String {
    let mut s = text.to_string();
    let patterns = [
        r"(?i)(token|access[_-]?token|authorization|bearer)\s*[:=]\s*\S+",
        r"(?i)(password|passwd|pwd)\s*[:=]\s*\S+",
        r"[A-Za-z]:\\[^\s\]\)]+",
        r"/home/[^\s\]\)]+",
        r"/Users/[^\s\]\)]+",
        r"\b\d{1,3}(?:\.\d{1,3}){3}\b",
        r"\b[0-9a-fA-F]{32}\b",
    ];
    for p in patterns {
        if let Ok(re) = Regex::new(p) {
            s = re.replace_all(&s, "<скрыто>").to_string();
        }
    }
    s
}

pub async fn support_report(id: &str) -> Res<String> {
    let inst = store::read_instance(id).await?;
    let analysis = analyze(id).await?;
    let mut out = String::new();
    out.push_str("Arbuz Launcher — отчёт для поддержки\n");
    out.push_str(&format!(
        "Профиль: {} · {} {} · {}\n",
        inst.name,
        inst.loader.label(),
        inst.mc_version,
        inst.loader_version.as_deref().unwrap_or("—")
    ));
    out.push_str(&format!("Лог: {}\n\n", analysis.file.as_deref().unwrap_or("не найден")));
    if analysis.findings.is_empty() {
        out.push_str("Правила не нашли явных проблем.\n");
    } else {
        out.push_str("Найденные проблемы:\n");
        for f in &analysis.findings {
            out.push_str(&format!("- [{}] {}: {}\n", f.severity, f.title, f.detail));
        }
    }
    out.push_str("\nПоследние запуски:\n");
    for r in analysis.journal.iter().take(10) {
        out.push_str(&format!(
            "- {} → {}, код {:?}\n",
            r.started, r.ended, r.exit_code
        ));
    }
    if let Some((path, _)) = newest_log(id) {
        let text = read_capped(&path, 120_000);
        let tail: Vec<&str> = text.lines().rev().take(300).collect();
        out.push_str("\nХвост лога:\n");
        for line in tail.into_iter().rev() {
            out.push_str(&redact(line));
            out.push('\n');
        }
    }
    Ok(out)
}

/// Скачивает обновлённый набор правил и сохраняет его в `log-rules.json`.
pub async fn update_rules(state: &crate::state::AppState, url: &str) -> Res<usize> {
    if !url.starts_with("https://") {
        return err("Правила можно скачивать только по https://");
    }
    let text = state
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let list: Vec<Rule> = serde_json::from_str(&text)
        .map_err(|e| crate::error::AppError::Msg(format!("Некорректный файл правил: {e}")))?;
    if list.is_empty() {
        return err("В файле нет правил");
    }
    tokio::fs::write(rules_file(), serde_json::to_string_pretty(&list)?).await?;
    Ok(list.len())
}

/// Применяет исправление из результата анализа.
pub async fn apply_fix(
    app: &tauri::AppHandle,
    id: &str,
    kind: &str,
    value: Option<&str>,
) -> Res<()> {
    paths::validate_instance_id(id)?;
    match kind {
        "increase_ram" => {
            let mut inst = store::read_instance(id).await?;
            inst.ram_max_mb = (inst.ram_max_mb + 2048).clamp(2048, 16384);
            store::write_instance(&inst).await?;
            Ok(())
        }
        "reset_java" => {
            let mut inst = store::read_instance(id).await?;
            inst.java_major = crate::models::default_java(&inst.mc_version);
            inst.java_path = None;
            store::write_instance(&inst).await?;
            Ok(())
        }
        "reinstall_loader" => {
            let mut inst = store::read_instance(id).await?;
            inst.loader_version = None;
            store::write_instance(&inst).await?;
            Ok(())
        }
        "open_instance" => {
            let dir = paths::instance_minecraft_dir(id);
            std::fs::create_dir_all(&dir)?;
            crate::commands::open_path(app, &dir)?;
            Ok(())
        }
        "open_crash_reports" => {
            let dir = paths::instance_minecraft_dir(id).join("crash-reports");
            std::fs::create_dir_all(&dir)?;
            crate::commands::open_path(app, &dir)?;
            Ok(())
        }
        "open_configs" => {
            let dir = paths::instance_minecraft_dir(id).join("config");
            std::fs::create_dir_all(&dir)?;
            crate::commands::open_path(app, &dir)?;
            Ok(())
        }
        "open_conflicts" | "check_network" | "update_driver" | "relogin" | "backup_world" => {
            let _ = value;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn rules_json() -> Value {
    json!(load_rules())
}
