//! Бенчмарк сборки (Б5): измеряем только то, что можно снять честно из
//! лаунчера — время загрузки игры (до появления меню/атласов в логе) и
//! потребление RAM/CPU процесса за сессию. FPS из процесса не достать:
//! для него нужен мод внутри игры, этот вариант предложен пользователю
//! отдельно и здесь не реализуется.
//!
//! Прогон: `start` (до запуска игры) → игра стартует обычным путём →
//! `on_log` ловит маркер готовности → сэмплер пишет RAM/CPU раз в 2 с →
//! `finish` на game-exit сохраняет отчёт в `<instance>/benchmarks.json`.

use crate::error::Res;
use crate::paths;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Один сэмпл ресурсов: RAM МБ, CPU %.
#[derive(Clone, Copy, Debug)]
struct Sample {
    ram_mb: u64,
    cpu_pct: f32,
}

/// Активный прогон.
struct Run {
    id: String,
    world: Option<String>,
    started: Instant,
    pid: u32,
    /// Время до готовности (меню/атласы), если маркер уже видели.
    loaded_ms: Option<u64>,
    samples: Vec<Sample>,
    sys: sysinfo::System,
}

static RUN: Lazy<Mutex<Option<Run>>> = Lazy::new(|| Mutex::new(None));

/// Маркеры готовности игры (меню почти готово): звуковой движок или атласы.
const LOAD_MARKERS: &[&str] = &[
    "Sound engine started",
    "Created: 1024x",
    "Created: 512x",
    "atlas/blocks.png",
];

/// Сохранённый прогон (в `benchmarks.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BenchRun {
    /// unix sec.
    pub date: i64,
    pub mc_version: String,
    pub loader: String,
    /// Число файлов в mods/.
    pub mods: u32,
    /// Мир запуска (quickPlay) или null — загрузка до меню.
    pub world: Option<String>,
    /// 0 — маркер готовности не увидели.
    pub load_ms: u64,
    pub ram_avg_mb: u64,
    pub ram_peak_mb: u64,
    pub cpu_avg: f32,
    pub cpu_peak: f32,
    pub samples: u32,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BenchFile {
    runs: Vec<BenchRun>,
}

/// Начинает прогон: вызывается до запуска игры. Один прогон за раз.
pub fn start(id: &str, world: Option<&str>) -> Res<()> {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return crate::error::err("Бенчмарк уже идёт");
    }
    *guard = Some(Run {
        id: id.to_string(),
        world: world.map(str::to_string),
        started: Instant::now(),
        pid: 0,
        loaded_ms: None,
        samples: Vec::new(),
        sys: sysinfo::System::new(),
    });
    Ok(())
}

/// Отменяет прогон без сохранения (запуск не получился).
pub fn cancel() {
    *RUN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Активен ли какой-нибудь прогон (для сэмплера).
pub fn active() -> bool {
    RUN.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Идёт ли сейчас бенчмарк этого профиля.
pub fn is_running(id: &str) -> bool {
    RUN.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|r| r.id == id)
}

/// PID процесса игры появился (вызывает конвейер запуска).
pub fn on_pid(id: &str, pid: u32) {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = guard.as_mut() {
        if r.id == id {
            r.pid = pid;
        }
    }
}

/// Строка лога игры: ищем маркер готовности.
pub fn on_log(id: &str, line: &str) {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = guard.as_mut() {
        if r.id == id && r.loaded_ms.is_none() && LOAD_MARKERS.iter().any(|m| line.contains(m)) {
            r.loaded_ms = Some(r.started.elapsed().as_millis() as u64);
        }
    }
}

/// Один тик сэмплера (вызывается из задачи с интервалом).
pub fn tick() {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(r) = guard.as_mut() else { return };
    if r.pid == 0 {
        return;
    }
    let pid = sysinfo::Pid::from_u32(r.pid);
    r.sys.refresh_process(pid);
    if let Some(p) = r.sys.process(pid) {
        r.samples.push(Sample {
            ram_mb: p.memory() / 1024 / 1024,
            cpu_pct: p.cpu_usage(),
        });
    }
}

/// Считает агрегаты из сэмплов (ram_mb, cpu_pct).
pub(crate) fn stats(samples: &[(u64, f32)]) -> (u64, u64, f32, f32) {
    if samples.is_empty() {
        return (0, 0, 0.0, 0.0);
    }
    let ram_avg = samples.iter().map(|s| s.0).sum::<u64>() / samples.len() as u64;
    let ram_peak = samples.iter().map(|s| s.0).max().unwrap_or(0);
    let cpu_avg = samples.iter().map(|s| s.1).sum::<f32>() / samples.len() as f32;
    let cpu_peak = samples.iter().map(|s| s.1).fold(0.0, f32::max);
    (ram_avg, ram_peak, cpu_avg, cpu_peak)
}

/// Итоги прогона (для сохранения).
fn summarize(r: &Run, mods: u32) -> BenchRun {
    let (ram_avg, ram_peak, cpu_avg, cpu_peak) =
        stats(&r.samples.iter().map(|s| (s.ram_mb, s.cpu_pct)).collect::<Vec<_>>());
    BenchRun {
        date: chrono::Utc::now().timestamp(),
        mc_version: String::new(),
        loader: String::new(),
        mods,
        world: r.world.clone(),
        load_ms: r.loaded_ms.unwrap_or(0),
        ram_avg_mb: ram_avg,
        ram_peak_mb: ram_peak,
        cpu_avg,
        cpu_peak,
        samples: r.samples.len() as u32,
    }
}

/// Завершает прогон: считает итоги, дописывает в benchmarks.json профиля.
pub fn finish(id: &str, mc_version: &str, loader: &str, mods: u32) -> Option<BenchRun> {
    let mut guard = RUN.lock().unwrap_or_else(|e| e.into_inner());
    let run = guard.take()?;
    if run.id != id {
        // Чужой game-exit: вернём прогон обратно.
        *guard = Some(run);
        return None;
    }
    let mut bench = summarize(&run, mods);
    bench.mc_version = mc_version.to_string();
    bench.loader = loader.to_string();
    // Ошибки записи не роняем: бенчмарк не критичен.
    let _ = save_run(id, &bench);
    Some(bench)
}

fn benchmarks_json(id: &str) -> PathBuf {
    paths::instance_dir(id).join("benchmarks.json")
}

fn save_run(id: &str, run: &BenchRun) -> Res<()> {
    let path = benchmarks_json(id);
    let mut file: BenchFile = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    file.runs.push(run.clone());
    file.runs.truncate(100);
    let data = serde_json::to_vec_pretty(&file)?;
    std::fs::write(&path, data)?;
    Ok(())
}

/// Прошлые прогоны профиля, свежие сверху.
pub fn list(id: &str) -> Res<Vec<BenchRun>> {
    let path = benchmarks_json(id);
    let file: BenchFile = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut runs = file.runs;
    runs.reverse();
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn marker_line_is_detected() {
        assert!(LOAD_MARKERS
            .iter()
            .any(|m| "[Render thread/INFO]: Sound engine started".contains(m)));
        assert!(LOAD_MARKERS.iter().any(|m| "[Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png".contains(m)));
        assert!(!LOAD_MARKERS
            .iter()
            .any(|m| "[main/INFO]: Loading Minecraft 1.21".contains(m)));
    }

    #[test]
    fn stats_average_and_peaks() {
        let s: Vec<(u64, f32)> = vec![(1000, 10.0), (2000, 30.0), (3000, 20.0)];
        let (ram_avg, ram_peak, cpu_avg, cpu_peak) = stats(&s);
        assert_eq!(ram_avg, 2000);
        assert_eq!(ram_peak, 3000);
        assert!((cpu_avg - 20.0).abs() < 0.01);
        assert!((cpu_peak - 30.0).abs() < 0.01);
        assert_eq!(stats(&[]), (0, 0, 0.0, 0.0));
    }

    /// Полный цикл прогона на временной папке без реальной игры.
    #[test]
    fn start_log_finish_round_trip() {
        let id = "b3ncht3st00000000000000000000000ff";
        let dir = paths::instance_dir(id);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert!(start(id, Some("bench_world")).is_ok());
        assert!(start(id, None).is_err(), "повторный старт запрещён");
        on_pid(id, 42);
        on_log(id, "[main/INFO]: Loading Minecraft");
        std::thread::sleep(Duration::from_millis(2));
        on_log(id, "[Render thread/INFO]: Sound engine started");
        on_log(id, "[Render thread/INFO]: Sound engine started");

        let done = finish(id, "1.21.1", "fabric", 12);
        let run = done.expect("run finished");
        assert_eq!(run.mc_version, "1.21.1");
        assert_eq!(run.mods, 12);
        assert!(run.load_ms >= 2, "маркер загрузки учтён: {}", run.load_ms);
        assert_eq!(run.ram_avg_mb, 0, "без сэмплов память нулевая");

        let listed = list(id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].loader, "fabric");

        // После завершения можно стартовать снова; отмена чистит состояние.
        assert!(start(id, None).is_ok());
        cancel();
        assert!(start(id, None).is_ok());
        cancel();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
