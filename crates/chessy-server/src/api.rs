//! The REST API under `/api`: accounts, sessions, leaderboard and profiles.
//! Errors are `{"error": "<code>"}` with a matching status.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use rand::seq::IndexedRandom;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;

use crate::app::App;
use crate::store::{RegisterError, StoreError};
use crate::ws::client_ip;

const MAX_BODY_BYTES: usize = 4 * 1024;
const MAX_LEADERBOARD_PAGE: u32 = 100;
const DEFAULT_LEADERBOARD_PAGE: u32 = 50;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
        .route("/auth/recover", post(recover))
        .route("/me", get(me))
        .route("/me/recovery-code", post(rotate_recovery_code))
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

/// 32 symbols, so a symbol is exactly 5 bits: no `0 O 1 I`, which get mixed up
/// when read off a screen or a sheet of paper.
const RECOVERY_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const RECOVERY_GROUPS: usize = 4;
const RECOVERY_GROUP_LEN: usize = 5;

/// A recovery code: 20 random symbols (100 bits) in 4 groups of 5,
/// e.g. `K7QF2-M9XWB-3HNRA-TD8LC`. `rand::rng()` is a CSPRNG.
fn generate_recovery_code() -> String {
    let mut rng = rand::rng();
    let mut code = String::new();
    for group in 0..RECOVERY_GROUPS {
        if group > 0 {
            code.push('-');
        }
        for _ in 0..RECOVERY_GROUP_LEN {
            code.push(char::from(*RECOVERY_ALPHABET.choose(&mut rng).unwrap()));
        }
    }
    code
}

/// What is hashed and verified: the code without dashes or spaces, in upper
/// case, so that it can be typed in any way it was written down.
fn normalize_recovery_code(code: &str) -> String {
    code.chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
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
    // The recovery code is shown once, in this response; only its hash is kept.
    let recovery_code = generate_recovery_code();
    let (password, code) = (req.password, normalize_recovery_code(&recovery_code));
    let (hash, code_hash) =
        blocking(move || Ok::<_, ApiError>((hash_password(password)?, hash_password(code)?)))
            .await??;
    // An expired guest session is not promoted (and a live one is refreshed).
    let config = app.config();
    let guest_token = match req.guest_token.as_deref() {
        Some(t) => app
            .store()
            .session_player(t, config.session_ttl, config.session_touch_interval)?
            .map(|_| t),
        None => None,
    };
    let (player, token) = match app.store().register_with_recovery(
        &req.username,
        &hash,
        guest_token,
        Some(&code_hash),
    ) {
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
    Ok(Json(json!({
        "token": token,
        "player": me,
        "recovery_code": recovery_code,
    }))
    .into_response())
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

// No `Debug` on the requests that carry a recovery code or a password: they
// must not end up in a log by accident.
#[derive(Deserialize)]
struct RecoverRequest {
    username: String,
    recovery_code: String,
    new_password: String,
}

/// Sets a new password from the recovery code. Every failure (unknown name,
/// guest, account without a code, wrong code) gives the same `401
/// bad_recovery` after one argon2 verification, so neither the body nor the
/// timing says whether the account exists. On success all sessions of the
/// account are revoked, the code is replaced, and a fresh session is returned
/// together with the new code (shown once): `{token, player, recovery_code}`.
async fn recover(
    State(app): State<Arc<App>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let req: RecoverRequest = parse_body(body)?;
    let peer = peer.map(|Extension(ConnectInfo(addr))| addr.ip());
    let ip = client_ip(&headers, peer, app.config().trust_proxy);
    let bad = || ApiError(StatusCode::UNAUTHORIZED, "bad_recovery");
    // Checked before anything is hashed. Not the login lockout: someone who
    // forgot their password has probably locked their login already.
    if app.recovery_blocked(&req.username, ip) {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"));
    }
    // Before the code is looked at, so the answer does not depend on it and a
    // refused password never costs an attempt (or the code).
    if !valid_password(&req.new_password) {
        return Err(ApiError::bad_request("weak_password"));
    }
    let (found, stored) = match app.store().recovery_credentials(&req.username)? {
        Some((player, Some(hash))) => (Some(player), Some(hash)),
        _ => (None, None),
    };
    let verify_against = stored.clone().unwrap_or_else(|| dummy_hash().to_string());
    let code = normalize_recovery_code(&req.recovery_code);
    let ok = blocking(move || verify_password(&code, &verify_against)).await?;
    let (Some(player), Some(old_hash), true) = (found, stored, ok) else {
        app.recovery_failed(&req.username, ip);
        return Err(bad());
    };

    let new_code = generate_recovery_code();
    let (new_password, normalized) = (req.new_password, normalize_recovery_code(&new_code));
    let (password_hash, code_hash) = blocking(move || {
        Ok::<_, ApiError>((hash_password(new_password)?, hash_password(normalized)?))
    })
    .await??;
    // One transaction; `None` when a concurrent request redeemed the code first.
    let redeemed = app
        .store()
        .recover_account(&player, &old_hash, &code_hash, &password_hash)?;
    let Some(revoked) = redeemed else {
        app.recovery_failed(&req.username, ip);
        return Err(bad());
    };
    // Open WebSockets that logged in with one of these sessions must not outlive them.
    for token in &revoked {
        app.session_revoked(token);
    }
    app.recovery_succeeded(&req.username);
    app.login_succeeded(&req.username);
    let token = app.store().create_session(&player)?;
    let me = app.store().me(&player)?.ok_or_else(bad)?;
    Ok(Json(json!({
        "token": token,
        "player": me,
        "recovery_code": new_code,
    }))
    .into_response())
}

#[derive(Deserialize)]
struct RotateRecoveryRequest {
    password: String,
}

/// Generates (or replaces) the recovery code of the caller's account and
/// returns it once: `{recovery_code}`. This is how accounts created before
/// recovery existed get one. It takes the current password, and wrong
/// passwords count towards the login lockout of the account, so a stolen
/// session token is not a way to guess the password.
async fn rotate_recovery_code(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let player = authenticate(&app, &headers)?;
    let req: RotateRecoveryRequest = parse_body(body)?;
    let bad = || ApiError(StatusCode::UNAUTHORIZED, "bad_credentials");
    // A guest has no password and no username: nothing to recover.
    let username = app
        .store()
        .player_row(&player)?
        .and_then(|row| row.username)
        .ok_or_else(bad)?;
    if app.login_blocked(&username) {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"));
    }
    if req.password.len() > 1024 {
        return Err(bad());
    }
    let hash = app.store().password_hash_of(&player)?.ok_or_else(bad)?;
    let password = req.password;
    if !blocking(move || verify_password(&password, &hash)).await? {
        app.login_failed(&username);
        return Err(bad());
    }
    app.login_succeeded(&username);
    let code = generate_recovery_code();
    let normalized = normalize_recovery_code(&code);
    let code_hash = blocking(move || hash_password(normalized)).await??;
    app.store().set_recovery_hash(&player, &code_hash)?;
    Ok(Json(json!({ "recovery_code": code })).into_response())
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
