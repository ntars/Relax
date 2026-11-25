# Gatekeeping - 门禁策略

## 核心原则

1. **业务逻辑只在 Rust**：前端（TypeScript）只能调用 Tauri 命令，不能包含业务逻辑
2. **契约单一源**：API 错误码、版本范围只在 `contracts/` 中定义
3. **失败留痕**：所有降级必须输出 `ok=false + complete=false + reason`
4. **权限矩阵不可绕过**：每个 API 调用必须验证 token 权限

## 门禁层级

### Layer 1: Tauri Command Whitelist

```rust
// src-tauri/src/commands/mod.rs
mod auth;
mod sync;
mod chat;
mod announcement;
mod feed;
mod routine;
mod schedule;
mod file;
mod photo;
mod backup;

pub fn allowed_commands() -> &'static [&'static str] {
    &[
        "login", "enroll", "logout", "me",
        "sync_now", "sync_status", "outbox_list", "retry_op",
        "chat_list", "chat_send", "chat_mark_read",
        "announcement_list", "announcement_save", "announcement_read",
        "feed_list", "feed_save", "news_list", "news_set_state",
        "routine_list", "routine_save",
        "schedule_list", "schedule_save", "occurrence_list", "birthday_save",
        "folder_list", "folder_save", "file_list",
        "upload_start", "upload_status", "file_download", "file_delete",
        "recycle_list", "recycle_restore",
        "photo_timeline", "album_list", "album_save", "album_add",
        "backup_now", "backup_list", "restore_preview", "doctor", "audit_list", "quota",
        "app_info", "net_type", "notify_permission", "exact_alarm_permission", "battery_permission",
        "photo_sync_toggle",
    ]
}
```

### Layer 2: API Endpoint Guard

```rust
// crates/relax-server/src/middleware/guard.rs
use crate::state::AppState;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn guard_api_endpoint(
    state: AppState,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // 检查 token 权限
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    match auth_header {
        Some(token) => {
            let claims = state.validate_token(token)?;
            let path = req.uri().path();
            
            // 检查路径是否在权限矩阵中
            if !state.check_permission(&claims.role, path) {
                return Err((StatusCode::FORBIDDEN, "权限不足".to_string()));
            }
            
            Ok(next.run(req).await)
        }
        None => Err((StatusCode::UNAUTHORIZED, "未授权".to_string())),
    }
}
```

### Layer 3: Type-level Validation

```rust
// crates/relax-core/src/validation.rs
use validator::{Validate, ValidationError};

#[derive(Debug, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 3, max = 64))]
    pub username: String,
    
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

impl LoginRequest {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        validator::Validated::<Self>::validate(self)
    }
}
```

## 错误码门禁

```rust
// crates/relax-contract/src/error.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: u16,
    pub name: &'static str,
    pub http: u16,
    pub message: String,
    pub action: &'static str,
    pub retryable: bool,
}

impl ApiError {
    pub fn new(code: u16, message: impl Into<String>, action: &'static str) -> Self {
        let info = ERROR_CODES.get(&code).unwrap_or_else(|| {
            panic!("未注册的错误码: {}", code)
        });
        
        Self {
            code,
            name: info.name,
            http: info.http,
            message: message.into(),
            action: info.action,
            retryable: info.retryable,
        }
    }
}
```

## 门禁测试

```rust
// crates/relax-server/tests/gatekeeping.rs
#[tokio::test]
async fn test_csp_violation_blocked() {
    let app = test_app().await;
    
    // 尝试直接访问 API（应被拦截）
    let resp = app.get("/api/users").send().await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_permission_denied() {
    let app = test_app().await;
    
    // 使用 GUEST token 访问写接口（应被拦截）
    let resp = app.post("/api/chat/send")
        .bearer_auth("guest-token")
        .json(&serde_json::json!({
            "to": "user2",
            "content": "test"
        }))
        .send()
        .await;
    
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
```
