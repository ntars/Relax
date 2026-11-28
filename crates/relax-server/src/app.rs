//! app 组装：路由 + traceId 中间件 + 统一错误映射（手册 3.2 / 17.1）。
//! 成功响应由构造器包信封，失败响应由 AppError::into_response 包错误信封。

use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::{self, Next},
    response::Response,
    Router,
};

use crate::{
    common::trace::trace_id_from_headers,
    state::AppState,
};

/// 组装完整路由：注入 trace 层（提取/生成 traceId，写入扩展与响应头）。
pub fn app(state: AppState) -> Router {
    crate::routes::router(state).layer(middleware::from_fn(trace_layer))
}

async fn trace_layer(mut req: Request, next: Next) -> Response {
    let trace = trace_id_from_headers(req.headers());
    req.extensions_mut().insert(trace.clone());
    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(&trace.0) {
        resp.headers_mut().insert("x-trace-id", v);
    }
    resp
}
