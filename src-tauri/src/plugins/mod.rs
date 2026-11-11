//! Система Python-плагинов.
//!
//! `.aplug` — обычный Python-файл с необязательным манифестом в заголовке.
//! `.arbz` — zip-архив с несколькими `.aplug`, ассетами и темами.
//!
//! Плагины получают модуль `arbuz` (см. `runtime::ARBUZ_API`), подписываются
//! на хуки декоратором `@arbuz.hook("...")` и могут регистрировать источники
//! каталога, темы и UI-панели. Права проверяются по манифесту.

pub mod install;
pub mod manifest;
pub mod manager;
pub mod runtime;

pub use manager::call_hook;

use serde_json::Value;

/// Вызывает хук и возвращает все результаты. Если плагины выключены — пусто.
pub fn hook(event: &str, payload: Value) -> Vec<manager::HookResult> {
    manager::call_hook(event, &payload)
}

/// Последний непустой объект, который вернули плагины.
pub fn hook_value(event: &str, payload: Value) -> Option<Value> {
    manager::call_hook(event, &payload)
        .into_iter()
        .rev()
        .find(|r| r.error.is_none() && !r.result.is_null())
        .map(|r| r.result)
}

/// Проверяет `{"cancel": true}` в ответе хука. Возвращает текст для ошибки.
pub fn hook_cancel(event: &str, payload: Value) -> Option<String> {
    manager::call_hook(event, &payload)
        .into_iter()
        .find_map(|r| {
            if r.error.is_some() {
                return None;
            }
            if r.result.get("cancel").and_then(|v| v.as_bool()).unwrap_or(false) {
                Some(
                    r.result
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Действие отменено плагином")
                        .to_string(),
                )
            } else {
                None
            }
        })
}
