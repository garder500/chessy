//! The two queues: ranked pairing by Elo proximity that widens with waiting,
//! and first-come friendly pairing.

mod common;

use std::time::Duration;

use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use common::*;
use serde_json::json;

fn ranked() -> ClientMsg {
    ClientMsg::QueueJoin {
        ranked: Some(true),
        time: None,
    }
}

fn friendly() -> ClientMsg {
    ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    }
}

fn rated_account(
    app: &std::sync::Arc<chessy_server::App>,
    store: &chessy_server::store::Store,
    db: &TempDb,
    name: &str,
    elo: i32,
) -> Client {
    let c = account(app, store, name);
    set_rating(db, &c.id, elo, 0);
    c
}

fn world(
    config: HubConfig,
) -> (
    std::sync::Arc<chessy_server::App>,
    chessy_server::store::Store,
    TempDb,
) {
    let db = TempDb::new();
    let store = chessy_server::store::Store::open(db.path_str()).unwrap();
    (chessy_server::App::new(store.clone(), config), store, db)
}

#[test]
fn the_accepted_gap_widens_with_waiting_up_to_a_ceiling() {
    let c = HubConfig::default();
    assert_eq!(c.ranked_range(Duration::ZERO), 100);
    assert_eq!(c.ranked_range(Duration::from_secs(4)), 200);
    assert_eq!(c.ranked_range(Duration::from_millis(2500)), 162);
    assert_eq!(c.ranked_range(Duration::from_secs(28)), 800);
    assert_eq!(c.ranked_range(Duration::from_secs(3600)), 800);
}

#[tokio::test]
async fn ranked_pairs_the_closest_rating_within_range() {
    let (app, store, db) = world(HubConfig::default());
    let mut far = rated_account(&app, &store, &db, "far", 1500);
    let mut near = rated_account(&app, &store, &db, "near", 1230);
    let mut mid = rated_account(&app, &store, &db, "mid", 1290);
    let mut me = rated_account(&app, &store, &db, "me", 1200);

    far.send(ranked());
    assert_eq!(
        far.last("lobby")["status"],
        json!({"type": "queued", "ranked": true})
    );
    mid.send(ranked());
    near.send(ranked());
    // `mid` (1290) and `near` (1230) are 60 apart, so they meet each other.
    assert!(near.try_next("deck_select").is_some());
    assert!(mid.try_next("deck_select").is_some());
    me.send(ranked());
    assert!(
        me.try_next("deck_select").is_none(),
        "only `far` is left, 300 away"
    );
    assert_eq!(me.last("lobby")["status"]["type"], "queued");
    assert!(far.try_next("deck_select").is_none());
}

#[tokio::test]
async fn the_closest_candidate_wins_over_an_earlier_one() {
    let (app, store, db) = world(HubConfig::default());
    let mut a = rated_account(&app, &store, &db, "a", 1100);
    let mut b = rated_account(&app, &store, &db, "b", 1290);
    let mut c = rated_account(&app, &store, &db, "c", 1200);
    // 1100 and 1290 are 190 apart: not matched at first.
    a.send(ranked());
    b.send(ranked());
    assert!(a.try_next("deck_select").is_none());
    // The newcomer at 1200 is 100 from `a` and 90 from `b`: `b` is closer.
    c.send(ranked());
    assert!(c.try_next("deck_select").is_some());
    assert!(b.try_next("deck_select").is_some());
    assert!(a.try_next("deck_select").is_none());
}

#[tokio::test]
async fn waiting_widens_the_range_and_the_sweep_pairs_players() {
    let (app, store, db) = world(HubConfig {
        queue_sweep_interval: Duration::from_millis(30),
        ranked_range_per_second: 1000.0,
        ..HubConfig::default()
    });
    let mut low = rated_account(&app, &store, &db, "low", 1000);
    let mut high = rated_account(&app, &store, &db, "high", 1500);
    low.send(ranked());
    high.send(ranked());
    assert!(
        low.try_next("deck_select").is_none(),
        "500 apart is too far at first"
    );
    // Range = 100 + 1000 * seconds: 500 needs about 0.4 s.
    let a = low.wait_for("deck_select").await;
    let b = high.wait_for("deck_select").await;
    assert_ne!(a["you"], b["you"]);
    assert_eq!(a["rated"], true);
    assert_eq!(
        a["opponent"],
        json!({"username": "high", "elo": 1500, "guest": false})
    );
    assert_eq!(
        b["opponent"],
        json!({"username": "low", "elo": 1000, "guest": false})
    );
}

#[tokio::test]
async fn the_older_waiters_time_counts_when_someone_joins() {
    let (app, store, db) = world(HubConfig {
        // No sweep will run during this test: only a join can match them.
        queue_sweep_interval: Duration::from_secs(3600),
        ranked_range_per_second: 1000.0,
        ..HubConfig::default()
    });
    let mut old = rated_account(&app, &store, &db, "old", 1000);
    let mut new = rated_account(&app, &store, &db, "new", 1350);
    old.send(ranked());
    tokio::time::sleep(Duration::from_millis(400)).await; // range is now ~500
    new.send(ranked());
    assert!(new.try_next("deck_select").is_some());
    assert!(old.try_next("deck_select").is_some());
}

#[tokio::test]
async fn the_range_has_a_ceiling() {
    let (app, store, db) = world(HubConfig {
        queue_sweep_interval: Duration::from_millis(20),
        ranked_range_per_second: 5000.0,
        ranked_range_max: 300,
        ..HubConfig::default()
    });
    let mut low = rated_account(&app, &store, &db, "low", 1000);
    let mut high = rated_account(&app, &store, &db, "high", 1400);
    low.send(ranked());
    high.send(ranked());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(low.try_next("deck_select").is_none(), "400 > 300 forever");
    assert!(high.try_next("deck_select").is_none());
    // Someone within the ceiling still gets paired.
    let mut mid = rated_account(&app, &store, &db, "mid", 1250);
    mid.send(ranked());
    assert!(mid.try_next("deck_select").is_some());
}

#[tokio::test]
async fn leaving_or_disconnecting_removes_you_from_the_ranked_queue() {
    let (app, store, db) = world(HubConfig::default());
    let mut a = rated_account(&app, &store, &db, "a", 1200);
    let b = rated_account(&app, &store, &db, "b", 1200);
    let mut c = rated_account(&app, &store, &db, "c", 1200);
    a.send(ranked());
    a.send(ClientMsg::LeaveLobby);
    assert_eq!(a.last("lobby")["status"]["type"], "idle");
    b.send(ranked());
    assert!(a.try_next("deck_select").is_none());
    app.disconnect(&b.id, b.conn);
    c.send(ranked());
    assert!(c.try_next("deck_select").is_none(), "b is gone");
    assert_eq!(c.last("lobby")["status"]["ranked"], true);
}

#[tokio::test]
async fn friendly_queue_is_first_come_and_separate_from_ranked() {
    let (app, store, db) = world(HubConfig::default());
    let mut acct = rated_account(&app, &store, &db, "acct", 1200);
    let mut other = rated_account(&app, &store, &db, "other", 1200);
    let mut g1 = guest(&app);
    let mut g2 = guest(&app);

    acct.send(ranked());
    g1.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(
        g1.last("lobby")["status"],
        json!({"type": "queued", "ranked": false})
    );
    assert!(
        acct.try_next("deck_select").is_none(),
        "ranked and friendly never mix"
    );
    assert!(g1.try_next("deck_select").is_none());

    other.send(friendly());
    let d = other.next("deck_select");
    assert_eq!(d["rated"], false);
    assert_eq!(
        d["opponent"],
        json!({"username": null, "elo": null, "guest": true})
    );
    assert!(g1.try_next("deck_select").is_some());

    // Guests asking for ranked end up friendly and meet each other.
    g2.send(ranked());
    assert_eq!(g2.last("lobby")["status"]["ranked"], false);
    let mut g3 = guest(&app);
    g3.send(ranked());
    assert!(g2.try_next("deck_select").is_some());
    assert!(g3.try_next("deck_select").is_some());
}

#[tokio::test]
async fn a_player_cannot_sit_in_two_queues() {
    let (app, store, db) = world(HubConfig::default());
    let mut a = rated_account(&app, &store, &db, "a", 1200);
    let mut b = rated_account(&app, &store, &db, "b", 1200);
    a.send(ranked());
    a.send(friendly());
    assert_eq!(a.last("lobby")["status"]["ranked"], false);
    b.send(ranked());
    assert!(
        b.try_next("deck_select").is_none(),
        "a moved to the friendly queue"
    );
    a.send(ranked());
    assert!(a.try_next("deck_select").is_some());
}

#[tokio::test]
async fn players_only_meet_someone_who_asked_for_the_same_length() {
    use chessy_server::protocol::TimeControl::{Long, Short};
    let (app, store, db) = world(HubConfig::default());
    let mut a = rated_account(&app, &store, &db, "aa", 1200);
    let mut b = rated_account(&app, &store, &db, "bb", 1200);
    let mut c = rated_account(&app, &store, &db, "cc", 1200);

    a.send(ClientMsg::QueueJoin {
        ranked: Some(true),
        time: Some(Short),
    });
    b.send(ClientMsg::QueueJoin {
        ranked: Some(true),
        time: Some(Long),
    });
    assert!(
        a.try_next("deck_select").is_none(),
        "different lengths never pair"
    );
    assert!(b.try_next("deck_select").is_none());

    c.send(ClientMsg::QueueJoin {
        ranked: Some(true),
        time: Some(Short),
    });
    assert!(a.try_next("deck_select").is_some());
    assert!(c.try_next("deck_select").is_some());
    assert!(b.try_next("deck_select").is_none());
}

#[tokio::test]
async fn the_clock_starts_at_the_requested_length() {
    use chessy_server::protocol::TimeControl::Medium;
    let (app, _store, _db) = world(HubConfig::default());
    let mut a = guest(&app);
    let mut b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: Some(Medium),
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: Some(Medium),
    });
    a.next("deck_select");
    b.next("deck_select");
    a.send(ClientMsg::SelectDeck { skills: vec![] });
    b.send(ClientMsg::SelectDeck { skills: vec![] });
    let state = a.next("state");
    assert_eq!(state["clock"]["white_ms"], 15 * 60 * 1000);
    assert_eq!(state["clock"]["black_ms"], 15 * 60 * 1000);
}
