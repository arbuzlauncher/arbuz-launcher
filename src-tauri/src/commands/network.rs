use super::*;

// ---------------------------------------------------------------------------
// ЭТАП 10: игра по сети
// ---------------------------------------------------------------------------

/// Порт, который Minecraft открыл для сети (ищем в логе игры).
#[tauri::command]
pub async fn lan_port(id: String) -> Res<Option<u16>> {
    paths::validate_instance_id(&id)?;
    let port = crate::net::lan_port(&id).await?;
    crate::plugins::hook(
        "game.lan_port",
        serde_json::json!({ "id": id, "port": port }),
    );
    Ok(port)
}

/// Просит роутер пробросить порт — друзья смогут подключиться по внешнему IP.
#[tauri::command]
pub async fn net_share(port: u16) -> Res<crate::net::Share> {
    if port == 0 {
        return err("Неверный порт");
    }
    crate::plugins::hook("net.share.before", serde_json::json!({ "port": port }));
    let share = crate::net::share(port).await?;
    crate::plugins::hook(
        "net.share.after",
        serde_json::json!({ "port": port, "share": &share }),
    );
    Ok(share)
}

/// Закрывает проброс порта.
#[tauri::command]
pub async fn net_unshare() -> Res<()> {
    crate::plugins::hook("net.unshare.before", serde_json::json!({}));
    crate::net::unshare().await?;
    crate::plugins::hook("net.unshare.after", serde_json::json!({}));
    Ok(())
}

/// Открыт ли проброс прямо сейчас (например, после перезапуска окна).
#[tauri::command]
pub fn net_active() -> Option<crate::net::Share> {
    crate::net::active()
}

/// P2P: принимающий показывает код, друзьям проброс порта и белый IP не нужны.
#[tauri::command]
pub async fn p2p_host(port: u16, name: String) -> Res<crate::tunnel::P2p> {
    if port == 0 {
        return err("Неверный порт");
    }
    crate::plugins::hook(
        "p2p.host.before",
        serde_json::json!({ "port": port, "name": name }),
    );
    let p = crate::tunnel::host(port, name).await?;
    crate::plugins::hook("p2p.host.after", serde_json::json!({ "p2p": &p }));
    Ok(p)
}

/// P2P: комната заранее — код можно отдать друзьям до запуска игры.
/// Порт лаунчер найдёт сам, когда в игре нажмут «Открыть для сети».
#[tauri::command]
pub async fn p2p_room(name: String) -> Res<crate::tunnel::P2p> {
    crate::plugins::hook("p2p.room.before", serde_json::json!({ "name": name }));
    let p = crate::tunnel::room(name).await?;
    crate::plugins::hook("p2p.room.after", serde_json::json!({ "p2p": &p }));
    Ok(p)
}

/// P2P: подключаемся к другу по коду и открываем локальный порт для Minecraft.
#[tauri::command]
pub async fn p2p_join(code: String, name: String) -> Res<crate::tunnel::P2p> {
    crate::plugins::hook(
        "p2p.join.before",
        serde_json::json!({ "code": code, "name": name }),
    );
    let p = crate::tunnel::join(&code, name).await?;
    crate::plugins::hook("p2p.join.after", serde_json::json!({ "p2p": &p }));
    Ok(p)
}

/// P2P: закрыть канал.
#[tauri::command]
pub async fn p2p_stop() -> Res<()> {
    crate::plugins::hook("p2p.stop.before", serde_json::json!({}));
    crate::tunnel::stop().await;
    crate::plugins::hook("p2p.stop.after", serde_json::json!({}));
    Ok(())
}

/// P2P: открыт ли канал прямо сейчас.
#[tauri::command]
pub fn p2p_active() -> Option<crate::tunnel::P2p> {
    crate::tunnel::active()
}

/// P2P: кто сейчас в комнате и кто её владелец.
#[tauri::command]
pub fn p2p_room_state() -> Option<crate::tunnel::Room> {
    crate::tunnel::room_state()
}

/// P2P: выгнать игрока (хозяин — сразу, владелец-гость — через хозяина).
#[tauri::command]
pub async fn p2p_kick(slot: u64) -> Res<()> {
    crate::plugins::hook("p2p.kick.before", serde_json::json!({ "slot": slot }));
    crate::tunnel::kick(slot).await?;
    crate::plugins::hook("p2p.kick.after", serde_json::json!({ "slot": slot }));
    Ok(())
}

/// P2P: передать права владельца другому игроку.
#[tauri::command]
pub async fn p2p_admin(slot: u64) -> Res<()> {
    crate::plugins::hook("p2p.admin.before", serde_json::json!({ "slot": slot }));
    crate::tunnel::make_admin(slot).await?;
    crate::plugins::hook("p2p.admin.after", serde_json::json!({ "slot": slot }));
    Ok(())
}

/// P2P: новый код комнаты (старый перестаёт пускать).
#[tauri::command]
pub async fn p2p_new_code() -> Res<crate::tunnel::P2p> {
    crate::plugins::hook("p2p.new_code.before", serde_json::json!({}));
    let p = crate::tunnel::new_code().await?;
    crate::plugins::hook("p2p.new_code.after", serde_json::json!({ "p2p": &p }));
    Ok(p)
}

/// P2P: одноразовое сообщение игроку («вас исключили», «связь потерялась»).
#[tauri::command]
pub fn p2p_notice() -> Option<String> {
    crate::tunnel::notice()
}
