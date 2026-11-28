//! JSON 行日志：tracing-subscriber 的 json() 格式（手册 3.x）。
//! 启动期 init 返回 Result，调用方以 `// fail-fast:` 注释决定如何处理。

use tracing_subscriber::fmt;

/// 初始化 JSON 日志，RUST_LOG 控制级别（默认 info）。
/// 重复初始化（如测试多次调用）返回 Err，调用方忽略即可。
pub fn init_json_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    fmt()
        .json()
        .with_env_filter(filter)
        .with_target(true)
        .with_current_span(false)
        .try_init()?;
    Ok(())
}
