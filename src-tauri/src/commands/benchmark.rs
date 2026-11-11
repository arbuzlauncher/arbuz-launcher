use super::*;

// ---------------------------------------------------------------------------
// Бенчмарк сборки (Б5)
// ---------------------------------------------------------------------------

/// Начинает прогон бенчмарка (до запуска игры) и сэмплер ресурсов.
#[tauri::command]
pub async fn benchmark_start(id: String, world: Option<String>) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::plugins::hook(
        "benchmark.start.before",
        serde_json::json!({ "id": id, "world": world }),
    );
    crate::benchmark::start(&id, world.as_deref())?;
    crate::plugins::hook("benchmark.start.after", serde_json::json!({ "id": id }));
    tokio::spawn(async move {
        // Тик раз в 2 секунды, пока прогон жив.
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if !crate::benchmark::active() {
                return;
            }
            crate::benchmark::tick();
        }
    });
    Ok(())
}

/// Отменяет прогон (запуск игры не удался).
#[tauri::command]
pub fn benchmark_cancel() {
    crate::plugins::hook("benchmark.cancel", serde_json::json!({}));
    crate::benchmark::cancel();
}

/// Прошлые прогоны профиля, свежие сверху.
#[tauri::command]
pub fn benchmark_list(id: String) -> Res<Vec<crate::benchmark::BenchRun>> {
    paths::validate_instance_id(&id)?;
    let list = crate::benchmark::list(&id)?;
    crate::plugins::hook(
        "benchmark.list",
        serde_json::json!({ "id": id, "count": list.len() }),
    );
    Ok(list)
}

#[tauri::command]
pub fn game_monitor(state: State<'_, AppState>, id: String) -> MonitorSample {
    let Some(pid) = state.game_pids.blocking_lock().get(&id).copied() else {
        return MonitorSample { pid: 0, found: false, ram_mb: 0, cpu_pct: 0.0 };
    };
    let pid = sysinfo::Pid::from_u32(pid);
    // Один System на процесс: cpu_usage считается от прошлого опроса.
    let mut sys = state.monitor.lock().unwrap_or_else(|e| e.into_inner());
    sys.refresh_process(pid);
    match sys.process(pid) {
        Some(p) => MonitorSample {
            pid: p.pid().as_u32(),
            found: true,
            ram_mb: p.memory() / 1024 / 1024,
            cpu_pct: p.cpu_usage(),
        },
        None => MonitorSample { pid: pid.as_u32(), found: false, ram_mb: 0, cpu_pct: 0.0 },
    }
}

/// Окончательно удаляет файл из карантина.
#[tauri::command]
pub async fn scanner_quarantine_delete(id: String, name: String) -> Res<()> {
    crate::plugins::hook(
        "scanner.quarantine.delete.before",
        serde_json::json!({ "id": id, "file": name }),
    );
    crate::scanner::quarantine_delete(&id, &name).await?;
    crate::plugins::hook(
        "scanner.quarantine.delete.after",
        serde_json::json!({ "id": id, "file": name }),
    );
    Ok(())
}
