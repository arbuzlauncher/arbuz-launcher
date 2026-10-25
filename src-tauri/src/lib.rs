mod accounts;
mod ai;
mod benchmark;
mod builder;
mod catalog;
mod commands;
mod content;
mod crash;
mod error;
mod gamedir;
mod http;
mod java;
mod loaders;
mod modrinth_app;
mod mojang;
mod models;
mod net;
mod tunnel;
mod news;
mod packs;
mod paths;
mod prism;
mod sandbox;
mod servers;
mod scanner;
mod sync;
mod skins;
mod state;
mod store;
mod studio;
mod updates;
mod winproc;

use state::AppState;

/// Трей действительно создан. Если на Linux нет appindicator, прятать окно
/// нельзя: иконки не будет и лаунчер не вернуть — закрываемся по-настоящему.
static TRAY_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Показывает главное окно (из трея или по клику по иконке).
fn show_main(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        // Должен быть первым плагином: второй запуск не создаёт окно,
        // а поднимает уже работающий лаунчер.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            // Данные лаунчера лежат в своей папке, а не в AppData, и её путь
            // заранее неизвестен. Разрешаем asset-протоколу читать из неё,
            // иначе обои главной и иконки контента не открываются.
            use tauri::Manager;
            let root = paths::root();
            let _ = std::fs::create_dir_all(&root);
            let _ = app.asset_protocol_scope().allow_directory(&root, true);

            // Закрытие окна прячет лаунчер в трей (если так включено в настройках).
            if let Some(win) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let handle = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let to_tray = {
                                let state = handle.state::<AppState>();
                                let cfg = state.config.lock().await;
                                cfg.close_to_tray
                            };
                            if to_tray && TRAY_OK.load(std::sync::atomic::Ordering::Relaxed) {
                                if let Some(w) = handle.get_webview_window("main") {
                                    let _ = w.hide();
                                }
                            } else {
                                handle.exit(0);
                            }
                        });
                    }
                });
            }

            // Иконка в трее: открыть, скрыть, выйти. Ошибка трея не должна
            // мешать запуску лаунчера — без иконки он просто работает как раньше.
            let tray_result = (|| -> tauri::Result<()> {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;
                let show = MenuItem::with_id(app, "show", "Открыть", true, None::<&str>)?;
                let hide = MenuItem::with_id(app, "hide", "Скрыть", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &hide, &quit])?;
                let mut tray = TrayIconBuilder::with_id("main")
                    .tooltip("Arbuz Launcher")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, ev| match ev.id.as_ref() {
                        "show" => show_main(app),
                        "hide" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main(tray.app_handle());
                        }
                    });
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray.build(app)?;
                Ok(())
            })();
            // Ошибка трея не должна мешать запуску: без иконки лаунчер
            // работает как раньше, но тогда закрытие окна завершает процесс,
            // а не прячет его в несуществующий трей.
            if tray_result.is_ok() {
                TRAY_OK.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::export_settings,
            commands::import_settings,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::set_instance_game_dir,
            commands::set_instance_icon,
            commands::set_instance_icon_url,
            commands::clear_instance_icon,
            commands::open_instance_folder,
            commands::read_game_log,
            commands::open_logs_folder,
            commands::get_system_ram_mb,
            commands::list_mc_versions,
            commands::add_custom_version,
            commands::remove_custom_version,
            commands::list_loader_versions,
            commands::detect_javas,
            commands::account_status,
            commands::catalog_project,
            commands::install_java,
            commands::list_accounts,
            commands::offline_add,
            commands::set_active_account,
            commands::remove_account,
            commands::ely_login,
            commands::little_login,
            commands::ms_begin,
            commands::ms_poll,
            commands::ms_cancel,
            commands::launch_instance,
            commands::stop_instance,
            commands::list_content,
            commands::toggle_content,
            commands::toggle_isolation,
            commands::remove_content,
            commands::list_worlds,
            commands::list_screenshots,
            commands::open_screenshots_folder,
            commands::open_screenshot,
            commands::search_catalog,
            commands::install_catalog_item,
            commands::check_updates,
            commands::apply_updates,
            commands::install_modpack,
            commands::list_modpack_versions,
            commands::pack_update,
            commands::apply_pack_update,
            commands::import_pack,
            commands::import_prism_instance,
            commands::list_modrinth_profiles,
            commands::import_modrinth_profile,
            commands::diagnose_crash,
            commands::open_crash_reports,
            commands::ai_diagnose_crash,
            commands::ai_pack_plan,
            commands::ai_models,
            commands::pack_info,
            commands::save_pack_info,
            commands::pack_configs,
            commands::read_pack_config,
            commands::save_pack_config,
            commands::restore_pack_config,
            commands::validate_pack,
            commands::export_pack,
            commands::open_pack_configs,
            commands::account_skin,
            commands::get_news,
            commands::open_data_folder,
            commands::data_dir,
            commands::set_home_wallpaper,
            commands::clear_home_wallpaper,
            commands::lan_port,
            commands::net_share,
            commands::net_unshare,
            commands::net_active,
            commands::p2p_host,
            commands::p2p_room,
            commands::p2p_join,
            commands::p2p_stop,
            commands::p2p_active,
            commands::p2p_room_state,
            commands::p2p_kick,
            commands::p2p_admin,
            commands::p2p_new_code,
            commands::p2p_notice,
            commands::check_launcher_update,
            commands::scan_mods,
            commands::scanner_allow_mod,
            commands::scanner_quarantine,
            commands::scanner_quarantine_restore,
            commands::scanner_quarantine_delete,
            commands::ai_scan_note,
            commands::sync_master_options,
            commands::find_class_owner,
            commands::servers_catalog,
            commands::server_ping,
            commands::servers_dat_add,
            commands::pinned_servers,
            commands::pin_server,
            commands::game_monitor,
            commands::benchmark_start,
            commands::benchmark_cancel,
            commands::benchmark_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
