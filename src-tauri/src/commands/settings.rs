use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 1
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Res<Config> {
    // Читаем под тем же мьютексом, под которым пишет set_config, иначе
    // параллельный вызов может вернуть в память устаревший снимок.
    let mut guard = state.config.lock().await;
    let cfg = read_config().await?;
    crate::plugins::hook("config.load", serde_json::json!({ "config": &cfg }));
    *guard = cfg.clone();
    state
        .launcher_accel
        .store(cfg.launcher_accel, std::sync::atomic::Ordering::Relaxed);
    crate::state::set_manual_proxy(&cfg);
    crate::ai::set_settings(&cfg);
    // Ключ CurseForge, пароль прокси и API-ключ ИИ в webview не отдаём — только маску.
    let mut shown = cfg;
    if !shown.curseforge_api_key.is_empty() {
        shown.curseforge_api_key = CF_KEY_MASK.to_string();
    }
    if !shown.proxy_pass.is_empty() {
        shown.proxy_pass = PROXY_PASS_MASK.to_string();
    }
    if !shown.ai_api_key.is_empty() {
        shown.ai_api_key = AI_KEY_MASK.to_string();
    }
    Ok(shown)
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, mut cfg: Config) -> Res<()> {
    // Значения из webview приводим к безопасным диапазонам до записи на диск.
    cfg.sanitize();
    // Плагины могут изменить конфиг до сохранения (например, включить свою тему).
    let hook = crate::plugins::call_hook(
        "config.before_save",
        &serde_json::to_value(&cfg).unwrap_or_default(),
    );
    for r in hook {
        if r.error.is_some() {
            continue;
        }
        if r.result.get("cancel").and_then(|v| v.as_bool()).unwrap_or(false) {
            return err("Сохранение настроек отменено плагином");
        }
        if let Some(v) = r.result.get("config") {
            if let Ok(mut next) = serde_json::from_value::<Config>(v.clone()) {
                next.sanitize();
                cfg = next;
            }
        }
    }
    let mut guard = state.config.lock().await;
    // Пришла маска — значит ключ не меняли, оставляем сохранённый.
    if cfg.curseforge_api_key == CF_KEY_MASK {
        cfg.curseforge_api_key = if guard.curseforge_api_key.is_empty() {
            read_config().await?.curseforge_api_key
        } else {
            guard.curseforge_api_key.clone()
        };
    }
    if cfg.proxy_pass == PROXY_PASS_MASK {
        cfg.proxy_pass = if guard.proxy_pass.is_empty() {
            read_config().await?.proxy_pass
        } else {
            guard.proxy_pass.clone()
        };
    }
    // Маска API-ключа ИИ — значит ключ не меняли.
    if cfg.ai_api_key == AI_KEY_MASK {
        cfg.ai_api_key = if guard.ai_api_key.is_empty() {
            read_config().await?.ai_api_key
        } else {
            guard.ai_api_key.clone()
        };
    }
    store::write_json_atomic(&paths::config_file(), &cfg).await?;
    state
        .launcher_accel
        .store(cfg.launcher_accel, std::sync::atomic::Ordering::Relaxed);
    crate::state::set_manual_proxy(&cfg);
    crate::ai::set_settings(&cfg);
    let was_plugins_enabled = guard.plugins_enabled;
    *guard = cfg;
    let now_plugins_enabled = guard.plugins_enabled;
    let snapshot = serde_json::to_value(&*guard).unwrap_or_default();
    drop(guard);
    let _ = crate::plugins::call_hook("config.after_save", &snapshot);
    crate::plugins::hook("config.changed", snapshot);
    if was_plugins_enabled != now_plugins_enabled {
        crate::plugins::manager::reload(now_plugins_enabled);
        crate::plugins::runtime::emit(
            "plugins.changed",
            serde_json::json!({ "action": "global" }),
        );
    }
    Ok(())
}

/// Экспорт настроек в JSON. Ключ CurseForge, пароль прокси и API-ключ ИИ в файл не попадают.
#[tauri::command]
pub async fn export_settings(state: State<'_, AppState>, path: String) -> Res<()> {
    let mut cfg = state.config.lock().await.clone();
    cfg.curseforge_api_key = CF_KEY_MASK.to_string();
    if !cfg.proxy_pass.is_empty() {
        cfg.proxy_pass = PROXY_PASS_MASK.to_string();
    }
    if !cfg.ai_api_key.is_empty() {
        cfg.ai_api_key = AI_KEY_MASK.to_string();
    }
    let p = settings_path(&path)?;
    crate::plugins::hook("config.export.before", serde_json::json!({ "path": p.to_string_lossy() }));
    let data = serde_json::to_vec_pretty(&cfg)?;
    tokio::fs::write(&p, data).await?;
    crate::plugins::hook("config.export.after", serde_json::json!({ "path": p.to_string_lossy() }));
    Ok(())
}

/// Импорт настроек из JSON. Возвращает применённый конфиг.
#[tauri::command]
pub async fn import_settings(state: State<'_, AppState>, path: String) -> Res<Config> {
    let p = settings_path(&path)?;
    let meta = tokio::fs::metadata(&p).await?;
    if meta.len() > 1024 * 1024 {
        return err("Файл настроек слишком большой");
    }
    let text = tokio::fs::read_to_string(&p).await?;
    crate::plugins::hook("config.import.before", serde_json::json!({ "path": p.to_string_lossy() }));
    let mut cfg: Config = serde_json::from_str(&text)
        .map_err(|e| AppError::Msg(format!("Это не файл настроек Arbuz Launcher: {e}")))?;
    cfg.sanitize();
    // Маска вместо ключа или пароля — оставляем текущие значения.
    if cfg.curseforge_api_key == CF_KEY_MASK {
        cfg.curseforge_api_key = state.config.lock().await.curseforge_api_key.clone();
    }
    if cfg.proxy_pass == PROXY_PASS_MASK {
        cfg.proxy_pass = state.config.lock().await.proxy_pass.clone();
    }
    if cfg.ai_api_key == AI_KEY_MASK {
        cfg.ai_api_key = state.config.lock().await.ai_api_key.clone();
    }
    // Визард знакомства после импорта не нужен.
    cfg.onboarded = true;
    store::write_json_atomic(&paths::config_file(), &cfg).await?;
    state
        .launcher_accel
        .store(cfg.launcher_accel, std::sync::atomic::Ordering::Relaxed);
    crate::state::set_manual_proxy(&cfg);
    crate::ai::set_settings(&cfg);
    *state.config.lock().await = cfg.clone();
    let mut shown = cfg;
    if !shown.curseforge_api_key.is_empty() {
        shown.curseforge_api_key = CF_KEY_MASK.to_string();
    }
    if !shown.proxy_pass.is_empty() {
        shown.proxy_pass = PROXY_PASS_MASK.to_string();
    }
    if !shown.ai_api_key.is_empty() {
        shown.ai_api_key = AI_KEY_MASK.to_string();
    }
    crate::plugins::hook("config.import.after", serde_json::to_value(&shown).unwrap_or_default());
    Ok(shown)
}

/// Путь к файлу настроек: абсолютный, без `..`, с расширением `.json`.
fn settings_path(raw: &str) -> Res<std::path::PathBuf> {
    let p = raw.trim();
    if p.is_empty() {
        return err("Не указан файл настроек");
    }
    if p.contains("..") {
        return err("Некорректный путь к файлу настроек");
    }
    let path = std::path::PathBuf::from(p);
    if !path.is_absolute() {
        return err("Укажите полный путь к файлу настроек");
    }
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        return err("Файл настроек должен быть .json");
    }
    Ok(path)
}

#[tauri::command]
pub async fn list_instances() -> Res<Vec<Instance>> {
    let list = load_instances().await?;
    crate::plugins::hook(
        "instance.list",
        serde_json::json!({ "count": list.len(), "instances": &list }),
    );
    Ok(list)
}

#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    name: String,
    mc_version: Option<String>,
    loader: Loader,
) -> Res<Instance> {
    let mc = match mc_version {
        Some(v) if !v.trim().is_empty() => v,
        _ => latest_release(&state.http).await?,
    };

    let count = load_instances().await?.len();
    let biome = BIOMES[count % BIOMES.len()].to_string();
    let id = Uuid::new_v4().simple().to_string();
    let created = chrono::Utc::now().timestamp();

    let hook = crate::plugins::call_hook(
        "instance.create.before",
        &serde_json::json!({
            "id": id.clone(),
            "name": name.clone(),
            "mcVersion": mc.clone(),
            "loader": loader.label(),
        }),
    );
    if hook
        .iter()
        .any(|r| r.result.get("cancel").and_then(|v| v.as_bool()).unwrap_or(false))
    {
        return err("Создание профиля отменено плагином");
    }

    let instance = Instance {
        id: id.clone(),
        name,
        mc_version: mc.clone(),
        loader,
        loader_version: None,
        biome,
        icon: None,
        icon_preset: None,
        wallpaper: None,
        ram_min_mb: 1024,
        ram_max_mb: 4096,
        java_major: default_java(&mc),
        java_path: None,
        jvm_args: String::new(),
        width: 1280,
        height: 720,
        fullscreen: false,
        playtime_sec: 0,
        playtime_days: std::collections::BTreeMap::new(),
        last_played: None,
        scanner_mode: None,
        created,
        pack: None,
        game_dir: None,
    };

    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    let game_dir = paths::instance_minecraft_dir(&id);
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(game_dir.join(sub)).await?;
    }

    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    let _ = crate::plugins::call_hook(
        "instance.create.after",
        &serde_json::to_value(&instance).unwrap_or_default(),
    );
    Ok(instance)
}

/// Имя файла иконки профиля: одна безопасная компонента пути.
fn valid_icon_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\'])
        && crate::content::safe_rel(name).is_some()
}

#[tauri::command]
pub async fn update_instance(mut instance: Instance) -> Res<()> {
    paths::validate_instance_id(&instance.id)?;
    let hook = crate::plugins::call_hook(
        "instance.update.before",
        &serde_json::to_value(&instance).unwrap_or_default(),
    );
    for r in hook {
        if r.error.is_some() {
            continue;
        }
        if r.result.get("cancel").and_then(|v| v.as_bool()).unwrap_or(false) {
            return err("Изменение профиля отменено плагином");
        }
        if let Some(v) = r.result.get("instance") {
            if let Ok(next) = serde_json::from_value::<Instance>(v.clone()) {
                instance = next;
            }
        }
    }
    paths::validate_instance_id(&instance.id)?;
    validate_java_path(instance.java_path.as_deref().unwrap_or("")).await?;
    if instance.jvm_args.contains('\0') {
        return err("Недопустимые JVM-аргументы");
    }
    if let Some(icon) = &instance.icon {
        if !valid_icon_name(icon) {
            return err("Некорректное имя иконки профиля");
        }
    }
    store::write_json_atomic(&paths::instance_json(&instance.id), &instance).await?;
    let _ = crate::plugins::call_hook(
        "instance.update.after",
        &serde_json::to_value(&instance).unwrap_or_default(),
    );
    Ok(())
}

/// Своя папка игры: `path = None` — вернуть папку профиля.
#[tauri::command]
pub async fn set_instance_game_dir(id: String, path: Option<String>) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    if let Some(msg) = crate::plugins::hook_cancel(
        "instance.game_dir.before",
        serde_json::json!({ "id": id, "path": path }),
    ) {
        return err(&msg);
    }
    let mut inst = store::read_instance(&id).await?;
    crate::gamedir::switch(&mut inst, path.as_deref()).await?;
    store::write_instance(&inst).await?;
    crate::plugins::hook(
        "instance.game_dir.after",
        serde_json::json!({ "id": id, "instance": &inst }),
    );
    Ok(inst)
}

#[tauri::command]
pub async fn delete_instance(id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let hook = crate::plugins::call_hook("instance.delete.before", &serde_json::json!({ "id": id }));
    if hook
        .iter()
        .any(|r| r.result.get("cancel").and_then(|v| v.as_bool()).unwrap_or(false))
    {
        return err("Удаление профиля отменено плагином");
    }
    let dir = paths::instance_dir(&id);
    // Ссылку на вынесенную папку игры снимаем до удаления профиля,
    // иначе remove_dir_all заденет данные игрока.
    if let Ok(inst) = store::read_instance(&id).await {
        crate::gamedir::detach(&inst).await;
    }
    if tokio::fs::metadata(&dir).await.is_ok() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    let _ = crate::plugins::call_hook("instance.delete.after", &serde_json::json!({ "id": id }));
    Ok(())
}

#[tauri::command]
pub async fn set_instance_icon(id: String, source_path: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    let src = std::path::PathBuf::from(&source_path);
    if source_path.contains("..") || !src.is_absolute() {
        return err("Некорректный путь к иконке");
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "ico") {
        return err("Поддерживаются только PNG, JPG и ICO");
    }
    match tokio::fs::metadata(&src).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл иконки не найден"),
    }

    let mut instance = store::read_instance(&id).await?;

    if let Some(old) = instance.icon.clone() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            if tokio::fs::metadata(&old_path).await.is_ok() {
                let _ = tokio::fs::remove_file(&old_path).await;
            }
        }
    }

    let file = format!("icon.{ext}");
    let dst = paths::instance_icon(&id, &file);
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    tokio::fs::copy(&src, &dst).await?;

    instance.icon = Some(file);
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    crate::plugins::hook(
        "instance.icon.after",
        serde_json::json!({ "id": id, "instance": &instance }),
    );
    Ok(instance)
}

#[tauri::command]
pub async fn set_instance_icon_url(state: State<'_, AppState>, id: String, url: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return err("Некорректный адрес иконки");
    }
    let resp = state.http.get(&url).send().await?;
    let bytes = resp.error_for_status()?.bytes().await?;
    if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
        return err("Иконка слишком большая");
    }
    let mut instance = store::read_instance(&id).await?;
    if let Some(old) = instance.icon.clone() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            let _ = tokio::fs::remove_file(&old_path).await;
        }
    }
    let file = "icon.png".to_string();
    let dst = paths::instance_icon(&id, &file);
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    tokio::fs::write(&dst, &bytes).await?;
    instance.icon = Some(file);
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    crate::plugins::hook(
        "instance.icon.after",
        serde_json::json!({ "id": id, "instance": &instance, "url": url }),
    );
    Ok(instance)
}

#[tauri::command]
pub async fn clear_instance_icon(id: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    let mut instance = store::read_instance(&id).await?;
    if let Some(old) = instance.icon.take() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            if tokio::fs::metadata(&old_path).await.is_ok() {
                tokio::fs::remove_file(&old_path).await?;
            }
        }
    }
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    crate::plugins::hook(
        "instance.icon.after",
        serde_json::json!({ "id": id, "instance": &instance, "cleared": true }),
    );
    Ok(instance)
}

#[tauri::command]
pub async fn open_instance_folder(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id);
    tokio::fs::create_dir_all(&dir).await?;
    crate::plugins::hook(
        "instance.folder.open",
        serde_json::json!({ "id": id, "path": dir.to_string_lossy() }),
    );
    open_path(&app, &dir)?;
    Ok(())
}

/// Консоль игры из `logs/latest.log` — её видно даже после перезапуска
/// лаунчера, когда живых строк в памяти уже нет.
#[tauri::command]
pub async fn read_game_log(id: String, tail: Option<usize>) -> Res<Vec<String>> {
    paths::validate_instance_id(&id)?;
    let path = paths::instance_minecraft_dir(&id).join("logs").join("latest.log");
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    // Файл быстро растёт, читаем только хвост. Обрезанный первый байт
    // отбрасываем вместе с недописанной строкой.
    const MAX_BYTES: usize = 512 * 1024;
    let cut = bytes.len().saturating_sub(MAX_BYTES);
    let slice = &bytes[cut..];
    let end = std::str::from_utf8(slice)
        .map(|_| slice.len())
        .unwrap_or_else(|e| e.valid_up_to());
    let text = String::from_utf8_lossy(&slice[..end]);

    let limit = tail.unwrap_or(1200).clamp(50, 5000);
    let mut lines: Vec<&str> = text.lines().collect();
    if cut > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let start = lines.len().saturating_sub(limit);
    let out: Vec<String> = lines[start..]
        .iter()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    crate::plugins::hook(
        "game.log.read",
        serde_json::json!({ "id": id, "lines": out.len() }),
    );
    Ok(out)
}

/// Открывает папку `logs` профиля.
#[tauri::command]
pub async fn open_logs_folder(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("logs");
    tokio::fs::create_dir_all(&dir).await?;
    crate::plugins::hook(
        "game.logs.open",
        serde_json::json!({ "id": id, "path": dir.to_string_lossy() }),
    );
    open_path(&app, &dir)?;
    Ok(())
}

#[tauri::command]
pub fn get_system_ram_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}
