//! Sessions: inactivity expiry, throttled refresh, the purge task,
//! `logout-all` and the migration that backfills `last_used`.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use chessy_server::{spawn_session_purge, App};
use common::*;
use serde_json::json;

const DAY: Duration = Duration::from_secs(24 * 3600);
const ISO: &str = "%Y-%m-%dT%H:%M:%SZ";

/// An app over a database file, so the tests can reach into `sessions`.
fn world() -> (Api, Arc<App>, Store, TempDb) {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), HubConfig::default());
    (Api::new(&app), app, store, db)
}

/// Sets a session's `last_used` to `modifier` relative to now (e.g. `-31 days`).
fn set_last_used(db: &TempDb, token: &str, modifier: &str) {
    let changed = db
        .raw()
        .execute(
            &format!(
                "UPDATE sessions SET last_used = strftime('{ISO}', 'now', ?2) WHERE token = ?1"
            ),
            rusqlite::params![token, modifier],
        )
        .unwrap();
    assert_eq!(changed, 1, "unknown session");
}

/// How many seconds ago the session was last used.
fn idle_seconds(db: &TempDb, token: &str) -> i64 {
    db.raw()
        .query_row(
            "SELECT CAST(strftime('%s', 'now') AS INTEGER)
                    - CAST(strftime('%s', last_used) AS INTEGER)
             FROM sessions WHERE token = ?1",
            [token],
            |r| r.get(0),
        )
        .unwrap()
}

fn session_exists(db: &TempDb, token: &str) -> bool {
    db.raw()
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE token = ?1",
            [token],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0
}

#[tokio::test]
async fn an_expired_token_is_rejected_by_rest_and_websocket() {
    let (api, app, _, db) = world();
    let token = api.register("alice").await;
    assert_eq!(api.get("/api/me", Some(&token)).await.0, StatusCode::OK);
    let account = Client::connect(&app, Some(token.clone())).id;

    set_last_used(&db, &token, "-31 days");
    let (status, v) = api.get("/api/me", Some(&token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(v["error"], "unauthorized");
    assert_eq!(
        api.get("/api/me/games", Some(&token)).await.0,
        StatusCode::UNAUTHORIZED
    );
    // Where the token is optional it counts as no token: anonymous, not a 401.
    assert_eq!(api.get("/api/live", Some(&token)).await.0, StatusCode::OK);
    // The WebSocket hello falls back to a new guest, as for any unknown token.
    let again = Client::connect(&app, Some(token.clone()));
    assert_ne!(again.id, account);
    assert_ne!(again.token, token);
    assert_eq!(again.welcome["account"]["guest"], true);
}

#[tokio::test]
async fn an_expired_guest_session_is_not_promoted_by_register() {
    let (api, app, _, db) = world();
    let guest = Client::connect(&app, None);
    set_last_used(&db, &guest.token, "-31 days");

    let (status, v) = api
        .post(
            "/api/auth/register",
            json!({"username": "carol", "password": "correct horse", "guest_token": guest.token}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_ne!(v["player"]["player_id"], guest.id.as_str());
    let token = v["token"].as_str().unwrap();
    assert_ne!(token, guest.token);
    assert_eq!(api.get("/api/me", Some(token)).await.0, StatusCode::OK);

    // A live guest token still is promoted.
    let live = Client::connect(&app, None);
    let (status, v) = api
        .post(
            "/api/auth/register",
            json!({"username": "dora", "password": "correct horse", "guest_token": live.token}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["player"]["player_id"], live.id.as_str());
    assert_eq!(v["token"], live.token.as_str());
}

#[tokio::test]
async fn a_session_just_inside_the_ttl_still_works() {
    let (api, _, _, db) = world();
    let token = api.register("alice").await;
    set_last_used(&db, &token, "-29 days");
    assert_eq!(api.get("/api/me", Some(&token)).await.0, StatusCode::OK);
}

#[tokio::test]
async fn activity_extends_the_life_of_a_session_but_writes_only_now_and_then() {
    let (api, _, store, db) = world();
    let token = api.register("alice").await;

    // Used 10 minutes ago: well inside the touch interval, so no write.
    set_last_used(&db, &token, "-10 minutes");
    assert_eq!(api.get("/api/me", Some(&token)).await.0, StatusCode::OK);
    let idle = idle_seconds(&db, &token);
    assert!(
        (600..700).contains(&idle),
        "refresh was not throttled: {idle}s"
    );

    // Used 29 days ago: refreshed to now.
    set_last_used(&db, &token, "-29 days");
    assert_eq!(api.get("/api/me", Some(&token)).await.0, StatusCode::OK);
    assert!(idle_seconds(&db, &token) < 10);

    // The refresh is what keeps it alive: the same age without it is expired
    // under a shorter TTL, and a successful lookup resets the age.
    let touch = Duration::from_secs(3600);
    set_last_used(&db, &token, "-29 days");
    assert!(store
        .session_player(&token, 28 * DAY, touch)
        .unwrap()
        .is_none());
    assert!(store
        .session_player(&token, 30 * DAY, touch)
        .unwrap()
        .is_some());
    assert!(store.session_player(&token, DAY, touch).unwrap().is_some());
}

#[tokio::test]
async fn the_ttl_is_configurable() {
    let (api, app, _, db) = world();
    let token = api.register("alice").await;
    set_last_used(&db, &token, "-2 hours");

    let touch = app.config().session_touch_interval;
    let store = app.store();
    assert!(store
        .session_player(&token, Duration::from_secs(3600), touch)
        .unwrap()
        .is_none());
    assert!(store
        .session_player(&token, 3 * DAY, touch)
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn the_purge_removes_only_expired_sessions() {
    let (api, _, store, db) = world();
    let old = api.register("old_timer").await;
    let fresh = api.register("fresh").await;
    let quiet = api.register("quiet").await;
    set_last_used(&db, &old, "-31 days");
    set_last_used(&db, &quiet, "-29 days");

    assert_eq!(store.purge_expired_sessions(30 * DAY).unwrap(), 1);
    assert!(!session_exists(&db, &old));
    assert!(session_exists(&db, &fresh));
    assert!(session_exists(&db, &quiet));
    // Nothing left to purge.
    assert_eq!(store.purge_expired_sessions(30 * DAY).unwrap(), 0);
}

#[tokio::test]
async fn the_purge_task_runs_on_its_interval() {
    let (api, _, store, db) = world();
    let old = api.register("old_timer").await;
    let fresh = api.register("fresh").await;
    set_last_used(&db, &old, "-31 days");

    let task = spawn_session_purge(store, Duration::from_millis(20), 30 * DAY);
    for _ in 0..100 {
        if !session_exists(&db, &old) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    task.abort();
    assert!(!session_exists(&db, &old), "the task never purged");
    assert!(session_exists(&db, &fresh));
}

#[tokio::test]
async fn logout_all_ends_every_session_and_closes_the_live_socket() {
    let (api, app, _, db) = world();
    let first = api.register("alice").await;
    let login = |name: &'static str| {
        let api = &api;
        async move {
            let (status, v) = api
                .post(
                    "/api/auth/login",
                    json!({"username": name, "password": "correct horse"}),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{v}");
            v["token"].as_str().unwrap().to_string()
        }
    };
    let second = login("alice").await;
    let third = login("alice").await;
    let other = api.register("bob").await;
    let mut alice = Client::connect(&app, Some(first.clone()));
    let mut bob = Client::connect(&app, Some(other.clone()));
    alice.clear();
    bob.clear();

    // Called from another device (another session of the same account).
    let (status, _) = api
        .call("POST", "/api/auth/logout-all", Some(&second), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(alice.next("error")["code"], "session_revoked");
    assert!(!app.is_connected(&alice.id));
    for token in [&first, &second, &third] {
        assert!(!session_exists(&db, token));
        assert_eq!(
            api.get("/api/me", Some(token)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    // Another account is untouched.
    assert!(bob.try_next("error").is_none());
    assert!(app.is_connected(&bob.id));
    assert_eq!(api.get("/api/me", Some(&other)).await.0, StatusCode::OK);
    // The account itself survives: logging in again works.
    assert_eq!(login("alice").await.len(), 32);

    // Without a valid token, nothing happens.
    let (status, _) = api.call("POST", "/api/auth/logout-all", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = api
        .call("POST", "/api/auth/logout-all", Some(&second), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// The schema before accounts existed (tokens on `players`), with an account
/// created long ago: the migrations run over it, the last one included.
const LEGACY_SCHEMA: &str = "
    CREATE TABLE players (id TEXT PRIMARY KEY, token TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
    INSERT INTO players (id, token, created_at)
        VALUES ('aaaaaaaa', 'old-token-a', '2020-01-01 00:00:00');
";

#[tokio::test]
async fn migrating_an_existing_database_backfills_last_used() {
    let db = TempDb::new();
    db.raw().execute_batch(LEGACY_SCHEMA).unwrap();

    let store = Store::open(db.path_str()).unwrap();
    // The session is years old by `created_at`, but counts as used at migration time.
    assert!(idle_seconds(&db, "old-token-a") < 10);
    assert_eq!(
        store
            .session_player("old-token-a", 30 * DAY, Duration::from_secs(3600))
            .unwrap()
            .as_deref(),
        Some("aaaaaaaa")
    );
    assert_eq!(store.purge_expired_sessions(30 * DAY).unwrap(), 0);

    // Reopening changes nothing (migrations are idempotent), and new sessions are stamped.
    drop(store);
    let store = Store::open(db.path_str()).unwrap();
    let (_, token) = store.create_player().unwrap();
    assert!(idle_seconds(&db, &token) < 10);
    assert!(idle_seconds(&db, "old-token-a") < 10);
    let unstamped: i64 = db
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE last_used = ''",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unstamped, 0);
}
