pub mod announcements;
pub mod auth;
pub mod birthdays;
pub mod chat;
pub mod devices;
pub mod files;
pub mod folders;
pub mod invites;
pub mod news;
pub mod photos;
pub mod routines;
pub mod schedules;
pub mod sse;
pub mod sync;

use axum::{extract::Extension, Json, Router, routing::get};
use relax_contracts::Envelope;
use serde_json::{json, Value};
use crate::{
    common::{envelope::ok_envelope, trace::TraceId},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    let api_router = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth::router())
        .nest("/devices", devices::router())
        .nest("/invites", invites::router())
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

async fn health(Extension(trace): Extension<TraceId>) -> Json<Envelope<Value>> {
    Json(ok_envelope(
        json!({ "status": "ok", "service": "relax-server" }),
        &trace.0,
    ))
}
