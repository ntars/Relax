//! 设备路由接线（手册 5.2）：列出 / 绑定 / 撤销设备。
//! 撤销授权统一走 middleware::can_revoke_device（本人或 revoke_devices 能力）。

use axum::{
    Extension, Json, Router,
    extract::Path,
    http::HeaderMap,
    routing::{delete, get},
};
use chrono::Utc;
use relax_contracts::Envelope;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, middleware, store},
    common::{AppError, envelope::ok_envelope, trace::TraceId},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindDeviceRequest {
    pub device_id: String,
    pub device_label: String,
}

fn session_token(headers: &HeaderMap, trace: &TraceId) -> Result<String, AppError> {
    middleware::bearer_token(headers).ok_or_else(|| middleware::to_app_error(AuthError::Unauthenticated, &trace.0))
}

async fn list(
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let devices = store::list_devices(&conn, &principal.member_id)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data: Vec<Value> = devices
        .iter()
        .map(|d| json!({ "id": d.id, "label": d.label, "memberId": d.member_id }))
        .collect();
    Ok(Json(ok_envelope(json!(data), &trace.0)))
}

async fn bind(
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
    Json(req): Json<BindDeviceRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    if req.device_id.is_empty() {
        return Err(middleware::to_app_error(AuthError::MissingParam, &trace.0));
    }
    let now = Utc::now().to_rfc3339();
    store::insert_device(&conn, &req.device_id, &principal.member_id, &req.device_label, &now)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(
        json!({ "id": req.device_id, "label": req.device_label }),
        &trace.0,
    )))
}

async fn revoke(
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let mut conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    middleware::can_revoke_device(&conn, &principal, &id).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let now = Utc::now().to_rfc3339();
    store::revoke_device(&mut conn, &id, &now).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(json!({ "revoked": id }), &trace.0)))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list).post(bind))
        .route("/:id", delete(revoke))
}
