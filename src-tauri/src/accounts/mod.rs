pub mod elyby;
pub mod littleskin;
pub mod microsoft;
pub mod offline;

use crate::error::{err, Res};
use crate::models::{Account, Config};
use crate::{paths, store};

const SERVICE: &str = "arbuz-launcher";

/// Диспетчер учётных данных Windows принимает blob не длиннее
/// `CRED_MAX_CREDENTIAL_BLOB_SIZE` = 2560 байт, а `keyring` хранит пароль в
/// UTF-16 (см. `keyring::windows`), т.е. на одну запись приходится не более
/// 1280 UTF-16-юнитов. Токены Microsoft (refresh + minecraft) в это не влезают,
/// поэтому длинный секрет режется на части: в записи с самим `id` лежит только
/// заголовок `parts:<n>`, а куски — в записях `<id>#0`, `<id>#1`, …
///
/// Делим по UTF-16-юнитам, а не по `char`: символ вне BMP занимает два юнита,
/// и «1200 символов» могли бы не влезть в лимит.
const MAX_UNITS: usize = 1200;
const MAX_PARTS: usize = 16;
const PARTS_PREFIX: &str = "parts:";

fn part_key(id: &str, n: usize) -> String {
    format!("{id}#{n}")
}

/// Длина строки в UTF-16-юнитах — именно её проверяет `keyring` на Windows.
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Режет строку на куски не длиннее `MAX_UNITS` UTF-16-юнитов. Границы всегда
/// проходят по границе `char`, поэтому суррогатная пара не разрывается.
/// Короткая строка даёт один кусок.
fn split_parts(text: &str) -> Vec<String> {
    if utf16_len(text) <= MAX_UNITS {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut used = 0usize;
    for c in text.chars() {
        let w = c.len_utf16();
        if used + w > MAX_UNITS {
            out.push(std::mem::take(&mut cur));
            used = 0;
        }
        cur.push(c);
        used += w;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Секреты аккаунта в системном хранилище (`keyring`), значение — JSON-строка.
pub(crate) fn secret_get(id: &str) -> Option<serde_json::Value> {
    let head = keyring::Entry::new(SERVICE, id).ok()?.get_password().ok()?;
    let text = match head.strip_prefix(PARTS_PREFIX) {
        Some(n) => {
            let count: usize = n.parse().ok()?;
            if count == 0 || count > MAX_PARTS {
                return None;
            }
            let mut joined = String::new();
            for i in 0..count {
                joined.push_str(&keyring::Entry::new(SERVICE, &part_key(id, i)).ok()?.get_password().ok()?);
            }
            joined
        }
        None => head,
    };
    serde_json::from_str(&text).ok()
}

/// Токен аккаунта, созданного плагином: `{"token": "...", "provider": "..."}`.
pub(crate) fn plugin_token(id: &str) -> Option<String> {
    secret_get(id)?
        .get("token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Провайдер плагина, через который добавлен аккаунт.
pub(crate) fn plugin_provider(id: &str) -> Option<String> {
    secret_get(id)?
        .get("provider")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Записывает секреты аккаунта в системное хранилище.
pub(crate) fn secret_set(id: &str, value: &serde_json::Value) -> Res<()> {
    let parts = split_parts(&value.to_string());
    if parts.len() > MAX_PARTS {
        return err(format!(
            "Секрет аккаунта слишком длинный: {} частей при максимуме {MAX_PARTS}",
            parts.len()
        ));
    }
    // Сначала куски, заголовок — последним. Пока заголовок не переписан,
    // читатель видит прежний секрет целиком, а не половину нового.
    for (n, part) in parts.iter().enumerate() {
        keyring::Entry::new(SERVICE, &part_key(id, n))?.set_password(part)?;
    }
    let head = if parts.len() > 1 {
        format!("{PARTS_PREFIX}{}", parts.len())
    } else {
        parts[0].clone()
    };
    keyring::Entry::new(SERVICE, id)?.set_password(&head)?;
    // Хвосты от прошлого, более длинного секрета, больше не нужны.
    for n in parts.len()..MAX_PARTS {
        if let Ok(entry) = keyring::Entry::new(SERVICE, &part_key(id, n)) {
            let _ = entry.delete_credential();
        }
    }
    Ok(())
}

/// Удаляет заголовок и все части секрета из системного хранилища.
pub(crate) fn secret_clear(id: &str) {
    if let Ok(entry) = keyring::Entry::new(SERVICE, id) {
        let _ = entry.delete_credential();
    }
    for n in 0..MAX_PARTS {
        if let Ok(entry) = keyring::Entry::new(SERVICE, &part_key(id, n)) {
            let _ = entry.delete_credential();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_secret_is_one_part() {
        let parts = split_parts(r#"{"msRefresh":"abc","mcToken":"xyz"}"#);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], r#"{"msRefresh":"abc","mcToken":"xyz"}"#);
    }

    #[test]
    fn long_secret_survives_round_trip() {
        // Реальный секрет Microsoft: refresh (~1200) + minecraft (~2200).
        let raw = serde_json::json!({ "msRefresh": "M.R3_BAY_".repeat(150), "mcToken": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.".repeat(60) });
        let text = raw.to_string();
        let parts = split_parts(&text);
        assert!(parts.len() > 1, "длинный секрет должен резаться на части");
        assert!(parts.len() <= MAX_PARTS, "частей не больше {MAX_PARTS}");
        assert!(parts.iter().all(|p| utf16_len(p) <= MAX_UNITS));
        let joined: String = parts.concat();
        assert_eq!(joined, text);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&joined).unwrap(), raw);
    }

    #[test]
    fn every_part_fits_windows_credential_limit() {
        // keyring на Windows отвергает пароль длиннее 2560 байт в UTF-16.
        let parts = split_parts(&"a".repeat(50_000));
        assert!(parts.len() > 1);
        for part in &parts {
            assert!(
                utf16_len(part) * 2 <= 2560,
                "часть длиннее лимита Windows Credential Manager"
            );
        }
    }

    #[test]
    fn multibyte_secret_survives_round_trip() {
        // Символы вне BMP занимают два UTF-16-юнита и не должны разрываться.
        let text = "🪨токен".repeat(900);
        let parts = split_parts(&text);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| utf16_len(p) <= MAX_UNITS));
        assert_eq!(parts.concat(), text);
        for p in &parts {
            let units: Vec<u16> = p.encode_utf16().collect();
            assert!(
                String::from_utf16(&units).is_ok(),
                "часть разрезала суррогатную пару"
            );
        }
    }

    #[test]
    fn part_keys_do_not_collide_with_account_id() {
        assert_ne!(part_key("abc", 0), "abc");
        assert_ne!(part_key("abc", 0), part_key("abc", 1));
    }
}

/// Список аккаунтов из `accounts.json`.
pub async fn list() -> Res<Vec<Account>> {
    store::read_accounts().await
}

/// Активный аккаунт: из `config.active_account`, иначе первый в списке, иначе `None`.
pub async fn active() -> Res<Option<Account>> {
    let list = list().await?;
    if list.is_empty() {
        return Ok(None);
    }
    let cfg: Config = store::read_json(&paths::config_file()).await?;
    if let Some(id) = cfg.active_account {
        if let Some(found) = list.iter().find(|a| a.id == id) {
            return Ok(Some(found.clone()));
        }
    }
    Ok(Some(list[0].clone()))
}

/// Добавляет аккаунт в `accounts.json`.
pub async fn add(acc: Account) -> Res<()> {
    let mut list = list().await?;
    list.push(acc);
    store::write_accounts(&list).await
}

/// Записывает активный аккаунт в `config.json`.
pub async fn set_active(id: &str) -> Res<()> {
    let mut cfg: Config = store::read_json(&paths::config_file()).await?;
    cfg.active_account = Some(id.to_string());
    store::write_json_atomic(&paths::config_file(), &cfg).await
}

/// Удаляет аккаунт из `accounts.json` и из системного хранилища.
/// Если удалённый был активным — активным становится первый оставшийся.
pub async fn remove(id: &str) -> Res<()> {
    let mut list = list().await?;
    let pos = match list.iter().position(|a| a.id == id) {
        Some(p) => p,
        None => return err("Аккаунт не найден"),
    };
    list.remove(pos);
    store::write_accounts(&list).await?;

    secret_clear(id);

    let mut cfg: Config = store::read_json(&paths::config_file()).await?;
    if cfg.active_account.as_deref() == Some(id) {
        cfg.active_account = list.first().map(|a| a.id.clone());
        store::write_json_atomic(&paths::config_file(), &cfg).await?;
    }
    Ok(())
}
