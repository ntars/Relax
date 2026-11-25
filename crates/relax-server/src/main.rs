fn main() {
    tracing_subscriber::fmt::init();

    let app_state = relax_server::AppState {
        app_info: std::sync::Arc::new(tokio::sync::RwLock::new(relax_server::AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
        })),
    };

    let app = relax_server::router();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
                .await
                .expect("Failed to bind to port 3000");

            axum::serve(listener, app)
                .await
                .expect("Server failed");
        });
}
