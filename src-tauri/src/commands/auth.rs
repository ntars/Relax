//! 认证命令（手册 5.2 / 9.2 C2.5）：入网 / 登录（含 TOTP）/ 设备绑定，invoke only。
//! 客户端只按服务端信封的 error.code 分支（手册 3.2），把码编码进 Err 前缀
//! `"CODE:message"`，由前端 api.ts::errorCodeOf 提取；绝不按 message 或 HTTP 文案分支。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CODE_UNREACHABLE: &str = "3001";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollRequest {
    pub invite_code: String,
    pub name: String,
    pub device_id: String,
    pub device_label: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub member_id: String,
    pub password: String,
    #[serde(default)]
    pub totp: Option<String>,
    pub device_id: String,
}

/// 把服务端响应归一成 AuthToken；按 ok 分支，失败时取 error.code 编码进 Err。
fn unwrap_session(body: &Value) -> Result<Value, String> {
    let ok = body.get("ok").and_then(Value::as_bool).unwrap_or(false);
    if ok {
        let session = body.get("data").cloned().unwrap_or_else(|| json!({}));
        let token = session
            .get("sessionToken")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return Ok(json!({ "token": token, "expires_at": 0 }));
    }
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let message = body
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("认证失败");
    Err(format!("{}:{}", code, message))
}

fn request_err(e: reqwest::Error) -> String {
    format!("{}:{}", CODE_UNREACHABLE, e)
}

#[tauri::command]
pub async fn auth_enroll(request: EnrollRequest) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let body = client
        .post("http://localhost:3000/api/auth/enroll")
        .json(&request)
        .send()
        .await
        .map_err(request_err)?
        .json::<Value>()
        .await
        .map_err(request_err)?;
    unwrap_session(&body)
}

#[tauri::command]
pub async fn auth_login(request: LoginRequest) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let body = client
        .post("http://localhost:3000/api/auth/login")
        .json(&request)
        .send()
        .await
        .map_err(request_err)?
        .json::<Value>()
        .await
        .map_err(request_err)?;
    unwrap_session(&body)
}

#[tauri::command]
pub async fn auth_bind(device_id: String, label: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload = json!({ "deviceId": device_id, "label": label, "deviceLabel": label });
    let body = client
        .post("http://localhost:3000/api/devices")
        .json(&payload)
        .send()
        .await
        .map_err(request_err)?
        .json::<Value>()
        .await
        .map_err(request_err)?;
    unwrap_session(&body).map(|_| ())
}
