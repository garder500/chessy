//! Account recovery: the recovery code given at registration (or generated
//! later by an existing account), `POST /api/auth/recover`, its lockout, and
//! the guarantee that a code is only ever kept as a hash.

mod common;

use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const PASSWORD: &str = "correct horse";
const NEW_PASSWORD: &str = "battery staple";

/// An app over a database file, so the tests can reach into the tables.
fn world() -> (Api, Arc<App>, Store, TempDb) {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), HubConfig::default());
    (Api::new(&app), app, store, db)
}

/// Registers `name` over REST and returns `(token, recovery_code)`.
async fn sign_up(api: &Api, name: &str) -> (String, String) {
    let (status, v) = api
        .post(
            "/api/auth/register",
            json!({"username": name, "password": PASSWORD}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    (
        v["token"].as_str().unwrap().to_string(),
        v["recovery_code"].as_str().unwrap().to_string(),
    )
}

async fn recover(api: &Api, name: &str, code: &str, new_password: &str) -> (StatusCode, Value) {
    api.post(
        "/api/auth/recover",
        json!({"username": name, "recovery_code": code, "new_password": new_password}),
    )
    .await
}

async fn login(api: &Api, name: &str, password: &str) -> StatusCode {
    api.post(
        "/api/auth/login",
        json!({"username": name, "password": password}),
    )
    .await
    .0
}

fn forget_the_code(db: &TempDb, name: &str) {
    let removed = db
        .raw()
        .execute(
            "DELETE FROM recovery_codes
             WHERE player_id = (SELECT id FROM players WHERE username_lower = ?1)",
            [name.to_ascii_lowercase()],
        )
        .unwrap();
    assert_eq!(removed, 1, "{name} had no code");
}

fn stored_hash(db: &TempDb, name: &str) -> String {
    db.raw()
        .query_row(
            "SELECT r.code_hash FROM recovery_codes r JOIN players p ON p.id = r.player_id
             WHERE p.username_lower = ?1",
            [name.to_ascii_lowercase()],
            |r| r.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn registering_returns_a_code_that_looks_unguessable() {
    let (api, _, _, _db) = world();
    let mut seen = std::collections::HashSet::new();
    for i in 0..5 {
        let (_, code) = sign_up(&api, &format!("user_{i}")).await;
        // 4 groups of 5 symbols from a 32-symbol alphabet: 100 bits.
        let groups: Vec<&str> = code.split('-').collect();
        assert_eq!(groups.len(), 4, "{code}");
        for group in &groups {
            assert_eq!(group.len(), 5, "{code}");
            assert!(
                group
                    .chars()
                    .all(|c| "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(c)),
                "{code}"
            );
        }
        assert!(seen.insert(code), "two accounts got the same code");
    }
}

#[tokio::test]
async fn a_guest_who_registers_gets_a_code_too() {
    let (api, app, _, _db) = world();
    let guest = guest(&app);
    let (status, v) = api
        .post(
            "/api/auth/register",
            json!({"username": "promoted", "password": PASSWORD, "guest_token": guest.token}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let code = v["recovery_code"].as_str().unwrap();
    let (status, v) = recover(&api, "promoted", code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["player"]["player_id"], guest.id);
}

#[tokio::test]
async fn the_code_resets_the_password_and_logs_everyone_out() {
    let (api, app, _, _db) = world();
    let (first, code) = sign_up(&api, "Alice").await;
    let second = api
        .post(
            "/api/auth/login",
            json!({"username": "alice", "password": PASSWORD}),
        )
        .await
        .1["token"]
        .as_str()
        .unwrap()
        .to_string();
    let bob_token = api.register("bob").await;
    let mut alice = Client::connect(&app, Some(first.clone()));
    let mut bob = Client::connect(&app, Some(bob_token));
    alice.clear();
    bob.clear();

    let (status, v) = recover(&api, "ALICE", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["player"]["username"], "Alice");
    let fresh = v["token"].as_str().unwrap();
    let new_code = v["recovery_code"].as_str().unwrap();
    assert_ne!(new_code, code, "the code is rotated");

    // Every old session is gone, and the open WebSocket was told.
    for token in [&first, &second] {
        assert_eq!(
            api.get("/api/me", Some(token)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(alice.next("error")["code"], "session_revoked");
    assert!(!app.is_connected(&alice.id));
    // Other accounts are untouched; the session handed back works.
    assert!(app.is_connected(&bob.id));
    assert_eq!(
        api.get("/api/me", Some(fresh)).await.0,
        StatusCode::OK,
        "the session returned by recover is valid"
    );
    // The new password works, the old one does not.
    assert_eq!(login(&api, "alice", NEW_PASSWORD).await, StatusCode::OK);
    assert_eq!(
        login(&api, "alice", PASSWORD).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn the_code_can_be_typed_in_lower_case_without_dashes() {
    let (api, _, _, _db) = world();
    let (_, code) = sign_up(&api, "carol").await;
    let typed = code.replace('-', "").to_ascii_lowercase();
    let (status, v) = recover(&api, "carol", &format!("  {typed} "), NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn a_code_works_once() {
    let (api, _, _, db) = world();
    let (_, code) = sign_up(&api, "dave").await;
    let before = stored_hash(&db, "dave");
    let (status, v) = recover(&api, "dave", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_ne!(stored_hash(&db, "dave"), before);
    let new_code = v["recovery_code"].as_str().unwrap().to_string();

    let (status, v) = recover(&api, "dave", &code, "another password").await;
    assert_eq!((status, v["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("bad_recovery")));
    assert_eq!(login(&api, "dave", NEW_PASSWORD).await, StatusCode::OK);
    // The code that came with the reset is the live one.
    let (status, _) = recover(&api, "dave", &new_code, "yet another one").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_losing_redeemer_changes_nothing() {
    // Two requests that both verified the same code: only the first gets
    // through the compare-and-swap, the second leaves no trace.
    let (api, _, store, _db) = world();
    let (token, _) = sign_up(&api, "eve").await;
    let (player, old) = store.recovery_credentials("eve").unwrap().unwrap();
    let old = old.unwrap();
    let revoked = store
        .recover_account(&player, &old, "new-code-hash", "first-password-hash")
        .unwrap();
    assert_eq!(revoked, Some(vec![token]));
    let again = store
        .recover_account(&player, &old, "other-code-hash", "second-password-hash")
        .unwrap();
    assert_eq!(again, None);
    let (_, hash) = store.credentials("eve").unwrap().unwrap();
    assert_eq!(hash, "first-password-hash");
    let (_, code_hash) = store.recovery_credentials("eve").unwrap().unwrap();
    assert_eq!(code_hash.as_deref(), Some("new-code-hash"));
}

#[tokio::test]
async fn unknown_users_and_wrong_codes_get_the_same_answer() {
    let (api, _, _, db) = world();
    let (_, code) = sign_up(&api, "frank").await;
    sign_up(&api, "grace").await;
    forget_the_code(&db, "grace");
    let mut wrong = code.clone();
    wrong.replace_range(0..1, if code.starts_with('A') { "B" } else { "A" });

    let wrong_code = recover(&api, "frank", &wrong, NEW_PASSWORD).await;
    let unknown_user = recover(&api, "nobody_here", &code, NEW_PASSWORD).await;
    let no_code_yet = recover(&api, "grace", &code, NEW_PASSWORD).await;
    assert_eq!(wrong_code.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_code.1, json!({"error": "bad_recovery"}));
    assert_eq!(unknown_user, wrong_code);
    assert_eq!(no_code_yet, wrong_code);
    // Garbage in the code field is no different.
    assert_eq!(recover(&api, "frank", "", NEW_PASSWORD).await, wrong_code);
    assert_eq!(
        recover(&api, "frank", &"Z".repeat(2000), NEW_PASSWORD).await,
        wrong_code
    );
    // Nothing changed for frank.
    assert_eq!(login(&api, "frank", PASSWORD).await, StatusCode::OK);
}

#[tokio::test]
async fn a_weak_new_password_is_refused_and_the_code_is_kept() {
    let (api, _, _, _db) = world();
    let (_, code) = sign_up(&api, "heidi").await;
    for weak in ["short", "1234567", &"x".repeat(129)] {
        let (status, v) = recover(&api, "heidi", &code, weak).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(v["error"], "weak_password");
    }
    // The answer is the same whether or not the account exists.
    let (status, v) = recover(&api, "nobody_here", &code, "short").await;
    assert_eq!((status, v["error"].as_str()), (StatusCode::BAD_REQUEST, Some("weak_password")));
    // Refusals neither used the code up nor counted as failed guesses.
    assert_eq!(login(&api, "heidi", PASSWORD).await, StatusCode::OK);
    let (status, v) = recover(&api, "heidi", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn guessing_codes_locks_the_username_for_a_while() {
    let (api, _, _, _db) = world();
    let (_, code) = sign_up(&api, "ivan").await;
    let (_, judy_code) = sign_up(&api, "judy").await;
    for _ in 0..5 {
        let (status, _) = recover(&api, "ivan", "AAAAA-AAAAA-AAAAA-AAAAA", NEW_PASSWORD).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    // Even the right code is refused once locked out...
    let (status, v) = recover(&api, "IVAN", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["error"], "too_many_attempts");
    assert_eq!(login(&api, "ivan", PASSWORD).await, StatusCode::OK);
    // ...and names that do not exist are counted (and locked) the same way.
    for _ in 0..5 {
        recover(&api, "ghost", &code, NEW_PASSWORD).await;
    }
    let (status, _) = recover(&api, "ghost", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // Other accounts are unaffected.
    let (status, _) = recover(&api, "judy", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "not locked, just a wrong code");
    let (status, _) = recover(&api, "judy", &judy_code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_locked_login_does_not_lock_recovery_and_recovery_unlocks_login() {
    let (api, _, _, _db) = world();
    let (_, code) = sign_up(&api, "karl").await;
    for _ in 0..8 {
        login(&api, "karl", "not the password").await;
    }
    assert_eq!(
        login(&api, "karl", PASSWORD).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    let (status, v) = recover(&api, "karl", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(login(&api, "karl", NEW_PASSWORD).await, StatusCode::OK);
}

#[tokio::test]
async fn an_existing_account_can_get_a_code_with_its_password() {
    let (api, _, _, db) = world();
    let (token, _) = sign_up(&api, "laura").await;
    // As if the account predated recovery codes.
    forget_the_code(&db, "laura");
    let (status, _) = recover(&api, "laura", "AAAAA-AAAAA-AAAAA-AAAAA", NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let url = "/api/me/recovery-code";
    // A session token alone is not enough.
    let (status, v) = api.call("POST", url, None, Some(json!({"password": PASSWORD}))).await;
    assert_eq!((status, v["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("unauthorized")));
    let (status, v) = api
        .call("POST", url, Some(&token), Some(json!({"password": "wrong wrong"})))
        .await;
    assert_eq!((status, v["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("bad_credentials")));
    let (status, v) = api.call("POST", url, Some(&token), Some(json!({}))).await;
    assert_eq!((status, v["error"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_request")));

    let (status, v) = api
        .call("POST", url, Some(&token), Some(json!({"password": PASSWORD})))
        .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let code = v["recovery_code"].as_str().unwrap().to_string();
    assert!(stored_hash(&db, "laura").starts_with("$argon2id$"));

    // Asking again rotates it: the first code stops working.
    let (_, v) = api
        .call("POST", url, Some(&token), Some(json!({"password": PASSWORD})))
        .await;
    let newer = v["recovery_code"].as_str().unwrap().to_string();
    assert_ne!(newer, code);
    let (status, _) = recover(&api, "laura", &code, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, v) = recover(&api, "laura", &newer, NEW_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

/// `recover` as seen from client address `ip` (through `X-Forwarded-For`, which
/// the server only reads with `trust_proxy`).
async fn recover_from(api: &Api, ip: &str, name: &str, code: &str) -> (StatusCode, Value) {
    let body = json!({"username": name, "recovery_code": code, "new_password": NEW_PASSWORD});
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/recover")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = api.router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn failures_from_one_address_lock_that_address_when_the_limit_is_set() {
    let store = Store::open(":memory:").unwrap();
    let app = App::new(
        store,
        HubConfig {
            trust_proxy: true,
            recovery_max_failures_per_ip: 4,
            ..HubConfig::default()
        },
    );
    let api = Api::new(&app);
    let (_, code) = sign_up(&api, "olga").await;
    let attacker = "203.0.113.7";
    // Four wrong guesses at four different names: no username is near its own limit.
    for i in 0..4 {
        let (status, _) = recover_from(&api, attacker, &format!("target_{i}"), &code).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, v) = recover_from(&api, attacker, "olga", &code).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["error"], "too_many_attempts");
    // Another address is not affected.
    let (status, v) = recover_from(&api, "198.51.100.9", "olga", &code).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn by_default_no_address_is_ever_locked_out() {
    // Behind a proxy every client may share one address: even with the address
    // known (trust_proxy), the per-address count is off unless asked for.
    let store = Store::open(":memory:").unwrap();
    let app = App::new(
        store,
        HubConfig {
            trust_proxy: true,
            ..HubConfig::default()
        },
    );
    let api = Api::new(&app);
    let (_, code) = sign_up(&api, "peggy").await;
    for i in 0..30 {
        let (status, _) = recover_from(&api, "203.0.113.7", &format!("target_{i}"), &code).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, v) = recover_from(&api, "203.0.113.7", "peggy", &code).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn guessing_the_password_through_the_code_route_is_throttled() {
    let (api, app, _, _db) = world();
    let (token, _) = sign_up(&api, "mallory").await;
    let url = "/api/me/recovery-code";
    for _ in 0..8 {
        let (status, _) = api
            .call("POST", url, Some(&token), Some(json!({"password": "guess guess"})))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, v) = api
        .call("POST", url, Some(&token), Some(json!({"password": PASSWORD})))
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["error"], "too_many_attempts");
    // The same counter as login: the lockout is shared.
    assert_eq!(
        login(&api, "mallory", PASSWORD).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // A guest has no password to give.
    let guest = guest(&app);
    let (status, _) = api
        .call("POST", url, Some(&guest.token), Some(json!({"password": PASSWORD})))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Everything logged by the process, for the test below.
fn captured_logs() -> &'static Mutex<Vec<u8>> {
    static LOGS: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        struct Sink;
        impl io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                captured_logs().lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        // Global, because the password hashing runs on other threads.
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(|| Sink)
            .try_init();
        Mutex::new(Vec::new())
    })
}

#[tokio::test]
async fn a_code_is_only_kept_as_a_hash_and_never_logged() {
    let logs = captured_logs();
    // The capture itself works (otherwise "nothing was logged" proves nothing).
    tracing::info!("recovery log canary");
    assert!(String::from_utf8_lossy(&logs.lock().unwrap()).contains("recovery log canary"));
    let (api, _, _, db) = world();
    let (token, code) = sign_up(&api, "nina").await;
    let (_, v) = api
        .call(
            "POST",
            "/api/me/recovery-code",
            Some(&token),
            Some(json!({"password": PASSWORD})),
        )
        .await;
    let rotated = v["recovery_code"].as_str().unwrap().to_string();
    // A failed attempt, whose code and password go through the error paths.
    recover(&api, "nina", "AAAAA-AAAAA-AAAAA-AAAAA", "some new password").await;
    let (_, v) = recover(&api, "nina", &rotated, NEW_PASSWORD).await;
    let latest = v["recovery_code"].as_str().unwrap().to_string();

    let hash = stored_hash(&db, "nina");
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    for secret in [&code, &rotated, &latest] {
        let plain = secret.replace('-', "");
        assert!(!hash.contains(&plain));
        // Not in the database file, whatever the table, nor in the logs.
        let file = std::fs::read(&db.path).unwrap();
        // The write-ahead log, if any, is next to it.
        let wal = std::fs::read(format!("{}-wal", db.path_str())).unwrap_or_default();
        for bytes in [file, wal] {
            for needle in [secret.as_str(), plain.as_str()] {
                assert!(
                    !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                    "{needle} is in the database"
                );
            }
        }
        let logged = logs.lock().unwrap().clone();
        for needle in [secret.as_str(), plain.as_str()] {
            assert!(
                !logged.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} was logged"
            );
        }
    }
    let logged = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(!logged.contains(NEW_PASSWORD) && !logged.contains("some new password"));
}
