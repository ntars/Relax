mod commands;
mod state;

use tauri::{App, Manager};
use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .setup(|app| {
            app.manage(state::AppInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running relax application");
}
