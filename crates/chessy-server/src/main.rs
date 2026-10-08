use std::path::Path;
use std::time::Duration;

use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use chessy_server::{router, spawn_session_purge, App};
use tower_http::services::{ServeDir, ServeFile};

/// A whole number from the environment; a value that does not parse is ignored.
fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.trim().parse().ok()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chessy_server=info".to_string()),
        )
        .init();

    let db_path = std::env::var("CHESSY_DB").unwrap_or_else(|_| "chessy.sqlite".to_string());
    let addr = std::env::var("CHESSY_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let web_dir = std::env::var("CHESSY_WEB_DIR").unwrap_or_else(|_| "web/dist".to_string());

    let mut config = HubConfig::default();
    if let Some(days) = env_number("CHESSY_SESSION_TTL_DAYS") {
        config.session_ttl = Duration::from_secs(days * 24 * 3600);
    }
    if let Some(secs) = env_number("CHESSY_SESSION_PURGE_SECS") {
        config.session_purge_interval = Duration::from_secs(secs.max(1));
    }

    let store = Store::open(&db_path).expect("open database");
    spawn_session_purge(
        store.clone(),
        config.session_purge_interval,
        config.session_ttl,
    );
    let mut app = router(App::new(store, config));

    // Serve the built client when present; in development Vite serves it instead.
    if Path::new(&web_dir).join("index.html").exists() {
        let index = ServeFile::new(Path::new(&web_dir).join("index.html"));
        app = app.fallback_service(ServeDir::new(&web_dir).not_found_service(index));
        tracing::info!("serving client from {web_dir}");
    }

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind address");
    tracing::info!("chessy-server listening on http://{addr} (db: {db_path})");
    axum::serve(listener, app).await.expect("server error");
}
