mod commands;
mod state;

use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // fail-fast: Tauri 事件循环启动失败必须立刻暴露，不能带病运行。
    tauri::Builder::default()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running relax application");
}

#[derive(Clone)]
pub struct AppInfo {
    pub version: String,
}
