use super::*;

// ---------------------------------------------------------------------------
// Умная консоль (Б2)
// ---------------------------------------------------------------------------

/// Какому модулю принадлежит класс из стектрейса.
#[tauri::command]
pub async fn find_class_owner(
    id: String,
    class: String,
) -> Res<Option<crate::scanner::classindex::ClassOwner>> {
    crate::scanner::classindex::find_owner(&id, &class).await
}
