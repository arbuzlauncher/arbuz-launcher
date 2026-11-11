#![cfg(windows)]

//! Изоляция игры.
//!
//! Песочница работает на **всех** уровнях, а не только на `strict`:
//! - низкая целостность (low integrity): игра и моды могут писать только
//!   внутрь папки профиля, всё остальное — только чтение;
//! - job-объект: игра умирает вместе с лаунчером;
//! - `std`/`strict`: запрет создавать дочерние процессы (мод не запустит
//!   `cmd.exe`, установщик или сторонний загрузчик);
//! - `strict`: HOME/APPDATA/LOCALAPPDATA/TEMP перенаправляются в
//!   `instances/<id>/container`, чтобы мод не видел реальные пути профиля.
//!
//! Буфер обмена по умолчанию **не ограничивается**. Остальные UI-ограничения
//! job-объекта (`HANDLES`, `SYSTEMPARAMETERS`, `DISPLAYSETTINGS`,
//! `GLOBALATOMS`, `DESKTOP`) не используются: они ломают создание окна и
//! OpenGL-контекста, из-за чего Sodium и шейдеры не запускаются. Опционально
//! (`mod_isolation_clipboard`) включается только запрет чтения/записи буфера
//! обмена — он не влияет на окно и рендер.
//!
//! Дополнительно файлы лаунчера (`config.json`, `accounts.json`) получают
//! метку medium с `NO_READ_UP`: low-процесс игры не может их прочитать.

use crate::paths;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;

type Handle = *mut c_void;
type Bool = i32;
type Dword = u32;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct BasicLimit {
    per_process_time: i64,
    per_job_time: i64,
    limit_flags: Dword,
    min_ws: usize,
    max_ws: usize,
    active_processes: Dword,
    affinity: usize,
    priority: Dword,
    scheduling: Dword,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct IoCounters {
    read_ops: u64,
    write_ops: u64,
    other_ops: u64,
    read_bytes: u64,
    write_bytes: u64,
    other_bytes: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct ExtLimit {
    basic: BasicLimit,
    io: IoCounters,
    process_mem: usize,
    job_mem: usize,
    peak_process_mem: usize,
    peak_job_mem: usize,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SidAndAttributes {
    sid: *mut c_void,
    attributes: Dword,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UiRestrictions {
    ui_restrictions: Dword,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TokenMandatoryLabel {
    label: SidAndAttributes,
}

extern "system" {
    fn CreateJobObjectW(attrs: *mut c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: Dword, info: *mut c_void, len: Dword) -> Bool;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> Bool;
    fn OpenProcessToken(process: Handle, access: Dword, token: *mut Handle) -> Bool;
    fn SetTokenInformation(token: Handle, class: Dword, info: *mut c_void, len: Dword) -> Bool;
    fn CreateWellKnownSid(kind: Dword, domain: *mut c_void, sid: *mut c_void, size: *mut Dword)
        -> Bool;
    fn CloseHandle(h: Handle) -> Bool;
}

extern "system" {
    fn InitializeAcl(acl: *mut c_void, len: Dword, revision: Dword) -> Bool;
    fn AddMandatoryAce(
        acl: *mut c_void,
        ace_revision: Dword,
        flags: Dword,
        policy: Dword,
        label_sid: *mut c_void,
    ) -> Bool;
    fn SetNamedSecurityInfoW(
        name: *mut u16,
        object_type: i32,
        security_info: Dword,
        owner: *mut c_void,
        group: *mut c_void,
        dacl: *mut c_void,
        sacl: *mut c_void,
    ) -> Dword;
}

const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: Dword = 9;
const JOB_OBJECT_BASIC_UI_RESTRICTIONS: Dword = 4;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: Dword = 0x2000;
const JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION: Dword = 0x400;
const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: Dword = 0x0000_0008;
const JOB_OBJECT_UILIMIT_READCLIPBOARD: Dword = 0x0000_0002;
const JOB_OBJECT_UILIMIT_WRITECLIPBOARD: Dword = 0x0000_0004;

const TOKEN_ADJUST_DEFAULT: Dword = 0x0080;
const TOKEN_QUERY: Dword = 0x0008;
const TOKEN_INTEGRITY_LEVEL: Dword = 25;
const SE_GROUP_INTEGRITY: Dword = 0x0000_0020;
const WIN_LOW_LABEL_SID: Dword = 66;
const WIN_MEDIUM_LABEL_SID: Dword = 67;

const ACL_REVISION: Dword = 2;
const SE_FILE_OBJECT: i32 = 1;
const LABEL_SECURITY_INFORMATION: Dword = 0x0000_0010;
const SYSTEM_MANDATORY_LABEL_NO_WRITE_UP: Dword = 0x0000_0001;
const SYSTEM_MANDATORY_LABEL_NO_READ_UP: Dword = 0x0000_0002;

const CREATE_NO_WINDOW: Dword = 0x0800_0000;

pub const OFF: &str = "off";
pub const MIN: &str = "min";
pub const STD: &str = "std";
pub const STRICT: &str = "strict";

pub fn level_of(enabled: bool, level: &str, has_isolated: bool) -> &'static str {
    if !enabled {
        return OFF;
    }
    match level {
        "strict" => STRICT,
        "std" => STD,
        _ if has_isolated => STD,
        _ => MIN,
    }
}

/// Готовит файловую изоляцию ДО запуска игры: помечает дерево профиля низкой
/// целостностью. Возвращает `false`, если метку поставить не удалось — тогда
/// токен игры не понижаем, иначе она не сможет писать даже в свой профиль.
pub fn prepare(level: &str, id: &str) -> bool {
    if level == OFF {
        return true;
    }
    let ok = ensure_low_tree(&paths::instance_dir(id));
    if ok {
        protect_secrets();
    }
    ok
}

/// Вешает job и понижает целостность уже запущенного процесса.
///
/// `std`/`strict` дополнительно запрещают дочерние процессы. Токен понижаем
/// на всех уровнях: иначе `min`/`std` оставались бы обычным medium-процессом.
pub fn apply(handle: Handle, level: &str, ready: bool, clipboard: bool) {
    if handle.is_null() || level == OFF {
        return;
    }
    unsafe {
        if let Some(job) = make_job(level, clipboard) {
            let _ = AssignProcessToJobObject(job, handle);
        }
        if ready {
            lower_token(handle);
        }
    }
}

unsafe fn make_job(level: &str, clipboard: bool) -> Option<Handle> {
    let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
    if job.is_null() {
        return None;
    }
    let mut info = ExtLimit::default();
    info.basic.limit_flags =
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    if level != MIN {
        // 1 = только сама игра. Любая попытка создать процесс получит отказ.
        info.basic.limit_flags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        info.basic.active_processes = 1;
    }
    SetInformationJobObject(
        job,
        JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ExtLimit>() as Dword,
    );
    // Только буфер обмена: остальные UI-ограничения ломают окно и OpenGL.
    if clipboard {
        let mut ui = UiRestrictions {
            ui_restrictions: JOB_OBJECT_UILIMIT_READCLIPBOARD | JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
        };
        SetInformationJobObject(
            job,
            JOB_OBJECT_BASIC_UI_RESTRICTIONS,
            &mut ui as *mut _ as *mut c_void,
            std::mem::size_of::<UiRestrictions>() as Dword,
        );
    }
    Some(job)
}

unsafe fn lower_token(handle: Handle) {
    let mut sid_buf = [0u8; 68];
    let mut len = sid_buf.len() as Dword;
    if CreateWellKnownSid(
        WIN_LOW_LABEL_SID,
        std::ptr::null_mut(),
        sid_buf.as_mut_ptr() as *mut c_void,
        &mut len,
    ) == 0
    {
        return;
    }
    let sid = sid_buf.as_mut_ptr() as *mut c_void;

    let mut token: Handle = std::ptr::null_mut();
    if OpenProcessToken(handle, TOKEN_ADJUST_DEFAULT | TOKEN_QUERY, &mut token) == 0 {
        return;
    }
    let mut label = TokenMandatoryLabel {
        label: SidAndAttributes {
            sid,
            attributes: SE_GROUP_INTEGRITY,
        },
    };
    SetTokenInformation(
        token,
        TOKEN_INTEGRITY_LEVEL,
        &mut label as *mut _ as *mut c_void,
        std::mem::size_of::<TokenMandatoryLabel>() as Dword,
    );
    CloseHandle(token);
}

fn ensure_low_tree(root: &Path) -> bool {
    let marker = root.join(".arbuz-isolated-v2");
    // Проверяем метку, а не только маркер: старый/битый маркер не должен
    // приводить к «low token + medium files».
    if marker.is_file() && is_low(root) {
        return true;
    }
    if !label_tree(root) {
        return false;
    }
    // Ключевые каталоги должны быть low, иначе игра не запишет логи/конфиги.
    for sub in [
        "minecraft/logs",
        "minecraft/config",
        "minecraft/saves",
        "minecraft/mods",
        "container",
    ] {
        let p = root.join(sub);
        if p.exists() && !is_low(&p) {
            return false;
        }
    }
    if !is_low(root) {
        return false;
    }
    let _ = std::fs::write(&marker, b"arbuz isolation v2");
    true
}

/// Файлы лаунчера не должны быть доступны low-процессу игры на чтение:
/// там лежат настройки, список аккаунтов и прочие внутренние данные.
fn protect_secrets() {
    for p in [paths::config_file(), paths::accounts_file()] {
        let _ = protect_secret(&p);
    }
}

/// Ставит на файл метку medium с `NO_WRITE_UP | NO_READ_UP`: medium-лаунчер
/// читает и пишет как обычно, low-игра — не читает и не пишет.
fn protect_secret(path: &Path) -> bool {
    if !path.is_file() {
        return true;
    }
    unsafe {
        let mut sid_buf = [0u8; 68];
        let mut sid_len = sid_buf.len() as Dword;
        if CreateWellKnownSid(
            WIN_MEDIUM_LABEL_SID,
            std::ptr::null_mut(),
            sid_buf.as_mut_ptr() as *mut c_void,
            &mut sid_len,
        ) == 0
        {
            return false;
        }

        // ACL требует выравнивания; [u32] даёт его на всех целевых платформах.
        let mut acl_buf = [0u32; 64];
        let acl = acl_buf.as_mut_ptr() as *mut c_void;
        if InitializeAcl(acl, (acl_buf.len() * 4) as Dword, ACL_REVISION) == 0 {
            return false;
        }
        let policy = SYSTEM_MANDATORY_LABEL_NO_WRITE_UP | SYSTEM_MANDATORY_LABEL_NO_READ_UP;
        if AddMandatoryAce(
            acl,
            ACL_REVISION,
            0,
            policy,
            sid_buf.as_mut_ptr() as *mut c_void,
        ) == 0
        {
            return false;
        }

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        SetNamedSecurityInfoW(
            wide.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
        ) == 0
    }
}

/// Абсолютный путь к `icacls.exe`, чтобы не подхватить одноимённый файл из PATH.
fn icacls_exe() -> std::path::PathBuf {
    std::env::var_os("SystemRoot")
        .map(|r| std::path::PathBuf::from(r).join("System32").join("icacls.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| std::path::PathBuf::from("icacls.exe"))
}

/// `icacls` ставит метку рекурсивно и включает наследование для новых файлов.
/// Это надёжнее самописного `SetNamedSecurityInfo` и работает без админа.
fn label_tree(root: &Path) -> bool {
    let status = std::process::Command::new(icacls_exe())
        .arg(root)
        .arg("/setintegritylevel")
        .arg("(OI)(CI)L")
        .arg("/T")
        .arg("/C")
        .arg("/Q")
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    matches!(status, Ok(s) if s.success())
}

/// Низкая метка в `icacls` всегда содержит `(NW)` (No Write Up).
fn is_low(path: &Path) -> bool {
    let Ok(out) = std::process::Command::new(icacls_exe())
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout).contains("(NW)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_temp_tree_low_and_verifies() {
        let dir = std::env::temp_dir().join("arbuz-sandbox-label-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();

        assert!(label_tree(&dir), "icacls не смог поставить метку");
        assert!(is_low(&dir), "на папке нет (NW)");

        // medium-процесс (сам тест) должен сохранять доступ на запись.
        std::fs::write(dir.join("a.txt"), b"world").unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn protects_secret_but_keeps_medium_access() {
        let dir = std::env::temp_dir().join("arbuz-sandbox-secret-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.json");
        std::fs::write(&file, br#"{"ok":true}"#).unwrap();

        assert!(protect_secret(&file), "не удалось поставить NO_READ_UP");

        // Лаунчер работает на medium и должен читать/писать без изменений.
        assert_eq!(std::fs::read(&file).unwrap(), br#"{"ok":true}"#);
        std::fs::write(&file, br#"{"ok":false}"#).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), br#"{"ok":false}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "меняет метку реального профиля; нужен ARBUZ_TEST_INSTANCE"]
    fn labels_real_instance() {
        let id = std::env::var("ARBUZ_TEST_INSTANCE").expect("ARBUZ_TEST_INSTANCE");
        assert!(prepare(STRICT, &id), "не удалось пометить профиль");
        assert!(is_low(&paths::instance_dir(&id)));
        assert!(is_low(&paths::instance_dir(&id).join("minecraft/logs")));
    }
}
