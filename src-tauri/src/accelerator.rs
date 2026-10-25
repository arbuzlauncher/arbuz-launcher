//! Локальный HTTP CONNECT-прокси для «Ускорителя лаунчера».
//!
//! Первый пакет от клиента (обычно TLS ClientHello с SNI) отправляется
//! наверх двумя кусками: 2 байта, пауза, остальное. Это ломает простой
//! DPI, который смотрит только на отдельные TCP-сегменты. Прокси слушает
//! только 127.0.0.1 и требует пароль, чтобы им не мог пользоваться кто-то
//! ещё на этой машине.

use base64::Engine;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration};

const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Пауза между первым куском и остальным ClientHello.
const FRAGMENT_DELAY: Duration = Duration::from_millis(12);
/// Какой кусок отправляем первым. 2-3 байта достаточно, чтобы SNI не
/// оказался в первом сегменте, но слишком мало для разбора DPI.
const FIRST_CHUNK: usize = 2;

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

/// Обрабатывает один CONNECT: проверяет пароль, соединяется с целью и
/// ретранслирует байты, разбивая первый кусок.
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
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if !method.eq_ignore_ascii_case("CONNECT") {
        let _ = reply(&mut client, "405 Method Not Allowed").await;
        return Ok(());
    }

    let auth = lines.find_map(|line| {
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

    let Some((host, port)) = parse_target(target) else {
        let _ = reply(&mut client, "400 Bad Request").await;
        return Ok(());
    };
    // Весь трафик лаунчера идёт по HTTPS; лишние порты не проксируем.
    if port != 443 {
        let _ = reply(&mut client, "403 Forbidden").await;
        return Ok(());
    }

    let upstream = match timeout(CONNECT_TIMEOUT, TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(s)) => s,
        _ => {
            let _ = reply(&mut client, "502 Bad Gateway").await;
            return Ok(());
        }
    };
    let _ = upstream.set_nodelay(true);
    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    tunnel(client, upstream, &buf[header_end..]).await
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

/// Пересылает данные в обе стороны. Первый кусок от клиента — отдельно.
async fn tunnel(client: TcpStream, upstream: TcpStream, leftover: &[u8]) -> io::Result<()> {
    let (mut cr, mut cw) = client.into_split();
    let (mut ur, mut uw) = upstream.into_split();

    let c2u = async move {
        let mut first = true;
        if !leftover.is_empty() {
            split_write(&mut uw, leftover).await?;
            first = false;
        }
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            let n = cr.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            if first {
                split_write(&mut uw, &buf[..n]).await?;
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

async fn split_write<W: AsyncWrite + Unpin>(dst: &mut W, data: &[u8]) -> io::Result<()> {
    if data.len() > FIRST_CHUNK {
        dst.write_all(&data[..FIRST_CHUNK]).await?;
        dst.flush().await?;
        sleep(FRAGMENT_DELAY).await;
        dst.write_all(&data[FIRST_CHUNK..]).await?;
    } else {
        dst.write_all(data).await?;
    }
    dst.flush().await
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{parse_target, split_write};
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

    #[tokio::test]
    async fn split_write_keeps_all_bytes() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let data = b"hello fragmented world";
        split_write(&mut a, data).await.unwrap_or_else(|e| panic!("{e}"));
        drop(a);
        let mut out = Vec::new();
        b.read_to_end(&mut out).await.unwrap_or(0);
        assert_eq!(out, data);
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
