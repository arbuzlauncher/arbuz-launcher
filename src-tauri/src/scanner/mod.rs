//! Сканер модов: статический анализ jar-файлов профиля.
//!
//! Конвейер: чтение jar → улики (`evidence`) → правила (`rules`) → скоринг
//! с учётом режима → отчёт. Результаты кэшируются по SHA-256 (изменившийся
//! jar автоматически сканируется заново), хеш, найденный в каталогах, снижает
//! score, но hard-находки правил 1–5 не гасятся. Перед запуском игры стоит
//! «гейт»: в обычном режиме блокирует только danger, в усиленном — ещё и
//! моментальные красные флаги, в параноидальном — всё, кроме чистых известных
//! модов (заблокированные уходят в карантин, не удаляются).

pub(crate) mod ai_note;
pub(crate) mod cache;
pub(crate) mod classfile;
pub(crate) mod classindex;
pub(crate) mod config;
pub(crate) mod decode;
pub(crate) mod evidence;
pub(crate) mod lookup;
pub(crate) mod malware_db;
pub(crate) mod quarantine;
pub(crate) mod rules;

/// Генератор безобидных синтетических jar/class для юнит-тестов сканера.
#[cfg(test)]
pub(crate) mod synthetic;

use crate::error::{err, AppError, Res};
use crate::{paths, store};
use cache::{CacheEntry, FindingDto};
use config::{ScanMode, ScannerConfig};
use malware_db::MalwareDb;
use rules::{Finding, Neighbors, ScanContext};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Находки этих кодов блокируют запуск в усиленном режиме при любом score
/// (ТЗ: динамическая загрузка, нативные библиотеки, обфускация, java-агент).
const STRICT_BLOCK_CODES: &[&str] = &["r4_dynamic", "r6_native", "r7_obfuscation", "r12_agent"];

/// В параноидальном режиме эти коды дают danger (остальные находки —
/// suspicious, что тоже блокирует запуск).
const PARANOID_DANGER_CODES: &[&str] = &[
    "r4_dynamic", "r6_native", "r7_obfuscation", "r9_domain", "r12_agent", "r13_infect",
    "r14_system_load", "r20_session_mixin", "r21_override",
];

/// Префиксы классов, которые мод не должен «перекрывать» (правило 21).
const OVERRIDE_PREFIXES: &[&str] = &["net/minecraft/", "com/mojang/"];

/// Максимум имён классов, уходящих в кэш (для правил 21).
const MAX_CACHED_CLASSES: usize = 20_000;

/// Одновременных запросов к каталогам при проверке хешей.
const LOOKUP_CONCURRENCY: usize = 8;

// ---------------------------------------------------------------------------
// Отчёт
// ---------------------------------------------------------------------------

/// Уровень отчёта по одному файлу.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Suspicious,
    Danger,
}

impl Level {
    pub fn label(&self) -> &'static str {
        match self {
            Level::Ok => "чисто",
            Level::Suspicious => "подозрительно",
            Level::Danger => "опасно",
        }
    }
}

/// Отчёт сканирования одного файла контента.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModReport {
    /// Относительный путь от `minecraft/`: `mods/foo.jar`.
    pub file: String,
    /// Имя файла без расширения (имя мода из метаданных недоступно после
    /// кэширования — интерфейс подставит своё из списка контента).
    pub name: String,
    pub sha256: String,
    /// 0..100 после бонусов/штрафов режима.
    pub score: u32,
    pub level: Level,
    pub findings: Vec<FindingDto>,
    /// Хеш найден в Modrinth/CurseForge.
    pub known_hash: bool,
    pub modrinth_project: Option<String>,
    /// Каталоги были недоступны — «хеш не найден» может быть неточным.
    pub offline_lookup: bool,
    /// Файл в ручных исключениях профиля: гейт его не блокирует.
    pub allowed: bool,
}

// ---------------------------------------------------------------------------
// Скоринг
// ---------------------------------------------------------------------------

/// Считает score и уровень по находкам с учётом режима.
pub(crate) fn score(
    findings: &[FindingDto],
    known_hash: bool,
    mode: ScanMode,
    cfg: &ScannerConfig,
) -> (u32, Level) {
    let t = mode.thresholds(cfg);
    let hard_sum: i32 = findings.iter().filter(|f| f.hard).map(|f| f.weight).sum();
    let soft_sum: i32 = findings.iter().filter(|f| !f.hard).map(|f| f.weight).sum();

    let mut total = hard_sum + soft_sum;
    if known_hash {
        // Хеш из каталога снижает score, но не ниже суммы hard-находок:
        // правила 1–5 не гасятся известностью мода.
        total -= t.modrinth_known_bonus;
        total = total.max(hard_sum);
    } else {
        total += t.modrinth_unknown_penalty;
    }
    let total = total.clamp(0, 100) as u32;

    let level = match mode {
        ScanMode::Paranoid => {
            if findings.is_empty() && known_hash {
                Level::Ok
            } else if !known_hash
                || findings
                    .iter()
                    .any(|f| f.hard || PARANOID_DANGER_CODES.contains(&f.code.as_str()))
            {
                Level::Danger
            } else {
                Level::Suspicious
            }
        }
        ScanMode::Strict => {
            if findings.iter().any(|f| STRICT_BLOCK_CODES.contains(&f.code.as_str()))
                || total >= t.danger as u32
            {
                Level::Danger
            } else if total >= t.suspicious as u32 {
                Level::Suspicious
            } else {
                Level::Ok
            }
        }
        ScanMode::Normal => {
            if total >= t.danger as u32 {
                Level::Danger
            } else if total >= t.suspicious as u32 {
                Level::Suspicious
            } else {
                Level::Ok
            }
        }
    };
    (total, level)
}

// ---------------------------------------------------------------------------
// Конфиги и списки
// ---------------------------------------------------------------------------

/// Читает `scanner.json` из корня данных (веса/пороги). Нет файла — дефолт.
async fn load_config() -> ScannerConfig {
    let mut cfg: ScannerConfig = store::read_json(&paths::scanner_config_file())
        .await
        .unwrap_or_default();
    if cfg.whitelist_domains.is_empty() {
        cfg = ScannerConfig::with_default_whitelist();
    }
    cfg.sanitize();
    cfg
}

/// Режим сканера: переопределение инстанса важнее глобальной настройки.
pub(crate) fn effective_mode(global: &str, instance: Option<&str>) -> ScanMode {
    let raw = instance.filter(|s| !s.is_empty()).unwrap_or(global);
    match raw {
        "strict" => ScanMode::Strict,
        "paranoid" => ScanMode::Paranoid,
        _ => ScanMode::Normal,
    }
}

/// Список ручных исключений инстанса: `scanner.json` в папке профиля.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct InstanceScanner {
    /// Разрешённые файлы: `mods/foo.jar`.
    allowed: BTreeSet<String>,
}

async fn load_allowed(id: &str) -> BTreeSet<String> {
    let sc: InstanceScanner = store::read_json(&paths::instance_scanner_json(id))
        .await
        .unwrap_or_default();
    sc.allowed
}

/// Добавляет или убирает файл из ручных исключений инстанса.
pub async fn allow_mod(id: &str, file: &str, allow: bool) -> Res<()> {
    paths::validate_instance_id(id)?;
    let file = file.replace('\\', "/");
    if file.is_empty() || file.contains("..") {
        return err("Некорректный путь файла");
    }
    let mut sc: InstanceScanner = store::read_json(&paths::instance_scanner_json(id))
        .await
        .unwrap_or_default();
    if allow {
        sc.allowed.insert(file);
    } else {
        sc.allowed.remove(&file);
    }
    store::write_json_atomic(&paths::instance_scanner_json(id), &sc).await
}

// ---------------------------------------------------------------------------
// Сканирование
// ---------------------------------------------------------------------------

/// Один просканированный файл до скоринга.
struct Scanned {
    entry: CacheEntry,
    /// Пришёл из кэша (хеши в каталогах уже проверены).
    from_cache: bool,
    /// Каталоги были недоступны при проверке хеша.
    offline: bool,
    /// Находки правил 21 — считаются по всем jar, не кэшируются.
    neighbor_findings: Vec<Finding>,
}

/// Расширения, которые сканируем как архивы.
fn is_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.ends_with(".jar") || lower.ends_with(".zip")) && !lower.ends_with(".disabled")
}

/// Собирает архивы из папки: относительный путь от `game` -> абсолютный.
fn collect_archives(dir: &Path, game: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_archive(name) {
            if let Ok(rel) = p.strip_prefix(game) {
                out.push((crate::content::rel_slash(rel), p));
            }
        }
    }
}

/// Сканирует архивы перечисленных папок и возвращает отчёты под режим.
/// `do_lookup = false` пропускает проверку хешей в каталогах (тесты).
async fn scan_dirs(
    http: &reqwest::Client,
    game: &Path,
    dirs: &[PathBuf],
    cfg: &ScannerConfig,
    db: &MalwareDb,
    cf_key: Option<&str>,
    cache_dir: &Path,
    mode: ScanMode,
    do_lookup: bool,
) -> Vec<ModReport> {
    // 1. Список файлов: сами папки + датапаки внутри saves/<мир>/datapacks.
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for d in dirs {
        collect_archives(d, game, &mut files);
        if !d.ends_with("datapacks") {
            let saves = game.join("saves");
            if let Ok(rd) = std::fs::read_dir(&saves) {
                for w in rd.flatten() {
                    collect_archives(&w.path().join("datapacks"), game, &mut files);
                }
            }
        }
    }
    files.sort();

    // 2. Чтение + хеши + улики + правила (без 21); кэш — без повторного
    //    разбора. Тяжёлое — в blocking-потоке, по одному файлу за раз.
    let mut scanned: Vec<Scanned> = Vec::new();
    for (rel, path) in &files {
        let lim = cfg.limits;
        let p = path.clone();
        let rel2 = rel.clone();
        let db2 = db.clone();
        let cfg2 = cfg.clone();
        let cache_dir2 = cache_dir.to_path_buf();
        let done = tokio::task::spawn_blocking(move || -> Scanned {
            let Ok(bytes) = std::fs::read(&p) else {
                return Scanned {
                    entry: CacheEntry::from_findings(&rel2, "", "", Vec::new(), Vec::new()),
                    from_cache: false,
                    offline: false,
                    neighbor_findings: Vec::new(),
                };
            };
            let (sha256, sha1) = cache::hashes(&bytes);
            if let Some(mut entry) = cache::load(&cache_dir2, &sha256) {
                entry.file = rel2.clone();
                return Scanned {
                    entry,
                    from_cache: true,
                    offline: false,
                    neighbor_findings: Vec::new(),
                };
            }
            let ev = evidence::evidence_from_bytes(&bytes, &lim);
            let ctx = ScanContext {
                malware_indicators: db2.indicators.clone(),
                known_malware: db2.hashes.iter().any(|h| h == &sha256),
                neighbors: None,
            };
            let findings = rules::analyze(&ev, &ctx, &cfg2);
            let mut classes: Vec<String> = ev.classes.iter().map(|c| c.name.clone()).collect();
            classes.truncate(MAX_CACHED_CLASSES);
            let entry = CacheEntry::from_findings(
                &rel2,
                &sha256,
                &sha1,
                findings.iter().map(|f| f.to_dto()).collect(),
                classes,
            );
            cache::save(&cache_dir2, &entry);
            Scanned {
                entry,
                from_cache: false,
                offline: false,
                neighbor_findings: Vec::new(),
            }
        })
        .await;
        if let Ok(s) = done {
            scanned.push(s);
        }
    }

    // 3. Проверка хешей в каталогах — только для свежепросканированных.
    if do_lookup {
        let idx: Vec<usize> = scanned
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.from_cache && !s.entry.sha256.is_empty())
            .map(|(i, _)| i)
            .collect();
        use futures_util::stream::{self, StreamExt};
        let results: Vec<(usize, lookup::LookupResult)> = stream::iter(idx)
            .map(|i| {
                let e = &scanned[i].entry;
                let sha1 = e.sha1.clone();
                let key = cf_key.map(str::to_string);
                async move {
                    let r = lookup::by_hash(http, &sha1, key.as_deref()).await;
                    (i, r)
                }
            })
            .buffer_unordered(LOOKUP_CONCURRENCY)
            .collect()
            .await;
        for (i, r) in results {
            {
                let e = &mut scanned[i].entry;
                e.modrinth_found = r.modrinth_found;
                e.modrinth_project = r.modrinth_project.clone();
                e.curseforge_found = r.curseforge_found;
            }
            scanned[i].offline = r.offline;
            cache::save(cache_dir, &scanned[i].entry.clone());
        }
    }

    // 4. Правила 21: дубли классов между модами и перекрытие классов игры.
    let mut owner: HashMap<&str, usize> = HashMap::new();
    let mut dupes: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); scanned.len()];
    let mut overrides: Vec<BTreeSet<String>> = vec![BTreeSet::new(); scanned.len()];
    for (i, s) in scanned.iter().enumerate() {
        for class in &s.entry.classes {
            if OVERRIDE_PREFIXES.iter().any(|p| class.starts_with(p)) {
                overrides[i].insert(class.clone());
            }
            if let Some(&j) = owner.get(class.as_str()) {
                dupes[i].insert(j);
                dupes[j].insert(i);
            } else {
                owner.insert(class.as_str(), i);
            }
        }
    }
    let mut neighbor_findings: Vec<Vec<Finding>> = vec![Vec::new(); scanned.len()];
    for (i, s) in scanned.iter().enumerate() {
        let mut nb = Neighbors::default();
        for j in &dupes[i] {
            for c in &scanned[*j].entry.classes {
                if s.entry.classes.contains(c) && !nb.duplicated.iter().any(|(x, _)| x == c) {
                    nb.duplicated.push((c.clone(), scanned[*j].entry.file.clone()));
                    if nb.duplicated.len() >= 3 {
                        break;
                    }
                }
            }
            if nb.duplicated.len() >= 3 {
                break;
            }
        }
        nb.overrides = overrides[i].iter().take(3).cloned().collect();
        if !nb.duplicated.is_empty() || !nb.overrides.is_empty() {
            let ctx = ScanContext {
                malware_indicators: Vec::new(),
                known_malware: false,
                neighbors: Some(nb),
            };
            // Пустые улики: сработают только правила 21 по соседям.
            let ev = evidence::JarEvidence::default();
            neighbor_findings[i] = rules::analyze(&ev, &ctx, cfg);
        }
    }
    for (i, s) in scanned.iter_mut().enumerate() {
        s.neighbor_findings = std::mem::take(&mut neighbor_findings[i]);
    }

    // 5. Скоринг и отчёты.
    scanned
        .into_iter()
        .map(|s| {
            let known = s.entry.modrinth_found || s.entry.curseforge_found;
            let mut findings = s.entry.findings.clone();
            findings.extend(s.neighbor_findings.iter().map(|f| f.to_dto()));
            let (total, level) = score(&findings, known, mode, cfg);
            let name = s
                .entry
                .file
                .rsplit('/')
                .next()
                .unwrap_or(&s.entry.file)
                .trim_end_matches(".jar")
                .trim_end_matches(".zip")
                .to_string();
            ModReport {
                file: s.entry.file.clone(),
                name,
                sha256: s.entry.sha256.clone(),
                score: total,
                level,
                findings,
                known_hash: known,
                modrinth_project: s.entry.modrinth_project.clone(),
                offline_lookup: s.offline,
                allowed: false,
            }
        })
        .collect()
}

/// Полный скан контента профиля под режим (глобальный + переопределение
/// инстанса). Используется командой для интерфейса и гейтом запуска.
pub async fn scan_instance(
    state: &crate::state::AppState,
    id: &str,
) -> Res<Vec<ModReport>> {
    paths::validate_instance_id(id)?;
    let inst = store::read_instance(id).await?;
    let (global_mode, cf_key) = {
        let g = state.config.lock().await;
        (
            g.scanner_mode.clone(),
            if g.curseforge_api_key.is_empty() {
                None
            } else {
                Some(g.curseforge_api_key.clone())
            },
        )
    };
    let mode = effective_mode(&global_mode, inst.scanner_mode.as_deref());

    let cfg = load_config().await;
    let db_path = paths::malware_db_file();
    let db = tokio::task::spawn_blocking(move || malware_db::load(&db_path)).await?;
    let game = paths::instance_minecraft_dir(id);

    let mut dirs = vec![game.join("mods")];
    if mode != ScanMode::Normal {
        dirs.push(game.join("config"));
        dirs.push(game.join("resourcepacks"));
        dirs.push(game.join("shaderpacks"));
        dirs.push(game.join("datapacks"));
    }

    let mut reports = scan_dirs(
        &state.http,
        &game,
        &dirs,
        &cfg,
        &db,
        cf_key.as_deref(),
        &paths::scanner_cache_dir(),
        mode,
        true,
    )
    .await;
    let allowed = load_allowed(id).await;
    for r in &mut reports {
        r.allowed = allowed.contains(&r.file);
    }
    reports.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.file.cmp(&b.file)));
    Ok(reports)
}

/// Гейт перед запуском: сканирует контент и решает, пускать ли игру.
/// `Err` — запуск остановлен с перечнем файлов; в параноидальном режиме
/// заблокированные файлы переносятся в карантин.
pub async fn launch_gate(state: &crate::state::AppState, id: &str) -> Res<()> {
    let enabled = state.config.lock().await.scanner_enabled;
    if !enabled {
        return Ok(());
    }
    let reports = scan_instance(state, id).await?;
    let mode = {
        let g = state.config.lock().await;
        let inst_mode = store::read_instance(id).await.ok().and_then(|i| i.scanner_mode);
        effective_mode(&g.scanner_mode, inst_mode.as_deref())
    };
    let allowed: BTreeSet<String> = load_allowed(id).await;

    let blocked: Vec<&ModReport> = reports
        .iter()
        .filter(|r| {
            !allowed.contains(&r.file)
                && r.level != Level::Ok
                && (mode == ScanMode::Paranoid || r.level == Level::Danger)
        })
        .collect();
    if blocked.is_empty() {
        return Ok(());
    }

    // Параноидальный режим: переносим заблокированное в карантин.
    if mode == ScanMode::Paranoid {
        let game = paths::instance_minecraft_dir(id);
        let qdir = paths::instance_quarantine_dir(id);
        for b in &blocked {
            let src = game.join(&b.file);
            if src.is_file() {
                let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("mod.jar");
                let _ = quarantine::move_file(&src, &qdir, name);
            }
        }
    }

    let list: Vec<String> = blocked
        .iter()
        .map(|b| format!("{} — {}, score {}", b.name, b.level.label(), b.score))
        .collect();
    let note = if mode == ScanMode::Paranoid {
        " Файлы перенесены в папку quarantine профиля."
    } else {
        ""
    };
    Err(AppError::Msg(format!(
        "Запуск остановлен сканером модов: {}.{} Подробности — во вкладке «Сканер».",
        list.join(", "),
        note
    )))
}

// ---------------------------------------------------------------------------
// Карантин: обёртки для команд
// ---------------------------------------------------------------------------

pub async fn quarantine_list(id: &str) -> Res<Vec<String>> {
    paths::validate_instance_id(id)?;
    Ok(quarantine::list(&paths::instance_quarantine_dir(id)))
}

pub async fn quarantine_restore(id: &str, name: &str) -> Res<()> {
    paths::validate_instance_id(id)?;
    quarantine::restore(
        &paths::instance_quarantine_dir(id),
        &paths::instance_minecraft_dir(id).join("mods"),
        name,
    )
}

pub async fn quarantine_delete(id: &str, name: &str) -> Res<()> {
    paths::validate_instance_id(id)?;
    quarantine::remove(&paths::instance_quarantine_dir(id), name)
}

impl Finding {
    /// DTO для кэша и IPC.
    pub(crate) fn to_dto(&self) -> FindingDto {
        FindingDto {
            rule: self.rule.to_string(),
            code: self.code.to_string(),
            weight: self.weight,
            hard: self.hard,
            place: self.place.clone(),
            what: self.what.clone(),
        }
    }
}

/// ИИ-заметка по одному моду: только для suspicious/danger и только при
/// включённом переключателе. Статический вердикт не меняется.
pub async fn ai_note_for(
    state: &crate::state::AppState,
    id: &str,
    file: &str,
    lang: &str,
) -> Res<ai_note::AiNote> {
    let ai_on = state.config.lock().await.ai_scanner;
    if !ai_on {
        return err("ИИ-этап сканера выключен в настройках");
    }
    let reports = scan_instance(state, id).await?;
    let r = reports
        .iter()
        .find(|r| r.file == file)
        .ok_or_else(|| AppError::Msg("Файл не найден в отчёте сканера".into()))?;
    if r.level == Level::Ok {
        return err("ИИ-заметка доступна только для подозрительных файлов");
    }
    let findings: Vec<(String, String, i32, String)> = r
        .findings
        .iter()
        .map(|f| (f.rule.clone(), f.what.clone(), f.weight, f.place.clone()))
        .collect();
    ai_note::note(
        &r.file,
        &r.sha256,
        r.known_hash,
        r.score,
        r.level.label(),
        &findings,
        lang,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::config::ScannerConfig;
    use super::synthetic::{jar, ClassBuilder};
    use super::*;

    fn dto(rule: &str, code: &str, weight: i32, hard: bool) -> FindingDto {
        FindingDto {
            rule: rule.into(),
            code: code.into(),
            weight,
            hard,
            place: "x".into(),
            what: "тест".into(),
        }
    }

    #[test]
    fn normal_mode_score_with_modrinth_bonus() {
        let cfg = ScannerConfig::with_default_whitelist();
        // Мягкая находка 30 + известный хеш: 30-50 -> 0, suspicious (35) не достигнут.
        let (sc, lv) = score(&[dto("R9", "r9_domain", 30, false)], true, ScanMode::Normal, &cfg);
        assert_eq!(sc, 0);
        assert_eq!(lv, Level::Ok);
    }

    #[test]
    fn hard_findings_are_not_canceled_by_modrinth() {
        let cfg = ScannerConfig::with_default_whitelist();
        let (sc, lv) = score(&[dto("R1", "r1_exfil", 45, true)], true, ScanMode::Normal, &cfg);
        assert_eq!(sc, 45);
        assert_eq!(lv, Level::Suspicious);
        // Две hard-находки = 85 -> danger даже при известном хеше.
        let (sc2, lv2) = score(
            &[dto("R1", "r1_exfil", 45, true), dto("R2", "r2_theft", 40, true)],
            true,
            ScanMode::Normal,
            &cfg,
        );
        assert_eq!(sc2, 85);
        assert_eq!(lv2, Level::Danger);
    }

    #[test]
    fn strict_mode_instant_blocks_and_unknown_penalty() {
        let cfg = ScannerConfig::with_default_whitelist();
        // Нативная библиотека — блок при любой находке, даже известного мода.
        let (_, lv) = score(&[dto("R6", "r6_native", 20, false)], true, ScanMode::Strict, &cfg);
        assert_eq!(lv, Level::Danger);
        // Неизвестный хеш: 6 + 20 = 26 — ниже порогов.
        let (sc, lv) = score(&[dto("R9", "r9_domain", 6, false)], false, ScanMode::Strict, &cfg);
        assert_eq!(sc, 26);
        assert_eq!(lv, Level::Ok);
    }

    #[test]
    fn paranoid_requires_known_clean_hash() {
        let cfg = ScannerConfig::with_default_whitelist();
        assert_eq!(score(&[], true, ScanMode::Paranoid, &cfg).1, Level::Ok);
        // Неизвестный даже чистый мод заблокирован.
        assert_eq!(score(&[], false, ScanMode::Paranoid, &cfg).1, Level::Danger);
        // Известный с мягкой находкой — не ok.
        assert_eq!(
            score(&[dto("R16", "r16_fingerprint", 8, false)], true, ScanMode::Paranoid, &cfg).1,
            Level::Suspicious
        );
        // Известный с красным флагом — danger.
        assert_eq!(
            score(&[dto("R4", "r4_dynamic", 30, false)], true, ScanMode::Paranoid, &cfg).1,
            Level::Danger
        );
    }

    #[test]
    fn effective_mode_prefers_instance() {
        assert_eq!(effective_mode("normal", Some("strict")), ScanMode::Strict);
        assert_eq!(effective_mode("paranoid", None), ScanMode::Paranoid);
        assert_eq!(effective_mode("paranoid", Some("")), ScanMode::Paranoid);
        assert_eq!(effective_mode("мусор", None), ScanMode::Normal);
    }

    /// Полный конвейер на временной папке: чистый мод, опасный мод и кэш.
    /// Сеть выключена (`do_lookup = false`) — тест не ходит наружу.
    #[tokio::test]
    async fn scan_dirs_reports_and_caches() {
        let dir = std::env::temp_dir().join(format!("arbuz-scan-full-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let game = dir.join("minecraft");
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();

        let clean = jar(&[
            (
                "fabric.mod.json".into(),
                r#"{"id":"good","version":"1.0","description":"хороший"}"#.as_bytes().to_vec(),
            ),
            ("com/good/Main.class".into(), ClassBuilder::new("com/good/Main").build()),
        ]);
        std::fs::write(mods.join("good-mod.jar"), clean).unwrap();

        let evil_cls = ClassBuilder::new("com/evil/E")
            .string("https://discord.com/api/webhooks/1/abc")
            .method_ref("java/lang/Runtime", "exec", "(Ljava/lang/String;)Ljava/lang/Process;")
            .build();
        let evil = jar(&[
            (
                "fabric.mod.json".into(),
                r#"{"id":"evil","version":"1.0","description":"плохой"}"#.as_bytes().to_vec(),
            ),
            ("com/evil/E.class".into(), evil_cls),
        ]);
        std::fs::write(mods.join("evil-mod.jar"), evil).unwrap();

        let http = reqwest::Client::new();
        let cfg = ScannerConfig::with_default_whitelist();
        let db = MalwareDb::default();
        let cache_dir = dir.join("cache");
        let reports = scan_dirs(&http, &game, &[mods.clone()], &cfg, &db, None, &cache_dir, ScanMode::Normal, false).await;

        assert_eq!(reports.len(), 2, "оба мода в отчёте");
        let evil_r = reports.iter().find(|r| r.file.contains("evil")).expect("evil");
        let good_r = reports.iter().find(|r| r.file.contains("good")).expect("good");
        assert!(evil_r.findings.iter().any(|f| f.rule == "R1" && f.hard));
        assert!(evil_r.findings.iter().any(|f| f.rule == "R3"));
        // Хешей в каталогах не проверяли -> обычный режим без штрафа.
        assert!(good_r.findings.is_empty(), "чистый мод без находок: {:?}", good_r.findings);
        assert_eq!(good_r.level, Level::Ok);

        // Повторный прогон берёт находки из кэша по sha256.
        let reports2 = scan_dirs(&http, &game, &[mods], &cfg, &db, None, &cache_dir, ScanMode::Normal, false).await;
        let evil2 = reports2.iter().find(|r| r.file.contains("evil")).expect("evil2");
        assert!(evil2.findings.iter().any(|f| f.rule == "R1" && f.hard));
        assert_eq!(evil2.sha256, evil_r.sha256);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Изменившийся jar меняет sha256 -> кэш не попадает.
    #[tokio::test]
    async fn changed_jar_is_rescanned() {
        let dir = std::env::temp_dir().join(format!("arbuz-scan-chg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let game = dir.join("minecraft");
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();

        let v1 = jar(&[("fabric.mod.json".into(), br#"{"id":"x","description":"d"}"#.to_vec())]);
        std::fs::write(mods.join("x.jar"), v1).unwrap();
        let http = reqwest::Client::new();
        let cfg = ScannerConfig::with_default_whitelist();
        let cache_dir = dir.join("cache");
        let r1 = scan_dirs(&http, &game, &[mods.clone()], &cfg, &MalwareDb::default(), None, &cache_dir, ScanMode::Normal, false).await;
        let first_sha = r1[0].sha256.clone();

        let v2 = jar(&[("fabric.mod.json".into(), br#"{"id":"x","description":"d2"}"#.to_vec())]);
        std::fs::write(mods.join("x.jar"), v2).unwrap();
        let r2 = scan_dirs(&http, &game, &[mods], &cfg, &MalwareDb::default(), None, &cache_dir, ScanMode::Normal, false).await;
        assert_ne!(r2[0].sha256, first_sha);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Отладка на реальном jar: ARBUZ_SCAN_JAR=/путь/к/mod.jar cargo test
    /// dump_real_jar -- --ignored --nocapture. Показывает все находки правил.
    #[tokio::test]
    #[ignore = "нужен путь к jar в ARBUZ_SCAN_JAR"]
    async fn dump_real_jar() {
        let path = std::env::var("ARBUZ_SCAN_JAR").expect("ARBUZ_SCAN_JAR");
        let bytes = std::fs::read(&path).expect("read jar");
        let cfg = ScannerConfig::with_default_whitelist();
        let ev = evidence::evidence_from_bytes(&bytes, &cfg.limits);
        let ctx = ScanContext::default();
        let findings = rules::analyze(&ev, &ctx, &cfg);
        println!("== {} : классов {}, записей {}", path, ev.classes.len(), ev.entries.len());
        let sum: i32 = findings.iter().map(|f| f.weight).sum();
        for f in &findings {
            println!("{:>3}  {}  [{}]  {}  @ {}", f.weight, f.rule, f.code, f.what, f.place);
        }
        println!("== сумма весов: {sum}");
    }
}
