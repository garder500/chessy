//! The REST API under `/api`: accounts, sessions, leaderboard and profiles.
//! Errors are `{"error": "<code>"}` with a matching status.

use std::sync::{Arc, OnceLock};

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;

use crate::app::App;
use crate::store::{RegisterError, StoreError};

const MAX_BODY_BYTES: usize = 4 * 1024;
const MAX_LEADERBOARD_PAGE: u32 = 100;
const DEFAULT_LEADERBOARD_PAGE: u32 = 50;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
        .route("/me", get(me))
        .route("/me/skills", get(my_skills))
        .route("/leaderboard", get(leaderboard))
        .route("/players/{username}", get(profile))
        .route("/live", get(crate::api_live::live))
        .route("/skills/forged", get(crate::api_skills::forged))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

struct ApiError(StatusCode, &'static str);

impl ApiError {
    fn bad_request(code: &'static str) -> Self {
        ApiError(StatusCode::BAD_REQUEST, code)
    }
    fn unauthorized() -> Self {
        ApiError(StatusCode::UNAUTHORIZED, "unauthorized")
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        tracing::error!("store error: {e}");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

/// Parses a JSON body ourselves so that every failure is a JSON error.
fn parse_body<T: DeserializeOwned>(body: Result<Bytes, BytesRejection>) -> ApiResult<T> {
    let body = body.map_err(|_| ApiError(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"))?;
    serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("bad_request"))
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn authenticate(app: &App, headers: &HeaderMap) -> ApiResult<String> {
    let token = bearer(headers).ok_or_else(ApiError::unauthorized)?;
    let config = app.config();
    app.store()
        .session_player(token, config.session_ttl, config.session_touch_interval)?
        .ok_or_else(ApiError::unauthorized)
}

/// `{entries: [HistoryEntry], deck: [skill_id]}`: what the player got, forged and
/// lost (newest first), and what they own now.
async fn my_skills(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<Response> {
    let player = authenticate(&app, &headers)?;
    let entries = app.store().skill_history(&player)?;
    let deck = app.store().deck(&player)?;
    Ok(Json(json!({ "entries": entries, "deck": deck })).into_response())
}

pub fn valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn valid_password(password: &str) -> bool {
    (8..=128).contains(&password.chars().count())
}

fn hash_password(password: String) -> ApiResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| {
            tracing::error!("password hashing failed: {e}");
            ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal")
        })
}

/// A real hash to verify against when the username is unknown, so that
/// unknown names and wrong passwords take the same time.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        Argon2::default()
            .hash_password(b"not a real password")
            .map(|h| h.to_string())
            .unwrap_or_default()
    })
}

fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

/// Password hashing is CPU-heavy by design; keep it off the async workers.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal"))
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    guest_token: Option<String>,
}

async fn register(
    State(app): State<Arc<App>>,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let req: RegisterRequest = parse_body(body)?;
    if !valid_username(&req.username) {
        return Err(ApiError::bad_request("invalid_username"));
    }
    if !valid_password(&req.password) {
        return Err(ApiError::bad_request("weak_password"));
    }
    let hash = blocking(move || hash_password(req.password)).await??;
    // An expired guest session is not promoted (and a live one is refreshed).
    let config = app.config();
    let guest_token = match req.guest_token.as_deref() {
        Some(t) => app
            .store()
            .session_player(t, config.session_ttl, config.session_touch_interval)?
            .map(|_| t),
        None => None,
    };
    let (player, token) =
        match app.store().register(&req.username, &hash, guest_token) {
            Ok(found) => found,
            Err(RegisterError::UsernameTaken) => {
                return Err(ApiError(StatusCode::CONFLICT, "username_taken"))
            }
            Err(RegisterError::Db) => {
                return Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal"))
            }
        };
    let me = app
        .store()
        .me(&player)?
        .ok_or_else(ApiError::unauthorized)?;
    app.account_changed(&player);
    Ok(Json(json!({ "token": token, "player": me })).into_response())
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(
    State(app): State<Arc<App>>,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let req: LoginRequest = parse_body(body)?;
    let bad = || ApiError(StatusCode::UNAUTHORIZED, "bad_credentials");
    if app.login_blocked(&req.username) {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"));
    }
    if req.password.len() > 1024 {
        return Err(bad());
    }
    let found = app.store().credentials(&req.username)?;
    let hash = found
        .as_ref()
        .map_or_else(|| dummy_hash().to_string(), |(_, h)| h.clone());
    let password = req.password;
    let ok = blocking(move || verify_password(&password, &hash)).await?;
    let Some((player, _)) = found.filter(|_| ok) else {
        app.login_failed(&req.username);
        return Err(bad());
    };
    app.login_succeeded(&req.username);
    let token = app.store().create_session(&player)?;
    let me = app.store().me(&player)?.ok_or_else(bad)?;
    Ok(Json(json!({ "token": token, "player": me })).into_response())
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<StatusCode> {
    authenticate(&app, &headers)?;
    let token = bearer(&headers).ok_or_else(ApiError::unauthorized)?;
    app.store().delete_session(token)?;
    // An open WebSocket that logged in with this session must not outlive it.
    app.session_revoked(token);
    Ok(StatusCode::NO_CONTENT)
}

/// Ends every session of the caller's account, on every device.
async fn logout_all(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<StatusCode> {
    let player = authenticate(&app, &headers)?;
    let tokens = app.store().delete_sessions_of(&player)?;
    // Open WebSockets that logged in with one of these sessions must not outlive them.
    for token in &tokens {
        app.session_revoked(token);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<Response> {
    let player = authenticate(&app, &headers)?;
    let me = app
        .store()
        .me(&player)?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(Json(me).into_response())
}

#[derive(Deserialize)]
struct Page {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn leaderboard(
    State(app): State<Arc<App>>,
    page: Result<Query<Page>, QueryRejection>,
) -> ApiResult<Response> {
    let Query(page) = page.map_err(|_| ApiError::bad_request("bad_request"))?;
    let limit = page
        .limit
        .unwrap_or(DEFAULT_LEADERBOARD_PAGE)
        .clamp(1, MAX_LEADERBOARD_PAGE);
    let board = app.store().leaderboard(limit, page.offset.unwrap_or(0))?;
    Ok(Json(board).into_response())
}

async fn profile(State(app): State<Arc<App>>, Path(username): Path<String>) -> ApiResult<Response> {
    let not_found = || ApiError(StatusCode::NOT_FOUND, "not_found");
    if !valid_username(&username) {
        return Err(not_found());
    }
    let profile = app
        .store()
        .public_profile(&username)?
        .ok_or_else(not_found)?;
    Ok(Json(profile).into_response())
}
