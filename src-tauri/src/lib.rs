mod commands;
mod state;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // fail-fast: Tauri 事件循环启动失败必须立刻暴露，不能带病运行。
    tauri::Builder::default()
        .plugin(tauri_plugin_relax_mobile::init())
        .invoke_handler(tauri::generate_handler![
            commands::auth_enroll,
            commands::auth_login,
            commands::auth_bind,
            commands::login,
            commands::chat_send,
            commands::chat_list,
            commands::sync_data,
            commands::outbox_list,
            commands::retry_op,
            commands::announcement_list,
            commands::announcement_mark_read,
            commands::announcement_publish,
            commands::announcement_download_attachment,
            commands::news_list,
            commands::news_add_feed,
            commands::news_remove_feed,
            commands::news_mark_read,
            commands::routine_list,
            commands::routine_create,
            commands::schedule_list,
            commands::schedule_create,
            commands::schedule_plan_alarms,
            commands::file_list,
            commands::file_upload,
            commands::file_recycle_list,
            commands::file_restore,
            commands::file_delete,
            commands::photo_list,
            commands::album_list,
            commands::album_create,
            commands::album_add_files,
            commands::recycle_list_admin,
            commands::recycle_restore_admin,
            commands::recycle_clear_admin,
            commands::audit_list,
            commands::server_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running relax application");
}

#[derive(Clone)]
pub struct AppInfo {
    pub version: String,
}
