pub mod announcements;
pub mod auth;
pub mod birthdays;
pub mod chat;
pub mod files;
pub mod folders;
pub mod news;
pub mod photos;
pub mod routines;
pub mod schedules;
pub mod sse;
pub mod sync;

use axum::{Router, routing::get};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api_router = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth::router())
        .nest("/chat", chat::router())
        .nest("/sync", sync::router())
        .merge(sse::router())
        .nest("/announcements", announcements::router())
        .nest("/birthdays", birthdays::router())
        .nest("/files", files::router())
        .nest("/folders", folders::router())
        .nest("/news", news::router())
        .nest("/photos", photos::router())
        .nest("/routines", routines::router())
        .nest("/schedules", schedules::router());
    
    Router::new()
        .with_state(state)
        .nest("/api/v1", api_router)
}

async fn health() -> &'static str {
    "ok"
}
