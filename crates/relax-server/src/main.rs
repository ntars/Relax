use tracing_subscriber::fmt;
use std::net::SocketAddr;
use std::path::Path;

mod state;
mod middleware;
mod routes;
mod db;

use state::{AppState, AppInfo};
use routes::router;
use db::Database;

#[tokio::main]
async fn main() {
    fmt()
        .init();

    let db_path = Path::new("data/relax.db");
    
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
            tracing::warn!("Failed to create data directory: {}", e);
        });
    }

    let database = Database::open(db_path).expect("Failed to open database");
    database.init().expect("Failed to initialize database");

    let app_state = AppState {
        app_info: std::sync::Arc::new(tokio::sync::RwLock::new(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
        })),
    };

    let app = router(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
