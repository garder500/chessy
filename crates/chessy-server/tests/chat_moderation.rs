//! Chat moderation: block lists, the chat mute, reports (docs/spec-v2.md §4).

mod common;

use std::sync::Arc;
use std::time::Duration;

use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

fn config() -> HubConfig {
    HubConfig {
        chat_interval: Duration::ZERO,
        spectator_delay: Duration::ZERO,
        ..HubConfig::default()
    }
}

fn setup() -> (Arc<App>, Store) {
    new_app(config())
}

/// Like `setup`, with a database file the test can read.
fn world(config: HubConfig) -> (Arc<App>, Store, TempDb) {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    (App::new(store.clone(), config), store, db)
}

/// Two connected accounts in a friendly game (opening messages thrown away).
fn game(app: &Arc<App>, store: &Store, a: &str, b: &str) -> (Client, Client) {
    let a = account(app, store, a);
    let b = account(app, store, b);
    for c in [&a, &b] {
        c.send(ClientMsg::QueueJoin {
            ranked: Some(false),
            time: None,
        });
    }
    matched(a, b)
}

fn befriend(a: &mut Client, b: &mut Client, a_name: &str, b_name: &str) {
    a.say(json!({"type": "friend_request", "username": b_name}));
    b.say(json!({"type": "friend_respond", "username": a_name, "accept": true}));
    a.clear();
    b.clear();
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|f| f["username"].as_str().unwrap().to_string())
        .collect()
}

fn report_rows(db: &TempDb) -> Vec<(String, String, String, Option<String>, Option<String>)> {
    let conn = db.raw();
    let mut stmt = conn
        .prepare("SELECT reporter, target, reason, game_id, context FROM reports ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get(0).unwrap(),
                r.get(1).unwrap(),
                r.get(2).unwrap(),
                r.get(3).unwrap(),
                r.get(4).unwrap(),
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    rows
}

// ---- who may use it ----------------------------------------------------------

#[tokio::test]
async fn guests_are_refused() {
    let (app, _) = setup();
    let mut g = guest(&app);
    g.clear();
    for msg in [
        json!({"type": "block_user", "username": "someone"}),
        json!({"type": "unblock_user", "username": "someone"}),
        json!({"type": "blocks_list"}),
        json!({"type": "set_chat_muted", "muted": true}),
        json!({"type": "report_user", "username": "someone", "reason": "spam"}),
    ] {
        g.say(msg.clone());
        assert_eq!(g.error_code(), "account_required", "{msg}");
    }
    assert!(g.types().is_empty());
}

#[tokio::test]
async fn nobody_blocks_or_reports_themselves() {
    let (app, store, db) = world(config());
    let mut a = account(&app, &store, "alice");
    a.clear();
    a.say(json!({"type": "block_user", "username": "ALICE"}));
    assert_eq!(a.error_code(), "invalid_target");
    a.say(json!({"type": "report_user", "username": "alice", "reason": "spam"}));
    assert_eq!(a.error_code(), "invalid_target");
    a.say(json!({"type": "blocks_list"}));
    assert_eq!(a.next("blocks")["blocked"], json!([]));
    assert!(report_rows(&db).is_empty());
}

#[tokio::test]
async fn unknown_accounts_are_user_not_found() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    a.clear();
    for ty in ["block_user", "unblock_user"] {
        a.say(json!({"type": ty, "username": "ghost"}));
        assert_eq!(a.notice("user_not_found")["username"], "ghost");
    }
    a.say(json!({"type": "report_user", "username": "ghost", "reason": "spam"}));
    assert_eq!(a.notice("user_not_found")["username"], "ghost");
}

// ---- chat ----------------------------------------------------------------------

/// `blocker` blocks `other`, who is in a game with them.
async fn chat_under_block(blocker_is_first: bool) {
    let (app, store) = setup();
    let (a, b) = game(&app, &store, "alice", "bob");
    let (mut blocker, mut other, other_name) = if blocker_is_first {
        (a, b, "bob")
    } else {
        (b, a, "alice")
    };
    blocker.say(json!({"type": "block_user", "username": other_name}));
    assert_eq!(names(&blocker.next("blocks")["blocked"]), [other_name]);
    // Nothing happened for the blocked player.
    assert!(other.types().is_empty(), "{:?}", other.types());

    // Their chat does not reach the blocker, and they see their own line as usual.
    other.say(json!({"type": "chat", "text": "salut"}));
    let echo = other.next("chat");
    assert_eq!(echo["mine"], true);
    assert_eq!(echo["text"], "salut");
    assert!(blocker.try_next("chat").is_none());

    // The blocker's own messages still go through.
    blocker.say(json!({"type": "chat", "text": "bonjour"}));
    assert_eq!(blocker.next("chat")["mine"], true);
    let got = other.next("chat");
    assert_eq!(got["mine"], false);
    assert_eq!(got["text"], "bonjour");

    // Playing is untouched: the blocked player can move and be answered.
    let (white, black) = if blocker.color == Some(chessy_engine::Color::White) {
        (&blocker, &other)
    } else {
        (&other, &blocker)
    };
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    assert!(blocker.try_next("state").is_some());
    assert!(other.try_next("state").is_some());

    // Unblocking brings delivery back.
    blocker.say(json!({"type": "unblock_user", "username": other_name}));
    assert_eq!(blocker.last("blocks")["blocked"], json!([]));
    other.clear();
    other.say(json!({"type": "chat", "text": "re"}));
    assert_eq!(blocker.next("chat")["text"], "re");
}

#[tokio::test]
async fn a_blocked_users_chat_is_not_delivered_whoever_blocks() {
    chat_under_block(true).await;
    chat_under_block(false).await;
}

#[tokio::test]
async fn mute_drops_all_chat_and_survives_a_reconnect() {
    let (app, store) = setup();
    let (mut a, mut b) = game(&app, &store, "alice", "bob");
    assert_eq!(a.welcome["account"]["chat_muted"], false);
    a.say(json!({"type": "set_chat_muted", "muted": true}));
    assert_eq!(a.next("chat_settings")["chat_muted"], true);
    b.say(json!({"type": "chat", "text": "hello?"}));
    assert_eq!(b.next("chat")["mine"], true);
    assert!(a.try_next("chat").is_none());
    // Muting only hides what comes in: the muted player still writes.
    a.say(json!({"type": "chat", "text": "pas de reponse"}));
    assert_eq!(b.next("chat")["text"], "pas de reponse");

    // The setting is on the account, not the connection.
    let token = a.token.clone();
    let mut a = Client::connect(&app, Some(token));
    assert_eq!(a.welcome["account"]["chat_muted"], true);
    a.clear();
    b.say(json!({"type": "chat", "text": "toujours la ?"}));
    assert!(a.try_next("chat").is_none());

    a.say(json!({"type": "set_chat_muted", "muted": false}));
    assert_eq!(a.next("chat_settings")["chat_muted"], false);
    b.say(json!({"type": "chat", "text": "encore"}));
    assert_eq!(a.next("chat")["text"], "encore");
}

#[tokio::test]
async fn the_block_list_survives_a_reconnect() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let _b = account(&app, &store, "Bob");
    let _c = account(&app, &store, "carol");
    a.say(json!({"type": "block_user", "username": "carol"}));
    a.say(json!({"type": "block_user", "username": "bob"}));
    a.clear();
    let token = a.token.clone();
    let mut a = Client::connect(&app, Some(token));
    a.clear();
    a.say(json!({"type": "blocks_list"}));
    assert_eq!(names(&a.next("blocks")["blocked"]), ["Bob", "carol"]);
    a.say(json!({"type": "unblock_user", "username": "BOB"}));
    assert_eq!(names(&a.last("blocks")["blocked"]), ["carol"]);
    // Blocking twice changes nothing.
    a.say(json!({"type": "block_user", "username": "carol"}));
    assert_eq!(names(&a.last("blocks")["blocked"]), ["carol"]);
}

// ---- friends and challenges ----------------------------------------------------

#[tokio::test]
async fn a_blocked_friend_request_looks_like_any_other_to_its_sender() {
    let (app, store) = setup();
    let mut alice = account(&app, &store, "alice");
    let mut bob = account(&app, &store, "bob");
    let mut carol = account(&app, &store, "carol");
    alice.clear();
    bob.clear();
    carol.clear();
    alice.say(json!({"type": "block_user", "username": "bob"}));
    alice.clear();
    assert!(bob.types().is_empty(), "blocking a stranger tells nobody");

    // The same request to somebody who did not block them.
    bob.say(json!({"type": "friend_request", "username": "carol"}));
    let normal = bob.types();
    let normal_friends = bob.last("friends");
    bob.say(json!({"type": "friend_request", "username": "alice"}));
    assert_eq!(bob.types(), normal, "same messages as a normal request");
    let hidden = bob.last("friends");
    assert_eq!(hidden["friends"], normal_friends["friends"]);
    assert_eq!(names(&hidden["outgoing"]), ["alice", "carol"]);
    // ...and the blocker hears and sees nothing.
    assert!(alice.types().is_empty(), "{:?}", alice.types());
    alice.say(json!({"type": "friends_list"}));
    assert_eq!(alice.next("friends")["incoming"], json!([]));
    assert!(carol.has_notice("friend_request_received"));
    // Nor does a search offer to accept it.
    alice.say(json!({"type": "user_search", "query": "bob"}));
    assert_eq!(alice.next("user_results")["users"][0]["relation"], "none");

    // They cannot accept it by name either, nor ask the one they blocked.
    alice.say(json!({"type": "friend_respond", "username": "bob", "accept": true}));
    assert_eq!(alice.error_code(), "no_such_request");
    alice.say(json!({"type": "friend_request", "username": "bob"}));
    assert_eq!(alice.error_code(), "blocked");
    bob.say(json!({"type": "friends_list"}));
    assert_eq!(bob.last("friends")["friends"], json!([]));

    // Once unblocked, the request that waited is there to answer.
    alice.say(json!({"type": "unblock_user", "username": "bob"}));
    assert_eq!(names(&alice.last("friends")["incoming"]), ["bob"]);
}

#[tokio::test]
async fn blocking_ends_the_friendship_and_refuses_challenges() {
    let (app, store) = setup();
    let mut alice = account(&app, &store, "alice");
    let mut bob = account(&app, &store, "bob");
    let mut carol = account(&app, &store, "carol");
    befriend(&mut alice, &mut bob, "alice", "bob");
    befriend(&mut alice, &mut carol, "alice", "carol");

    // What a plain unfriending tells the other side...
    alice.say(json!({"type": "friend_remove", "username": "carol"}));
    let removed = carol.types();
    let removed_friends = carol.last("friends");
    // ...is all a block tells them, on top of an open challenge being dropped.
    alice.say(json!({"type": "challenge", "username": "bob"}));
    assert_eq!(bob.next("challenge_received")["from"]["username"], "alice");
    bob.clear();
    alice.clear();
    alice.say(json!({"type": "block_user", "username": "bob"}));
    assert_eq!(bob.notice("challenge_cancelled")["username"], "alice");
    assert_eq!(bob.notice("friend_removed")["username"], "alice");
    let mut after = bob.types();
    after.retain(|t| t != "notice");
    let mut expected = removed.clone();
    expected.retain(|t| t != "notice");
    assert_eq!(after, expected);
    let bob_friends = bob.last("friends");
    assert_eq!(bob_friends["friends"], removed_friends["friends"]);
    assert_eq!(bob_friends["friends"], json!([]));
    let mine = alice.last("friends");
    assert_eq!(mine["friends"], json!([]));
    assert_eq!(names(&alice.next("blocks")["blocked"]), ["bob"]);

    // Challenges from them fail like a stranger's.
    bob.say(json!({"type": "challenge", "username": "alice"}));
    let err = bob.next("error");
    let mut dave = account(&app, &store, "dave");
    dave.clear();
    bob.say(json!({"type": "challenge", "username": "dave"}));
    assert_eq!(err, bob.next("error"));
    assert_eq!(err["code"], "not_friends");
    assert!(alice.types().is_empty());
    assert!(dave.types().is_empty());

    // Nor can the friendship come back by a request in either direction.
    bob.say(json!({"type": "friend_request", "username": "alice"}));
    bob.clear();
    alice.say(json!({"type": "friends_list"}));
    assert_eq!(alice.next("friends")["friends"], json!([]));
}

#[tokio::test]
async fn blocking_declines_a_challenge_the_blocked_user_sent() {
    let (app, store) = setup();
    let mut alice = account(&app, &store, "alice");
    let mut bob = account(&app, &store, "bob");
    befriend(&mut alice, &mut bob, "alice", "bob");
    bob.say(json!({"type": "challenge", "username": "alice"}));
    alice.next("challenge_received");
    bob.clear();
    alice.clear();
    alice.say(json!({"type": "block_user", "username": "bob"}));
    assert_eq!(bob.notice("challenge_declined")["username"], "alice");
    // The challenge is gone: accepting it fails.
    alice.say(json!({"type": "challenge_respond", "username": "bob", "accept": true}));
    assert_eq!(alice.last("error")["code"], "no_challenge");
}

// ---- spectators ----------------------------------------------------------------

#[tokio::test]
async fn spectators_neither_read_nor_write_chat() {
    let (app, store) = setup();
    let (mut a, mut b) = game(&app, &store, "alice", "bob");
    // There is no spectator chat: players' lines go to the opponent only.
    let mut game_id = None;
    let (white, black) = if a.color == Some(chessy_engine::Color::White) {
        (&a, &b)
    } else {
        (&b, &a)
    };
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    for c in [&mut a, &mut b] {
        if let Some(s) = c.try_next("state") {
            game_id = s["game_id"].as_str().map(str::to_string);
        }
    }
    let game_id = game_id.expect("a state with the game id");
    let mut s = guest(&app);
    s.say(json!({"type": "spectate", "game_id": game_id}));
    let _ = s.next("spectate_state");
    a.say(json!({"type": "chat", "text": "psst"}));
    b.say(json!({"type": "chat", "text": "re psst"}));
    assert!(a.try_next("chat").is_some());
    s.say(json!({"type": "chat", "text": "hello"}));
    assert_eq!(s.error_code(), "not_in_game");
    assert!(!s.types().iter().any(|t| t == "chat"), "{:?}", s.types());
    // Moderation messages work for a spectator account like any other.
    let mut sa = account(&app, &store, "carol");
    sa.say(json!({"type": "spectate", "game_id": game_id}));
    let _ = sa.next("spectate_state");
    sa.say(json!({"type": "block_user", "username": "alice"}));
    assert_eq!(names(&sa.last("blocks")["blocked"]), ["alice"]);
    assert!(!sa.types().iter().any(|t| t == "chat"));
}

// ---- reports -------------------------------------------------------------------

#[tokio::test]
async fn a_report_is_stored_bounded_and_acknowledged_to_the_reporter_only() {
    let (app, store, db) = world(config());
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    let _c = account(&app, &store, "carol");
    a.clear();
    b.clear();
    let long = format!("{}\u{7}\n\u{e9}", "\u{e9}".repeat(2000));
    a.say(json!({
        "type": "report_user",
        "username": "BOB",
        "reason": "harassment",
        "game_id": "g-123_x",
        "context": long,
    }));
    let ack = a.next("report_ack");
    assert_eq!(ack["username"], "bob");
    assert!(b.types().is_empty(), "the target is never told");

    let rows = report_rows(&db);
    assert_eq!(rows.len(), 1);
    let (reporter, target, reason, game_id, context) = rows[0].clone();
    assert_eq!(reporter, a.id);
    assert_eq!(target, b.id);
    assert_eq!(reason, "harassment");
    assert_eq!(game_id.as_deref(), Some("g-123_x"));
    let context = context.unwrap();
    assert!(context.len() <= 1024, "{}", context.len());
    assert!(context.len() >= 1022, "cut at a character, not before");
    assert!(context.chars().all(|c| !c.is_control()));

    // The same report within the window is acknowledged and not stored again.
    a.say(json!({"type": "report_user", "username": "bob", "reason": "spam"}));
    assert_eq!(a.next("report_ack")["username"], "bob");
    assert!(b.types().is_empty());
    assert_eq!(report_rows(&db).len(), 1);

    // Another target, odd game id, no context.
    a.say(json!({
        "type": "report_user",
        "username": "carol",
        "reason": "cheating",
        "game_id": "x'; DROP TABLE reports; --",
    }));
    a.next("report_ack");
    let rows = report_rows(&db);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].2, "cheating");
    assert_eq!(rows[1].3, None, "an implausible game id is dropped");
    assert_eq!(rows[1].4, None);

    // Somebody else may report the same account.
    b.say(json!({"type": "report_user", "username": "carol", "reason": "other"}));
    b.next("report_ack");
    assert_eq!(report_rows(&db).len(), 3);
}

#[tokio::test]
async fn reports_per_reporter_are_capped() {
    let (app, store, db) = world(HubConfig {
        report_max: 2,
        ..config()
    });
    let mut a = account(&app, &store, "alice");
    for n in ["b1", "b2", "b3"] {
        account(&app, &store, n);
    }
    a.clear();
    for n in ["b1", "b2"] {
        a.say(json!({"type": "report_user", "username": n, "reason": "spam"}));
        a.next("report_ack");
    }
    a.say(json!({"type": "report_user", "username": "b3", "reason": "spam"}));
    assert_eq!(a.error_code(), "rate_limited");
    assert_eq!(report_rows(&db).len(), 2);
    // A repeat of an accepted report is still just acknowledged.
    a.say(json!({"type": "report_user", "username": "b1", "reason": "spam"}));
    a.next("report_ack");
}

#[tokio::test]
async fn reports_are_rate_limited_by_the_connection_quota() {
    // Reports cost `expensive_cost` tokens each: a burst of 8 allows two.
    let (app, store, db) = world(HubConfig {
        msg_burst: 8,
        msg_rate: 0.001,
        ..config()
    });
    let mut a = account(&app, &store, "alice");
    for n in ["b1", "b2", "b3"] {
        account(&app, &store, n);
    }
    a.clear();
    for n in ["b1", "b2", "b3"] {
        a.say(json!({"type": "report_user", "username": n, "reason": "spam"}));
    }
    assert_eq!(a.error_code(), "rate_limited");
    assert_eq!(report_rows(&db).len(), 2);
}

// ---- storage -------------------------------------------------------------------

#[tokio::test]
async fn the_migration_can_run_again_over_its_own_tables() {
    let db = TempDb::new();
    {
        let store = Store::open(db.path_str()).unwrap();
        let (id, _) = store.register("alice", "unused-hash", None).unwrap();
        store.register("bob", "unused-hash", None).unwrap();
        store.block_user(&id, "bob").unwrap();
    }
    // Rewound the way tests/history.rs does: every later step runs again.
    db.raw()
        .execute_batch("DROP TABLE skill_history; PRAGMA user_version = 4;")
        .unwrap();
    let store = Store::open(db.path_str()).unwrap();
    let id = store.account_by_name("alice").unwrap().unwrap().id;
    assert_eq!(store.blocked_names(&id).unwrap(), ["bob"]);
}
