//! Локальный HTTP CONNECT-прокси для «Ускорителя лаунчера».
//!
//! Что делает:
//! * первый пакет от клиента (обычно TLS ClientHello с SNI) отправляется
//!   наверх несколькими кусками с паузами — простой DPI не собирает домен
//!   из одного TCP-сегмента;
//! * обычный HTTP разрезается так, что строка запроса (`GET ...`) и
//!   заголовки (`Host: ...`) уходят отдельными пакетами.
//!
//! Фейк-пакеты, подмена TTL и перестановка TCP-сегментов требуют raw-сокетов
//! (WinDivert или права администратора): в обычном TCP-потоке нельзя изменить
//! порядок байт или TTL отдельного сегмента, не сломав соединение. Поэтому
//! здесь их нет — прокси работает без драйверов и без прав администратора.
//!
//! Прокси слушает только 127.0.0.1 и требует пароль, чтобы им не мог
//! пользоваться кто-то ещё на этой машине.

use base64::Engine;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration};

const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Какой кусок ClientHello отправляем первым. 2-3 байта достаточно, чтобы
/// SNI не оказался в первом сегменте, но слишком мало для разбора DPI.
const FIRST_CHUNK: usize = 2;
/// Второй кусок — небольшой, чтобы SNI оказался уже в третьем сегменте.
const SECOND_CHUNK_MIN: usize = 3;
const SECOND_CHUNK_MAX: usize = 8;

/// Запущенный прокси: порт, пароль и общий выключатель.
pub struct Proxy {
    pub port: u16,
    pub token: String,
    pub enabled: Arc<AtomicBool>,
}

/// Поднимает прокси на случайном порту 127.0.0.1. Если порт занят или
/// система не даёт слушать сокет, возвращает `port = 0` — тогда клиент
/// просто ходит напрямую.
pub fn start(initial: bool) -> Proxy {
    let enabled = Arc::new(AtomicBool::new(initial));
    let token = uuid::Uuid::new_v4().simple().to_string();
    let listener = match std::net::TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(_) => {
            return Proxy {
                port: 0,
                token,
                enabled,
            }
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let _ = listener.set_nonblocking(true);

    let expected = basic_auth(&token);
    let spawned = std::thread::Builder::new()
        .name("arbuz-accelerator".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            rt.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(listener) {
                    Ok(l) => l,
                    Err(_) => return,
                };
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let expected = expected.clone();
                            tokio::spawn(async move {
                                let _ = handle(stream, expected).await;
                            });
                        }
                        Err(_) => sleep(Duration::from_millis(200)).await,
                    }
                }
            });
        });

    if spawned.is_err() {
        return Proxy {
            port: 0,
            token,
            enabled,
        };
    }

    Proxy {
        port,
        token,
        enabled,
    }
}

fn basic_auth(token: &str) -> String {
    let raw = format!("arbuz:{token}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    )
}

/// Обрабатывает один запрос: CONNECT для HTTPS или обычный HTTP.
async fn handle(mut client: TcpStream, expected: String) -> io::Result<()> {
    let _ = client.set_nodelay(true);
    let mut buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 1024];
    let header_end;

    loop {
        let n = timeout(HEADER_TIMEOUT, client.read(&mut tmp))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "proxy header timeout"))??;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if buf.len() > 32 * 1024 {
            return Ok(());
        }
    }

    let head = String::from_utf8_lossy(&buf[..header_end]);
    let header_lines: Vec<&str> = head.split("\r\n").collect();
    let first = header_lines.first().copied().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("HTTP/1.1");

    let auth = header_lines.iter().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim().eq_ignore_ascii_case("proxy-authorization") {
            Some(value.trim().to_string())
        } else {
            None
        }
    });
    if auth.as_deref() != Some(expected.as_str()) {
        let _ = reply(&mut client, "407 Proxy Authentication Required").await;
        return Ok(());
    }

    if method.eq_ignore_ascii_case("CONNECT") {
        let Some((host, port)) = parse_target(target) else {
            let _ = reply(&mut client, "400 Bad Request").await;
            return Ok(());
        };
        // Весь HTTPS-трафик лаунчера идёт на 443; лишние порты не проксируем.
        if port != 443 {
            let _ = reply(&mut client, "403 Forbidden").await;
            return Ok(());
        }

        let upstream = match connect(&host, port).await {
            Some(s) => s,
            None => {
                let _ = reply(&mut client, "502 Bad Gateway").await;
                return Ok(());
            }
        };
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        return tunnel(client, upstream, &buf[header_end..]).await;
    }

    // Обычный HTTP: разрезаем строку запроса и заголовки на два пакета.
    let Some((host, port, path)) = parse_http_target(target, &header_lines) else {
        let _ = reply(&mut client, "400 Bad Request").await;
        return Ok(());
    };
    if port != 80 {
        let _ = reply(&mut client, "403 Forbidden").await;
        return Ok(());
    }

    let mut upstream = match connect(&host, port).await {
        Some(s) => s,
        None => {
            let _ = reply(&mut client, "502 Bad Gateway").await;
            return Ok(());
        }
    };
    let request = build_http_request(method, &path, version, &header_lines);
    write_http_request(&mut upstream, &request, &buf[header_end..]).await?;
    tunnel(client, upstream, &[]).await
}

async fn connect(host: &str, port: u16) -> Option<TcpStream> {
    match timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port))).await {
        Ok(Ok(s)) => {
            let _ = s.set_nodelay(true);
            Some(s)
        }
        _ => None,
    }
}

async fn reply(stream: &mut TcpStream, status: &str) -> io::Result<()> {
    let msg = format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    stream.write_all(msg.as_bytes()).await
}

fn parse_target(target: &str) -> Option<(String, u16)> {
    let (host, port) = target.rsplit_once(':')?;
    let host = host.trim().trim_matches(|c| c == '[' || c == ']');
    let port = port.parse::<u16>().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port))
}

/// Разбирает цель обычного HTTP: абсолютный URI (`http://host/path`) или
/// origin-form (`/path`) вместе с заголовком Host.
fn parse_http_target(target: &str, headers: &[&str]) -> Option<(String, u16, String)> {
    let (authority, path) = if let Some(rest) = target.strip_prefix("http://") {
        let cut = rest.find(|c| c == '/' || c == '?');
        match cut {
            Some(i) if rest.as_bytes()[i] == b'/' => (&rest[..i], rest[i..].to_string()),
            Some(i) => (&rest[..i], format!("/{}", &rest[i..])),
            None => (rest, "/".to_string()),
        }
    } else if target.starts_with('/') {
        let host = headers.iter().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            if key.trim().eq_ignore_ascii_case("host") {
                Some(value.trim())
            } else {
                None
            }
        })?;
        (host, target.to_string())
    } else {
        return None;
    };

    // user:pass@host в URL наверх не отдаём.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = split_authority(authority, 80)?;
    Some((host, port, path))
}

/// `host`, `host:port`, `[::1]`, `[::1]:8080`.
fn split_authority(raw: &str, default_port: u16) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (host, port) = if let Some(rest) = raw.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = tail
            .strip_prefix(':')
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(default_port);
        (host, port)
    } else {
        match raw.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().ok()?),
            None => (raw, default_port),
        }
    };
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    Some((host.to_string(), port))
}

/// Собирает запрос в origin-form и выкидывает заголовки локального прокси,
/// чтобы пароль не ушёл на чужой сервер.
fn build_http_request(method: &str, path: &str, version: &str, headers: &[&str]) -> Vec<u8> {
    let mut out = Vec::with_capacity(512);
    out.extend_from_slice(format!("{method} {path} {version}\r\n").as_bytes());
    for line in headers.iter().skip(1) {
        if line.is_empty() {
            continue;
        }
        let Some((key, _)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.eq_ignore_ascii_case("proxy-authorization")
            || key.eq_ignore_ascii_case("proxy-connection")
        {
            continue;
        }
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out
}

/// Строка запроса уходит первым пакетом, заголовки и тело — вторым.
async fn write_http_request<W: AsyncWrite + Unpin>(
    dst: &mut W,
    request: &[u8],
    body: &[u8],
) -> io::Result<()> {
    let line_end = find(request, b"\r\n")
        .map(|p| p + 2)
        .unwrap_or(request.len());
    dst.write_all(&request[..line_end]).await?;
    dst.flush().await?;
    sleep(jitter()).await;
    dst.write_all(&request[line_end..]).await?;
    if !body.is_empty() {
        dst.write_all(body).await?;
    }
    dst.flush().await
}

/// Пересылает данные в обе стороны. Первый кусок от клиента — отдельно.
async fn tunnel(client: TcpStream, upstream: TcpStream, leftover: &[u8]) -> io::Result<()> {
    let (mut cr, mut cw) = client.into_split();
    let (mut ur, mut uw) = upstream.into_split();

    let c2u = async move {
        let mut first = true;
        if !leftover.is_empty() {
            fragment_write(&mut uw, leftover).await?;
            first = false;
        }
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            let n = cr.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            if first {
                fragment_write(&mut uw, &buf[..n]).await?;
                first = false;
            } else {
                uw.write_all(&buf[..n]).await?;
            }
        }
        uw.shutdown().await
    };

    let u2c = async move {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            let n = ur.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            cw.write_all(&buf[..n]).await?;
        }
        cw.shutdown().await
    };

    tokio::select! {
        r = c2u => r,
        r = u2c => r,
    }
}

/// Режет первый кусок на три части с паузами: 2 байта, 3–8 байт, остальное.
async fn fragment_write<W: AsyncWrite + Unpin>(dst: &mut W, data: &[u8]) -> io::Result<()> {
    if data.len() <= FIRST_CHUNK {
        dst.write_all(data).await?;
        return dst.flush().await;
    }
    let extra = SECOND_CHUNK_MIN + (rand64() as usize % (SECOND_CHUNK_MAX - SECOND_CHUNK_MIN + 1));
    let second_end = (FIRST_CHUNK + extra).min(data.len());

    dst.write_all(&data[..FIRST_CHUNK]).await?;
    dst.flush().await?;
    sleep(jitter()).await;

    if second_end < data.len() {
        dst.write_all(&data[FIRST_CHUNK..second_end]).await?;
        dst.flush().await?;
        sleep(jitter()).await;
    }
    dst.write_all(&data[second_end..]).await?;
    dst.flush().await
}

/// Небольшая пауза между сегментами, чтобы DPI терял состояние потока.
fn jitter() -> Duration {
    Duration::from_millis(8 + rand64() % 13)
}

/// Дешёвый xorshift: не нужен криптостойкий ГПСЧ, важно лишь разнообразие.
fn rand64() -> u64 {
    static SEED: AtomicU64 = AtomicU64::new(0);
    let mut x = SEED.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    x
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{build_http_request, fragment_write, parse_http_target, parse_target};
    use tokio::io::AsyncReadExt;

    #[test]
    fn parses_connect_target() {
        assert_eq!(
            parse_target("api.modrinth.com:443"),
            Some(("api.modrinth.com".into(), 443))
        );
        assert_eq!(parse_target("bad"), None);
        assert_eq!(parse_target(":443"), None);
    }

    #[test]
    fn parses_http_targets() {
        let headers = ["GET http://example.com/a?b HTTP/1.1", "Host: example.com"];
        assert_eq!(
            parse_http_target("http://example.com/a?b", &headers),
            Some(("example.com".into(), 80, "/a?b".into()))
        );
        assert_eq!(
            parse_http_target("/x", &headers),
            Some(("example.com".into(), 80, "/x".into()))
        );
        assert_eq!(
            parse_http_target("http://example.com:8080", &headers),
            Some(("example.com".into(), 8080, "/".into()))
        );
        assert_eq!(parse_http_target("ftp://example.com/", &headers), None);
        assert_eq!(
            parse_http_target("http://[::1]:8080/x", &headers),
            Some(("::1".into(), 8080, "/x".into()))
        );
    }

    #[test]
    fn http_request_strips_proxy_headers() {
        let headers = [
            "GET http://example.com/ HTTP/1.1",
            "Host: example.com",
            "Proxy-Authorization: Basic secret",
            "Proxy-Connection: keep-alive",
            "User-Agent: test",
        ];
        let req = build_http_request("GET", "/", "HTTP/1.1", &headers);
        let text = String::from_utf8(req).unwrap_or_default();
        assert!(text.starts_with("GET / HTTP/1.1\r\n"));
        assert!(text.contains("Host: example.com"));
        assert!(text.contains("User-Agent: test"));
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("proxy-authorization"));
        assert!(!lower.contains("proxy-connection"));
    }

    #[tokio::test]
    async fn fragment_write_keeps_all_bytes() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let data = b"hello fragmented world";
        fragment_write(&mut a, data)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        drop(a);
        let mut out = Vec::new();
        b.read_to_end(&mut out).await.unwrap_or(0);
        assert_eq!(out, data);
    }

    /// Живая проверка: обычный HTTP через прокси (строка запроса и заголовки
    /// уходят разными пакетами).
    #[tokio::test]
    #[ignore = "network"]
    async fn live_proxy_splits_http() {
        let p = super::start(true);
        if p.port == 0 {
            return;
        }
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", p.port))
            .unwrap_or_else(|e| panic!("{e}"))
            .basic_auth("arbuz", &p.token);
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        let res = client
            .get("http://example.com/")
            .header("User-Agent", "ArbuzLauncher/1.0")
            .send()
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            res.status().is_success() || res.status().is_redirection(),
            "status {}",
            res.status()
        );
    }

    /// Живая проверка: CONNECT с паролем, TLS и фрагментация первого пакета.
    #[tokio::test]
    #[ignore = "network"]
    async fn live_proxy_reaches_modrinth() {
        let p = super::start(true);
        if p.port == 0 {
            return;
        }
        let port = p.port;
        let proxy = reqwest::Proxy::custom(move |url| {
            if url.scheme() == "https" {
                Some(format!("http://127.0.0.1:{port}"))
            } else {
                None
            }
        })
        .basic_auth("arbuz", &p.token);
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        let res = client
            .get("https://api.modrinth.com/v2/search?query=sodium&limit=1")
            .header("User-Agent", "ArbuzLauncher/1.0")
            .send()
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(res.status().is_success(), "status {}", res.status());
    }
}
