use axum::{
    body::Body,
    http::{Request, StatusCode},
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
        Some(token) => Ok(next.run(req).await),
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
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'".parse().unwrap()
    );
    
    response
}
