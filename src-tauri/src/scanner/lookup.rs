//! Проверка хеша мода в каталогах (Modrinth и CurseForge, обе по SHA-1:
//! Modrinth принимает только sha1/sha512, CurseForge — sha1/sha512/md5).
//!
//! Ошибки сети не роняют сканирование: хеш считается «не найденным»,
//! а признак недоступности сети возвращается наружу для честного сообщения
//! в параноидальном режиме.

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";

#[derive(Clone, Debug, Default)]
pub(crate) struct LookupResult {
    pub modrinth_found: bool,
    pub modrinth_project: Option<String>,
    pub curseforge_found: bool,
    /// Сеть была недоступна (для сообщения в параноидальном режиме).
    pub offline: bool,
}

/// Ищет хеш в Modrinth (`version_file`) и, если задан ключ CurseForge,
/// в CurseForge (`files`). Обе проверки идут по sha1. Не паникует и не
/// возвращает Err.
pub(crate) async fn by_hash(
    http: &reqwest::Client,
    sha1: &str,
    cf_key: Option<&str>,
) -> LookupResult {
    let mut out = LookupResult::default();

    let modrinth = async {
        // Modrinth знает только sha1/sha512 — sha256 он честно отвечает 404.
        let url = format!("{MODRINTH}/version_file/{sha1}?algorithm=sha1");
        match http.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json().await.unwrap_or_default();
                Some((
                    true,
                    v.get("project_id").and_then(|p| p.as_str()).map(str::to_string),
                ))
            }
            // 404 — хеша нет в каталоге: это нормальный ответ, не ошибка сети.
            Ok(r) if r.status().as_u16() == 404 => Some((false, None)),
            Ok(_) => None,
            Err(_) => None,
        }
    };

    let curseforge = async {
        let key = cf_key?;
        if sha1.is_empty() {
            return Some((false, false));
        }
        let body = serde_json::json!({ "hashes": [sha1], "algorithm": "sha1" });
        match http
            .post(format!("{CURSEFORGE}/files"))
            .header("x-api-key", key)
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json().await.unwrap_or_default();
                Some((v.get("data").and_then(|d| d.as_array()).is_some_and(|a| !a.is_empty()), true))
            }
            Ok(r) if r.status().as_u16() == 404 => Some((false, true)),
            Ok(_) => None,
            Err(_) => None,
        }
    };

    let (mr, cf) = tokio::join!(modrinth, curseforge);
    match mr {
        Some((found, project)) => {
            out.modrinth_found = found;
            out.modrinth_project = project;
        }
        None => out.offline = true,
    }
    match cf {
        Some((found, _)) => out.curseforge_found = found,
        None => {
            if cf_key.is_some() {
                out.offline = true;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Живая проверка против Modrinth: несуществующий sha1 не находится.
    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn modrinth_rejects_unknown_hash() {
        let http = reqwest::Client::new();
        let sha1 = "0000000000000000000000000000000000000000";
        let r = by_hash(&http, sha1, None).await;
        assert!(!r.modrinth_found, "пустой хеш не должен находиться");
    }
}
