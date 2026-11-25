use tauri::command;

#[command]
pub fn app_info() -> String {
    "0.1.0".to_string()
}

#[command]
pub fn net_type() -> &'static str {
    "unknown"
}
