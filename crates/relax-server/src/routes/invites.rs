//! 邀请码路由接线（手册 5.2）：列出 / 创建，仅 OWNER/ADMIN（manage_members 能力）。
//! 明文 code 只在创建响应出现一次，列表只返回 sha256 摘要不含明文。

use axum::{Extension, Json, Router, http::HeaderMap, routing::get};
use chrono::{Duration, Utc};
use relax_contracts::Envelope;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, middleware, service, store},
    common::{AppError, envelope::ok_envelope, trace::TraceId},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteRequest {
    pub role: String,
    #[serde(default = "default_max_uses")]
    pub max_uses: i64,
    /// RFC3339；缺省 7 天后过期。
    #[serde(default)]
    pub expires_at: Option<String>,
}

fn default_max_uses() -> i64 {
    1
}

/// 邀请码管理是 OWNER/ADMIN 能力：鉴权 + 能力判定一次完成（中间件单点）。
fn authorize_manager(
    conn: &rusqlite::Connection,
    headers: &HeaderMap,
    trace: &TraceId,
) -> Result<crate::auth::middleware::Principal, AppError> {
    let token = middleware::bearer_token(headers)
        .ok_or_else(|| middleware::to_app_error(AuthError::Unauthenticated, &trace.0))?;
    middleware::authorize(conn, &token, "manage_members")
        .map_err(|e| middleware::to_app_error(e, &trace.0))
}

async fn list(
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
) -> Result<Json<Envelope<Value>>, AppError> {
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    authorize_manager(&conn, &headers, &trace)?;
    let invites = store::list_invites(&conn).map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let data: Vec<Value> = invites
        .iter()
        .map(|i| json!({
            "codeHash": i.code_hash, "role": i.role, "maxUses": i.max_uses,
            "usedCount": i.used_count, "expiresAt": i.expires_at, "createdBy": i.created_by,
        }))
        .collect();
    Ok(Json(ok_envelope(json!(data), &trace.0)))
}

async fn create(
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
    Json(req): Json<CreateInviteRequest>,
) -> Result<Json<Envelope<Value>>, AppError> {
    let conn = middleware::open_db().map_err(|e| middleware::to_app_error(e, &trace.0))?;
    let principal = authorize_manager(&conn, &headers, &trace)?;
    let expires = req.expires_at
        .unwrap_or_else(|| (Utc::now() + Duration::days(7)).to_rfc3339());
    let code = service::create_invite(&conn, &req.role, req.max_uses, &expires, &principal.member_id)
        .map_err(|e| middleware::to_app_error(e, &trace.0))?;
    Ok(Json(ok_envelope(json!({ "code": code, "role": req.role, "expiresAt": expires }), &trace.0)))
}

pub fn router() -> Router {
    Router::new().route("/", get(list).post(create))
}
