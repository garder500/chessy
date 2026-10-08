//! REST: replays, analysis and exploration of recorded games
//! (docs/spec-v4.md §2). Errors are `{"error": "<code>"}` like the rest of
//! `/api`. Authentication is optional (`Authorization: Bearer <token>`):
//! Solo games are only readable by their player, the others are public.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Semaphore;

use crate::app::App;
use crate::games_store::StoredGame;
use crate::replay::{self, ExploreRequest, MAX_LINE};
use crate::{analysis, store::StoreError};

const DEFAULT_PAGE: u32 = 20;
const MAX_PAGE: u32 = 100;
/// Bodies of `explore` hold up to 200 actions.
const MAX_EXPLORE_BYTES: usize = 64 * 1024;
const DEFAULT_DEPTH: u32 = 3;
const MAX_DEPTH: u32 = 5;
/// How long the search for the best move of an explored position may run.
const EXPLORE_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
/// Analyses and explorations searching at once; the others wait their turn
/// (and then often find the analysis in the cache).
static SEARCHES: Semaphore = Semaphore::const_new(2);

/// Routes to merge into the `/api` router. Not under its 4 KiB body limit.
pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/me/games", get(my_games))
        .route("/games/{id}", get(game))
        .route("/games/{id}/analysis", get(game_analysis))
        .route(
            "/games/{id}/explore",
            post(explore).layer(DefaultBodyLimit::max(MAX_EXPLORE_BYTES)),
        )
}

struct ApiError(StatusCode, &'static str);

impl ApiError {
    fn not_found() -> Self {
        ApiError(StatusCode::NOT_FOUND, "not_found")
    }
    fn bad_request(code: &'static str) -> Self {
        ApiError(StatusCode::BAD_REQUEST, code)
    }
    fn internal(code: &'static str) -> Self {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, code)
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        tracing::error!("store error: {e}");
        ApiError::internal("internal")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// The caller, if they sent a known token.
fn viewer(app: &App, headers: &HeaderMap) -> ApiResult<Option<String>> {
    match bearer(headers) {
        Some(token) => {
            let config = app.config();
            Ok(app.store().session_player(
                token,
                config.session_ttl,
                config.session_touch_interval,
            )?)
        }
        None => Ok(None),
    }
}

/// One lock per `(game, depth)` analysis, shared by whoever asks for it. It only
/// serialises requests: what they compute is read from and written to their own store.
fn in_flight(key: String) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if locks.len() > 256 {
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
    locks.entry(key).or_default().clone()
}

fn json_body(body: String) -> Response {
    ([(CONTENT_TYPE, "application/json")], body).into_response()
}

/// The recorded game `id` if `headers` may read it (anything unreadable is
/// reported as missing, so Solo games leak nothing).
fn readable_game(app: &App, headers: &HeaderMap, id: &str) -> ApiResult<StoredGame> {
    let viewer = viewer(app, headers)?;
    app.store()
        .stored_game(id)?
        .filter(|g| g.readable_by(viewer.as_deref()))
        .ok_or_else(ApiError::not_found)
}

/// Like [`readable_game`], for the endpoints that replay it: games recorded
/// before replays existed answer `404 no_replay`.
fn replayable_game(app: &App, headers: &HeaderMap, id: &str) -> ApiResult<StoredGame> {
    let game = readable_game(app, headers, id)?;
    if game.replayable().is_none() {
        return Err(ApiError(StatusCode::NOT_FOUND, "no_replay"));
    }
    Ok(game)
}

fn replay_failed(id: &str, at: usize) -> ApiError {
    tracing::error!("game {id} cannot be replayed: action {at} was refused");
    ApiError::internal("replay_failed")
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| ApiError::internal("internal"))
}

#[derive(Deserialize)]
struct Page {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn my_games(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    page: Result<Query<Page>, QueryRejection>,
) -> ApiResult<Response> {
    let Query(page) = page.map_err(|_| ApiError::bad_request("bad_request"))?;
    let player =
        viewer(&app, &headers)?.ok_or(ApiError(StatusCode::UNAUTHORIZED, "unauthorized"))?;
    let limit = page.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let list = app
        .store()
        .games_of(&player, limit, page.offset.unwrap_or(0))?;
    Ok(Json(list).into_response())
}

async fn game(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let stored = replayable_game(&app, &headers, &id)?;
    let game_id = stored.id.clone();
    let built = blocking(move || {
        replay::view(&stored)
            .expect("a replayable game has a view")
            .map(|view| serde_json::to_string(&view).expect("replays serialize"))
    })
    .await?;
    match built {
        Ok(text) => Ok(json_body(text)),
        Err(at) => Err(replay_failed(&game_id, at)),
    }
}

#[derive(Deserialize)]
struct DepthQuery {
    depth: Option<u32>,
}

fn checked_depth(depth: Option<u32>) -> ApiResult<u32> {
    let depth = depth.unwrap_or(DEFAULT_DEPTH);
    if (1..=MAX_DEPTH).contains(&depth) {
        Ok(depth)
    } else {
        Err(ApiError::bad_request("invalid_depth"))
    }
}

async fn game_analysis(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<DepthQuery>, QueryRejection>,
) -> ApiResult<Response> {
    let Query(query) = query.map_err(|_| ApiError::bad_request("bad_request"))?;
    let depth = checked_depth(query.depth)?;
    let stored = replayable_game(&app, &headers, &id)?;
    if let Some(cached) = app.store().analysis_get(&stored.id, depth)? {
        return Ok(json_body(cached));
    }
    // The same analysis requested twice is computed once: the second request
    // waits for the first, then finds it in the cache.
    let same_analysis = in_flight(format!("{}:{depth}", stored.id));
    let _first = same_analysis.lock().await;
    if let Some(cached) = app.store().analysis_get(&stored.id, depth)? {
        return Ok(json_body(cached));
    }
    let _turn = SEARCHES
        .acquire()
        .await
        .map_err(|_| ApiError::internal("internal"))?;
    let game_id = stored.id.clone();
    let result = blocking(move || {
        let (loadouts, actions) = stored.replayable().expect("checked above");
        let replayed = replay::replay(loadouts, actions)?;
        let analysed = analysis::analyze(&replayed, depth, analysis::BUDGET);
        Ok(serde_json::to_string(&analysed).expect("analyses serialize"))
    })
    .await?;
    let text = result.map_err(|at: usize| replay_failed(&game_id, at))?;
    app.store().analysis_put(&game_id, depth, &text)?;
    Ok(json_body(text))
}

async fn explore(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let body = body.map_err(|_| ApiError(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"))?;
    let request: ExploreRequest =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("bad_request"))?;
    if request.line.len() > MAX_LINE {
        return Err(ApiError::bad_request("line_too_long"));
    }
    let depth = checked_depth(request.depth)?;
    let stored = replayable_game(&app, &headers, &id)?;
    let _turn = SEARCHES
        .acquire()
        .await
        .map_err(|_| ApiError::internal("internal"))?;
    let game_id = stored.id.clone();
    let result = blocking(move || {
        let (loadouts, actions) = stored.replayable().expect("checked above");
        replay::explore(loadouts, actions, &request, depth, EXPLORE_BUDGET)
    })
    .await?;
    let answer = result.map_err(|at| replay_failed(&game_id, at))?;
    Ok(Json(answer).into_response())
}
