use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn guard_api_endpoint(
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    match auth_header {
        Some(_token) => Ok(next.run(req).await),
        None => Err((StatusCode::UNAUTHORIZED, "未授权".to_string())),
    }
}

pub async fn device_binding_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let device_id = req.headers()
        .get("X-Device-ID")
        .and_then(|h| h.to_str().ok());
    
    match device_id {
        Some(_id) => Ok(next.run(req).await),
        None => Err((StatusCode::BAD_REQUEST, "缺少设备ID".to_string())),
    }
}

pub async fn permission_checker_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    match auth_header {
        Some(_token) => Ok(next.run(req).await),
        None => Err((StatusCode::UNAUTHORIZED, "未授权".to_string())),
    }
}

pub async fn csp_header_middleware(
    req: Request<Body>,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    
    response.headers_mut().insert(
        "Content-Security-Policy",
        // CSP 是编译期常量，from_static 无解析、不会 panic；connect-src 'none' 强制逻辑全在 Rust。
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
        ),
    );
    
    response
}

pub async fn ssrf_guard_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let uri = req.uri().clone();
    let path = uri.path();
    
    if path.contains("..") || path.contains("etc/passwd") {
        return Err((StatusCode::BAD_REQUEST, "非法请求路径".to_string()));
    }
    
    Ok(next.run(req).await)
}
