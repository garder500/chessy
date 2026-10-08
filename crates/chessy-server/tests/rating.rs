//! Rated games: Elo, counters, history, leaderboard and profiles.

mod common;

use chessy_server::elo;
use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

struct World {
    app: std::sync::Arc<App>,
    store: Store,
    db: TempDb,
}

fn world() -> World {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    // A wide matching range: these tests are about ratings, not pairing.
    let config = HubConfig {
        ranked_range_base: 800,
        // ...and the same two accounts meet many times.
        rated_pair_max: u32::MAX,
        ..HubConfig::default()
    };
    let app = App::new(store.clone(), config);
    World { app, store, db }
}

/// A ranked game between two accounts in which `loser` resigns after four plies.
/// Returns (winner's game_over, loser's game_over).
fn rated_win(winner: Client, loser: Client) -> (Client, Client, Value, Value) {
    let (mut w, mut l) = ranked_match(winner, loser);
    four_plies(&w, &l);
    l.send(ClientMsg::Resign);
    let wo = w.next("game_over");
    let lo = l.next("game_over");
    (w, l, wo, lo)
}

#[test]
fn k_factor_boundaries_are_exact() {
    assert_eq!(elo::new_rating(1200, 1200, 1.0, 29), 1220);
    assert_eq!(elo::new_rating(1200, 1200, 1.0, 30), 1210);
}

#[tokio::test]
async fn a_rated_win_moves_both_ratings_with_k_40() {
    let w = world();
    let alice = account(&w.app, &w.store, "alice");
    let bob = account(&w.app, &w.store, "bob");
    let (alice_id, bob_id) = (alice.id.clone(), bob.id.clone());
    let (_a, _b, wo, lo) = rated_win(alice, bob);

    assert_eq!(wo["rated"], true);
    assert_eq!(wo["reason"], "resignation");
    assert_eq!(
        wo["elo"],
        json!({"you_before": 1200, "you_after": 1220, "opp_before": 1200, "opp_after": 1180})
    );
    assert_eq!(
        lo["elo"],
        json!({"you_before": 1200, "you_after": 1180, "opp_before": 1200, "opp_after": 1220})
    );

    let a = w.store.me(&alice_id).unwrap().unwrap();
    let b = w.store.me(&bob_id).unwrap().unwrap();
    assert_eq!(
        (a.elo, a.games, a.wins, a.losses, a.draws),
        (1220, 1, 1, 0, 0)
    );
    assert_eq!(
        (b.elo, b.games, b.wins, b.losses, b.draws),
        (1180, 1, 0, 1, 0)
    );
    assert_eq!((a.rank, b.rank), (Some(1), Some(2)));
}

#[tokio::test]
async fn k_drops_to_20_after_thirty_games_and_gains_need_not_mirror() {
    let w = world();
    let veteran = account(&w.app, &w.store, "veteran");
    let rookie = account(&w.app, &w.store, "rookie");
    set_rating(&w.db, &veteran.id, 1200, 30);
    let (vid, rid) = (veteran.id.clone(), rookie.id.clone());
    let (_a, _b, wo, lo) = rated_win(veteran, rookie);
    assert_eq!(wo["elo"]["you_after"], 1210, "K = 20 for the veteran");
    assert_eq!(lo["elo"]["you_after"], 1180, "K = 40 for the rookie");
    assert_eq!(w.store.me(&vid).unwrap().unwrap().elo, 1210);
    assert_eq!(w.store.me(&rid).unwrap().unwrap().elo, 1180);
}

#[tokio::test]
async fn upsets_pay_more_and_ratings_never_fall_below_100() {
    let w = world();
    let low = account(&w.app, &w.store, "lowly");
    let high = account(&w.app, &w.store, "highly");
    // Close enough to be matched at once.
    set_rating(&w.db, &low.id, 105, 0);
    set_rating(&w.db, &high.id, 105, 0);
    let (_a, _b, wo, lo) = rated_win(high, low);
    assert_eq!(wo["elo"]["you_after"], 125);
    assert_eq!(lo["elo"]["you_after"], 100, "85 is clamped to the floor");

    let w = world();
    let fav = account(&w.app, &w.store, "fav");
    let dog = account(&w.app, &w.store, "dog");
    set_rating(&w.db, &fav.id, 1400, 0);
    set_rating(&w.db, &dog.id, 1200, 0);
    let (_a, _b, wo, lo) = rated_win(dog, fav);
    assert_eq!(wo["elo"]["you_after"], 1230);
    assert_eq!(lo["elo"]["you_after"], 1400 - 30);
}

#[tokio::test]
async fn an_agreed_draw_is_rated_and_favours_the_underdog() {
    let w = world();
    let strong = account(&w.app, &w.store, "strong");
    let weak = account(&w.app, &w.store, "weak");
    set_rating(&w.db, &strong.id, 1400, 0);
    set_rating(&w.db, &weak.id, 1200, 0);
    let (sid, wid) = (strong.id.clone(), weak.id.clone());
    let (mut s, mut k) = ranked_match(strong, weak);
    four_plies(&s, &k);
    s.send(ClientMsg::OfferDraw);
    k.send(ClientMsg::RespondDraw { accept: true });
    let over = s.next("game_over");
    assert_eq!(over["outcome"], json!({"type": "draw_agreed"}));
    assert_eq!(over["reason"], "agreed_draw");
    assert_eq!(over["rated"], true);
    assert_eq!(over["elo"]["you_after"], 1390);
    assert_eq!(k.next("game_over")["elo"]["you_after"], 1210);
    let a = w.store.me(&sid).unwrap().unwrap();
    assert_eq!((a.draws, a.wins, a.losses, a.games), (1, 0, 0, 1));
    assert_eq!(w.store.me(&wid).unwrap().unwrap().draws, 1);
}

#[tokio::test]
async fn friendly_and_short_games_do_not_touch_ratings() {
    let w = world();
    let a = account(&w.app, &w.store, "amy");
    let b = account(&w.app, &w.store, "ben");
    let (aid, bid) = (a.id.clone(), b.id.clone());

    // Friendly queue.
    a.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let (mut a, mut b) = matched(a, b);
    four_plies(&a, &b);
    b.send(ClientMsg::Resign);
    let over = a.next("game_over");
    assert_eq!(over["rated"], false);
    assert!(over["elo"].is_null());
    let _ = b.next("game_over");

    // Ranked queue but resigned after three plies.
    let (mut a, mut b) = ranked_match(a, b);
    let (white, black) = if a.color == Some(chessy_engine::Color::White) {
        (&a, &b)
    } else {
        (&b, &a)
    };
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.mv("g1", "f3");
    black.send(ClientMsg::Resign);
    let over = a.next("game_over");
    assert_eq!(over["rated"], false, "too short to rate");
    assert!(over["elo"].is_null());
    let _ = b.next("game_over");

    // Private room.
    a.send(ClientMsg::CreateRoom { time: None });
    let code = a.last("lobby")["status"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    b.send(ClientMsg::JoinRoom { code });
    let (mut a, mut b) = matched(a, b);
    four_plies(&a, &b);
    b.send(ClientMsg::Resign);
    assert_eq!(a.next("game_over")["rated"], false);
    let _ = b.next("game_over");

    for id in [&aid, &bid] {
        let me = w.store.me(id).unwrap().unwrap();
        assert_eq!((me.elo, me.games), (1200, 0));
    }
}

#[tokio::test]
async fn guests_never_play_rated() {
    let w = world();
    let g = guest(&w.app);
    let acct = account(&w.app, &w.store, "solo");
    // A guest asking for ranked is silently put in the friendly queue.
    g.send(ClientMsg::QueueJoin {
        ranked: Some(true),
        time: None,
    });
    let mut g = g;
    assert_eq!(
        g.last("lobby")["status"],
        json!({"type": "queued", "ranked": false})
    );
    acct.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let (mut g, mut acct) = matched(g, acct);
    four_plies(&g, &acct);
    acct.send(ClientMsg::Resign);
    let over = g.next("game_over");
    assert_eq!(over["rated"], false);
    assert!(over["elo"].is_null());
    let _ = acct.next("game_over");
}

#[tokio::test]
async fn profile_history_streak_and_recent_games() {
    let w = world();
    let api = Api::new(&w.app);
    let mut alice = account(&w.app, &w.store, "alice");
    let mut bob = account(&w.app, &w.store, "bob");
    let (mut ra, mut rb) = (1200, 1200);
    let mut expected = vec![1200];
    for _ in 0..3 {
        let (a, b, _, _) = rated_win(alice, bob);
        let (na, nb) = (
            elo::new_rating(ra, rb, 1.0, expected.len() as u32 - 1),
            elo::new_rating(rb, ra, 0.0, expected.len() as u32 - 1),
        );
        (ra, rb) = (na, nb);
        expected.push(ra);
        alice = a;
        bob = b;
    }
    let (_, p) = api.get("/api/players/alice", None).await;
    assert_eq!(p["elo"], ra);
    assert_eq!(p["peak_elo"], ra);
    assert_eq!(
        (p["games"].clone(), p["wins"].clone()),
        (json!(3), json!(3))
    );
    assert_eq!(p["streak"], 3);
    assert_eq!(p["rank"], 1);
    let history: Vec<i32> = p["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["elo"].as_i64().unwrap() as i32)
        .collect();
    assert_eq!(history, expected, "chronological, starting at 1200");
    let recent = p["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0]["result"], "win");
    assert_eq!(recent[0]["opponent"], "bob");
    assert_eq!(recent[0]["rated"], true);
    assert_eq!(recent[0]["reason"], "resignation");
    assert_eq!(recent[0]["elo_delta"], expected[3] - expected[2]);
    assert!(["white", "black"].contains(&recent[0]["color"].as_str().unwrap()));
    assert!(recent[0]["game_id"].as_str().unwrap().starts_with('g'));

    let (_, p) = api.get("/api/players/bob", None).await;
    assert_eq!(p["streak"], -3);
    assert_eq!(p["rank"], 2);
    assert_eq!(p["recent"][0]["result"], "loss");
    assert!(p["recent"][0]["elo_delta"].as_i64().unwrap() < 0);

    // A draw ends the streak.
    let (mut a, mut b) = ranked_match(alice, bob);
    four_plies(&a, &b);
    a.send(ClientMsg::OfferDraw);
    b.send(ClientMsg::RespondDraw { accept: true });
    let _ = a.next("game_over");
    let _ = b.next("game_over");
    let (_, p) = api.get("/api/players/alice", None).await;
    assert_eq!(p["streak"], 0);
    assert_eq!(p["draws"], 1);
    assert_eq!(p["recent"][0]["result"], "draw");
    assert_eq!(p["recent"][0]["reason"], "agreed_draw");
}

#[tokio::test]
async fn friendly_games_appear_in_recent_without_a_delta() {
    let w = world();
    let api = Api::new(&w.app);
    let a = account(&w.app, &w.store, "amy");
    let g = guest(&w.app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    g.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    // Queue kinds differ (ranked vs friendly), so nobody is matched yet.
    let mut a = a;
    assert_eq!(a.last("lobby")["status"]["ranked"], true);
    g.send(ClientMsg::LeaveLobby);
    a.send(ClientMsg::LeaveLobby);
    a.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    g.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, mut g) = matched(a, g);
    four_plies(&a, &g);
    g.send(ClientMsg::Resign);
    let _ = a.next("game_over");
    let _ = g.next("game_over");
    let (_, p) = api.get("/api/players/amy", None).await;
    assert_eq!(p["games"], 0, "rated counters ignore friendly games");
    assert_eq!(p["recent"][0]["rated"], false);
    assert!(p["recent"][0]["elo_delta"].is_null());
    assert!(
        p["recent"][0]["opponent"].is_null(),
        "the guest has no name"
    );
    assert_eq!(p["recent"][0]["result"], "win");
    assert_eq!(p["streak"], 0, "friendly games do not count");
}

#[tokio::test]
async fn history_keeps_the_last_thirty_points() {
    let w = world();
    let api = Api::new(&w.app);
    let (id, _) = w.store.register("grinder", "x", None).unwrap();
    let raw = w.db.raw();
    for i in 0..35 {
        raw.execute(
            "INSERT INTO rating_history (player_id, game_id, elo) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, format!("g{i}"), 1000 + i],
        )
        .unwrap();
    }
    let (_, p) = api.get("/api/players/grinder", None).await;
    let history = p["history"].as_array().unwrap();
    assert_eq!(history.len(), 30);
    assert_eq!(history[0]["elo"], 1005);
    assert_eq!(history[29]["elo"], 1034);
}
