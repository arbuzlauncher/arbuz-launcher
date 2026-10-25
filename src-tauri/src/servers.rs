//! Каталог серверов (Б3): свой Server List Ping по TCP, статичный список
//! серверов из JSON по URL из настроек (формат — в README), кэш списка,
//! закреплённые серверы на профиль и добавление в servers.dat.
//!
//! Протокол SLP: TCP → Handshake (version=-1, host, port, next=1) →
//! Status Request → Status Response (JSON с версией, игроках и MOTD).

use crate::error::{err, Res};
use crate::sync::write_bytes_atomic;
use crate::{paths, store};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const READ_TIMEOUT: Duration = Duration::from_secs(4);
/// Максимум байтов ответа статуса: больше — точно не JSON.
const MAX_STATUS_BYTES: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// VarInt и пакеты
// ---------------------------------------------------------------------------

fn write_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

/// Читает VarInt из буфера. `None` — данных не хватило.
fn read_varint(buf: &[u8], pos: &mut usize) -> Option<u32> {
    let mut result: u32 = 0;
    for i in 0..5 {
        let b = *buf.get(*pos)?;
        *pos += 1;
        result |= ((b & 0x7F) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Some(result);
        }
    }
    None
}

/// Пакет Handshake: протокол, адрес, порт, следующий статус.
pub(crate) fn handshake_packet(host: &str, port: u16) -> Vec<u8> {
    let mut body = Vec::with_capacity(host.len() + 8);
    write_varint(&mut body, 0); // packet id: Handshake
    write_varint(&mut body, 0); // protocol version: -1 не нужен, 0 тоже валиден
    let hb = host.as_bytes();
    write_varint(&mut body, hb.len() as u32);
    body.extend_from_slice(hb);
    body.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut body, 1); // next state: Status
    let mut out = Vec::with_capacity(body.len() + 5);
    write_varint(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    out
}

/// Пакет Status Request (пустой).
pub(crate) fn status_request_packet() -> Vec<u8> {
    vec![1, 0] // длина 1, id 0
}

// ---------------------------------------------------------------------------
// Разбор ответа
// ---------------------------------------------------------------------------

/// Вытаскивает текст из компонента чата (строка или {text, extra:[...]}).
pub(crate) fn chat_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(m) => {
            let mut out = m.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if let Some(extra) = m.get("extra").and_then(|e| e.as_array()) {
                for e in extra {
                    out.push_str(&chat_text(e));
                }
            }
            out
        }
        _ => String::new(),
    }
}

/// Разбирает тело Status Response: после id идёт строка JSON.
pub(crate) fn parse_status_payload(body: &[u8]) -> Option<serde_json::Value> {
    let mut pos = 0usize;
    let _id = read_varint(body, &mut pos)?;
    let len = read_varint(body, &mut pos)? as usize;
    if len > MAX_STATUS_BYTES {
        return None;
    }
    let json = body.get(pos..pos + len)?;
    serde_json::from_slice(json).ok()
}

// ---------------------------------------------------------------------------
// Пинг
// ---------------------------------------------------------------------------

/// Результат пинга одного сервера.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PingInfo {
    pub host: String,
    pub port: u16,
    pub online: bool,
    pub latency_ms: u64,
    pub version: String,
    pub players_online: i64,
    pub players_max: i64,
    pub motd: String,
}

/// «host» или «host:port». Порт по умолчанию 25565.
pub(crate) fn split_addr(raw: &str) -> Res<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 253 + 6 {
        return err("Некорректный адрес сервера");
    }
    // IPv6 в квадратных скобках: [::1]:25565
    if let Some(rest) = raw.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or_else(|| crate::error::AppError::Msg("Некорректный адрес сервера".into()))?;
        let port = tail
            .strip_prefix(':')
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(25565);
        return Ok((host.to_string(), port));
    }
    match raw.rsplit_once(':') {
        Some((h, p)) => {
            let port = p.parse::<u16>().map_err(|_| crate::error::AppError::Msg("Некорректный адрес сервера".into()))?;
            Ok((h.to_string(), port))
        }
        None => Ok((raw.to_string(), 25565)),
    }
}

/// Один Server List Ping. Ошибки не бросаем: `online=false` в ответе.
pub async fn ping(raw: &str) -> PingInfo {
    let (host, port) = match split_addr(raw) {
        Ok(v) => v,
        Err(e) => {
            let _ = e;
            return PingInfo {
                host: raw.to_string(),
                port: 0,
                online: false,
                ..PingInfo::default()
            };
        }
    };
    let started = Instant::now();
    let mut info = PingInfo {
        host: host.clone(),
        port,
        ..PingInfo::default()
    };
    let addr = format!("{host}:{port}");
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(&addr)).await
    {
        Ok(Ok(s)) => s,
        _ => return info,
    };
    let _ = stream.set_nodelay(true);
    let (mut rd, mut wr) = stream.into_split();

    if wr.write_all(&handshake_packet(&host, port)).await.is_err() {
        return info;
    }
    if wr.write_all(&status_request_packet()).await.is_err() {
        return info;
    }
    if wr.flush().await.is_err() {
        return info;
    }

    // Читаем до полного пакета (length-prefixed) или таймаута.
    let mut buf: Vec<u8> = Vec::with_capacity(512);
    let mut chunk = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + READ_TIMEOUT;
    loop {
        // Пакет уже полный?
        let mut pos = 0usize;
        if let Some(len) = read_varint(&buf, &mut pos) {
            let total = pos + len as usize;
            if total <= MAX_STATUS_BYTES && buf.len() >= total {
                info.latency_ms = started.elapsed().as_millis() as u64;
                info.online = true;
                if let Some(v) = parse_status_payload(&buf[..total]) {
                    info.version = v
                        .pointer("/version/name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .chars()
                        .take(60)
                        .collect();
                    info.players_online = v.pointer("/players/online").and_then(|x| x.as_i64()).unwrap_or(0);
                    info.players_max = v.pointer("/players/max").and_then(|x| x.as_i64()).unwrap_or(0);
                    info.motd = chat_text(v.get("description").unwrap_or(&serde_json::Value::Null))
                        .chars()
                        .take(240)
                        .collect();
                }
                return info;
            }
            if total > MAX_STATUS_BYTES {
                return info;
            }
        }
        let remain = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remain.is_zero() {
            return info;
        }
        match tokio::time::timeout(remain, rd.read(&mut chunk)).await {
            Ok(Ok(0)) => return info, // соединение закрыто
            Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
            Ok(Err(_)) | Err(_) => return info,
        }
    }
}

// ---------------------------------------------------------------------------
// Каталог из статичного JSON
// ---------------------------------------------------------------------------

/// Сервер из каталога.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogServer {
    pub name: String,
    pub ip: String,
    /// Например «1.20.x» — для фильтра.
    pub version: String,
    /// Режим: survival | anarchy | minigames | creative…
    pub mode: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct CatalogFile {
    fetched_at: i64,
    servers: Vec<CatalogServer>,
}

fn catalog_cache_path() -> PathBuf {
    paths::root().join("cache").join("servers-catalog.json")
}

/// Список серверов из кэша; при `refresh` или пустом кэше — скачивание по
/// URL из настроек. Без URL — ошибка с понятным сообщением.
pub async fn catalog(http: &reqwest::Client, url: &str, refresh: bool) -> Res<Vec<CatalogServer>> {
    let cache = catalog_cache_path();
    if !refresh {
        let cached: CatalogFile = store::read_json(&cache).await.unwrap_or_default();
        if !cached.servers.is_empty() {
            return Ok(cached.servers);
        }
    }
    if url.trim().is_empty() {
        // URL не задан, но кэш мог быть — отдаём что есть.
        let cached: CatalogFile = store::read_json(&cache).await.unwrap_or_default();
        if !cached.servers.is_empty() {
            return Ok(cached.servers);
        }
        return err("URL списка серверов не задан в настройках");
    }
    let text = http
        .get(url.trim())
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    if text.len() > 4 * 1024 * 1024 {
        return err("Список серверов слишком большой");
    }
    // Формат допускает и {"servers":[...]}, и голый массив.
    let servers: Vec<CatalogServer> = if text.trim_start().starts_with('[') {
        serde_json::from_str(&text)?
    } else {
        let v: serde_json::Value = serde_json::from_str(&text)?;
        serde_json::from_value(v.get("servers").cloned().unwrap_or(serde_json::Value::Array(Vec::new())))?
    };
    let mut servers = servers;
    servers.retain(|s| !s.name.is_empty() && !s.ip.is_empty() && s.name.len() <= 100 && s.ip.len() <= 260);
    for s in &mut servers {
        s.version.truncate(30);
        s.mode.truncate(30);
    }
    servers.truncate(500);
    let file = CatalogFile {
        fetched_at: chrono::Utc::now().timestamp(),
        servers: servers.clone(),
    };
    store::write_json_atomic(&cache, &file).await?;
    Ok(servers)
}

// ---------------------------------------------------------------------------
// servers.dat: добавление сервера
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct DatServer {
    pub name: String,
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<i8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct DatRoot {
    #[serde(default)]
    servers: Vec<DatServer>,
}

/// Собирает обновлённое содержимое servers.dat: существующий с таким же
/// адресом обновляется, а не дублируется. Нет файла — создаётся.
pub(crate) fn build_servers_dat(path: &Path, name: &str, ip: &str) -> Res<Vec<u8>> {
    if name.is_empty() || ip.is_empty() || name.len() > 100 || ip.len() > 260 {
        return err("Некорректные имя или адрес сервера");
    }
    let mut root: DatRoot = std::fs::read(path)
        .ok()
        .and_then(|b| fastnbt::from_bytes(&b).ok())
        .unwrap_or_default();
    if let Some(existing) = root.servers.iter_mut().find(|s| s.ip == ip) {
        existing.name = name.to_string();
    } else {
        root.servers.push(DatServer {
            name: name.to_string(),
            ip: ip.to_string(),
            icon: None,
            hidden: None,
        });
    }
    let mut bytes = Vec::new();
    fastnbt::to_writer(&mut bytes, &root).map_err(|e| crate::error::AppError::Msg(format!("NBT: {e}")))?;
    Ok(bytes)
}

/// Добавляет сервер в servers.dat профиля.
pub async fn add_server(id: &str, name: &str, ip: &str) -> Res<()> {
    crate::paths::validate_instance_id(id)?;
    let path = paths::instance_minecraft_dir(id).join("servers.dat");
    let bytes = build_servers_dat(&path, name, ip)?;
    write_bytes_atomic(&path, &bytes).await
}

// ---------------------------------------------------------------------------
// Закреплённые серверы на профиль
// ---------------------------------------------------------------------------

/// Закреплённый сервер.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PinnedServer {
    pub name: String,
    pub ip: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct PinnedFile {
    pinned: Vec<PinnedServer>,
}

fn pinned_path(id: &str) -> PathBuf {
    paths::instance_dir(id).join("servers.json")
}

/// Список закреплённых серверов профиля.
pub async fn pinned_list(id: &str) -> Res<Vec<PinnedServer>> {
    crate::paths::validate_instance_id(id)?;
    let f: PinnedFile = store::read_json(&pinned_path(id)).await.unwrap_or_default();
    Ok(f.pinned)
}

/// Закрепляет сервер на профиле (или снимает закрепление по адресу).
pub async fn pin(id: &str, name: &str, ip: &str, on: bool) -> Res<()> {
    crate::paths::validate_instance_id(id)?;
    if ip.is_empty() || ip.len() > 260 || name.len() > 100 {
        return err("Некорректные имя или адрес сервера");
    }
    let mut f: PinnedFile = store::read_json(&pinned_path(id)).await.unwrap_or_default();
    f.pinned.retain(|p| p.ip != ip);
    if on {
        f.pinned.push(PinnedServer {
            name: name.to_string(),
            ip: ip.to_string(),
        });
    }
    f.pinned.truncate(50);
    store::write_json_atomic(&pinned_path(id), &f).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        for v in [0u32, 1, 127, 128, 255, 25565, 0xFFFF, u32::MAX - 5] {
            let mut out = Vec::new();
            write_varint(&mut out, v);
            let mut pos = 0;
            assert_eq!(read_varint(&out, &mut pos), Some(v));
            assert_eq!(pos, out.len());
        }
        assert_eq!(read_varint(&[], &mut 0), None);
    }

    #[test]
    fn handshake_bytes_are_prefix_free() {
        let p = handshake_packet("mc.hypixel.net", 25565);
        let mut pos = 0;
        let len = read_varint(&p, &mut pos).unwrap();
        assert_eq!(len as usize + pos, p.len(), "длина пакета совпадает с содержимым");
        let status = status_request_packet();
        assert_eq!(status, vec![1, 0]);
    }

    #[test]
    fn status_payload_parses_json() {
        let mut body = Vec::new();
        write_varint(&mut body, 0); // packet id
        let json = r#"{"version":{"name":"1.21.4"},"players":{"online":5,"max":100},"description":{"text":"Привет ","extra":[{"text":"мир"}]}}"#.as_bytes();
        write_varint(&mut body, json.len() as u32);
        body.extend_from_slice(json);
        let v = parse_status_payload(&body).expect("json");
        assert_eq!(v.pointer("/version/name").and_then(|x| x.as_str()), Some("1.21.4"));
        assert_eq!(v.pointer("/players/max").and_then(|x| x.as_i64()), Some(100));
        let motd = chat_text(v.get("description").unwrap());
        assert_eq!(motd, "Привет мир");
    }

    #[test]
    fn chat_text_accepts_plain_string() {
        let v: serde_json::Value = serde_json::from_str("\"просто motd\"").unwrap();
        assert_eq!(chat_text(&v), "просто motd");
        assert_eq!(chat_text(&serde_json::Value::Null), "");
    }

    #[test]
    fn addr_split_variants() {
        assert_eq!(split_addr("mc.example.net").unwrap(), ("mc.example.net".into(), 25565));
        assert_eq!(split_addr("mc.example.net:25577").unwrap(), ("mc.example.net".into(), 25577));
        assert_eq!(split_addr("[::1]:25565").unwrap(), ("::1".into(), 25565));
        assert_eq!(split_addr("  spaced.ru  ").unwrap(), ("spaced.ru".into(), 25565));
        assert!(split_addr("host:порт").is_err());
        assert!(split_addr("").is_err());
    }

    #[test]
    fn servers_dat_add_updates_and_creates() {
        let dir = std::env::temp_dir().join(format!("arbuz-srvdat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("servers.dat");

        std::fs::write(&path, build_servers_dat(&path, "Первый", "one.example.net").unwrap()).unwrap();
        let two = build_servers_dat(&path, "Второй", "two.example.net:25577").unwrap();
        std::fs::write(&path, &two).unwrap();
        let upd = build_servers_dat(&path, "Первый (обновлён)", "one.example.net").unwrap();
        std::fs::write(&path, &upd).unwrap();

        let raw = std::fs::read(&path).unwrap();
        let root: DatRoot = fastnbt::from_bytes(&raw).unwrap();
        assert_eq!(root.servers.len(), 2, "дубль по ip обновляется, не добавляется");
        assert_eq!(root.servers[0].name, "Первый (обновлён)");
        assert_eq!(root.servers[1].ip, "two.example.net:25577");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Живой пинг публичного сервера — только вручную.
    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn live_ping_works() {
        let info = ping("mc.hypixel.net").await;
        println!("{info:?}");
        assert!(info.online, "сервер должен ответить");
    }
}
