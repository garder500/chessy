pub mod analysis;
pub mod api;
pub mod api_campaign;
pub mod api_games;
pub mod api_live;
pub mod api_skills;
pub mod api_title;
pub mod app;
pub mod boss_forge_store;
pub mod bot;
pub mod campaign;
pub mod campaign_store;
pub mod elo;
pub mod forged_store;
pub mod games_store;
pub mod history_store;
pub mod hub;
pub mod limits;
pub mod moderation;
pub mod pending_rewards_store;
pub mod protocol;
pub mod replay;
pub mod reward_outcome_store;
pub mod seo;
pub mod social;
pub mod store;
pub mod ws;

use std::sync::Arc;
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use store::Store;

pub use app::App;

/// The HTTP router: a WebSocket endpoint at `/ws` and a health check.
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api::routes().merge(api_games::routes()))
        .with_state(app)
}

/// Deletes the sessions unused for more than `ttl`, once now and then every
/// `every`. Runs on its own tokio task, outside the (synchronous) hub; the
/// task ends with the runtime or when the handle is aborted.
pub fn spawn_session_purge(
    store: Store,
    every: Duration,
    ttl: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        loop {
            tick.tick().await;
            let store = store.clone();
            match tokio::task::spawn_blocking(move || store.purge_expired_sessions(ttl)).await {
                Ok(Ok(0)) => {}
                Ok(Ok(n)) => tracing::info!("purged {n} expired sessions"),
                Ok(Err(e)) => tracing::warn!("session purge failed: {e}"),
                Err(e) => tracing::warn!("session purge task failed: {e}"),
            }
        }
    })
}
