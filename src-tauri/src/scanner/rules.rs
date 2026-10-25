//! Правила сканера (1–22). Каждое правило ищет конкретные признаки в собранных
//! уликах (`JarEvidence`) и возвращает находки с весом из конфига.
//!
//! Формулировки для пользователя: «найдены подозрительные признаки», не «вирус».
//! `hard = true` у находок правил 1–5 и базы малвари: их не гасит совпадение
//! хеша с Modrinth.


use super::classfile::ClassData;
use super::config::ScannerConfig;
use super::decode;
use super::evidence::JarEvidence;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeSet;

/// Максимум находок одного правила: остальные сворачиваются в «и ещё N».
const MAX_PER_RULE: usize = 5;

/// Общий предел находок на один jar.
const MAX_FINDINGS: usize = 64;

// ---------------------------------------------------------------------------
// Типы
// ---------------------------------------------------------------------------

/// Одна находка правила: что нашли, где и сколько это весит.
#[derive(Clone, Debug)]
pub(crate) struct Finding {
    /// Идентификатор правила: «R1»..«R22».
    pub rule: &'static str,
    /// Машиночитаемый код находки: «r6_native», «r12_agent» и т.п. —
    /// по нему режимы решают, блокировать ли запуск при любой находке.
    pub code: &'static str,
    pub weight: i32,
    /// Находки правил 1–5 и известной малвари не гасятся Modrinth-хешем.
    pub hard: bool,
    /// Где найдено: класс, манифест, запись архива.
    pub place: String,
    /// Что нашли (для списка причин в интерфейсе).
    pub what: String,
}

/// Соседние моды (для правил 21): дубли и перекрытия, посчитанные по всем jar.
#[derive(Clone, Debug, Default)]
pub(crate) struct Neighbors {
    /// (класс, имя другого jar с тем же классом).
    pub duplicated: Vec<(String, String)>,
    /// Классы Minecraft/загрузчика, перекрываемые этим jar.
    pub overrides: Vec<String>,
}

/// Контекст, который нельзя получить из самого jar.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScanContext {
    /// Строковые индикаторы из локальной базы малвари.
    pub malware_indicators: Vec<String>,
    /// SHA-256 этого jar найден в локальной базе малвари.
    pub known_malware: bool,
    /// Дубли классов среди остальных модов профиля.
    pub neighbors: Option<Neighbors>,
}

/// Строковая сигнатура: подстрока (в нижнем регистре) -> правило и описание.
struct Sig {
    needle: &'static str,
    rule: &'static str,
    code: &'static str,
    kind: &'static str,
}

/// Сигнатуры правил 1–3, которые ищутся простым вхождением подстроки.
/// Ими же прогоняются декодированные base64/hex/XOR-строки (правило 8).
const SIGS: &[Sig] = &[
    // R1: каналы вывода данных
    Sig { needle: "discord.com/api/webhooks", rule: "R1", code: "r1_exfil", kind: "вебхук Discord" },
    Sig { needle: "discordapp.com/api/webhooks", rule: "R1", code: "r1_exfil", kind: "вебхук Discord" },
    Sig { needle: "ptb.discord.com/api/webhooks", rule: "R1", code: "r1_exfil", kind: "вебхук Discord (ptb)" },
    Sig { needle: "canary.discord.com/api/webhooks", rule: "R1", code: "r1_exfil", kind: "вебхук Discord (canary)" },
    Sig { needle: "api.telegram.org/bot", rule: "R1", code: "r1_exfil", kind: "Telegram-бот" },
    Sig { needle: "pastebin.com", rule: "R1", code: "r1_exfil", kind: "pastebin" },
    Sig { needle: "hastebin.com", rule: "R1", code: "r1_exfil", kind: "hastebin" },
    Sig { needle: "paste.ee", rule: "R1", code: "r1_exfil", kind: "paste.ee" },
    Sig { needle: "transfer.sh", rule: "R1", code: "r1_exfil", kind: "transfer.sh" },
    Sig { needle: "ngrok", rule: "R1", code: "r1_exfil", kind: "туннель ngrok" },
    Sig { needle: "iplogger", rule: "R1", code: "r1_exfil", kind: "iplogger" },
    Sig { needle: "2no.co", rule: "R1", code: "r1_exfil", kind: "iplogger" },
    Sig { needle: "ipify.org", rule: "R1", code: "r1_exfil", kind: "ipify" },
    // R2: кража данных
    Sig { needle: "launcher_accounts.json", rule: "R2", code: "r2_theft", kind: "файл аккаунтов лаунчера" },
    Sig { needle: "launcher_profiles.json", rule: "R2", code: "r2_theft", kind: "профили лаунчера" },
    // «tdata» — только как элемент пути: голая подстрока ловит CompactData.
    Sig { needle: "\\tdata", rule: "R2", code: "r2_theft", kind: "сессии Telegram (tdata)" },
    Sig { needle: "/tdata", rule: "R2", code: "r2_theft", kind: "сессии Telegram (tdata)" },
    Sig { needle: "wallet.dat", rule: "R2", code: "r2_theft", kind: "кошелёк Bitcoin" },
    Sig { needle: "keystore/utc--", rule: "R2", code: "r2_theft", kind: "keystore Ethereum" },
    Sig { needle: "ethereum/keystore", rule: "R2", code: "r2_theft", kind: "keystore Ethereum" },
    Sig { needle: "exodus", rule: "R2", code: "r2_theft", kind: "кошелёк Exodus" },
    Sig { needle: "metamask", rule: "R2", code: "r2_theft", kind: "кошелёк MetaMask" },
    Sig { needle: "login data", rule: "R2", code: "r2_theft", kind: "пароли браузера" },
    Sig { needle: "\\local state", rule: "R2", code: "r2_theft", kind: "ключи шифрования браузера" },
    // R3: запуск процессов (строковые команды)
    Sig { needle: "powershell -enc", rule: "R3", code: "r3_exec", kind: "powershell -enc" },
    Sig { needle: "-encodedcommand", rule: "R3", code: "r3_exec", kind: "powershell -EncodedCommand" },
    Sig { needle: "cmd /c", rule: "R3", code: "r3_exec", kind: "cmd /c" },
    Sig { needle: "mshta", rule: "R3", code: "r3_exec", kind: "mshta" },
    Sig { needle: "certutil", rule: "R3", code: "r3_exec", kind: "certutil" },
    Sig { needle: "bitsadmin", rule: "R3", code: "r3_exec", kind: "bitsadmin" },
    Sig { needle: "schtasks", rule: "R3", code: "r3_exec", kind: "schtasks" },
    Sig { needle: "currentversion\\run", rule: "R3", code: "r3_exec", kind: "автозапуск через реестр" },
    Sig { needle: "currentversion/run", rule: "R3", code: "r3_exec", kind: "автозапуск через реестр" },
    Sig { needle: "reg add ", rule: "R3", code: "r3_exec", kind: "запись в реестр" },
    // R16: фингерпринт
    Sig { needle: "tasklist", rule: "R16", code: "r16_fingerprint", kind: "список процессов (tasklist)" },
    Sig { needle: "wmic ", rule: "R16", code: "r16_fingerprint", kind: "wmic" },
    Sig { needle: "reg query", rule: "R16", code: "r16_fingerprint", kind: "чтение реестра" },
];

/// Маркеры для XOR-поиска: подстроки правил 1 и 3.
const XOR_MARKERS: &[&str] = &[
    "discord.com/api/webhooks",
    "api.telegram.org/bot",
    "pastebin.com",
    "ngrok",
    "iplogger",
    "powershell",
    "certutil",
    "mshta",
];

/// Классы сессии/аккаунтов/сети для правила mixin (R20).
const SESSION_CLASSES: &[&str] = &[
    "util/session",
    "authlib/yggdrasil",
    "authlib/minecraft",
    "network/connection",
    "network/protocol",
    "minecraft/client/minecraft",
];

/// Префиксы известных библиотек: классы под ними не считаются «чужими» (R6).
const LIB_PREFIXES: &[&str] = &[
    "net/minecraft", "com/mojang", "net/fabricmc", "net/quiltmc", "net/neoforged",
    "net/minecraftforge", "org/spongepowered", "com/google", "org/apache", "io/netty",
    "org/slf4j", "org/apache/logging", "org/jetbrains", "org/intellij", "it/unimi",
    "com/mojang/datafixers", "java/", "javax/", "sun/", "jdk/", "com/ibm", "org/graalvm",
];

static RE_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bhttps?://[^\s"'<>()\\]+"#).unwrap());
static RE_URL_IP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^https?://\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?").unwrap());

/// Сигнатуры, сработавшие в тексте (регистр не важен).
fn sig_hits(text: &str) -> Vec<&'static Sig> {
    let lower = text.to_ascii_lowercase();
    SIGS.iter().filter(|s| lower.contains(s.needle)).collect()
}

/// Урезает длинные значения для сообщения о находке.
fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n).collect();
        format!("{cut}…")
    }
}

/// Взвешенная находка.
struct Sink {
    findings: Vec<Finding>,
}

impl Sink {
    fn new() -> Self {
        Self { findings: Vec::new() }
    }

    /// Счётчик находок по правилу, чтобы свернуть лишние в «и ещё N».
    fn count(&self, rule: &str) -> usize {
        self.findings.iter().filter(|f| f.rule == rule).count()
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        rule: &'static str,
        code: &'static str,
        weight: i32,
        hard: bool,
        place: &str,
        what: String,
    ) {
        if self.findings.len() >= MAX_FINDINGS {
            return;
        }
        let n = self.count(rule);
        if n >= MAX_PER_RULE {
            // Последняя находка правила дописывает счётчик.
            if let Some(last) = self.findings.iter_mut().filter(|f| f.rule == rule).next_back() {
                last.what = format!("{}; и ещё находки этого типа", last.what);
            }
            return;
        }
        self.findings.push(Finding {
            rule,
            code,
            weight,
            hard,
            place: place.to_string(),
            what,
        });
    }
}

// ---------------------------------------------------------------------------
// Метаданные мода
// ---------------------------------------------------------------------------

/// Все URL из контактных полей метаданных — их домены считаются допустимыми.
fn metadata_hosts(ev: &JarEvidence) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut visit = |v: &serde_json::Value| {
        collect_json_urls(v, &mut out);
    };
    if let Some(j) = &ev.mod_json {
        if let Some(c) = j.get("contact") {
            visit(c);
        }
        if let Some(c) = j.get("links") {
            visit(c);
        }
    }
    if let Some(t) = &ev.mods_toml {
        if let Some(m) = t.get("mods").and_then(|m| m.as_array()).and_then(|a| a.first()) {
            for key in ["displayURL", "issueTrackerURL", "logoFile"] {
                if let Some(u) = m.get(key).and_then(|v| v.as_str()) {
                    if let Some(h) = url_host(u) {
                        out.insert(h);
                    }
                }
            }
        }
    }
    out
}

fn collect_json_urls(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::String(s) => {
            if let Some(h) = url_host(s) {
                out.insert(h);
            }
        }
        serde_json::Value::Object(m) => {
            for x in m.values() {
                collect_json_urls(x, out);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                collect_json_urls(x, out);
            }
        }
        _ => {}
    }
}

/// Хост из URL (нижний регистр) или `None`.
fn url_host(u: &str) -> Option<String> {
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .or_else(|| u.strip_prefix("HTTPS://"))
        .or_else(|| u.strip_prefix("HTTP://"))?;
    let host = rest.split(['/', '?', '#', ':']).next()?;
    let host = host.trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

/// Хост допущен: точное совпадение или поддомен белого списка.
fn host_allowed(host: &str, whitelist: &[String], meta: &BTreeSet<String>) -> bool {
    if meta.contains(host) {
        return true;
    }
    whitelist
        .iter()
        .any(|d| host == d || host.strip_suffix(d).is_some_and(|p| p.ends_with('.')))
}

// ---------------------------------------------------------------------------
// Помощники по class-файлам
// ---------------------------------------------------------------------------

/// Классы, у которых есть вызов, удовлетворяющий предикату (класс, метод).
fn classes_calling<'a>(ev: &'a JarEvidence, f: impl Fn(&str, &str) -> bool + Copy) -> Vec<&'a ClassData> {
    ev.classes
        .iter()
        .filter(|c| c.method_calls.iter().any(|m| f(&m.class, &m.name)))
        .collect()
}

fn has_call(c: &ClassData, class: &str, name: &str) -> bool {
    c.method_calls
        .iter()
        .any(|m| (class.is_empty() || m.class == class) && m.name == name)
}

/// Класс пишет на диск (поток/Files) — часть комбо правила 5 и правила 13.
fn writes_files(c: &ClassData) -> bool {
    c.method_calls.iter().any(|m| {
        m.class == "java/io/FileOutputStream"
            || m.class == "java/io/RandomAccessFile"
            || m.class == "java/util/zip/ZipOutputStream"
            || (m.class == "java/nio/file/Files" && matches!(m.name.as_str(), "write" | "copy" | "newOutputStream" | "writeString"))
    })
}

/// Класс запускает процессы (правило 3).
fn runs_processes(c: &ClassData) -> bool {
    c.method_calls.iter().any(|m| {
        m.class == "java/lang/ProcessBuilder" || (m.class == "java/lang/Runtime" && m.name == "exec")
    })
}

// ---------------------------------------------------------------------------
// Главное
// ---------------------------------------------------------------------------

/// Прогоняет улики через все правила. Возвращает находки с весами из конфига.
pub(crate) fn analyze(ev: &JarEvidence, ctx: &ScanContext, cfg: &ScannerConfig) -> Vec<Finding> {
    let mut s = Sink::new();
    let w = &cfg.weights;

    // --- R11: известная малварь (локальная база) ---
    if ctx.known_malware {
        s.push("R11", "r11_hash", w.known_malware_hash, true, "база малвари", "SHA-256 совпадает с известной малварью".into());
    }
    if !ctx.malware_indicators.is_empty() {
        for c in &ev.classes {
            for ind in &ctx.malware_indicators {
                let ind = ind.to_ascii_lowercase();
                if c.strings.iter().any(|str_| str_.to_ascii_lowercase().contains(&ind)) {
                    s.push("R11", "r11_indicator", w.malware_indicator, true, &c.name, format!("индикатор малвари «{}»", trunc(ind.trim(), 40)));
                    break;
                }
            }
        }
    }

    // --- R1/R2/R3/R16: строковые сигнатуры по классам ---
    for c in &ev.classes {
        for str_ in &c.strings {
            for sig in sig_hits(str_) {
                let weight = sig_weight(sig.rule, cfg);
                let hard = matches!(sig.rule, "R1" | "R2" | "R3");
                let kind = static_kind(sig.kind);
                s.push(sig.rule, sig.code, weight, hard, &c.name, format!("{kind}: {}", trunc(str_, 80)));
            }
        }
    }

    // Голые IP в URL (R1).
    for c in &ev.classes {
        for str_ in &c.strings {
            if RE_URL_IP.is_match(str_) {
                s.push("R1", "r1_exfil", w.exfil_channel, true, &c.name, format!("URL с IP-адресом: {}", trunc(str_, 80)));
            }
        }
    }

    // Браузерные куки/пароли: связка «браузер + файл учетных данных» (R2).
    for c in &ev.classes {
        for str_ in &c.strings {
            let lower = str_.to_ascii_lowercase();
            let browser = ["chrome", "chromium", "edg\\", "edge\\", "firefox", "mozilla", "brave"]
                .iter()
                .any(|b| lower.contains(b));
            let creds = ["cookies", "login data", "local state", "web data"]
                .iter()
                .any(|x| lower.contains(x));
            if browser && creds {
                s.push("R2", "r2_theft", w.data_theft, true, &c.name, format!("данные браузера: {}", trunc(str_, 80)));
            }
            if lower.contains("discord") && (lower.contains("local storage") || lower.contains("leveldb")) {
                s.push("R2", "r2_theft", w.data_theft, true, &c.name, "хранилище Discord (Local Storage)".into());
            }
        }
    }

    // --- R2: скриншоты, буфер обмена, токены ---
    for c in &ev.classes {
        if c.method_calls.iter().any(|m| m.class == "java/awt/Robot" && m.name.starts_with("createScreenCapture")) {
            s.push("R2", "r2_theft", w.data_theft, true, &c.name, "снимок экрана через java.awt.Robot".into());
        }
        if c.method_calls.iter().any(|m| m.class == "java/awt/datatransfer/Clipboard" && m.name == "getContents") {
            s.push("R2", "r2_theft", w.data_theft, true, &c.name, "чтение буфера обмена".into());
        }
        for m in &c.method_calls {
            if m.name == "getAccessToken" {
                s.push("R2", "r2_token", w.session_token_access, false, &c.name, format!("getAccessToken ({})", m.class));
            }
            if m.name == "getSession" && (m.class.contains("authlib") || m.class.to_ascii_lowercase().contains("session")) {
                s.push("R2", "r2_token", w.session_token_access, false, &c.name, format!("getSession ({})", m.class));
            }
        }
    }

    // URL из строковых констант — нужны правилам 3 (эскалация) и 5.
    let mut urls: Vec<(String, String)> = Vec::new(); // (класс, url)
    for c in &ev.classes {
        for str_ in &c.strings {
            for m in RE_URL.find_iter(str_) {
                urls.push((c.name.clone(), m.as_str().to_string()));
            }
        }
    }
    let exec_ext = |u: &str| -> Option<&'static str> {
        let path = u.split(['?', '#']).next().unwrap_or(u).to_ascii_lowercase();
        [".exe", ".dll", ".bat", ".cmd", ".ps1", ".scr", ".msi", ".jar"]
            .into_iter()
            .find(|e| path.ends_with(e))
    };

    // --- R3: запуск процессов (вызовы) ---
    // Сам по себе exec — приём легитимных модов (проба GPU, открытие ссылки
    // в браузере, xdg-open): мягкий вес. Жёстким делаем только комбинацию с
    // записью файлов, скачиванием или подозрительными командами.
    let any_exec = ev
        .classes
        .iter()
        .any(|c| c.method_calls.iter().any(|m| m.class == "java/lang/ProcessBuilder" || (m.class == "java/lang/Runtime" && m.name == "exec")));
    let exec_escalation = any_exec
        && (ev.classes.iter().any(|c| writes_files(c))
            || urls.iter().any(|(_, u)| exec_ext(u).is_some())
            || ev
                .classes
                .iter()
                .flat_map(|c| c.strings.iter())
                .any(|str_| sig_hits(str_).iter().any(|s| s.rule == "R3")));
    for c in classes_calling(ev, |cl, _| cl == "java/lang/ProcessBuilder") {
        s.push("R3", "r3_exec", w.process_exec, exec_escalation, &c.name, "запуск процессов через ProcessBuilder".into());
    }
    for c in classes_calling(ev, |cl, n| cl == "java/lang/Runtime" && n == "exec") {
        s.push("R3", "r3_exec", w.process_exec, exec_escalation, &c.name, "запуск процессов через Runtime.exec".into());
    }

    // --- R4: динамическая загрузка кода ---
    for c in &ev.classes {
        // Полноценная загрузка кода: URLClassLoader/defineClass.
        let loader = c.method_calls.iter().any(|m| {
            m.class == "java/net/URLClassLoader"
                || (m.class.ends_with("ClassLoader") && matches!(m.name.as_str(), "defineClass" | "loadClass"))
                || (m.class == "java/lang/invoke/MethodHandles$Lookup"
                    && matches!(m.name.as_str(), "defineClass" | "privateLookupIn"))
        });
        if loader {
            s.push("R4", "r4_dynamic", w.dynamic_code_load, false, &c.name, "динамическая загрузка кода (ClassLoader/defineClass)".into());
        }
        // Unsafe сам по себе — приём оптимизаторов (lithium и др.): отдельный
        // мягкий вес, не мгновенный блок в усиленном режиме.
        if c.class_refs.iter().any(|r| {
            r == "sun/misc/Unsafe"
                || r == "jdk/internal/misc/Unsafe"
                || r == "jdk/internal/unsafe/Unsafe"
        }) {
            s.push("R4", "r4_unsafe", w.unsafe_use, false, &c.name, "обращение к sun.misc.Unsafe".into());
        }
        // Расшифровка ресурсов в классы: чтение ресурса + «*.class» + шифрование.
        let crypto = c.method_calls.iter().any(|m| {
            (m.class == "javax/crypto/Cipher" && matches!(m.name.as_str(), "getInstance" | "doFinal"))
                || (m.class == "java/util/Base64" && m.name == "decode")
        });
        if crypto && has_call(c, "", "getResourceAsStream") && c.strings.iter().any(|x| x.ends_with(".class")) {
            s.push("R4", "r4_dynamic", w.dynamic_code_load, false, &c.name, "расшифровка ресурса в class-файл".into());
        }
    }

    // --- R5: загрузка исполняемых файлов (URL собраны выше, для R3) ---
    let mut has_exec_url = false;
    let mut has_jar_url = false;
    for (class, u) in &urls {
        match exec_ext(u) {
            Some(".jar") => has_jar_url = true,
            Some(_) => {
                has_exec_url = true;
                s.push("R5", "r5_url", w.executable_download_url_only, true, class, format!("URL исполняемого файла: {}", trunc(u, 80)));
            }
            None => {}
        }
    }
    let writes = ev.classes.iter().any(|c| writes_files(c));
    let runs = ev.classes.iter().any(|c| runs_processes(c));
    if (has_exec_url || has_jar_url) && writes && runs {
        s.push("R5", "r5_download", w.executable_download, true, "jar", "скачивание файла + запись на диск + запуск процесса".into());
    }

    // --- R6: структура jar ---
    if ev.zip_error {
        s.push("R6", "r6_broken", w.broken_class, false, "архив", "архив повреждён или не открывается".into());
    }
    if ev.over_limits {
        s.push("R6", "r6_limits", w.archive_limits, false, "архив", "превышены лимиты архива (подозрение на zip-бомбу)".into());
    }
    if ev.entries.iter().any(|e| e.contains("../") || e.starts_with('/')) {
        s.push("R6", "r6_traversal", w.path_traversal, false, "архив", "записи с выходом за пределы каталога (../)".into());
    }
    let mut natives: BTreeSet<&str> = BTreeSet::new();
    let mut jar_in_jar = 0usize;
    for e in &ev.entries {
        let lower = e.to_ascii_lowercase();
        if lower.starts_with("meta-inf/jars/") && lower.ends_with(".jar") {
            jar_in_jar += 1;
            continue;
        }
        for ext in [".dll", ".so", ".dylib", ".jnilib", ".exe", ".sys"] {
            if lower.ends_with(ext) {
                natives.insert(match ext {
                    ".dll" => "dll",
                    ".so" | ".dylib" | ".jnilib" => "so",
                    ".exe" => "exe",
                    _ => "sys",
                });
            }
        }
    }
    for n in natives {
        s.push("R6", "r6_native", w.native_binaries, false, "архив", format!("нативная библиотека в jar (.{n})"));
    }
    if jar_in_jar > 0 {
        s.push("R6", "r6_jij", w.jar_in_jar, false, "архив", format!("jar-in-jar в META-INF/jars ({jar_in_jar} шт., обычно легитимно)"));
    }
    if ev.broken_classes > 0 {
        s.push("R6", "r6_broken", w.broken_class, false, "архив", format!("битые class-файлы: {}", ev.broken_classes));
    }

    // Классы вне пакета modid (R6). modid ↔ имя пакета связано не всегда
    // (shaded-библиотеки живут под доменом автора), поэтому смотрим на
    // структуру: классы без пакета или россыпь несвязанных корневых пакетов.
    {
        let mut roots: BTreeSet<String> = BTreeSet::new();
        let mut default_pkg = 0usize;
        for c in &ev.classes {
            match c.name.split_once('/') {
                None => default_pkg += 1,
                Some((root, _)) => {
                    if !LIB_PREFIXES.iter().any(|p| c.name.starts_with(p)) {
                        roots.insert(root.to_string());
                    }
                }
            }
        }
        if default_pkg > 0 {
            s.push("R6", "r6_outside", w.classes_outside_modid, false, "архив", format!("классы без пакета: {default_pkg}"));
        } else if roots.len() > 4 {
            let list: Vec<String> = roots.into_iter().take(5).collect();
            s.push("R6", "r6_outside", w.classes_outside_modid, false, "архив", format!("классы из несвязанных пакетов: {}", list.join(", ")));
        }
    }

    // --- R7: обфускация (одна находка со всеми сработавшими эвристиками) ---
    {
        let mut reasons: Vec<&str> = Vec::new();
        let total = ev.classes.len();
        if total >= 10 {
            let short_classes = ev
                .classes
                .iter()
                .filter(|c| {
                    let simple = c.name.rsplit('/').next().unwrap_or("").split('$').next().unwrap_or("");
                    simple.len() <= 2
                })
                .count();
            if short_classes * 10 > total * 7 {
                reasons.push("имена классов из 1–2 символов");
            }
            let methods: Vec<&str> = ev
                .classes
                .iter()
                .flat_map(|c| c.declared_methods.iter().map(String::as_str))
                .filter(|n| !n.starts_with('<'))
                .collect();
            if methods.len() >= 30 {
                let short = methods.iter().filter(|n| n.len() <= 2).count();
                if short * 10 > methods.len() * 7 {
                    reasons.push("имена методов из 1–2 символов");
                }
            }
        }
        let high_entropy = ev
            .classes
            .iter()
            .flat_map(|c| c.strings.iter())
            .filter(|s| {
                s.len() >= 96
                    && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
                    && decode::shannon_entropy(s.as_bytes()) >= 5.2
            })
            .count();
        if high_entropy >= 3 {
            reasons.push("строки с аномальной энтропией");
        }
        let control = ev
            .classes
            .iter()
            .flat_map(|c| c.strings.iter())
            .filter(|s| s.bytes().filter(|b| *b < 0x20 && *b != b'\n' && *b != b'\r' && *b != b'\t').count() >= 8)
            .count();
        if control >= 3 {
            reasons.push("непечатаемые строки (зашифрованные данные)");
        }
        if !reasons.is_empty() {
            s.push("R7", "r7_obfuscation", w.obfuscation, false, "jar", format!("сильная обфускация: {}", reasons.join(", ")));
        }
    }

    // --- R8: base64/hex/XOR ---
    {
        let mut encoded_n = 0usize;
        let mut decoded_hit_n = 0usize;
        let mut xor_scanned = 0usize;
        for c in &ev.classes {
            for str_ in &c.strings {
                // base64
                if decode::looks_base64(str_) {
                    if let Some(bytes) = decode::base64_decode(str_) {
                        if decode::printable_ratio(&bytes) >= 0.85 {
                            let text = String::from_utf8_lossy(&bytes).to_string();
                            for sig in sig_hits(&text) {
                                if decoded_hit_n < MAX_PER_RULE {
                                    decoded_hit_n += 1;
                                    let hard = matches!(sig.rule, "R1" | "R2" | "R3");
                                    let kind = static_kind(sig.kind);
                                    s.push(sig.rule, sig.code, sig_weight(sig.rule, cfg), hard, &c.name, format!("base64 скрывает {kind}: {}", trunc(&text, 60)));
                                }
                            }
                        }
                    }
                    if encoded_n < 3 {
                        encoded_n += 1;
                        s.push("R8", "r8_encoded", w.encoded_payload, false, &c.name, "закодированные данные (base64)".into());
                    }
                }
                // hex
                if decode::looks_hex(str_) {
                    if let Some(bytes) = decode::hex_decode(str_) {
                        if decode::printable_ratio(&bytes) >= 0.85 {
                            let text = String::from_utf8_lossy(&bytes).to_string();
                            for sig in sig_hits(&text) {
                                if decoded_hit_n < MAX_PER_RULE {
                                    decoded_hit_n += 1;
                                    let hard = matches!(sig.rule, "R1" | "R2" | "R3");
                                    let kind = static_kind(sig.kind);
                                    s.push(sig.rule, sig.code, sig_weight(sig.rule, cfg), hard, &c.name, format!("hex скрывает {kind}: {}", trunc(&text, 60)));
                                }
                            }
                        }
                    }
                }
                // однобайтовый XOR: не больше 200 строк на jar — этого
                // хватает на реальные обфускаторы, а сканирование дешёвое.
                if (24..=512).contains(&str_.len()) && !str_.contains("http") && xor_scanned < 200 {
                    if let Some((key, text)) = decode::xor_find_marker(str_, XOR_MARKERS) {
                        s.push("R1", "r1_exfil", w.exfil_channel, true, &c.name, format!("XOR(0x{key:02x}) скрывает {}", trunc(&text, 60)));
                    }
                    xor_scanned += 1;
                }
            }
        }
    }

    // --- R9: домены вне белого списка ---
    {
        let meta_hosts = metadata_hosts(ev);
        let mut unknown: BTreeSet<String> = BTreeSet::new();
        for c in &ev.classes {
            for str_ in &c.strings {
                for m in RE_URL.find_iter(str_) {
                    if let Some(h) = url_host(m.as_str()) {
                        if !RE_URL_IP.is_match(m.as_str()) && !host_allowed(&h, &cfg.whitelist_domains, &meta_hosts) {
                            unknown.insert(h);
                        }
                    }
                }
            }
        }
        for h in unknown.into_iter().take(MAX_PER_RULE) {
            s.push("R9", "r9_domain", w.unknown_domain, false, "jar", format!("обращение к домену вне белого списка: {h}"));
        }
    }

    // --- R12: манифест ---
    {
        let get = |k: &str| -> Option<String> {
            ev.manifest
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
        };
        for key in ["Premain-Class", "Agent-Class", "Launcher-Agent-Class"] {
            if let Some(v) = get(key) {
                s.push("R12", "r12_agent", w.manifest_agent, false, "MANIFEST.MF", format!("{key}: {}", trunc(&v, 60)));
            }
        }
        if let Some(cp) = get("Class-Path") {
            if cp.contains("http://") || cp.contains("https://") || cp.contains(':') && (cp.contains("/../") || cp.contains("\\")) {
                s.push("R12", "r12_classpath", w.manifest_classpath, false, "MANIFEST.MF", format!("подозрительный Class-Path: {}", trunc(&cp, 60)));
            }
        }
    }

    // --- R13: заражение других модов ---
    for c in &ev.classes {
        let mods_jar = c
            .strings
            .iter()
            .any(|s| s.contains(".jar") && (s.contains("mods/") || s.contains("\\mods\\") || s.split('/').any(|p| p == "mods")));
        if mods_jar && writes_files(c) {
            s.push("R13", "r13_infect", w.infect_other_mods, false, &c.name, "запись в jar-файлы папки mods".into());
        }
    }

    // --- R14: нативные вызовы ---
    for c in classes_calling(ev, |cl, n| cl == "java/lang/System" && (n == "load" || n == "loadLibrary")) {
        s.push("R14", "r14_system_load", w.system_load, false, &c.name, "System.load/loadLibrary".into());
    }
    {
        let native = ev.classes.iter().map(|c| c.native_methods).sum::<usize>();
        if native > 0 {
            s.push("R14", "r14_native_methods", w.jni_native_methods, false, "jar", format!("native-методы (JNI): {native}"));
        }
    }

    // --- R15: скриптовые движки ---
    {
        let engines = [
            ("javax/script/ScriptEngineManager", "ScriptEngine"),
            ("javax/script/Invocable", "ScriptEngine"),
            ("groovy/lang/GroovyShell", "Groovy"),
            ("org/mozilla/javascript/Context", "Rhino"),
            ("org/mozilla/javascript/Script", "Rhino"),
        ];
        for c in &ev.classes {
            for (class, name) in engines {
                if c.method_calls.iter().any(|m| m.class == class) || c.class_refs.iter().any(|r| r == class) {
                    s.push("R15", "r15_script", w.script_engine, false, &c.name, format!("скриптовый движок {name}"));
                    break;
                }
            }
        }
    }

    // --- R16: фингерпринт системы (лёгкий сигнал — максимум 3 находки) ---
    for c in &ev.classes {
        if s.count("R16") >= 3 {
            break;
        }
        if has_call(c, "java/lang/System", "getenv") {
            s.push("R16", "r16_fingerprint", w.fingerprinting, false, &c.name, "чтение переменных окружения".into());
        }
        if s.count("R16") >= 3 {
            break;
        }
        if has_call(c, "java/lang/System", "getProperty") {
            let keys = ["user.name", "os.name", "user.home", "user.dir"];
            if c.strings.iter().any(|s| keys.iter().any(|k| s == k)) {
                s.push("R16", "r16_fingerprint", w.fingerprinting, false, &c.name, "чтение системных свойств (user.name/os.name)".into());
            }
        }
    }

    // --- R17: отложенный запуск ---
    for c in &ev.classes {
        if has_call(c, "java/lang/Thread", "sleep")
            && c.int_constants.iter().any(|&v| v >= 3_600_000)
        {
            s.push("R17", "r17_delayed", w.delayed_execution, false, &c.name, "Thread.sleep на час и больше".into());
        }
    }

    // --- R18: анти-анализ ---
    for c in &ev.classes {
        let vm = c
            .class_refs
            .iter()
            .any(|r| r.starts_with("com/sun/tools/attach") || r.starts_with("com/sun/jdi"))
            || c.strings.iter().any(|s| s.contains("isDebuggerConnected"));
        if vm {
            s.push("R18", "r18_vm", w.anti_analysis, false, &c.name, "проверка отладчика/VM".into());
        }
        if has_call(c, "java/lang/Runtime", "addShutdownHook") {
            s.push("R18", "r18_hook", w.shutdown_hook, false, &c.name, "addShutdownHook".into());
        }
    }

    // --- R19: массовый setAccessible ---
    {
        let n = classes_calling(ev, |cl, n| cl == "java/lang/reflect/AccessibleObject" && n == "setAccessible").len();
        if n >= 5 {
            s.push("R19", "r19_setaccessible", w.mass_set_accessible, false, "jar", format!("setAccessible(true) в {n} классах"));
        }
    }

    // --- R20: mixin на сессию/аккаунты/сеть ---
    for c in &ev.classes {
        let is_mixin = c
            .interfaces
            .iter()
            .any(|i| i == "org/spongepowered/asm/mixin/Mixin" || i == "org/spongepowered/asm/mixin/Extends");
        if is_mixin {
            let target = c
                .class_refs
                .iter()
                .chain(c.strings.iter())
                .find(|r| SESSION_CLASSES.iter().any(|t| r.to_ascii_lowercase().contains(t)));
            if let Some(t) = target {
                s.push("R20", "r20_session_mixin", w.session_mixin, false, &c.name, format!("mixin на класс сессии/сети: {}", trunc(t, 70)));
            }
        }
    }

    // --- R21: дубли и перекрытия классов ---
    if let Some(nb) = &ctx.neighbors {
        if !nb.overrides.is_empty() {
            let list: Vec<String> = nb.overrides.iter().take(3).cloned().collect();
            s.push("R21", "r21_override", w.override_mc_loader, false, "jar", format!("перекрытие классов игры/загрузчика: {}", list.join(", ")));
        }
        if !nb.duplicated.is_empty() {
            let list: Vec<String> = nb
                .duplicated
                .iter()
                .take(3)
                .map(|(c, other)| format!("{c} (также в {other})"))
                .collect();
            s.push("R21", "r21_duplicate", w.duplicate_classes, false, "jar", format!("классы, дублирующиеся с другими модами: {}", list.join("; ")));
        }
    }

    // --- R22: метаданные ---
    if !ev.classes.is_empty() && ev.mod_json.is_none() && ev.mods_toml.is_none() {
        s.push("R22", "r22_no_meta", w.no_metadata, false, "jar", "нет метаданных мода (fabric.mod.json/mods.toml)".into());
    } else {
        let mut bare = false;
        if let Some(j) = &ev.mod_json {
            let no_desc = j.get("description").and_then(|v| v.as_str()).unwrap_or("").trim().is_empty();
            let no_contact = j.get("contact").map(|c| c.as_object().map(|m| m.is_empty()).unwrap_or(true)).unwrap_or(true);
            if no_desc && no_contact {
                bare = true;
            }
        }
        if bare {
            s.push("R22", "r22_bare", w.bare_metadata, false, "jar", "метаданные без описания и контактных ссылок".into());
        }
        // modid из fabric и forge метаданных не совпадают.
        if let (Some(j), Some(t)) = (&ev.mod_json, &ev.mods_toml) {
            let fid = j.get("id").and_then(|v| v.as_str());
            let tid = t
                .get("mods")
                .and_then(|m| m.as_array())
                .and_then(|a| a.first())
                .and_then(|m| m.get("modId"))
                .and_then(|v| v.as_str());
            if let (Some(a), Some(b)) = (fid, tid) {
                if !a.eq_ignore_ascii_case(b) {
                    s.push("R22", "r22_bare", w.bare_metadata, false, "jar", format!("разные modid в метаданных: {a} и {b}"));
                }
            }
        }
    }

    s.findings
}

/// Вес строковой сигнатуры по правилу.
fn sig_weight(rule: &str, cfg: &ScannerConfig) -> i32 {
    let w = &cfg.weights;
    match rule {
        "R1" => w.exfil_channel,
        "R2" => w.data_theft,
        "R3" => w.process_exec,
        "R16" => w.fingerprinting,
        _ => 0,
    }
}

/// `&'static str` из сигнатуры (значения и так статические).
fn static_kind(s: &str) -> &str {
    SIGS.iter().find(|sig| sig.kind == s).map(|sig| sig.kind).unwrap_or("признак")
}

#[cfg(test)]
mod tests {
    use super::super::config::ScannerConfig;
    use super::super::evidence::evidence_from_bytes;
    use super::super::synthetic::{jar, ClassBuilder};
    use super::*;

    fn scan(entries: &[(String, Vec<u8>)]) -> Vec<Finding> {
        let bytes = jar(entries);
        let ev = evidence_from_bytes(&bytes, &super::super::config::Limits::default());
        analyze(&ev, &ScanContext::default(), &ScannerConfig::with_default_whitelist())
    }

    fn has(findings: &[Finding], rule: &str) -> bool {
        findings.iter().any(|f| f.rule == rule)
    }

    fn meta(id: &str) -> (String, Vec<u8>) {
        (
            "fabric.mod.json".into(),
            format!(r#"{{"id":"{id}","version":"1.0","description":"тест"}}"#).into_bytes(),
        )
    }

    /// Нормальный безобидный мод: метаданные, пакеты по modid, ничего лишнего.
    #[test]
    fn clean_mod_has_no_findings() {
        let cls = ClassBuilder::new("com/example/cleanmod/Main")
            .string("Привет из мода")
            .method("onInitialize", "()V")
            .build();
        let findings = scan(&[meta("cleanmod"), ("com/example/cleanmod/Main.class".into(), cls)]);
        assert!(findings.is_empty(), "находки: {findings:?}");
    }

    /// Регрессии с настоящих чистых модов (cloth-config, lithium, modernfix).
    #[test]
    fn regress_no_false_positives_on_clean_mods() {
        // «CompactData» содержит подстроку tdata — не должно ловиться.
        let yaml = ClassBuilder::new("org/yaml/CompactData")
            .string("CompactData notation")
            .build();
        let f = scan(&[meta("yaml"), ("org/yaml/CompactData.class".into(), yaml)]);
        assert!(!f.iter().any(|x| x.what.contains("tdata")), "находки: {f:?}");

        // Настоящий путь к tdata ловится.
        let tg = ClassBuilder::new("com/x/T")
            .string(r"C:\Users\bob\AppData\Roaming\Telegram Desktop\tdata\key_datas")
            .build();
        let f2 = scan(&[meta("t"), ("com/x/T.class".into(), tg)]);
        assert!(f2.iter().any(|x| x.what.contains("tdata")));

        // Shaded-библиотеки под одним доменом автора — это не «чужие пакеты».
        let mut entries = vec![meta("cloth-config")];
        for i in 0..6 {
            let cls = ClassBuilder::new(&format!("me/author/cloth/Class{i}")).build();
            entries.push((format!("me/author/cloth/Class{i}.class"), cls));
        }
        let f3 = scan(&entries);
        assert!(!f3.iter().any(|x| x.code == "r6_outside"), "находки: {f3:?}");

        // Unsafe отдельно от ClassLoader — мягкий код и вес.
        let li = ClassBuilder::new("net/mods/lithium/Chunk")
            .method_ref("jdk/internal/misc/Unsafe", "allocateUninitializedArray", "(Ljava/lang/Class;I)Ljava/lang/Object;")
            .build();
        let f4 = scan(&[meta("lithium"), ("net/mods/lithium/Chunk.class".into(), li)]);
        let unsafe_f = f4.iter().find(|x| x.code == "r4_unsafe").expect("unsafe finding");
        assert_eq!(unsafe_f.weight, 10);
        assert!(!f4.iter().any(|x| x.code == "r4_dynamic"));

        // getResourceAsStream + «.class» без шифрования — не признак.
        let mf = ClassBuilder::new("org/mods/fix/Cfg")
            .method_ref("java/lang/Class", "getResourceAsStream", "(Ljava/lang/String;)Ljava/io/InputStream;")
            .string("coremods/patch.class")
            .build();
        let f5 = scan(&[meta("modernfix"), ("org/mods/fix/Cfg.class".into(), mf)]);
        assert!(!f5.iter().any(|x| x.code == "r4_dynamic"), "находки: {f5:?}");

        // То же с Cipher — уже расшифровка классов.
        let enc = ClassBuilder::new("org/evil/Enc")
            .method_ref("java/lang/Class", "getResourceAsStream", "(Ljava/lang/String;)Ljava/io/InputStream;")
            .method_ref("javax/crypto/Cipher", "doFinal", "([B)[B")
            .string("payload/hidden.class")
            .build();
        let f6 = scan(&[meta("encmod"), ("org/evil/Enc.class".into(), enc)]);
        assert!(f6.iter().any(|x| x.code == "r4_dynamic"));
    }

    #[test]
    fn scattered_packages_flagged() {
        let mut entries = vec![meta("scattered")];
        for pkg in ["alpha", "beta", "gamma", "delta", "zeta", "eta"] {
            let cls = ClassBuilder::new(&format!("{pkg}/C")).build();
            entries.push((format!("{pkg}/C.class"), cls));
        }
        let f = scan(&entries);
        assert!(f.iter().any(|x| x.code == "r6_outside"), "находки: {f:?}");
    }

    #[test]
    fn r1_discord_webhook_is_hard() {
        let cls = ClassBuilder::new("com/example/w/W")
            .string("https://discord.com/api/webhooks/123456/abcdef")
            .build();
        let f = scan(&[meta("w"), ("com/example/w/W.class".into(), cls)]);
        assert!(has(&f, "R1"));
        let hit = f.iter().find(|x| x.rule == "R1").unwrap();
        assert!(hit.hard);
        assert_eq!(hit.weight, 45);
    }

    #[test]
    fn r1_bare_ip_url() {
        let cls = ClassBuilder::new("com/example/ip/Ip")
            .string("http://185.10.10.10:8080/ping")
            .build();
        let f = scan(&[meta("ip"), ("com/example/ip/Ip.class".into(), cls)]);
        assert!(has(&f, "R1"));
    }

    #[test]
    fn r2_browser_cookies_combo() {
        let cls = ClassBuilder::new("com/example/b/B")
            .string(r"C:\Users\x\AppData\Local\Google\Chrome\User Data\Login Data")
            .build();
        let f = scan(&[meta("b"), ("com/example/b/B.class".into(), cls)]);
        assert!(has(&f, "R2"));
    }

    #[test]
    fn r2_cookies_without_browser_context_is_clean() {
        // Слово Cookies само по себе не должно карать.
        let cls = ClassBuilder::new("com/example/c/C")
            .string(" Мы используем cookies на сайте мода ")
            .build();
        let f = scan(&[meta("c"), ("com/example/c/C.class".into(), cls)]);
        assert!(!has(&f, "R2"), "находки: {f:?}");
    }

    #[test]
    fn r2_screen_capture() {
        let cls = ClassBuilder::new("com/example/s/S")
            .method_ref("java/awt/Robot", "createScreenCapture", "(Ljava/awt/Rectangle;)Ljava/awt/image/BufferedImage;")
            .build();
        let f = scan(&[meta("s"), ("com/example/s/S.class".into(), cls)]);
        assert!(has(&f, "R2"));
    }

    #[test]
    fn r3_runtime_exec() {
        let cls = ClassBuilder::new("com/example/e/E")
            .method_ref("java/lang/Runtime", "exec", "(Ljava/lang/String;)Ljava/lang/Process;")
            .build();
        let f = scan(&[meta("e"), ("com/example/e/E.class".into(), cls)]);
        assert!(has(&f, "R3"));
    }

    #[test]
    fn r5_executable_url_and_combo() {
        let url = ClassBuilder::new("com/example/d/D")
            .string("https://github.com/evil/x/releases/download/1.0/payload.exe")
            .build();
        let f = scan(&[meta("d"), ("com/example/d/D.class".into(), url.clone())]);
        assert!(has(&f, "R5"), "url only: {f:?}");

        let writer = ClassBuilder::new("com/example/d/Writer")
            .method_ref("java/io/FileOutputStream", "<init>", "(Ljava/lang/String;)V")
            .build();
        let runner = ClassBuilder::new("com/example/d/Runner")
            .method_ref("java/lang/ProcessBuilder", "<init>", "([Ljava/lang/String;)V")
            .build();
        let f2 = scan(&[
            meta("d"),
            ("com/example/d/D.class".into(), url),
            ("com/example/d/Writer.class".into(), writer),
            ("com/example/d/Runner.class".into(), runner),
        ]);
        let combo = f2.iter().find(|x| x.what.contains("запуск процесса"));
        assert!(combo.is_some(), "комбо: {f2:?}");
        assert_eq!(combo.unwrap().weight, 60);
    }

    #[test]
    fn r5_whitelisted_domain_is_still_flagged_for_exe() {
        // Красный флаг на ЛЮБОМ домене, включая modrinth cdn.
        let cls = ClassBuilder::new("com/example/m/M")
            .string("https://cdn.modrinth.com/data/xxx/files/tool.exe")
            .build();
        let f = scan(&[meta("m"), ("com/example/m/M.class".into(), cls)]);
        assert!(has(&f, "R5"));
    }

    #[test]
    fn r6_path_traversal_and_natives() {
        let cls = ClassBuilder::new("com/example/t/T").build();
        let f = scan(&[
            meta("t"),
            ("com/example/t/T.class".into(), cls),
            ("../evil.jar".into(), b"x".to_vec()),
            ("native/win32/x.dll".into(), b"MZ".to_vec()),
        ]);
        assert!(has(&f, "R6"));
        assert!(f.iter().any(|x| x.what.contains("../")));
        assert!(f.iter().any(|x| x.what.contains("нативная")));
    }

    #[test]
    fn r6_jar_in_jar_is_light() {
        let cls = ClassBuilder::new("com/example/j/J").build();
        let f = scan(&[
            meta("j"),
            ("com/example/j/J.class".into(), cls),
            ("META-INF/jars/library-1.0.jar".into(), b"PK".to_vec()),
        ]);
        let jij = f.iter().find(|x| x.what.contains("jar-in-jar"));
        assert!(jij.is_some());
        assert!(jij.unwrap().weight <= 5);
    }

    #[test]
    fn r7_obfuscated_names() {
        let mut entries = vec![meta("obf")];
        for n in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"] {
            let cls = ClassBuilder::new(&format!("com/example/obf/{n}")).build();
            entries.push((format!("com/example/obf/{n}.class"), cls));
        }
        let f = scan(&entries);
        assert!(has(&f, "R7"), "находки: {f:?}");
    }

    #[test]
    fn r8_base64_hides_webhook() {
        use super::super::decode::base64_decode;
        let enc = base64_decode("aHR0cHM6Ly9hcGkudGVsZWdyYW0ub3JnL2JvdDEyMzphYmM=").unwrap();
        let mut b64 = String::new();
        b64.push_str(&enc.iter().map(|&b| b as char).collect::<String>());
        // закодируем заново вручную нельзя — используем уже base64 строку:
        let b64 = "aHR0cHM6Ly9hcGkudGVsZWdyYW0ub3JnL2JvdDEyMzphYmM=".to_string();
        let cls = ClassBuilder::new("com/example/enc/Enc")
            .string(&b64)
            .build();
        let f = scan(&[meta("enc"), ("com/example/enc/Enc.class".into(), cls)]);
        assert!(has(&f, "R1"), "декодированные находки: {f:?}");
        assert!(f.iter().any(|x| x.what.contains("base64 скрывает")));
    }

    #[test]
    fn r9_unknown_domain_flagged_and_known_allowed() {
        let bad = ClassBuilder::new("com/example/n/N")
            .string("https://totally-evil.example.xyz/collect")
            .build();
        let f = scan(&[meta("n"), ("com/example/n/N.class".into(), bad)]);
        assert!(has(&f, "R9"));

        let good = ClassBuilder::new("com/example/n2/N")
            .string("https://api.modrinth.com/v2/project/x")
            .build();
        let f2 = scan(&[meta("n2"), ("com/example/n2/N.class".into(), good)]);
        assert!(!has(&f2, "R9"), "находки: {f2:?}");
    }

    #[test]
    fn r9_metadata_repo_host_allowed() {
        let meta_json = r#"{"id":"n3","version":"1.0","description":"d","contact":{"repo":"https://gitlab.com/user/n3"}}"#;
        let cls = ClassBuilder::new("com/example/n3/N")
            .string("https://gitlab.com/user/n3/-/raw/master/update.json")
            .build();
        let f = scan(&[
            ("fabric.mod.json".into(), meta_json.as_bytes().to_vec()),
            ("com/example/n3/N.class".into(), cls),
        ]);
        assert!(!has(&f, "R9"), "находки: {f:?}");
    }

    #[test]
    fn r11_known_malware_hash() {
        let cls = ClassBuilder::new("com/example/mm/M").build();
        let bytes = jar(&[meta("mm"), ("com/example/mm/M.class".into(), cls)]);
        let ev = evidence_from_bytes(&bytes, &super::super::config::Limits::default());
        let ctx = ScanContext {
            known_malware: true,
            ..ScanContext::default()
        };
        let f = analyze(&ev, &ctx, &ScannerConfig::with_default_whitelist());
        assert!(f.iter().any(|x| x.rule == "R11" && x.hard && x.weight >= 100));
    }

    #[test]
    fn r12_manifest_agent() {
        let manifest = b"Manifest-Version: 1.0\nPremain-Class: evil.Agent\n";
        let cls = ClassBuilder::new("com/example/ag/Ag").build();
        let f = scan(&[
            meta("ag"),
            ("com/example/ag/Ag.class".into(), cls),
            ("META-INF/MANIFEST.MF".into(), manifest.to_vec()),
        ]);
        assert!(has(&f, "R12"));
    }

    #[test]
    fn r13_writes_into_mods_jar() {
        let cls = ClassBuilder::new("com/example/inf/Inf")
            .string("mods/other-mod.jar")
            .method_ref("java/io/FileOutputStream", "<init>", "(Ljava/lang/String;)V")
            .build();
        let f = scan(&[meta("inf"), ("com/example/inf/Inf.class".into(), cls)]);
        assert!(has(&f, "R13"), "находки: {f:?}");
    }

    #[test]
    fn r14_system_load_and_native() {
        let cls = ClassBuilder::new("com/example/nl/Nl")
            .method_ref("java/lang/System", "load", "(Ljava/lang/String;)V")
            .native_method("nativeThing", "()V")
            .build();
        let f = scan(&[meta("nl"), ("com/example/nl/Nl.class".into(), cls)]);
        assert!(f.iter().filter(|x| x.rule == "R14").count() >= 2);
    }

    #[test]
    fn r15_script_engine() {
        let cls = ClassBuilder::new("com/example/sc/Sc")
            .method_ref("javax/script/ScriptEngineManager", "<init>", "()V")
            .build();
        let f = scan(&[meta("sc"), ("com/example/sc/Sc.class".into(), cls)]);
        assert!(has(&f, "R15"));
    }

    #[test]
    fn r16_fingerprint() {
        let cls = ClassBuilder::new("com/example/fp/Fp")
            .method_ref("java/lang/System", "getenv", "(Ljava/lang/String;)Ljava/lang/String;")
            .string("user.name")
            .string("os.name")
            .method_ref("java/lang/System", "getProperty", "(Ljava/lang/String;)Ljava/lang/String;")
            .build();
        let f = scan(&[meta("fp"), ("com/example/fp/Fp.class".into(), cls)]);
        assert!(f.iter().filter(|x| x.rule == "R16").count() >= 2);
    }

    #[test]
    fn r17_long_sleep() {
        let cls = ClassBuilder::new("com/example/dl/Dl")
            .method_ref("java/lang/Thread", "sleep", "(J)V")
            .int_constant(86_400_000)
            .build();
        let f = scan(&[meta("dl"), ("com/example/dl/Dl.class".into(), cls)]);
        assert!(has(&f, "R17"));
    }

    #[test]
    fn r18_shutdown_hook_and_debugger() {
        let cls = ClassBuilder::new("com/example/aa/Aa")
            .method_ref("java/lang/Runtime", "addShutdownHook", "(Ljava/lang/Thread;)V")
            .string("com.sun.jdi.isDebuggerConnected")
            .build();
        let f = scan(&[meta("aa"), ("com/example/aa/Aa.class".into(), cls)]);
        assert!(f.iter().filter(|x| x.rule == "R18").count() >= 2);
    }

    #[test]
    fn r19_mass_set_accessible() {
        let mut entries = vec![meta("sa")];
        for i in 0..6 {
            let cls = ClassBuilder::new(&format!("com/example/sa/C{i}"))
                .method_ref("java/lang/reflect/AccessibleObject", "setAccessible", "(Z)V")
                .build();
            entries.push((format!("com/example/sa/C{i}.class"), cls));
        }
        let f = scan(&entries);
        assert!(has(&f, "R19"));
    }

    #[test]
    fn r20_mixin_on_session() {
        let cls = ClassBuilder::new("com/example/mx/MxSession")
            .interface("org/spongepowered/asm/mixin/Mixin")
            .method_ref("net/minecraft/util/Session", "getToken", "()Ljava/lang/String;")
            .build();
        let f = scan(&[meta("mx"), ("com/example/mx/MxSession.class".into(), cls)]);
        assert!(has(&f, "R20"), "находки: {f:?}");
    }

    #[test]
    fn r21_overrides_and_duplicates() {
        let cls = ClassBuilder::new("com/example/ov/Ov").build();
        let bytes = jar(&[meta("ov"), ("com/example/ov/Ov.class".into(), cls)]);
        let ev = evidence_from_bytes(&bytes, &super::super::config::Limits::default());
        let ctx = ScanContext {
            neighbors: Some(Neighbors {
                duplicated: vec![("com/example/shared/S".into(), "other.jar".into())],
                overrides: vec!["net/minecraft/server/MinecraftServer".into()],
            }),
            ..ScanContext::default()
        };
        let f = analyze(&ev, &ctx, &ScannerConfig::with_default_whitelist());
        assert!(has(&f, "R21"));
        assert!(f.iter().any(|x| x.what.contains("перекрытие")));
    }

    #[test]
    fn r22_no_metadata() {
        let cls = ClassBuilder::new("com/example/nm/Nm").build();
        let f = scan(&[("com/example/nm/Nm.class".into(), cls)]);
        assert!(has(&f, "R22"));
    }

    #[test]
    fn xor_encoded_webhook_is_found() {
        let enc: String = "https://pastebin.com/api"
            .bytes()
            .map(|b| (b ^ 0x42) as char)
            .collect();
        let cls = ClassBuilder::new("com/example/xr/Xr")
            .string(&enc)
            .build();
        let f = scan(&[meta("xr"), ("com/example/xr/Xr.class".into(), cls)]);
        assert!(f.iter().any(|x| x.what.contains("XOR")), "находки: {f:?}");
    }
}
