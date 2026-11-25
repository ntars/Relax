pub mod auth;
pub mod chat;
pub mod sync;

use axum::{Router, routing::get};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api_router = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth::router())
        .nest("/chat", chat::router())
        .nest("/sync", sync::router());
    
    Router::new()
        .with_state(state)
        .nest("/api/v1", api_router)
}

async fn health() -> &'static str {
    "ok"
}
