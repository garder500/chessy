//! Friends, presence, user search and challenges over the hub.

mod common;

use std::time::Duration;

use chessy_server::hub::HubConfig;
use common::*;
use serde_json::{json, Value};

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|f| f["username"].as_str().unwrap().to_string())
        .collect()
}

fn setup() -> (
    std::sync::Arc<chessy_server::App>,
    chessy_server::store::Store,
) {
    new_app(HubConfig::default())
}

/// Makes `a` and `b` friends and clears both inboxes.
fn befriend(a: &mut Client, b: &mut Client, a_name: &str, b_name: &str) {
    a.say(json!({"type": "friend_request", "username": b_name}));
    b.say(json!({"type": "friend_respond", "username": a_name, "accept": true}));
    a.clear();
    b.clear();
}

#[tokio::test]
async fn guests_cannot_use_social_features() {
    let (app, _) = setup();
    let mut g = guest(&app);
    assert!(
        g.try_next("friends").is_none(),
        "guests get no friends list"
    );
    for msg in [
        json!({"type": "friend_request", "username": "someone"}),
        json!({"type": "friend_respond", "username": "someone", "accept": true}),
        json!({"type": "friend_remove", "username": "someone"}),
        json!({"type": "friends_list"}),
        json!({"type": "user_search", "query": "some"}),
        json!({"type": "challenge", "username": "someone"}),
        json!({"type": "challenge_respond", "username": "someone", "accept": true}),
        json!({"type": "challenge_cancel"}),
    ] {
        g.say(msg.clone());
        assert_eq!(g.error_code(), "account_required", "{msg}");
    }
}

#[tokio::test]
async fn accounts_get_their_friends_after_welcome() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let f = a.next("friends");
    assert_eq!(f["friends"], json!([]));
    assert_eq!(f["incoming"], json!([]));
    assert_eq!(f["outgoing"], json!([]));
    a.say(json!({"type": "friends_list"}));
    assert_eq!(a.next("friends")["friends"], json!([]));
}

#[tokio::test]
async fn a_request_accepted_makes_friends_with_presence() {
    let (app, store) = setup();
    let mut alice = account(&app, &store, "Alice");
    let mut bob = account(&app, &store, "Bob");
    alice.clear();
    bob.clear();

    alice.say(json!({"type": "friend_request", "username": "bob"}));
    let f = alice.last("friends");
    assert_eq!(f["outgoing"], json!([{"username": "Bob"}]));
    assert_eq!(f["friends"], json!([]));
    let n = bob.notice("friend_request_received");
    assert_eq!(n["username"], "Alice");
    let f = bob.last("friends");
    assert_eq!(f["incoming"], json!([{"username": "Alice", "elo": 1200}]));

    // A second request changes nothing.
    alice.say(json!({"type": "friend_request", "username": "Bob"}));
    assert_eq!(
        alice.last("friends")["outgoing"].as_array().unwrap().len(),
        1
    );
    assert!(!bob.has_notice("friend_request_received"));

    bob.say(json!({"type": "friend_respond", "username": "alice", "accept": true}));
    assert_eq!(alice.notice("friend_accepted")["username"], "Bob");
    let fa = alice.last("friends");
    assert_eq!(fa["outgoing"], json!([]));
    assert_eq!(fa["friends"][0]["username"], "Bob");
    assert_eq!(fa["friends"][0]["elo"], 1200);
    assert_eq!(fa["friends"][0]["presence"], "online");
    assert!(fa["friends"][0]["last_seen"].is_null());
    let fb = bob.last("friends");
    assert_eq!(names(&fb["friends"]), ["Alice"]);
    assert_eq!(fb["incoming"], json!([]));

    // Asking someone who is already a friend.
    alice.say(json!({"type": "friend_request", "username": "bob"}));
    assert_eq!(alice.notice("already_friends")["username"], "bob");
}

#[tokio::test]
async fn declining_removing_and_withdrawing() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.clear();
    b.clear();

    a.say(json!({"type": "friend_request", "username": "bob"}));
    b.say(json!({"type": "friend_respond", "username": "alice", "accept": false}));
    assert_eq!(a.last("friends")["outgoing"], json!([]));
    assert_eq!(b.last("friends")["incoming"], json!([]));
    assert!(!a.has_notice("friend_accepted"));
    b.say(json!({"type": "friend_respond", "username": "alice", "accept": true}));
    assert_eq!(b.error_code(), "no_such_request");

    // Withdraw an outgoing request.
    a.say(json!({"type": "friend_request", "username": "bob"}));
    a.say(json!({"type": "friend_remove", "username": "bob"}));
    assert_eq!(a.last("friends")["outgoing"], json!([]));
    assert_eq!(b.last("friends")["incoming"], json!([]));
    assert!(
        !b.has_notice("friend_removed"),
        "only real friends are told"
    );

    // Remove a friend.
    befriend(&mut a, &mut b, "alice", "bob");
    b.say(json!({"type": "friend_remove", "username": "ALICE"}));
    assert_eq!(a.notice("friend_removed")["username"], "bob");
    assert_eq!(a.last("friends")["friends"], json!([]));
    assert_eq!(b.last("friends")["friends"], json!([]));
    b.say(json!({"type": "friend_remove", "username": "alice"}));
    assert_eq!(b.notice("user_not_found")["username"], "alice");
}

#[tokio::test]
async fn unknown_users_and_yourself() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    a.clear();
    a.say(json!({"type": "friend_request", "username": "ghost"}));
    assert_eq!(a.notice("user_not_found")["username"], "ghost");
    a.say(json!({"type": "friend_request", "username": "ALICE"}));
    assert!(
        a.has_notice("user_not_found"),
        "you cannot befriend yourself"
    );
    // A guest has no username to find.
    let g = guest(&app);
    a.say(json!({"type": "friend_request", "username": g.id}));
    assert!(a.has_notice("user_not_found"));
}

#[tokio::test]
async fn crossing_requests_become_a_friendship() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.clear();
    b.clear();
    a.say(json!({"type": "friend_request", "username": "bob"}));
    b.say(json!({"type": "friend_request", "username": "alice"}));
    assert_eq!(names(&a.last("friends")["friends"]), ["bob"]);
    assert_eq!(names(&b.last("friends")["friends"]), ["alice"]);
    assert_eq!(a.notice("friend_accepted")["username"], "bob");
}

#[tokio::test]
async fn presence_is_pushed_on_connect_disconnect_and_games() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    befriend(&mut a, &mut b, "alice", "bob");

    // Bob goes away.
    app.disconnect(&b.id, b.conn);
    let f = a.last("friends");
    assert_eq!(f["friends"][0]["presence"], "offline");
    assert!(f["friends"][0]["last_seen"].is_string());

    // Bob comes back.
    let token = b.token.clone();
    let mut b = Client::connect(&app, Some(token));
    assert_eq!(a.last("friends")["friends"][0]["presence"], "online");
    assert_eq!(b.last("friends")["friends"][0]["presence"], "online");

    // Bob starts a game against a stranger; Alice sees it begin and end.
    let c = account(&app, &store, "carol");
    b.clear();
    b.send(chessy_server::protocol::ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    c.send(chessy_server::protocol::ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    assert_eq!(a.last("friends")["friends"][0]["presence"], "in_game");
    let (mut b, mut c) = matched(b, c);
    c.send(chessy_server::protocol::ClientMsg::Resign);
    let _ = (b.next("game_over"), c.next("game_over"));
    assert_eq!(a.last("friends")["friends"][0]["presence"], "online");
}

#[tokio::test]
async fn user_search_finds_prefixes_with_relations() {
    let (app, store) = setup();
    let mut me = account(&app, &store, "marie");
    let mut friend = account(&app, &store, "Mark");
    let _incoming = account(&app, &store, "mario");
    let outgoing = account(&app, &store, "Marcus");
    let _stranger = account(&app, &store, "maya");
    let _other = account(&app, &store, "zed");
    let _guest = guest(&app);
    befriend(&mut me, &mut friend, "marie", "Mark");
    outgoing.say(json!({"type": "friend_request", "username": "marie"}));
    me.say(json!({"type": "friend_request", "username": "mario"}));
    me.clear();

    me.say(json!({"type": "user_search", "query": "MA"}));
    let r = me.next("user_results");
    assert_eq!(r["query"], "MA");
    let got: Vec<(String, String)> = r["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["username"].as_str().unwrap().to_string(),
                u["relation"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let want = [
        ("Marcus", "incoming"),
        ("marie", "self"),
        ("mario", "outgoing"),
        ("Mark", "friend"),
        ("maya", "none"),
    ];
    assert_eq!(
        got,
        want.map(|(a, b)| (a.to_string(), b.to_string())),
        "sorted by name, case-insensitive"
    );
    assert_eq!(r["users"][0]["elo"], 1200);

    me.say(json!({"type": "user_search", "query": "mar"}));
    assert_eq!(
        me.next("user_results")["users"].as_array().unwrap().len(),
        4
    );
    me.say(json!({"type": "user_search", "query": "m"}));
    assert_eq!(me.next("user_results")["users"], json!([]), "too short");
    me.say(json!({"type": "user_search", "query": "ma%"}));
    assert_eq!(me.next("user_results")["users"], json!([]), "no wildcards");
}

#[tokio::test]
async fn user_search_returns_at_most_ten() {
    let (app, store) = setup();
    let mut me = account(&app, &store, "searcher");
    for i in 0..14 {
        store.register(&format!("bot{i:02}"), "x", None).unwrap();
    }
    me.clear();
    me.say(json!({"type": "user_search", "query": "bot"}));
    let users = me.next("user_results")["users"].clone();
    assert_eq!(users.as_array().unwrap().len(), 10);
    assert_eq!(users[0]["username"], "bot00");
}

// ---- challenges -------------------------------------------------------

#[tokio::test]
async fn challenges_need_an_online_free_friend() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.clear();
    b.clear();

    a.say(json!({"type": "challenge", "username": "bob"}));
    assert_eq!(a.error_code(), "not_friends");
    a.say(json!({"type": "challenge", "username": "ghost"}));
    assert_eq!(a.notice("user_not_found")["username"], "ghost");

    befriend(&mut a, &mut b, "alice", "bob");
    app.disconnect(&b.id, b.conn);
    a.clear();
    a.say(json!({"type": "challenge", "username": "bob"}));
    assert_eq!(a.notice("friend_offline")["username"], "bob");

    let token = b.token.clone();
    let b = Client::connect(&app, Some(token));
    let c = account(&app, &store, "carol");
    b.send(chessy_server::protocol::ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    c.send(chessy_server::protocol::ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let (_b, _c) = matched(b, c);
    a.clear();
    a.say(json!({"type": "challenge", "username": "bob"}));
    assert_eq!(a.notice("friend_busy")["username"], "bob");
}

#[tokio::test]
async fn an_accepted_challenge_starts_a_friendly_game() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    befriend(&mut a, &mut b, "alice", "bob");

    a.say(json!({"type": "challenge", "username": "BOB"}));
    assert_eq!(a.next("challenge_sent")["username"], "bob");
    let got = b.next("challenge_received");
    assert_eq!(got["from"], json!({"username": "alice", "elo": 1200}));

    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    let da = a.next("deck_select");
    let db = b.next("deck_select");
    assert_ne!(da["you"], db["you"]);
    assert_eq!(da["rated"], false);
    assert_eq!(
        da["opponent"],
        json!({"username": "bob", "elo": 1200, "guest": false})
    );
    assert_eq!(db["opponent"]["username"], "alice");

    // Both are busy now, and the challenge is spent.
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(b.error_code(), "no_challenge");

    a.color = serde_json::from_value(da["you"].clone()).ok();
    b.color = serde_json::from_value(db["you"].clone()).ok();
    a.pick_nothing();
    b.pick_nothing();
    four_plies(&a, &b);
    b.send(chessy_server::protocol::ClientMsg::Resign);
    let over = a.next("game_over");
    assert_eq!(over["rated"], false, "challenges are friendly games");
    let _ = b.next("game_over");
}

#[tokio::test]
async fn declined_cancelled_and_expired_challenges() {
    let (app, store) = new_app(HubConfig {
        challenge_ttl: Duration::from_millis(100),
        ..HubConfig::default()
    });
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    befriend(&mut a, &mut b, "alice", "bob");

    a.say(json!({"type": "challenge", "username": "bob"}));
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": false}));
    assert_eq!(a.notice("challenge_declined")["username"], "bob");
    assert!(b.try_next("deck_select").is_none());
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(b.error_code(), "no_challenge");

    a.clear();
    b.clear();
    a.say(json!({"type": "challenge", "username": "bob"}));
    a.say(json!({"type": "challenge_cancel"}));
    assert_eq!(b.notice("challenge_cancelled")["username"], "alice");
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(b.last("error")["code"], "no_challenge");

    // Expiry tells both sides, and the answer comes too late.
    a.clear();
    b.clear();
    a.say(json!({"type": "challenge", "username": "bob"}));
    assert_eq!(a.wait_for("notice").await["code"], "challenge_expired");
    assert_eq!(b.notice("challenge_expired")["username"], "alice");
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(b.last("error")["code"], "no_challenge");
}

#[tokio::test]
async fn a_new_challenge_replaces_the_old_one_and_leaving_cancels() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    let mut c = account(&app, &store, "carol");
    befriend(&mut a, &mut b, "alice", "bob");
    befriend(&mut a, &mut c, "alice", "carol");

    a.say(json!({"type": "challenge", "username": "bob"}));
    a.say(json!({"type": "challenge", "username": "carol"}));
    assert_eq!(b.notice("challenge_cancelled")["username"], "alice");
    b.clear();
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(b.last("error")["code"], "no_challenge");

    // The challenger disconnecting withdraws the challenge.
    c.clear();
    app.disconnect(&a.id, a.conn);
    assert_eq!(c.notice("challenge_cancelled")["username"], "alice");
    c.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert_eq!(c.last("error")["code"], "no_challenge");
}

#[tokio::test]
async fn challenging_someone_who_challenged_you_accepts() {
    let (app, store) = setup();
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    befriend(&mut a, &mut b, "alice", "bob");
    a.say(json!({"type": "challenge", "username": "bob"}));
    b.say(json!({"type": "challenge", "username": "alice"}));
    assert!(a.try_next("deck_select").is_some());
    assert!(b.try_next("deck_select").is_some());
}
