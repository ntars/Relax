use axum::Router;
use tracing_subscriber::fmt;
use std::net::SocketAddr;
use tracing_subscriber::fmt::SubscriberBuilder;

mod state;
mod middleware;
mod routes;

use state::{AppState, AppInfo};
use routes::router;

#[tokio::main]
async fn main() {
    fmt()
        .init();

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
