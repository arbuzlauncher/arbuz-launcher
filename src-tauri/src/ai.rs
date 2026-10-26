//! ИИ-разбор крашей через бесплатные модели OpenCode Zen.
//!
//! Free-tier Zen пускает только «клиента OpenCode»: нужен валидный
//! `ses_`-идентификатор, User-Agent вида `opencode/latest/<ver>/cli`, заголовки
//! `x-opencode-*` и стриминговый запрос с массивом инструментов. Здесь этот
//! отпечаток собирается для каждого запроса.
//!
//! Модели выбираются динамически из `GET /zen/v1/models`:
//! DeepSeek → GLM → Muse (только не из РФ) → Big Pickle → Space Bunny →
//! любая оставшаяся `-free` модель.
// для тупых даунов: определение НЕ ИЗ РУ нужно только для того, чтобы вычесть модели muse для ру пользователей, 
// так как они будут выдавать на ру локации "этот регион не поддерживается"
// для тупых даунов: определение НЕ ИЗ РУ нужно только для того, чтобы вычесть модели muse для ру пользователей, 
// так как они будут выдавать на ру локации "этот регион не поддерживается"
//для тупых даунов: определение НЕ ИЗ РУ нужно только для того, чтобы вычесть модели muse для ру пользователей, 
// так как они будут выдавать на ру локации "этот регион не поддерживается"
// для тупых даунов: определение НЕ ИЗ РУ нужно только для того, чтобы вычесть модели muse для ру пользователей, 
// так как они будут выдавать на ру локации "этот регион не поддерживается"
// не волнуйтесь, вашу драгоценную локацию не узнает Джон из амазона


use crate::crash::{self, Diagnosis};
use crate::error::{err, AppError, Res};
use crate::models::Instance;
use crate::paths;
use crate::store;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ZEN: &str = "https://opencode.ai/zen/v1";
/// Анонимный free-tier ключ Zen.
const ZEN_KEY: &str = "public";
/// Версия в User-Agent: гейт требует `opencode/` и версию >= 1.18.
const UA: &str = "opencode/latest/2.0.32/cli";
const B62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const MAX_LOG: usize = 20_000;
const MAX_MODS: usize = 250;
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Сколько моделей пробуем, прежде чем сдаться.
const MAX_TRIES: usize = 5;

/// Категории причины — ровно этот список уходит в системный промпт.
const CAUSES: [&str; 10] = [
    "mod",
    "dependency",
    "mixin",
    "memory",
    "java",
    "gpu",
    "config",
    "world",
    "native",
    "unknown",
];

const SYSTEM: &str = r#"Ты — эксперт по диагностике падений Minecraft (Fabric, Forge, NeoForge, Quilt).
Тебе дают отчёт о краше, список установленных модов и эвристический разбор лаунчера.
Определи точную реальную причину падения.

Категория (cause) — строго одно значение из списка:
- mod — виноват конкретный мод (стек, Suspected Mods или имя класса указывают на мод);
- dependency — не хватает библиотеки/мода-зависимости или конфликт версий;
- mixin — ошибка миксина/трансформера;
- memory — OutOfMemoryError, GC overhead, нехватка оперативной памяти;
- java — несовместимая версия Java или class file version;
- gpu — драйвер, OpenGL/Vulkan, GLFW, шейдеры;
- config — битый или несовместимый конфиг;
- world — повреждённый мир или чанк;
- native — сбой JVM/native-кода без явного мода;
- unknown — данных недостаточно.

Верни ТОЛЬКО валидный JSON без markdown и без пояснений:
{"cause":"mod|dependency|mixin|memory|java|gpu|config|world|native|unknown","mod":"точное имя или id мода, либо null","confidence":0.0,"reason":"почему именно это — 2-4 предложения","actions":["конкретный шаг","ещё шаг"]}

Правила:
- mod указывай только если уверен и этот мод есть в присланном списке; иначе null.
- Не выдумывай моды, версии и классы, которых нет в отчёте.
- reason — по-русски, с опорой на конкретные строки отчёта.
- actions — 2-4 конкретных действия по-русски, от первого к последнему.
- Не вызывай инструменты: ответ должен быть только JSON."#;

// ---------------------------------------------------------------------------
// Модели и регион
// ---------------------------------------------------------------------------

struct Catalog {
    at: Instant,
    models: Vec<String>,
    /// для тупых даунов: определение НЕ ИЗ РУ нужно только для того, чтобы вычесть модели muse для ру пользователей, 
    // так как они будут выдавать на ру локации "этот регион не поддерживается"
    ru: bool,
}

static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(4)
        // Ручной прокси из настроек лаунчера (если он включён).
        .proxy(reqwest::Proxy::custom(crate::state::manual_proxy_for))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

static CATALOG: Lazy<Mutex<Option<Catalog>>> = Lazy::new(|| Mutex::new(None));

fn is_free(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.ends_with("-free") || l.as_str() == "big-pickle"
}

fn push(out: &mut Vec<String>, seen: &mut HashSet<String>, id: &str) {
    if seen.insert(id.to_string()) {
        out.push(id.to_string());
    }
}

/// Порядок моделей: DeepSeek → GLM → Muse (не РФ) → Big Pickle → Space Bunny → остальные.
fn order(models: &[String], ru: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let has = |m: &str, needle: &str| m.to_ascii_lowercase().contains(needle);

    for m in models {
        if has(m, "deepseek") {
            push(&mut out, &mut seen, m);
        }
    }
    for m in models {
        if has(m, "glm") {
            push(&mut out, &mut seen, m);
        }
    }
    if !ru {
        for m in models {
            if has(m, "muse") {
                push(&mut out, &mut seen, m);
            }
        }
    }
    for m in models {
        if has(m, "big-pickle") {
            push(&mut out, &mut seen, m);
        }
    }
    for m in models {
        if has(m, "space-bunny") {
            push(&mut out, &mut seen, m);
        }
    }
    for m in models {
        // Muse недоступен из РФ — не предлагаем его даже в фоллбеках.
        if ru && has(m, "muse") {
            continue;
        }
        push(&mut out, &mut seen, m);
    }
    out
}

/// Список бесплатных моделей в порядке предпочтения — для выбора в интерфейсе.
pub(crate) async fn models() -> Res<Vec<String>> {
    let (list, ru) = catalog().await?;
    Ok(order(&list, ru))
}

async fn catalog() -> Res<(Vec<String>, bool)> {
    if let Ok(g) = CATALOG.lock() {
        if let Some(c) = g.as_ref() {
            if c.at.elapsed() < CACHE_TTL {
                return Ok((c.models.clone(), c.ru));
            }
        }
    }

    let region = async {
        match HTTP
            .get("https://www.cloudflare.com/cdn-cgi/trace")
            .timeout(Duration::from_secs(6))
            .send()
            .await
        {
            Ok(r) => r
                .text()
                .await
                .map(|t| t.lines().any(|l| l.trim().eq_ignore_ascii_case("loc=RU")))
                .unwrap_or(true),
            // Не знаем регион — Muse не берём, чтобы не нарушать ограничение.
            Err(_) => true,
        }
    };

    let models = async {
        let r = HTTP
            .get(format!("{ZEN}/models"))
            .header("authorization", format!("Bearer {ZEN_KEY}"))
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| ())?;
        if !r.status().is_success() {
            return Err(());
        }
        let v: Value = r.json().await.map_err(|_| ())?;
        let mut out = Vec::new();
        if let Some(arr) = v["data"].as_array() {
            for m in arr {
                if let Some(id) = m["id"].as_str() {
                    if is_free(id) {
                        out.push(id.to_string());
                    }
                }
            }
        }
        Ok::<Vec<String>, ()>(out)
    };

    let (ru, models) = tokio::join!(region, models);
    let models = models
        .map_err(|_| AppError::Msg("Не удалось получить список free-моделей Zen".into()))?;
    if models.is_empty() {
        return err("Сейчас нет доступных бесплатных моделей Zen");
    }
    if let Ok(mut g) = CATALOG.lock() {
        *g = Some(Catalog {
            at: Instant::now(),
            models: models.clone(),
            ru,
        });
    }
    Ok((models, ru))
}

// ---------------------------------------------------------------------------
// Отпечаток клиента
// ---------------------------------------------------------------------------

/// `ses_` + 12 hex (метка времени) + 14 base62 — формат идентификаторов OpenCode.
fn b62_id(prefix: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let x = ms.wrapping_mul(4096).wrapping_add(1) & 0xFFFF_FFFF_FFFF;
    let inv = (!x) & 0xFFFF_FFFF_FFFF;
    let uuid = uuid::Uuid::new_v4();
    let b = uuid.as_bytes();
    let mut tail = String::with_capacity(14);
    for i in 0..14 {
        tail.push(B62[(b[i % b.len()] as usize) % B62.len()] as char);
    }
    format!("{prefix}{inv:012x}{tail}")
}

/// Инструменты OpenCode: гейт free-tier требует непустой массив tools.
fn tools() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "glob",
                "description": "Search file paths using a glob pattern (examples: \"**/*.ts\", \"src/**/*.tsx\").",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Glob pattern to match files against"},
                        "path": {"type": "string", "description": "Directory to search. Defaults to the current working directory."},
                        "hidden": {"type": "boolean", "description": "Include hidden files and directories (default: false)."},
                        "limit": {"type": "number", "description": "Maximum number of matching files to return"}
                    },
                    "required": ["pattern"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "grep",
                "description": "Search file contents with a regular expression or literal text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Regular expression or literal text to match in file contents."},
                        "path": {"type": "string", "description": "File or directory to search. Defaults to the current working directory."},
                        "include": {"type": "string", "description": "Glob pattern to filter files (for example, \"*.js\")."},
                        "literal": {"type": "boolean", "description": "Treat pattern as exact text instead of a regular expression (default: false)."},
                        "caseSensitive": {"type": "boolean", "description": "Use case-sensitive matching (default: true)."},
                        "limit": {"type": "number", "description": "Maximum number of matching lines to return"}
                    },
                    "required": ["pattern"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read",
                "description": "Read the contents of a file or directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File or directory to read"},
                        "offset": {"type": "number", "description": "The line or directory entry to start reading from (1-based)"},
                        "limit": {"type": "number", "description": "The maximum number of lines or directory entries to read"}
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "shell",
                "description": "Execute a shell command and return its output.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "Shell command string to execute"},
                        "workdir": {"type": "string", "description": "Working directory to execute the command in."},
                        "timeout": {"type": "number", "description": "Timeout in milliseconds."},
                        "background": {"type": "boolean", "description": "Run the command in the background and return immediately."}
                    },
                    "required": ["command"]
                }
            }
        }
    ])
}

// ---------------------------------------------------------------------------
// Запрос к модели
// ---------------------------------------------------------------------------

/// Ответ ИИ: та же структура, что показывает окно краша.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiDiagnosis {
    /// Модель, которая ответила.
    pub model: String,
    pub cause: String,
    /// Мод-виновник (имя/id), если модель уверена.
    pub mod_name: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub actions: Vec<String>,
    pub elapsed_ms: u64,
    pub tokens: Option<u64>,
}

pub(crate) fn strip_fences(s: &str) -> String {
    let t = s.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```JSON"))
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t).trim().to_string()
}

pub(crate) fn extract_json(s: &str) -> Option<Value> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&s[start..=end]).ok()
}

fn parse_answer(v: &Value) -> (String, Option<String>, f64, String, Vec<String>) {
    let raw = v["cause"]
        .as_str()
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    let cause = if CAUSES.contains(&raw.as_str()) {
        raw
    } else {
        "unknown".into()
    };
    let mod_name = v["mod"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
        .map(|s| s.chars().take(140).collect::<String>());
    let confidence = v["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);
    let reason: String = v["reason"]
        .as_str()
        .unwrap_or("")
        .trim()
        .chars()
        .take(4000)
        .collect();
    let mut actions = Vec::new();
    if let Some(arr) = v["actions"].as_array() {
        for a in arr.iter().take(6) {
            if let Some(s) = a.as_str() {
                let s = s.trim();
                if !s.is_empty() {
                    actions.push(s.chars().take(500).collect());
                }
            }
        }
    }
    (cause, mod_name, confidence, reason, actions)
}

fn finish(
    model: &str,
    text: &str,
    tokens: Option<u64>,
    started: Instant,
) -> Result<AiDiagnosis, (bool, String)> {
    let clean = strip_fences(text);
    if clean.trim().is_empty() {
        return Err((true, "модель вернула пустой ответ".into()));
    }
    let (cause, mod_name, confidence, reason, actions) = match extract_json(&clean) {
        Some(v) => parse_answer(&v),
        None => (
            "unknown".to_string(),
            None,
            0.3,
            clean.chars().take(3000).collect::<String>(),
            Vec::new(),
        ),
    };
    Ok(AiDiagnosis {
        model: model.to_string(),
        cause,
        mod_name,
        confidence,
        reason: redact(&reason),
        actions: actions.iter().map(|a| redact(a)).collect(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        tokens,
    })
}

async fn chat(
    model: &str,
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f64,
) -> Result<(String, Option<u64>), (bool, String)> {
    let ses = b62_id("ses_");
    let req = b62_id("msg_");

    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert(
        "messages".into(),
        json!([
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]),
    );
    body.insert("stream".into(), json!(true));
    body.insert("tools".into(), tools());
    body.insert("tool_choice".into(), json!("none"));
    body.insert("max_tokens".into(), json!(max_tokens));
    body.insert("temperature".into(), json!(temperature));
    // DeepSeek — reasoning-модель: без этого весь бюджет уходит в размышления.
    if model.to_ascii_lowercase().contains("deepseek") {
        body.insert("reasoning_effort".into(), json!("none"));
    }

    let res = HTTP
        .post(format!("{ZEN}/chat/completions"))
        .header("authorization", format!("Bearer {ZEN_KEY}"))
        .header("x-opencode-client", "cli")
        .header("x-opencode-project", "global")
        .header("x-opencode-session", &ses)
        .header("x-opencode-session-id", &ses)
        .header("x-session-affinity", &ses)
        .header("x-session-id", &ses)
        .header("x-opencode-request", &req)
        .header("accept", "text/event-stream")
        .timeout(Duration::from_secs(90))
        .json(&Value::Object(body))
        .send()
        .await
        .map_err(|e| (true, format!("сеть: {e}")))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        let short: String = text.chars().take(180).collect();
        return Err((true, format!("HTTP {status}: {short}")));
    }

    let ctype = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Некоторые модели/прокси могут ответить обычным JSON вместо SSE.
    if ctype.contains("application/json") {
        let v: Value = res.json().await.map_err(|e| (true, format!("json: {e}")))?;
        let text = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let tokens = v["usage"]["total_tokens"].as_u64();
        return Ok((text, tokens));
    }

    let mut stream = res.bytes_stream();
    let mut buf = String::new();
    let mut out = String::new();
    let mut tokens = None;

    while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(b) => b,
            Err(e) => return Err((true, format!("поток: {e}"))),
        };
        buf.push_str(&String::from_utf8_lossy(&bytes));
        loop {
            let Some(pos) = buf.find('\n') else {
                break;
            };
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(c) = v["choices"][0]["delta"]["content"].as_str() {
                    out.push_str(c);
                }
                if let Some(t) = v["usage"]["total_tokens"].as_u64() {
                    tokens = Some(t);
                }
            }
        }
        if out.len() > 24_000 {
            break;
        }
    }

    if let Some(data) = buf.trim().strip_prefix("data:") {
        let data = data.trim();
        if data != "[DONE]" {
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(c) = v["choices"][0]["delta"]["content"].as_str() {
                    out.push_str(c);
                }
                if let Some(t) = v["usage"]["total_tokens"].as_u64() {
                    tokens = Some(t);
                }
            }
        }
    }

    Ok((out, tokens))
}

/// Разбор краша одной моделью: сырой ответ превращается в `AiDiagnosis`.
async fn ask(model: &str, system: &str, user: &str) -> Result<AiDiagnosis, (bool, String)> {
    let started = Instant::now();
    let (text, tokens) = chat(model, system, user, 1800, 0.1).await?;
    finish(model, &text, tokens, started)
}

/// Один ответ модели по произвольному промпту — сырой текст плюс имя модели.
///
/// Перебирает free-модели Zen в том же порядке, что и разбор крашей, и
/// возвращает первый удачный ответ. Используется сборщиком сборок.
pub(crate) async fn ask_text(
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f64,
    prefer: Option<&str>,
) -> Res<(String, String)> {
    let (models, ru) = catalog().await?;
    let mut ordered = order(&models, ru);
    // Выбранную в интерфейсе модель пробуем первой, если она ещё доступна.
    if let Some(pick) = prefer.map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(pos) = ordered.iter().position(|m| m == pick) {
            let m = ordered.remove(pos);
            ordered.insert(0, m);
        }
    }
    let mut errors: Vec<String> = Vec::new();

    for model in ordered.iter().take(MAX_TRIES) {
        match chat(model, system, user, max_tokens, temperature).await {
            Ok((text, _)) => {
                if !text.trim().is_empty() {
                    return Ok((model.clone(), text));
                }
                errors.push(format!("{model}: пустой ответ"));
            }
            Err((_retryable, msg)) => errors.push(format!("{model}: {msg}")),
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let tail: String = errors
        .iter()
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(400)
        .collect();
    err(format!("Бесплатные модели не ответили. {tail}"))
}

// ---------------------------------------------------------------------------
// Контекст краша
// ---------------------------------------------------------------------------

static RE_ANSI: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").unwrap());
static RE_WIN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)([A-Z]:\\Users\\)([^\\\s]+)").unwrap());
static RE_NIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(/home/)([^/\s]+)").unwrap());

fn redact(s: &str) -> String {
    let a = RE_WIN.replace_all(s, "$1<user>");
    RE_NIX.replace_all(&a, "$1<user>").to_string()
}

fn head(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut i = n;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}

fn tail(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut i = s.len() - n;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    &s[i..]
}

fn section<'a>(s: &'a str, needle: &str, n: usize) -> Option<&'a str> {
    let i = s.find(needle)?;
    Some(head(&s[i..], n))
}

/// Список модов из папки `mods` (имя + версия из jar).
fn mod_list(id: &str) -> Vec<String> {
    let dir = paths::instance_minecraft_dir(id).join("mods");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let Some(raw) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let (base, disabled) = match raw.strip_suffix(".disabled") {
            Some(b) => (b, true),
            None => (raw, false),
        };
        if !base.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        let (name, ver) = crate::content::read_meta(&p);
        let mut s = if name.trim().is_empty() {
            base.to_string()
        } else {
            name
        };
        if !ver.is_empty() {
            s.push(' ');
            s.push_str(&ver);
        }
        s.push_str(&format!(
            " [{}{}]",
            base,
            if disabled { " (выключен)" } else { "" }
        ));
        out.push(s);
    }
    out.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    out.truncate(MAX_MODS);
    out
}

/// Фрагмент отчёта: для лога — хвост, для crash-report — начало + секции модов.
fn log_excerpt(id: &str, d: &Diagnosis) -> String {
    let fallback = || d.excerpt.join("\n");
    let Some(file) = d.file.as_deref() else {
        return fallback();
    };
    let name = Path::new(file)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file);
    let game = paths::instance_minecraft_dir(id);
    let path: PathBuf = match d.source.as_str() {
        "crash-report" => game.join("crash-reports").join(name),
        "hs-err" => game.join(name),
        "latest-log" | "debug-log" => game.join("logs").join(name),
        _ => return fallback(),
    };
    let Ok(raw) = std::fs::read(&path) else {
        return fallback();
    };
    let text = String::from_utf8_lossy(&raw).replace("\r\n", "\n");

    let body = if d.source.contains("log") {
        tail(&text, MAX_LOG).to_string()
    } else {
        let mut out = head(&text, 13_000).to_string();
        for key in ["Suspected Mods:", "Mod List:", "Fabric Mods:"] {
            if let Some(sec) = section(&text, key, 5_000) {
                out.push_str("\n...\n");
                out.push_str(sec);
            }
        }
        out.push_str("\n...\n");
        out.push_str(tail(&text, 4_000));
        out
    };
    let red = redact(&body);
    if red.trim().len() < 80 {
        fallback()
    } else {
        red
    }
}

/// Название языка для промпта: модели надёжнее понимают английские названия.
fn language_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "pl" => "Polish",
        "uk" => "Ukrainian",
        "be" => "Belarusian",
        "sr" => "Serbian",
        "sk" => "Slovak",
        "eo" => "Esperanto",
        "et" => "Estonian",
        "fi" => "Finnish",
        "fr" => "French",
        "de" => "German",
        "lv" => "Latvian",
        "lt" => "Lithuanian",
        "zh" => "Chinese (Simplified)",
        "ja" => "Japanese",
        "kk" => "Kazakh",
        "ky" => "Kyrgyz",
        "uz" => "Uzbek",
        "tg" => "Tajik",
        "iu" => "Inuktitut",
        "ka" => "Georgian",
        "olb" => "Russian",
        _ => "Russian",
    }
}

/// Системный промпт с выбранным языком интерфейса.
fn system_prompt(lang: &str) -> String {
    format!(
        "{SYSTEM}\n\nЯзык ответа: {}. Все текстовые поля (reason, actions) пиши строго на этом языке. \
         Названия модов, их id, имена классов и пути не переводи.",
        language_name(lang)
    )
}

/// Хвост консоли лаунчера: без ANSI-цветов и с вырезанными путями пользователя.
fn console_tail(s: &str) -> String {
    let clean = RE_ANSI.replace_all(s, "").replace("\r\n", "\n");
    redact(tail(&clean, 20_000))
}

fn build_user(
    inst: &Instance,
    d: &Diagnosis,
    mods: &[String],
    log: &str,
    console: &str,
) -> String {
    let mut s = String::with_capacity(log.len() + 8_000);
    s.push_str("Профиль: ");
    s.push_str(&inst.name);
    s.push('\n');
    s.push_str(&format!(
        "Minecraft: {} / {} {}\n",
        inst.mc_version,
        inst.loader.label(),
        inst.loader_version.as_deref().unwrap_or("")
    ));
    s.push_str(&format!("Java: {}\n\n", inst.java_major));
    s.push_str(&format!("Установленные моды ({}):\n", mods.len()));
    for m in mods {
        s.push_str("- ");
        s.push_str(m);
        s.push('\n');
    }
    s.push_str("\nЭвристический разбор лаунчера:\n");
    s.push_str(&format!("- категория: {}\n", d.kind));
    s.push_str(&format!("- заголовок: {}\n", d.title));
    if !d.summary.is_empty() {
        s.push_str(&format!("- кратко: {}\n", d.summary.replace('\n', " ")));
    }
    if let Some(c) = &d.culprit {
        s.push_str(&format!("- виновник: {c}\n"));
    }
    if !d.suspects.is_empty() {
        s.push_str(&format!("- подозреваемые: {}\n", d.suspects.join(", ")));
    }
    s.push_str("\nФрагмент отчёта");
    if let Some(f) = &d.file {
        s.push_str(&format!(" ({f})"));
    }
    s.push_str(":\n");
    s.push_str(log);

    if !console.trim().is_empty() {
        s.push_str(
            "\n\nКонсоль лаунчера (stdout/stderr; может отличаться от latest.log — в ней есть \
             строки, которые log4j не пишет в файл):\n",
        );
        s.push_str(console);
    }

    s.push_str(
        "\n\nСначала проверь, есть ли в отчёте настоящий стек ошибки. Если latest.log заканчивается \
         нормальным выходом, а в консоли есть ошибка — считай консоль более свежей и разбирай её. \
         Определи причину и верни JSON по схеме. Не вызывай инструменты.",
    );
    s
}

// ---------------------------------------------------------------------------
// Точка входа
// ---------------------------------------------------------------------------

pub async fn diagnose(id: &str, lang: &str, console: &str) -> Res<AiDiagnosis> {
    paths::validate_instance_id(id)?;
    let diag = crash::diagnose(id).await?;
    let inst = store::read_instance(id).await?;
    let mods = mod_list(id);
    let log = log_excerpt(id, &diag);
    let console = console_tail(console);
    let user = build_user(&inst, &diag, &mods, &log, &console);
    let system = system_prompt(lang);

    let (models, ru) = catalog().await?;
    let ordered = order(&models, ru);
    let mut errors: Vec<String> = Vec::new();

    for model in ordered.iter().take(MAX_TRIES) {
        match ask(model, &system, &user).await {
            Ok(d) => return Ok(d),
            Err((_retryable, msg)) => {
                errors.push(format!("{model}: {msg}"));
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        }
    }

    let tail = errors
        .iter()
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    let tail: String = tail.chars().take(400).collect();
    err(format!(
        "Бесплатные модели не ответили. {tail}"
    ))
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_has_opencode_shape() {
        let s = b62_id("ses_");
        assert!(s.starts_with("ses_"));
        assert_eq!(s.len(), 4 + 12 + 14);
        assert!(s[4..16].chars().all(|c| c.is_ascii_hexdigit()));
        assert!(s[16..].chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn free_filter_and_order() {
        let models = vec![
            "mimo-v2.5-free".to_string(),
            "space-bunny-free".to_string(),
            "deepseek-v4.1-flash-free".to_string(),
            "big-pickle".to_string(),
            "glm-5.3-flash-free".to_string(),
            "muse-spark-1.3-contributor-free".to_string(),
            "claude-opus-5".to_string(),
        ];
        let free: Vec<String> = models
            .iter()
            .filter(|m| is_free(m))
            .cloned()
            .collect();
        let ru = order(&free, true);
        assert_eq!(ru[0], "deepseek-v4.1-flash-free");
        assert_eq!(ru[1], "glm-5.3-flash-free");
        assert!(!ru.iter().any(|m| m.contains("muse")));
        assert!(ru.iter().any(|m| m.contains("big-pickle")));
        assert!(ru.iter().any(|m| m.contains("space-bunny")));

        let world = order(&free, false);
        assert!(world.iter().any(|m| m.contains("muse")));
        assert!(world.iter().position(|m| m.contains("muse")).unwrap() < world.iter().position(|m| m.contains("big-pickle")).unwrap());
    }

    #[test]
    fn json_from_fence_is_parsed() {
        let text = "```json\n{\"cause\":\"mod\",\"mod\":\"Sodium\",\"confidence\":0.9,\"reason\":\"x\",\"actions\":[\"a\"]}\n```";
        let v = extract_json(&strip_fences(text)).expect("json");
        let (cause, m, c, r, a) = parse_answer(&v);
        assert_eq!(cause, "mod");
        assert_eq!(m.as_deref(), Some("Sodium"));
        assert!((c - 0.9).abs() < 1e-9);
        assert_eq!(r, "x");
        assert_eq!(a, vec!["a".to_string()]);
    }

    #[test]
    fn bad_cause_falls_back_to_unknown() {
        let v: Value = serde_json::from_str(r#"{"cause":"wat","mod":null,"reason":"x"}"#).unwrap();
        assert_eq!(parse_answer(&v).0, "unknown");
    }

    #[test]
    fn redacts_user_paths() {
        let s = redact(r"C:\Users\bob\minecraft\logs and /home/bob/.minecraft");
        assert!(s.contains(r"C:\Users\<user>"));
        assert!(s.contains("/home/<user>"));
        assert!(!s.contains("bob"));
    }

    #[test]
    fn console_is_cleaned() {
        let s = console_tail("\x1b[32m[12:00] Hello\x1b[0m\r\nC:\\Users\\bob\\game");
        assert!(!s.contains('\x1b'));
        assert!(!s.contains('\r'));
        assert!(s.contains("C:\\Users\\<user>"));
        assert!(language_name("de").contains("German"));
    }

    /// Живая проверка отпечатка: сеть, free-модель, стрим и разбор JSON.
    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn live_zen_answers() {
        let user = "Краш: java.lang.NoSuchMethodError в com.example.mymod.MixinFoo, Caused by ClassNotFoundException: com.example.lib.Bar. Моды: sodium, create, fabric-api. Верни JSON.";
        let d = ask("deepseek-v4.1-flash-free", SYSTEM, user)
            .await
            .expect("zen answer");
        assert_eq!(d.model, "deepseek-v4.1-flash-free");
        assert!(!d.reason.is_empty());
        assert!(CAUSES.contains(&d.cause.as_str()));
    }

    /// Язык ответа и консоль, которой нет в latest.log.
    #[tokio::test]
    #[ignore = "ходит в сеть; нужен ARBUZ_TEST_INSTANCE"]
    async fn live_language_and_console() {
        let id = std::env::var("ARBUZ_TEST_INSTANCE").expect("ARBUZ_TEST_INSTANCE");
        let console = "[12:00:00] [main/ERROR]: GLFW error 65542: WGL: The driver does not appear to support OpenGL\n\
                       [12:00:01] [main/ERROR]: Failed to create window\n";
        let d = diagnose(&id, "de", console).await.expect("diagnosis");
        println!("model={} cause={} reason={}", d.model, d.cause, d.reason);
        assert_eq!(d.cause, "gpu", "должен разобрать ошибку из консоли");
        assert!(!d.reason.is_empty());
    }

    /// Полный путь на реальном профиле: нужен `ARBUZ_TEST_INSTANCE`.
    #[tokio::test]
    #[ignore = "ходит в сеть; нужен ARBUZ_TEST_INSTANCE"]
    async fn live_full_diagnosis() {
        let id = std::env::var("ARBUZ_TEST_INSTANCE").expect("ARBUZ_TEST_INSTANCE");
        let d = diagnose(&id, "ru", "").await.expect("diagnosis");
        println!(
            "model={} cause={} mod={:?} conf={} ms={} tokens={:?}",
            d.model, d.cause, d.mod_name, d.confidence, d.elapsed_ms, d.tokens
        );
        println!("reason={}", d.reason);
        for a in &d.actions {
            println!("action: {a}");
        }
        assert!(!d.reason.is_empty());
    }
}
