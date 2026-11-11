//! Скины аккаунтов для аватара: голова из текстуры вместо буквы.
//!
//! Источник зависит от типа аккаунта, чтобы скин Ely.by не «протекал» на
//! лицензию или пиратку:
//!
//! * лицензия — текстура из профиля Mojang по UUID аккаунта;
//! * Ely.by — `skinsystem.ely.by` по нику;
//! * офлайн (пиратка) — ник ищется в Mojang, и берётся текстура найденного
//!   профиля. Если такого ника нет, скина нет и фронтенд рисует Стива.
//!
//! Текстура отдаётся фронтенду data-URL'ом: так картинка не зависит от CSP и от
//! того, переживёт ли webview редирект `skinsystem.ely.by`.

use crate::error::Res;
use crate::models::{Account, AccountKind};
use crate::state::AppState;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ELY_SKIN: &str = "https://skinsystem.ely.by/skins";
const MOJANG_USERS: &str = "https://api.mojang.com/users/profiles/minecraft";
const MOJANG_PROFILE: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
const TTL: Duration = Duration::from_secs(30 * 60);

/// Размер PNG-файла, которого хватает, чтобы прочитать IHDR.
const PNG_HEADER: usize = 24;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// data-URL с PNG-текстурой скина.
    pub url: String,
    pub width: u32,
    pub height: u32,
}

static CACHE: Lazy<Mutex<HashMap<String, (Instant, Option<Skin>)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn cached(id: &str) -> Option<Option<Skin>> {
    let g = CACHE.lock().ok()?;
    let (at, v) = g.get(id)?;
    if at.elapsed() < TTL {
        Some(v.clone())
    } else {
        None
    }
}

fn remember(id: &str, v: &Option<Skin>) {
    if let Ok(mut g) = CACHE.lock() {
        g.insert(id.to_string(), (Instant::now(), v.clone()));
    }
}

/// Скин аккаунта (или `None`, если его нет — тогда рисуется Стив).
pub async fn account_skin(state: &AppState, id: &str) -> Res<Option<Skin>> {
    if let Some(hit) = cached(id) {
        return Ok(hit);
    }

    let acc = crate::accounts::list().await?.into_iter().find(|a| a.id == id);
    let Some(acc) = acc else {
        remember(id, &None);
        return Ok(None);
    };

    let url = skin_url(state, &acc).await;
    let skin = match url {
        Some(u) => fetch(state, &u).await,
        None => None,
    };
    remember(id, &skin);
    Ok(skin)
}

/// Ссылка на текстуру скина — своя для каждого типа аккаунта.
async fn skin_url(state: &AppState, acc: &Account) -> Option<String> {
    let nick = acc.name.trim();
    match acc.kind {
        // Лицензия: точный скин берётся по UUID профиля, а не по нику.
        AccountKind::Microsoft => mojang_texture(state, &acc.id).await,
        // Ely.by: только сервис Ely.by, чужой скин сюда не подставляется.
        AccountKind::Elyby => {
            if nick.is_empty() {
                return None;
            }
            Some(format!("{ELY_SKIN}/{}.png", urlencoding::encode(nick)))
        }
        AccountKind::Littleskin => littleskin_skin(state, &acc.id).await,
        // Пиратка: ник ищем в Mojang; нет такого ника — нет скина (Стив).
        AccountKind::Offline => {
            if nick.is_empty() {
                return None;
            }
            let uuid = mojang_uuid(state, nick).await?;
            mojang_texture(state, &uuid).await
        }
        AccountKind::Plugin => {
            let results = crate::plugins::call_hook(
                "auth.skin",
                &serde_json::json!({ "accountId": acc.id, "name": acc.name }),
            );
            results.into_iter().rev().find_map(|r| {
                r.result
                    .get("url")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("https://"))
                    .map(|s| s.to_string())
            })
        }
    }
}

/// Скин LittleSkin: профиль Yggdrasil содержит base64-свойство textures.
async fn littleskin_skin(state: &AppState, id: &str) -> Option<String> {
    let url = format!(
        "https://littleskin.cn/api/yggdrasil/sessionserver/session/minecraft/profile/{}",
        urlencoding::encode(id)
    );
    let v: serde_json::Value = state.http.get(url).send().await.ok()?.json().await.ok()?;
    let tex = v
        .get("properties")?
        .as_array()?
        .iter()
        .find(|p| p.get("name").and_then(|x| x.as_str()) == Some("textures"))?
        .get("value")?
        .as_str()?;
    let raw = String::from_utf8(base64_decode(tex)?).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("textures")?
        .get("SKIN")?
        .get("url")?
        .as_str()
        .map(str::to_string)
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = TABLE.iter().position(|&x| x == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// UUID профиля Mojang по нику. `None`, если такого ника нет.
async fn mojang_uuid(state: &AppState, nick: &str) -> Option<String> {    let r = state
        .http
        .get(format!("{MOJANG_USERS}/{}", urlencoding::encode(nick)))
        .send()
        .await
        .ok()?;
    // На несуществующий ник Mojang отвечает 404/204.
    if !r.status().is_success() {
        return None;
    }
    let v: Value = r.json().await.ok()?;
    v.get("id")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Ссылка на текстуру из свойства `textures` профиля Mojang.
async fn mojang_texture(state: &AppState, uuid: &str) -> Option<String> {
    let v: Value = state
        .http
        .get(format!("{MOJANG_PROFILE}/{uuid}"))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;

    let b64 = v
        .get("properties")
        .and_then(|x| x.as_array())
        .and_then(|a| {
            a.iter()
                .find(|p| p.get("name").and_then(|x| x.as_str()) == Some("textures"))
        })
        .and_then(|p| p.get("value").and_then(|x| x.as_str()))?;

    let tex: Value = serde_json::from_slice(&b64_decode(b64)?).ok()?;
    let url = tex.pointer("/textures/SKIN/url").and_then(|x| x.as_str())?;
    // Mojang отдаёт ссылку по http; качаем по https.
    Some(match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    })
}

/// Скачивает текстуру и упаковывает её в data-URL.
async fn fetch(state: &AppState, url: &str) -> Option<Skin> {
    let bytes = state
        .http
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .await
        .ok()?;

    // Не PNG (например, HTML-страница ошибки) — скина нет.
    if bytes.len() < PNG_HEADER || &bytes[..4] != b"\x89PNG" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if width == 0 || height == 0 {
        return None;
    }

    Some(Skin {
        url: format!("data:image/png;base64,{}", b64_encode(&bytes)),
        width,
        height,
    })
}

// ---------------------------------------------------------------------------
// base64
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn b64_value(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 26),
        b'0'..=b'9' => Some((c - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in text.as_bytes() {
        // Переносы строк и выравнивание пропускаем.
        if c == b'=' || c == b'\n' || c == b'\r' {
            continue;
        }
        let v = b64_value(c)?;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 % 256) as u8).collect();
            let text = b64_encode(&data);
            assert_eq!(b64_decode(&text).unwrap_or_default(), data, "длина {len}");
        }
    }

    #[test]
    fn base64_known_values() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_decodes_textures_property() {
        // Фрагмент реального значения свойства `textures` (с переносами строк).
        let raw = b"{\"textures\":{\"SKIN\":{\"url\":\"http://x/1\"}}}";
        let text = b64_encode(raw);
        assert_eq!(b64_decode(&text).unwrap_or_default(), raw);
    }

    #[test]
    fn base64_rejects_garbage() {
        assert!(b64_decode("not base64!!!").is_none());
    }

    fn acc(kind: AccountKind, name: &str) -> Account {
        Account {
            id: "test".to_string(),
            kind,
            name: name.to_string(),
            note: String::new(),
        }
    }

    #[tokio::test]
    async fn elyby_skin_uses_elyby_service() {
        let st = AppState::new();
        let u = skin_url(&st, &acc(AccountKind::Elyby, "Steve")).await;
        assert_eq!(u.as_deref(), Some("https://skinsystem.ely.by/skins/Steve.png"));
    }

    #[tokio::test]
    async fn elyby_skin_encodes_nick() {
        let st = AppState::new();
        let u = skin_url(&st, &acc(AccountKind::Elyby, "a b")).await;
        assert_eq!(u.as_deref(), Some("https://skinsystem.ely.by/skins/a%20b.png"));
    }

    #[tokio::test]
    async fn empty_nick_has_no_skin() {
        let st = AppState::new();
        for kind in [AccountKind::Elyby, AccountKind::Offline] {
            assert!(skin_url(&st, &acc(kind, "   ")).await.is_none());
        }
    }

    #[test]
    fn mojang_textures_property_yields_skin_url() {
        // Реальное значение свойства `textures` из профиля Notch.
        let b64 = "ewogICJ0aW1lc3RhbXAiIDogMTc5MjA1NzI0NzQ5NCwKICAicHJvZmlsZUlkIiA6ICIwNjlhNzlmNDQ0ZTk0NzI2YTViZWZjYTkwZTM4YWFmNSIsCiAgInByb2ZpbGVOYW1lIiA6ICJOb3RjaCIsCiAgInRleHR1cmVzIiA6IHsKICAgICJTS0lOIiA6IHsKICAgICAgInVybCIgOiAiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS8yOTIwMDlhNDkyNWI1OGYwMmM3N2RhZGMzZWNlZjA3ZWE0Yzc0NzJmNjRlMGZkYzMyY2U1NTIyNDg5MzYyNjgwIgogICAgfQogIH0KfQ==";
        let raw = b64_decode(b64).unwrap_or_default();
        let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
        let url = v
            .pointer("/textures/SKIN/url")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        assert_eq!(
            url,
            "http://textures.minecraft.net/texture/292009a4925b58f02c77dadc3ecef07ea4c7472f64e0fdc32ce5522489362680"
        );
    }
}
