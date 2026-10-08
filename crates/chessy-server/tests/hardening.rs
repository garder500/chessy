//! Security hardening: rewards only from rated games and only from what the
//! loser owned (docs/spec-v2.md §2), the cap on rated games between the same
//! two accounts, session revocation and per-connection quotas
//! (docs/spec-v4.md "Durcissement").

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use chessy_engine::{Color, SkillId};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, RewardChoice};
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

fn steal(skill: SkillId) -> ClientMsg {
    ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill,
            replace: None,
        },
    }
}

/// `winner` mates `loser` (nobody brings a skill, so the mate is final): four
/// plies when black wins, five when white does.
fn mate(winner: &Client, loser: &Client) {
    if winner.color == Some(Color::Black) {
        fools_mate(loser, winner);
    } else {
        winner.mv("e2", "e4");
        loser.mv("f7", "f6");
        winner.mv("d2", "d4");
        loser.mv("g7", "g5");
        winner.mv("d1", "h5");
    }
}

/// Reads both `deck_select`s, picks nothing; returns whether the game is rated.
fn start(a: &mut Client, b: &mut Client) -> bool {
    let da = a.next("deck_select");
    let db = b.next("deck_select");
    assert_eq!(da["rated"], db["rated"]);
    a.color = serde_json::from_value(da["you"].clone()).ok();
    b.color = serde_json::from_value(db["you"].clone()).ok();
    a.pick_nothing();
    b.pick_nothing();
    a.clear();
    b.clear();
    da["rated"] == true
}

/// Four plies then `b` resigns: both `game_over`s.
fn short_game(a: &mut Client, b: &mut Client) -> (Value, Value) {
    four_plies(a, b);
    b.send(ClientMsg::Resign);
    (a.next("game_over"), b.next("game_over"))
}

// ---- (2) rewards only for rated games ------------------------------------

#[tokio::test]
async fn a_rated_game_long_enough_rewards_the_winner() {
    let (app, store) = new_app(HubConfig::default());
    let (mut a, mut b) = ranked_match(account(&app, &store, "alice"), account(&app, &store, "bob"));
    mate(&a, &b);
    let over = a.next("game_over");
    assert_eq!(over["rated"], true);
    assert!(over["reward"].is_object(), "{over}");
    assert!(b.next("game_over")["reward"].is_null());
    a.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Skip,
    });
    assert!(a.try_next("error").is_none());
}

#[tokio::test]
async fn resigning_at_zero_or_two_plies_rewards_nobody() {
    for plies in [0, 2] {
        let (app, store) = new_app(HubConfig::default());
        let (a, b) = ranked_match(account(&app, &store, "alice"), account(&app, &store, "bob"));
        let (mut white, mut black) = if a.color == Some(Color::White) {
            (a, b)
        } else {
            (b, a)
        };
        if plies == 2 {
            white.mv("e2", "e4");
            black.mv("e7", "e5");
        }
        // White is to move and resigns: black wins, which used to pay a skill.
        white.send(ClientMsg::Resign);
        let (wo, lo) = (black.next("game_over"), white.next("game_over"));
        assert_eq!(wo["rated"], false, "{plies} plies");
        assert!(wo["reward"].is_null(), "no reward after {plies} plies");
        assert!(lo["reward"].is_null());
        black.send(ClientMsg::RewardChoice {
            choice: RewardChoice::Skip,
        });
        assert_eq!(black.error_code(), "no_reward");
        assert_eq!(store.player_row(&white.id).unwrap().unwrap().games, 0);
    }
}

#[tokio::test]
async fn abandoning_at_zero_plies_by_disconnect_rewards_nobody() {
    let (app, store) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(100),
        ..HubConfig::default()
    });
    let (a, mut b) = ranked_match(account(&app, &store, "alice"), account(&app, &store, "bob"));
    app.disconnect(&a.id, a.conn);
    let over = b.wait_for("game_over").await;
    assert_eq!(over["reason"], "disconnect");
    assert_eq!(over["rated"], false);
    assert!(over["reward"].is_null());
    b.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Skip,
    });
    assert_eq!(b.error_code(), "no_reward");
}

#[tokio::test]
async fn friendly_games_pay_no_reward() {
    // Two guests, even in a long game.
    let (app, _) = new_app(HubConfig::default());
    let (a, b) = (guest(&app), guest(&app));
    a.send(ClientMsg::QueueJoin {
        ranked: Some(true),
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, mut b) = matched(a, b);
    mate(&a, &b);
    assert!(a.next("game_over")["reward"].is_null());
    assert!(b.next("game_over")["reward"].is_null());

    // A private room between two accounts.
    let (app, store) = new_app(HubConfig::default());
    let (mut host, mut joiner) = (account(&app, &store, "alice"), account(&app, &store, "bob"));
    host.clear();
    host.send(ClientMsg::CreateRoom { time: None });
    let code = host.next("lobby")["status"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    joiner.send(ClientMsg::JoinRoom { code });
    assert!(!start(&mut host, &mut joiner), "rooms are friendly");
    mate(&host, &joiner);
    let over = host.next("game_over");
    assert_eq!(over["rated"], false);
    assert!(over["reward"].is_null(), "{over}");

    // A challenge between friends.
    let (app, store) = new_app(HubConfig::default());
    let (mut a, mut b) = (account(&app, &store, "alice"), account(&app, &store, "bob"));
    a.say(json!({"type": "friend_request", "username": "bob"}));
    b.say(json!({"type": "friend_respond", "username": "alice", "accept": true}));
    a.say(json!({"type": "challenge", "username": "bob"}));
    b.say(json!({"type": "challenge_respond", "username": "alice", "accept": true}));
    assert!(!start(&mut a, &mut b), "challenges are friendly");
    mate(&a, &b);
    assert!(a.next("game_over")["reward"].is_null());
    assert!(b.next("game_over")["reward"].is_null());

    // An unrated ranked request made as a friendly game between accounts.
    let (app, store) = new_app(HubConfig::default());
    let (a, b) = (account(&app, &store, "alice"), account(&app, &store, "bob"));
    a.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let (mut a, mut b) = matched(a, b);
    mate(&a, &b);
    assert!(a.next("game_over")["reward"].is_null());
    assert!(b.next("game_over")["reward"].is_null());
}

// ---- (3) pending rewards are tied to what the loser owned ----------------

const A_DECK: &[SkillId] = &[SkillId::Teleportation];
const B_DECK: &[SkillId] = &[SkillId::Imune, SkillId::Freeze];
const C_DECK: &[SkillId] = &[SkillId::Rollback, SkillId::Clone, SkillId::DestinySwapper];

/// Alice has just beaten Bob in a rated game and holds an unclaimed reward;
/// Bob and Carol are idle again with their decks above.
fn alice_beat_bob() -> (Arc<App>, Store, Client, Client, Client, Value) {
    let (app, store) = new_app(HubConfig::default());
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bob");
    let c = account(&app, &store, "carol");
    store.set_deck(&a.id, A_DECK).unwrap();
    store.set_deck(&b.id, B_DECK).unwrap();
    store.set_deck(&c.id, C_DECK).unwrap();
    let (mut a, b) = ranked_match(a, b);
    mate(&a, &b);
    let offer = a.next("game_over")["reward"].clone();
    assert_eq!(offer["steal_options"], json!(["imune", "freeze"]));
    (app, store, a, b, c, offer)
}

#[tokio::test]
async fn a_skill_won_after_the_game_cannot_be_taken_with_the_old_reward() {
    let (_app, store, mut a, b, c, _) = alice_beat_bob();
    // Bob now beats Carol and takes Rollback from her.
    let (mut b, c) = ranked_match(b, c);
    mate(&b, &c);
    let _ = b.next("game_over");
    b.send(steal(SkillId::Rollback));
    assert_eq!(b.next("deck_update")["gained"], "rollback");
    assert!(store.deck(&b.id).unwrap().contains(&SkillId::Rollback));

    // Alice's reward was computed on Bob's deck at the end of *her* game.
    a.send(steal(SkillId::Rollback));
    assert_eq!(a.error_code(), "invalid_reward");
    assert!(store.deck(&b.id).unwrap().contains(&SkillId::Rollback));
    assert!(!store.deck(&a.id).unwrap().contains(&SkillId::Rollback));
    // The claim stays open for what Bob had then.
    a.send(steal(SkillId::Imune));
    assert_eq!(a.next("deck_update")["gained"], "imune");
    drop(c);
}

#[tokio::test]
async fn a_skill_lost_since_the_game_makes_the_claim_invalid() {
    let (app, store, a, b, c, _) = alice_beat_bob();
    // Carol beats Bob and takes Imune from him.
    let (c, b) = ranked_match(c, b);
    mate(&c, &b);
    let mut c = c;
    let _ = c.next("game_over");
    c.send(steal(SkillId::Imune));
    assert_eq!(c.next("deck_update")["gained"], "imune");

    // Reconnecting shows only what can still be taken.
    let back = Client::connect(&app, Some(a.token.clone()));
    assert_eq!(
        back.welcome["pending_reward"]["steal_options"],
        json!(["freeze"])
    );
    let mut a = back;
    a.send(steal(SkillId::Imune));
    assert_eq!(a.error_code(), "invalid_reward");
    assert!(!store.deck(&a.id).unwrap().contains(&SkillId::Imune));
    a.send(steal(SkillId::Freeze));
    assert_eq!(a.next("deck_update")["gained"], "freeze");
    drop(b);
}

#[tokio::test]
async fn a_random_reward_only_takes_what_the_loser_owned_then() {
    for _ in 0..4 {
        let (_app, store, mut a, b, c, _) = alice_beat_bob();
        let (mut b, c) = ranked_match(b, c);
        mate(&b, &c);
        let _ = b.next("game_over");
        b.send(steal(SkillId::Rollback));
        let _ = b.next("deck_update");
        a.send(ClientMsg::RewardChoice {
            choice: RewardChoice::Random { replace: None },
        });
        let _ = a.wait_for("deck_update").await;
        let lost = b.last("deck_update")["lost"].clone();
        assert!(
            lost == "imune" || lost == "freeze",
            "Bob lost {lost} but only had Imune and Freeze when he lost"
        );
        assert!(store.deck(&b.id).unwrap().contains(&SkillId::Rollback));
        drop(c);
    }
}

#[tokio::test]
async fn a_reward_cannot_be_claimed_twice_and_expires() {
    let (app, store, mut a, b, _c, _) = alice_beat_bob();
    a.send(steal(SkillId::Imune));
    let _ = a.next("deck_update");
    a.send(steal(SkillId::Freeze));
    assert_eq!(a.error_code(), "no_reward");
    drop(b);

    let (app2, store2) = new_app(HubConfig {
        reward_ttl: Duration::from_millis(60),
        ..HubConfig::default()
    });
    let (mut a, b) = ranked_match(
        account(&app2, &store2, "alice"),
        account(&app2, &store2, "bob"),
    );
    mate(&a, &b);
    assert!(a.next("game_over")["reward"].is_object());
    tokio::time::sleep(Duration::from_millis(120)).await;
    a.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Random { replace: None },
    });
    assert_eq!(a.error_code(), "no_reward");
    let back = Client::connect(&app2, Some(a.token.clone()));
    assert!(back.welcome["pending_reward"].is_null());
    let _ = (app, store);
}

// ---- (4) cap on rated games between the same two accounts ----------------

#[tokio::test]
async fn rated_rematches_between_the_same_pair_stop_counting_after_three() {
    let (app, store) = new_app(HubConfig::default());
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    for game in 1..=5 {
        let rated = start(&mut a, &mut b);
        assert_eq!(rated, game <= 3, "game {game}");
        let (oa, ob) = short_game(&mut a, &mut b);
        for over in [&oa, &ob] {
            assert_eq!(over["rated"], rated, "game {game}");
            assert_eq!(over["elo"].is_object(), rated);
        }
        // `b` resigned, so `a` is the winner.
        assert_eq!(oa["reward"].is_object(), rated, "game {game}");
        assert!(ob["reward"].is_null());
        a.send(ClientMsg::RematchRequest);
        b.send(ClientMsg::RematchRequest);
    }
    let counted = |c: &Client| store.player_row(&c.id).unwrap().unwrap().games;
    assert_eq!(
        (counted(&a), counted(&b)),
        (3, 3),
        "only three moved the ratings"
    );
}

#[tokio::test]
async fn the_ranked_queue_does_not_pair_a_capped_pair_again() {
    let (app, store) = new_app(HubConfig {
        rated_pair_max: 1,
        ..HubConfig::default()
    });
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert!(start(&mut a, &mut b));
    short_game(&mut a, &mut b);

    // Same two again: they wait instead of being matched.
    a.clear();
    b.clear();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert!(a.try_next("deck_select").is_none() && b.try_next("deck_select").is_none());
    assert_eq!(b.last("lobby")["status"]["type"], "queued");

    // A third account is paired with one of them instead.
    let mut c = account(&app, &store, "carol");
    c.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let paired = usize::from(a.try_next("deck_select").is_some())
        + usize::from(b.try_next("deck_select").is_some());
    assert_eq!(paired, 1);
    assert!(c.try_next("deck_select").is_some());
}

#[tokio::test]
async fn a_capped_pair_alone_in_the_queue_is_paired_anyway_for_an_unrated_game() {
    // With only two players around, waiting for a third would be waiting for ever.
    let (app, store) = new_app(HubConfig {
        rated_pair_max: 1,
        capped_pair_wait: Duration::from_millis(150),
        queue_sweep_interval: Duration::from_millis(30),
        ..HubConfig::default()
    });
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert!(start(&mut a, &mut b));
    short_game(&mut a, &mut b);

    a.clear();
    b.clear();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    // Not at once: someone else is preferred for a moment.
    assert!(a.try_next("deck_select").is_none() && b.try_next("deck_select").is_none());
    let da = a.wait_for("deck_select").await;
    let db = b.wait_for("deck_select").await;
    assert_eq!(
        (da["rated"].clone(), db["rated"].clone()),
        (false.into(), false.into())
    );
    // Both are told why this game does not count.
    assert!(a.has_notice("rated_pair_capped"));
    assert!(b.has_notice("rated_pair_capped"));
}

#[tokio::test]
async fn games_older_than_the_window_do_not_count() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(
        store.clone(),
        HubConfig {
            rated_pair_max: 1,
            ..HubConfig::default()
        },
    );
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert!(start(&mut a, &mut b));
    short_game(&mut a, &mut b);
    db.raw()
        .execute(
            "UPDATE games SET finished_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 hours')",
            [],
        )
        .unwrap();
    a.send(ClientMsg::RematchRequest);
    b.send(ClientMsg::RematchRequest);
    assert!(
        start(&mut a, &mut b),
        "the earlier game is outside the hour"
    );
}

// ---- (5) logout revokes the open connection ------------------------------

#[tokio::test]
async fn logging_out_drops_the_connection_that_used_the_session() {
    let (app, _) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("alice").await;
    let mut alice = Client::connect(&app, Some(token.clone()));
    let mut guest_conn = guest(&app);

    let (status, _) = api
        .call("POST", "/api/auth/logout", Some(&token), None)
        .await;
    assert_eq!(status, 204);
    assert_eq!(alice.next("error")["code"], "session_revoked");
    // Nothing the revoked connection sends is acted on.
    alice.clear();
    alice.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    alice.send(ClientMsg::CreateRoom { time: None });
    assert!(alice.try_next("lobby").is_none());
    assert!(!app.is_connected(&alice.id));
    // Other connections are untouched.
    assert!(guest_conn.try_next("error").is_none());
    // The token is dead: hello with it starts a fresh guest.
    let again = Client::connect(&app, Some(token));
    assert_ne!(again.id, alice.id);
}

#[tokio::test]
async fn logging_out_one_session_keeps_the_other_session_connected() {
    let (app, _) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let first = api.register("alice").await;
    let (_, login) = api
        .post(
            "/api/auth/login",
            json!({"username": "alice", "password": "correct horse"}),
        )
        .await;
    let second = login["token"].as_str().unwrap().to_string();
    let mut on_second = Client::connect(&app, Some(second));
    let (status, _) = api
        .call("POST", "/api/auth/logout", Some(&first), None)
        .await;
    assert_eq!(status, 204);
    on_second.clear();
    on_second.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(on_second.next("lobby")["status"]["type"], "queued");
    assert!(on_second.try_next("error").is_none());
}

#[tokio::test]
async fn logging_out_mid_game_leaves_the_game_like_a_disconnect() {
    let (app, store) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(100),
        ..HubConfig::default()
    });
    let api = Api::new(&app);
    let (_, token) = store.register("alice", "unused-hash", None).unwrap();
    let a = Client::connect(&app, Some(token.clone()));
    let b = account(&app, &store, "bob");
    let (_a, mut b) = ranked_match(a, b);
    let (status, _) = api
        .call("POST", "/api/auth/logout", Some(&token), None)
        .await;
    assert_eq!(status, 204);
    assert_eq!(b.next("opponent_status")["connected"], false);
    let over = b.wait_for("game_over").await;
    assert_eq!(over["reason"], "disconnect");
}

// ---- (6) quotas ----------------------------------------------------------

#[tokio::test]
async fn a_burst_of_messages_is_cut_off_with_rate_limited_then_flooded() {
    let (app, store) = new_app(HubConfig::default());
    let mut f = account(&app, &store, "spammer");
    for _ in 0..1_000 {
        f.say(json!({"type": "friends_list"}));
    }
    let errors: Vec<String> = std::iter::from_fn(|| f.try_next("error"))
        .map(|e| e["code"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        errors,
        ["rate_limited", "flooded"],
        "told once, then hung up"
    );
    assert!(!app.is_connected(&f.id));
    // Heavier messages cost more: ten searches fit in the burst, not eleven.
    let (app, store) = new_app(HubConfig::default());
    let mut s = account(&app, &store, "searcher");
    for _ in 0..11 {
        s.say(json!({"type": "user_search", "query": "ab"}));
    }
    let mut results = 0;
    while s.try_next("user_results").is_some() {
        results += 1;
    }
    assert_eq!(results, 10);
    assert_eq!(s.error_code(), "rate_limited");
}

#[tokio::test]
async fn normal_play_stays_far_below_the_quota() {
    let (app, store) = new_app(HubConfig::default());
    let (mut a, mut b) = ranked_match(account(&app, &store, "alice"), account(&app, &store, "bob"));
    four_plies(&a, &b);
    a.send(ClientMsg::Resign);
    assert!(a.try_next("error").is_none());
    assert!(b.try_next("error").is_none());
}

#[tokio::test]
async fn the_quota_refills_with_time() {
    let (app, store) = new_app(HubConfig {
        msg_rate: 100.0,
        msg_burst: 5,
        ..HubConfig::default()
    });
    let mut a = account(&app, &store, "alice");
    for _ in 0..6 {
        a.send(ClientMsg::LeaveLobby);
    }
    assert_eq!(a.error_code(), "rate_limited");
    a.clear();
    tokio::time::sleep(Duration::from_millis(150)).await;
    a.send(ClientMsg::LeaveLobby);
    assert!(a.try_next("error").is_none());
    assert!(a.try_next("lobby").is_some());
}

#[tokio::test]
async fn the_lobby_is_capped() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(
        store.clone(),
        HubConfig {
            lobby_cap: 2,
            ..HubConfig::default()
        },
    );
    // Far apart in Elo, so nobody is paired.
    let mut players: Vec<Client> = ["p1", "p2", "p3"]
        .into_iter()
        .map(|n| account(&app, &store, n))
        .collect();
    for (p, elo) in players.iter().zip([300, 1200, 2100]) {
        set_rating(&db, &p.id, elo, 0);
    }
    for p in &mut players {
        p.send(ClientMsg::QueueJoin {
            ranked: None,
            time: None,
        });
    }
    assert_eq!(players[0].last("lobby")["status"]["type"], "queued");
    assert_eq!(players[1].last("lobby")["status"]["type"], "queued");
    assert_eq!(players[2].error_code(), "queue_full");

    for p in &mut players {
        p.send(ClientMsg::LeaveLobby);
        p.clear();
    }
    players[0].send(ClientMsg::CreateRoom { time: None });
    players[1].send(ClientMsg::CreateRoom { time: None });
    players[2].send(ClientMsg::CreateRoom { time: None });
    assert!(players[1].try_next("lobby").is_some());
    assert_eq!(players[2].error_code(), "rooms_full");
}

/// Floods the hub from one connection while another game runs on a short
/// clock: the flooder is cut off, no single hold of the hub lock is long and
/// the other game's flag still falls on time.
///
/// Returns how long the flood took, the longest hold of the hub lock, and
/// whether the flag had already fallen when the flood ended (the flood runs
/// on one worker thread; the timer fires on another).
async fn flood_during_a_timed_game(config: HubConfig) -> (Duration, Duration, bool) {
    let clock = Duration::from_millis(400);
    let (app, store) = new_app(HubConfig {
        clock_initial: clock,
        clock_increment: Duration::ZERO,
        ..config
    });
    let spammer = account(&app, &store, "spammer");
    let (a, b) = (guest(&app), guest(&app));
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (_white, mut black) = into_game(a, b);
    let started = Instant::now();
    app.reset_lock_hold();

    let flood = Instant::now();
    for _ in 0..10_000 {
        spammer.say(json!({"type": "user_search", "query": "ab"}));
    }
    let flood_took = flood.elapsed();
    let hold = app.max_lock_hold();

    // White never moves: black wins on time.
    let early = black.try_next("game_over");
    let over = match &early {
        Some(over) => over.clone(),
        None => black.wait_for("game_over").await,
    };
    assert_eq!(over["reason"], "timeout");
    assert!(
        started.elapsed() < clock + Duration::from_millis(1_500),
        "the flag fell late: {:?}",
        started.elapsed()
    );
    (flood_took, hold, early.is_some())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flooding_with_the_default_quota_barely_touches_the_hub() {
    let (flood, hold, early) = flood_during_a_timed_game(HubConfig::default()).await;
    eprintln!("10k messages with quota: {flood:?}, longest lock hold {hold:?}, flag fell during flood: {early}");
    assert!(hold < Duration::from_millis(50), "{hold:?}");
    // Only about a hundred messages get through, far less than the clock.
    assert!(flood < Duration::from_millis(400), "{flood:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn even_with_the_quota_off_messages_are_short_and_timers_run() {
    let open = HubConfig {
        msg_rate: 1e9,
        msg_burst: u32::MAX / 2,
        ..HubConfig::default()
    };
    let (flood, hold, early) = flood_during_a_timed_game(open).await;
    eprintln!("10k messages, no quota: {flood:?}, longest lock hold {hold:?}, flag fell during flood: {early}");
    assert!(hold < Duration::from_millis(50), "{hold:?}");
    if flood > Duration::from_millis(600) {
        // The flood outlasted the clock: the timer must not have been starved.
        assert!(early, "the flag waited for the flood to end");
    }
}
