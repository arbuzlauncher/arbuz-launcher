use crate::error::{err, AppError, Res};
use crate::http::{download_all, Dl, Progress};
use crate::models::{Account, AccountKind, Instance, Loader};
use crate::state::AppState;
use crate::{paths, store};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct GameLog {
    pub id: String,
    pub line: String,
}

/// Процесс игры запущен. Отдельное событие: кнопка «Остановить» не должна
/// зависеть от того, успела ли игра что-то написать в stdout.
#[derive(Clone, serde::Serialize)]
pub struct GameStart {
    pub id: String,
}

#[derive(Clone, serde::Serialize)]
pub struct GameExit {
    pub id: String,
    pub code: Option<i32>,
}

/// Результат конвейера запуска: игра завершилась либо пользователь нажал «Остановить».
enum Outcome {
    Done(Option<i32>),
    Cancelled,
}

/// Обёртка шага конвейера: ждём либо результат, либо сигнал остановки.
async fn step<T>(
    fut: impl std::future::Future<Output = Res<T>>,
    kill_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> Option<Res<T>> {
    tokio::select! {
        r = fut => Some(r),
        _ = &mut *kill_rx => None,
    }
}

/// Выполняет шаг; при остановке возвращает `Outcome::Cancelled` из конвейера.
macro_rules! step_or_cancel {
    ($kill:expr, $fut:expr) => {
        match step($fut, $kill).await {
            Some(r) => r?,
            None => return Ok(Outcome::Cancelled),
        }
    };
}

/// Ключ дедупликации classpath: `group:artifact[:classifier]`.
fn cp_key(name: &str) -> String {
    let p: Vec<&str> = name.split(':').collect();
    if p.len() >= 4 {
        format!("{}:{}:{}", p[0], p[1], p[3])
    } else if p.len() >= 2 {
        format!("{}:{}", p[0], p[1])
    } else {
        name.to_string()
    }
}

/// Разбивает строку JVM-аргументов по пробелам, учитывая двойные кавычки.
fn split_args(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut has = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                has = true;
            }
            c if c.is_whitespace() && !in_quote => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

/// Minecraft версии `mc` не младше `min_minor.min_patch`.
fn mc_at_least(mc: &str, min_minor: u32, min_patch: u32) -> bool {
    let p: Vec<u32> = mc.split('.').filter_map(|x| x.parse().ok()).collect();
    let minor = p.get(1).copied().unwrap_or(0);
    let patch = p.get(2).copied().unwrap_or(0);
    minor > min_minor || (minor == min_minor && patch >= min_patch)
}

/// Распаковка нативных jar-ов: пропускаем `META-INF/` и пути из `exclude`,
/// защищаемся от выхода за пределы каталога (включая `C:` и UNC-пути).
fn extract_natives(jar: &Path, out: &Path, excludes: &[String]) -> Res<()> {
    let f = std::fs::File::open(jar)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut entry = z.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let raw = entry.name().replace('\\', "/");
        if raw.starts_with("META-INF/") {
            continue;
        }
        if excludes.iter().any(|ex| raw.starts_with(ex.as_str())) {
            continue;
        }
        let Some(rel) = crate::content::safe_rel(&raw) else {
            continue;
        };
        let out_path = out.join(&rel);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut w = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut w)?;
    }
    Ok(())
}

/// Относительный путь библиотеки внутри `shared/libraries`: берётся из
/// `downloads.artifact.path` либо вычисляется из maven-имени. `None`, если
/// путь пустой или может вывести за пределы каталога библиотек.
fn lib_rel(path: Option<&String>, name: &str) -> Option<String> {
    let rel = match path {
        Some(p) => p.clone(),
        None => crate::mojang::rules::maven_path(name)?,
    };
    crate::content::safe_rel(&rel).map(|_| rel)
}

/// Запуск игры. Возвращает `Ok(())` и тогда, когда запуск отменён пользователем.
pub async fn launch(
    app: AppHandle,
    state: &AppState,
    id: String,
    world: Option<String>,
    server: Option<String>,
    scan_override: bool,
) -> Res<()> {
    // A. Профиль и активный аккаунт.
    let inst = store::read_instance(&id).await?;
    let acc = crate::accounts::active()
        .await?
        .ok_or_else(|| AppError::Msg("Выберите аккаунт".into()))?;

    // B. Загрузчик устанавливается на шаге F, когда уже известен id профиля.

    // C. Канал остановки и регистрация запущенного профиля.
    let (kill_tx, mut kill_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut running = state.running.lock().await;
        if running.contains_key(&id) {
            return err("Профиль уже запущен");
        }
        running.insert(id.clone(), kill_tx);
    }

    let result = run(&app, state, &id, &inst, &acc, world, server, scan_override, &mut kill_rx).await;
    state.running.lock().await.remove(&id);
    state.game_pids.lock().await.remove(&id);

    // Бенчмарк (если идёт): считаем итоги до очистки PID.
    if crate::benchmark::is_running(id.as_str()) {
        let mods = std::fs::read_dir(paths::instance_minecraft_dir(id.as_str()).join("mods"))
            .map(|rd| rd.flatten().filter(|e| e.path().is_file()).count() as u32)
            .unwrap_or(0);
        if let Some(run) = crate::benchmark::finish(id.as_str(), &inst.mc_version, inst.loader.label(), mods) {
            let _ = app.emit("bench-done", run);
        }
    }

    match result {
        Ok(Outcome::Done(code)) => {
            let _ = app.emit(
                "game-exit",
                GameExit {
                    id: id.clone(),
                    code,
                },
            );
            Ok(())
        }
        Ok(Outcome::Cancelled) => {
            let _ = app.emit(
                "game-exit",
                GameExit {
                    id: id.clone(),
                    code: None,
                },
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Конвейер: сканер, файлы, библиотеки, ассеты, Java, аргументы, процесс, учёт времени.
async fn run(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    acc: &Account,
    world: Option<String>,
    server: Option<String>,
    scan_override: bool,
    kill_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> Res<Outcome> {
    // E. Начало.
    let _ = app.emit("progress", Progress { id: id.to_string(), pct: 0.0 });

    // D. Сканер модов: до скачивания чего-либо. Пользователь мог явно
    // подтвердить «запустить всё равно» — тогда гейт пропускается.
    if !scan_override {
        crate::scanner::launch_gate(state, id).await?;
    }

    // D2. Синхронизация настроек: подмешать выбранные ключи options.txt
    // и servers.dat из главного профиля до старта игры.
    if let Some(sync) = state.config.lock().await.sync.clone() {
        crate::sync::apply_before_launch(id, &sync).await?;
    }

    // F. (0 → 30) version.json и client jar.
    // Для модового загрузчика сначала ставим его профиль и сливаем с vanilla.
    let vanilla = matches!(inst.loader, Loader::Vanilla);
    let jar_lo = if vanilla { 0.0 } else { 10.0 };

    let version_id = if vanilla {
        inst.mc_version.clone()
    } else {
        let installed = step_or_cancel!(
            kill_rx,
            crate::loaders::install(app, state, id, inst, 0.0, 10.0)
        );
        // Выбранную версию загрузчика фиксируем в instance.json (ТЗ §7.6).
        if inst.loader_version.as_deref() != Some(installed.loader_version.as_str()) {
            let mut fresh = store::read_instance(id).await?;
            fresh.loader_version = Some(installed.loader_version.clone());
            store::write_instance(&fresh).await?;
        }
        installed.profile_id
    };

    let vj = step_or_cancel!(
        kill_rx,
        crate::mojang::versions::merged(&state.http, &version_id)
    );

    // Проверяем требуемую Java сразу: иначе пользователь узнает о ней только
    // после скачивания сотен мегабайт ассетов.
    if let Some(jv) = &vj.java_version {
        if (inst.java_major as u32) < jv.major_version {
            return err(format!(
                "Для Minecraft {} нужна Java {}. Выберите её в «Настройках» профиля.",
                inst.mc_version, jv.major_version
            ));
        }
    }

    let mut client_items: Vec<Dl> = Vec::new();
    if let Some(c) = vj.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        client_items.push(Dl {
            url: c.url.clone(),
            path: paths::version_jar(&inst.mc_version),
            sha1: c.sha1.clone(),
            sha256: None,
            size: c.size,
        });
    }
    step_or_cancel!(
        kill_rx,
        download_all(app, &state.http, id, client_items, jar_lo, 30.0)
    );

    // G. (30 → 65) библиотеки и нативы.
    let mut feats: HashMap<&str, bool> = HashMap::new();
    feats.insert("has_custom_resolution", false);
    feats.insert("is_demo_user", false);
    feats.insert("has_quick_plays_support", false);

    let mut lib_items: Vec<Dl> = Vec::new();
    let mut cp_entries: Vec<(String, PathBuf)> = Vec::new();
    let mut native_jars: Vec<(PathBuf, Vec<String>)> = Vec::new();

    for lib in &vj.libraries {
        if !crate::mojang::rules::native_classifier_ok(&lib.name) {
            continue;
        }
        if !crate::mojang::rules::allowed(&lib.rules, &feats) {
            continue;
        }

        // Нативы (старые версии): скачиваем classifier, в classpath не кладём.
        if let Some(nat) = &lib.natives {
            let arch = if cfg!(target_arch = "x86") {
                "32"
            } else if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "64"
            };
            let key = nat
                .get(crate::mojang::rules::OS_NAME)
                .map(|s| s.replace("${arch}", arch));
            if let Some(key) = key {
                if let Some(art) = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .and_then(|c| c.get(&key))
                {
                    let rel = match lib_rel(art.path.as_ref(), &lib.name) {
                        Some(r) => r,
                        None => return err("Некорректный путь библиотеки в профиле версии"),
                    };
                    let abs = paths::libraries_dir().join(&rel);
                    lib_items.push(Dl {
                        url: art.url.clone(),
                        path: abs.clone(),
                        sha1: art.sha1.clone(),
                        sha256: None,
                        size: art.size,
                    });
                    let excludes = lib
                        .extract
                        .as_ref()
                        .map(|e| e.exclude.clone())
                        .unwrap_or_default();
                    native_jars.push((abs, excludes));
                }
            }
            continue;
        }

        // Обычная библиотека с артефактом.
        if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = match lib_rel(art.path.as_ref(), &lib.name) {
                Some(r) => r,
                None => return err("Некорректный путь библиотеки в профиле версии"),
            };
            let abs = paths::libraries_dir().join(&rel);
            lib_items.push(Dl {
                url: art.url.clone(),
                path: abs.clone(),
                sha1: art.sha1.clone(),
                sha256: None,
                size: art.size,
            });
            cp_entries.push((cp_key(&lib.name), abs));
            continue;
        }

        // Fabric-подобные: путь из maven-имени, url из базы библиотеки.
        // Профиль не содержит хеша, поэтому берём опубликованный `.sha1`.
        if let Some(base) = &lib.url {
            let Some(rel) = lib_rel(None, &lib.name) else {
                return err("Некорректное имя библиотеки в профиле версии");
            };
            let abs = paths::libraries_dir().join(&rel);
            let url = format!("{base}{rel}");
            let sha1 = crate::http::fetch_sha1(&state.http, &url).await;
            lib_items.push(Dl {
                url,
                path: abs.clone(),
                sha1,
                sha256: None,
                size: 0,
            });
            cp_entries.push((cp_key(&lib.name), abs));
        }
    }

    step_or_cancel!(
        kill_rx,
        download_all(app, &state.http, id, lib_items, 30.0, 65.0)
    );

    // H. Classpath: дедупликация по ключу, первое вхождение, client jar последним.
    let mut seen: HashSet<String> = HashSet::new();
    let mut cp: Vec<PathBuf> = Vec::new();
    for (k, p) in cp_entries {
        if seen.insert(k) {
            cp.push(p);
        }
    }
    cp.push(paths::version_jar(&inst.mc_version));
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = cp
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);

    // I. Распаковка нативов.
    let natives_dir = paths::instance_natives_dir(id);
    let jars = native_jars;
    let natives_out = natives_dir.clone();
    tokio::task::spawn_blocking(move || -> Res<()> {
        std::fs::create_dir_all(&natives_out)?;
        for (jar, excludes) in &jars {
            extract_natives(jar, &natives_out, excludes)?;
        }
        Ok(())
    })
    .await??;

    // J. (65 → 90) ассеты.
    if let Some(ai) = &vj.asset_index {
        let list = step_or_cancel!(
            kill_rx,
            crate::mojang::assets::plan(app, &state.http, id, ai, 65.0)
        );
        step_or_cancel!(
            kill_rx,
            download_all(app, &state.http, id, list, 66.0, 90.0)
        );
    }

    // K. (90 → 95) Java.
    let java: PathBuf = match inst.java_path.as_ref().filter(|p| !p.trim().is_empty()) {
        Some(p) => PathBuf::from(p),
        None => step_or_cancel!(
            kill_rx,
            crate::java::ensure_java(app, &state.http, id, inst.java_major, 90.0, 95.0)
        ),
    };

    // L. Токен.
    let token: String = match acc.kind {
        AccountKind::Offline => "0".to_string(),
        AccountKind::Elyby => step_or_cancel!(
            kill_rx,
            crate::accounts::elyby::token(&state.http, acc)
        ),
        AccountKind::Littleskin => step_or_cancel!(
            kill_rx,
            crate::accounts::littleskin::token(&state.http, acc)
        ),
        AccountKind::Microsoft => step_or_cancel!(
            kill_rx,
            crate::accounts::microsoft::token(&state.http, acc)
        ),
    };

    let injector_url: Option<&str> = match acc.kind {
        AccountKind::Elyby => Some("ely.by"),
        AccountKind::Littleskin => Some(crate::accounts::littleskin::INJECTOR_URL),
        _ => None,
    };
    let authlib: Option<(&str, PathBuf)> = match injector_url {
        Some(url) => Some((
            url,
            step_or_cancel!(
                kill_rx,
                crate::accounts::elyby::injector(&state.http)
            ),
        )),
        None => None,
    };

    // M. Аргументы.
    let mut subst: HashMap<&str, String> = HashMap::new();
    subst.insert("auth_player_name", acc.name.clone());
    subst.insert("version_name", vj.id.clone());
    subst.insert(
        "game_directory",
        paths::instance_minecraft_dir(id).to_string_lossy().to_string(),
    );
    subst.insert(
        "assets_root",
        paths::assets_dir().to_string_lossy().to_string(),
    );
    subst.insert(
        "assets_index_name",
        vj.asset_index
            .as_ref()
            .map(|a| a.id.clone())
            .unwrap_or_default(),
    );
    subst.insert("auth_uuid", acc.id.clone());
    subst.insert("auth_access_token", token.clone());
    subst.insert("clientid", "arbuz-launcher".to_string());
    subst.insert("auth_xuid", "0".to_string());
    subst.insert(
        "user_type",
        if matches!(acc.kind, AccountKind::Microsoft) {
            "msa"
        } else {
            "mojang"
        }
        .to_string(),
    );
    subst.insert("version_type", "release".to_string());
    subst.insert("resolution_width", inst.width.to_string());
    subst.insert("resolution_height", inst.height.to_string());
    subst.insert(
        "natives_directory",
        natives_dir.to_string_lossy().to_string(),
    );
    subst.insert("launcher_name", "Arbuz Launcher".to_string());
    subst.insert("launcher_version", "1.0.0".to_string());
    subst.insert("classpath", classpath.clone());
    subst.insert("classpath_separator", sep.to_string());
    subst.insert(
        "library_directory",
        paths::libraries_dir().to_string_lossy().to_string(),
    );
    subst.insert("user_properties", "{}".to_string());

    let mut args: Vec<String> = Vec::new();
    // 1. Память.
    args.push(format!("-Xms{}M", inst.ram_min_mb));
    args.push(format!("-Xmx{}M", inst.ram_max_mb));
    // 2. authlib-injector для Ely.by/LittleSkin.
    if let Some((url, jar)) = &authlib {
        args.push(format!("-javaagent:{}={}", jar.to_string_lossy(), url));
    }
    // 3. JVM-аргументы профиля.
    args.extend(split_args(&inst.jvm_args));
    // 4. JVM-аргументы версии.
    if let Some(pargs) = &vj.arguments {
        for a in crate::mojang::rules::flat(&pargs.jvm, &feats) {
            args.push(crate::mojang::rules::subst(&a, &subst));
        }
    } else {
        args.push(format!(
            "-Djava.library.path={}",
            natives_dir.to_string_lossy()
        ));
        args.push("-cp".to_string());
        args.push(classpath.clone());
    }
    // 5. Главный класс.
    let main_class = match &vj.main_class {
        Some(m) => m.clone(),
        None => return err("В версии нет mainClass"),
    };
    args.push(main_class);
    // 6. Игровые аргументы.
    if let Some(pargs) = &vj.arguments {
        for a in crate::mojang::rules::flat(&pargs.game, &feats) {
            args.push(crate::mojang::rules::subst(&a, &subst));
        }
    } else if let Some(ma) = &vj.minecraft_arguments {
        for a in ma.split(' ') {
            args.push(crate::mojang::rules::subst(a, &subst));
        }
    }
    // 7. Окно.
    if inst.fullscreen {
        args.push("--fullscreen".to_string());
    } else {
        args.push("--width".to_string());
        args.push(inst.width.to_string());
        args.push("--height".to_string());
        args.push(inst.height.to_string());
    }
    // 8. Запуск прямо в мир (1.20+).
    if let Some(w) = &world {
        if mc_at_least(&inst.mc_version, 1, 20) {
            args.push("--quickPlaySingleplayer".to_string());
            args.push(w.clone());
        }
    }
    // 8b. Запуск прямо на сервер.
    if let Some(s) = &server {
        let s = s.trim();
        if !s.is_empty() {
            if mc_at_least(&inst.mc_version, 1, 20) {
                args.push("--quickPlayMultiplayer".to_string());
                args.push(s.to_string());
            } else {
                let (host, port) = match s.rsplit_once(':') {
                    Some((h, p)) if p.parse::<u16>().is_ok() => (h, p),
                    _ => (s, "25565"),
                };
                args.push("--server".to_string());
                args.push(host.to_string());
                args.push("--port".to_string());
                args.push(port.to_string());
            }
        }
    }

    #[cfg(windows)]
    let guard = {
        let (on, lvl) = {
            let c = state.config.lock().await;
            (c.mod_isolation_enabled, c.mod_isolation_level.clone())
        };
        let has_isolated = !crate::content::load_meta(id).await.isolated.is_empty();
        crate::sandbox::level_of(on, &lvl, has_isolated)
    };
    #[cfg(not(windows))]
    let _guard = "off";

    // Песочница: TEMP всегда внутри профиля, чтобы JVM и нативы не лезли
    // в medium-%TEMP%. На strict ещё и HOME/APPDATA/LOCALAPPDATA → container,
    // чтобы мод не видел реальные пути профиля.
    #[cfg(windows)]
    let mut sandbox_temp: Option<std::path::PathBuf> = None;
    #[cfg(windows)]
    if guard != "off" {
        let cbox = paths::instance_dir(id).join("container");
        let ctemp = cbox.join("temp");
        let _ = std::fs::create_dir_all(&ctemp);
        args.push(format!("-Djava.io.tmpdir={}", ctemp.display()));
        // JVM пишет hsperfdata в %TEMP% (medium) — low-процессу туда нельзя.
        args.push("-XX:-UsePerfData".into());
        if guard == "strict" {
            for d in ["home", "appdata", "local"] {
                let _ = std::fs::create_dir_all(cbox.join(d));
            }
            args.push(format!("-Duser.home={}", cbox.join("home").display()));
        }
        sandbox_temp = Some(ctemp);
    }

    // Файловую изоляцию готовим до старта: icacls может идти несколько секунд,
    // а игра не должна успеть создать файлы в medium-папке.
    #[cfg(windows)]
    let sandbox_ready = crate::sandbox::prepare(guard, id);

    // O. Процесс.
    let mut cmd = tokio::process::Command::new(&java);
    cmd.args(&args)
        .current_dir(paths::instance_minecraft_dir(id))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    if let Some(t) = &sandbox_temp {
        cmd.env("TEMP", t).env("TMP", t);
    }
    #[cfg(windows)]
    if guard == "strict" {
        let cbox = paths::instance_dir(id).join("container");
        cmd.env("HOME", cbox.join("home"))
            .env("USERPROFILE", cbox.join("home"))
            .env("APPDATA", cbox.join("appdata"))
            .env("LOCALAPPDATA", cbox.join("local"));
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = cmd.spawn()?;

    #[cfg(windows)]
    if guard != "off" {
        if let Some(h) = child.raw_handle() {
            crate::sandbox::apply(h, guard, sandbox_ready);
        }
    }
    state.game_pids.lock().await.insert(id.to_string(), child.id().unwrap_or(0));
    crate::benchmark::on_pid(id, child.id().unwrap_or(0));
    let _ = app.emit(
        "game-start",
        GameStart {
            id: id.to_string(),
        },
    );

    // O. Чтение логов (токен маскируем, если он длиннее 8 символов).
    let mask = if token.len() > 8 {
        token.clone()
    } else {
        String::new()
    };

    let mut readers: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        let app2 = app.clone();
        let id2 = id.to_string();
        let m = mask.clone();
        readers.push(tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        crate::benchmark::on_log(&id2, &line);
                        let line = if m.is_empty() {
                            line
                        } else {
                            line.replace(&m, "***")
                        };
                        let _ = app2.emit(
                            "game-log",
                            GameLog {
                                id: id2.clone(),
                                line,
                            },
                        );
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let app2 = app.clone();
        let id2 = id.to_string();
        let m = mask;
        readers.push(tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        crate::benchmark::on_log(&id2, &line);
                        let line = if m.is_empty() {
                            line
                        } else {
                            line.replace(&m, "***")
                        };
                        let _ = app2.emit(
                            "game-log",
                            GameLog {
                                id: id2.clone(),
                                line,
                            },
                        );
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }));
    }

    // P. Готово к игре.
    let _ = app.emit("progress", Progress { id: id.to_string(), pct: 100.0 });

    // Q. Ожидание завершения.
    let start = std::time::Instant::now();
    let mut status: Option<std::process::ExitStatus> = None;
    let mut killed = false;
    tokio::select! {
        st = child.wait() => { status = st.ok(); }
        _ = &mut *kill_rx => { killed = true; }
    }
    if killed {
        let _ = child.kill().await;
        status = child.wait().await.ok();
    }

    // R. Учёт времени игры.
    let elapsed = start.elapsed().as_secs();
    // Дожидаемся задач-читателей: иначе последние строки лога придут во фронтенд
    // уже после game-exit и снова «оживят» завершённый профиль.
    for h in readers {
        let _ = h.await;
    }
    let mut fresh = store::read_instance(id).await?;
    fresh.playtime_sec += elapsed;
    let key = chrono::Local::now().format("%Y-%m-%d").to_string();
    *fresh.playtime_days.entry(key).or_insert(0) += elapsed;
    fresh.last_played = Some(chrono::Utc::now().timestamp());
    store::write_instance(&fresh).await?;

    Ok(Outcome::Done(status.and_then(|s| s.code())))
}
