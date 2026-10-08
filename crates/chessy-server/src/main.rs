use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use chessy_server::{router, spawn_session_purge, App};
use tower_http::services::{ServeDir, ServeFile};

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
    if let Some(days) = env_number::<u64>("CHESSY_SESSION_TTL_DAYS") {
        config.session_ttl = Duration::from_secs(days * 24 * 3600);
    }
    if let Some(secs) = env_number::<u64>("CHESSY_SESSION_PURGE_SECS") {
        config.session_purge_interval = Duration::from_secs(secs.max(1));
    }

    // 0 = unlimited. The per-IP cap is off unless asked for.
    if let Some(n) = env_number::<usize>("CHESSY_MAX_CONNECTIONS") {
        config.max_connections = n;
    }
    if let Some(n) = env_number::<usize>("CHESSY_MAX_CONNECTIONS_PER_IP") {
        config.max_connections_per_ip = n;
    }
    // 0 = no per-address count of failed recovery attempts (the default).
    if let Some(n) = env_number::<u32>("CHESSY_RECOVERY_MAX_FAILURES_PER_IP") {
        config.recovery_max_failures_per_ip = n;
    }
    // Behind a reverse proxy that sets X-Forwarded-For itself (`1` or `true`).
    config.trust_proxy = matches!(
        std::env::var("CHESSY_TRUST_PROXY").as_deref(),
        Ok("1" | "true")
    );

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
    // Connect info gives the WebSocket handler the peer address for the per-IP cap.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}

/// A non-negative integer from the environment; unset or unreadable is `None`.
fn env_number<T: std::str::FromStr>(name: &str) -> Option<T> {
    let raw = std::env::var(name).ok()?;
    match raw.trim().parse() {
        Ok(n) => Some(n),
        Err(_) => {
            tracing::warn!("ignoring {name}={raw:?}: not a number");
            None
        }
    }
}
