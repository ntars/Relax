//! 身份路由接线（手册 5.2）：enroll/login/refresh/logout/me/TOTP。
//! 认证与能力判定统一走 auth::middleware；本层只做请求解析与信封包装。

use axum::{Extension, Json, Router, routing::{get, post}};
use relax_contracts::Envelope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, middleware, service, store},
    common::{AppError, envelope::ok_envelope, trace::TraceId},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollRequest {
    pub invite_code: String,
    pub name: String,
    pub device_id: String,
    pub device_label: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub member_id: String,
    pub password: String,
    #[serde(default)]
    pub totp: Option<String>,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpConfirmRequest {
    pub code: String,
}

/// 会话对 → 手册 5.2 响应体（含 member / capabilities / contractVersion）。
fn session_json(conn: &Connection, pair: &service::SessionPair) -> Result<Value, AuthError> {
    let member = store::get_member(conn, &pair.member_id)?.ok_or(AuthError::Unauthenticated)?;
    Ok(json!({
        "sessionToken": pair.session_token,
        "refreshToken": pair.refresh_token,
        "member": { "id": member.id, "name": member.name, "role": member.role },
        "capabilities": middleware::capabilities_for(&pair.role),
        "contractVersion": relax_contracts::version().version,
    }))
}

/// 从请求头取 session token；缺失归 1001。
fn session_token(headers: &axum::http::HeaderMap, trace: &TraceId) -> Result<String, AppError> {
    middleware::bearer_token(headers).ok_or_else(|| middleware::to_app_error(AuthError::Unauthenticated, &trace.0))
}

async fn enroll(
    Extension(trace): Extension<TraceId>,
    Json(req): Json<EnrollRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let cfg = middleware::config();
    let pair = service::enroll(
        &mut conn, &cfg, &req.invite_code, &req.name, &req.password, &req.device_id, &req.device_label,
    )
    .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data = session_json(&conn, &pair).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(data, &trace.0)))
}

async fn login(
    Extension(trace): Extension<TraceId>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let cfg = middleware::config();
    let pair = service::login(
        &mut conn, &cfg, &req.member_id, &req.password, req.totp.as_deref(), &req.device_id,
    )
    .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data = session_json(&conn, &pair).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(data, &trace.0)))
}

async fn refresh(
    Extension(trace): Extension<TraceId>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let mut conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let cfg = middleware::config();
    let pair = service::refresh(&mut conn, &cfg, &req.refresh_token, &req.device_id)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data = session_json(&conn, &pair).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(data, &trace.0)))
}

async fn logout(
    Extension(trace): Extension<TraceId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    service::logout(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(json!({ "revoked": true }), &trace.0)))
}

async fn me(
    Extension(trace): Extension<TraceId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data = middleware::me_data(&conn, &principal).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(data, &trace.0)))
}

async fn totp_enable(
    Extension(trace): Extension<TraceId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let cfg = middleware::config();
    let (secret, uri) = service::totp_enable(&conn, &cfg, &principal.member_id)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(json!({ "secret": secret, "uri": uri }), &trace.0)))
}

async fn totp_confirm(
    Extension(trace): Extension<TraceId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpConfirmRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let token = session_token(&headers, &trace)?;
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = middleware::authenticate(&conn, &token).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let cfg = middleware::config();
    service::totp_confirm(&conn, &cfg, &principal.member_id, &req.code)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(json!({ "confirmed": true }), &trace.0)))
}

pub fn router() -> Router {
    Router::new()
        .route("/enroll", post(enroll))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/totp/enable", post(totp_enable))
        .route("/totp/confirm", post(totp_confirm))
}
