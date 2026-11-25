use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub app_info: Arc<RwLock<AppInfo>>,
}

#[derive(Clone)]
pub struct AppInfo {
    pub version: String,
}
