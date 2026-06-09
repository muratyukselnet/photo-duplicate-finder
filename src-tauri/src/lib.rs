mod commands;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::new().map_err(|e| std::io::Error::other(e))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::create_session,
            commands::delete_session,
            commands::get_scan_progress,
            commands::start_scan,
            commands::pause_scan,
            commands::resume_scan,
            commands::stop_scan,
            commands::list_duplicate_groups,
            commands::get_group_detail,
            commands::set_keepers,
            commands::keep_all_in_group,
            commands::move_duplicates_to_trash,
            commands::keep_selected_and_trash,
            commands::ensure_thumbnail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
