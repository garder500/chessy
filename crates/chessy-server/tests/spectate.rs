//! Spectators: the live list, watching, the anti-cheat delay and what stays hidden.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chessy_engine::{parse_square, Action, Color, SkillId, SkillKind, SkillTarget};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

fn sq(name: &str) -> u8 {
    parse_square(name).unwrap()
}

/// No delay for spectators, a quick bot.
fn cfg() -> HubConfig {
    delayed(0)
}

fn delayed(ms: u64) -> HubConfig {
    HubConfig {
        spectator_delay: Duration::from_millis(ms),
        bot_delay_min: Duration::from_millis(5),
        bot_delay_max: Duration::from_millis(10),
        bot_think_max: Duration::from_millis(100),
        ..HubConfig::default()
    }
}

/// Two guests in a friendly game with the given decks, each bringing up to
/// three classic skills of theirs. Returns `(white, black, game_id)` with the
/// opening messages thrown away.
fn start_game(
    app: &Arc<App>,
    store: &Store,
    white_deck: &[SkillId],
    black_deck: &[SkillId],
) -> (Client, Client, String) {
    let mut a = guest(app);
    let mut b = guest(app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    b.color = serde_json::from_value(b.next("deck_select")["you"].clone()).ok();
    let (mut white, mut black) = if a.color == Some(Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    store.set_deck(&white.id, white_deck).unwrap();
    store.set_deck(&black.id, black_deck).unwrap();
    let classic = |deck: &[SkillId]| -> Vec<SkillId> {
        deck.iter()
            .copied()
            .filter(|s| s.kind() == SkillKind::Classic)
            .take(3)
            .collect()
    };
    white.pick(&classic(white_deck));
    black.pick(&classic(black_deck));
    let game_id = white.last("state")["game_id"].as_str().unwrap().to_string();
    white.clear();
    black.clear();
    (white, black, game_id)
}

/// A game with whatever decks the guests were dealt.
fn plain_game(app: &Arc<App>) -> (Client, Client, String) {
    let mut a = guest(app);
    let mut b = guest(app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    b.color = serde_json::from_value(b.next("deck_select")["you"].clone()).ok();
    a.pick_nothing();
    b.pick_nothing();
    let game_id = a.last("state")["game_id"].as_str().unwrap().to_string();
    a.clear();
    b.clear();
    if a.color == Some(Color::White) {
        (a, b, game_id)
    } else {
        (b, a, game_id)
    }
}

fn watcher(app: &Arc<App>, game_id: &str) -> Client {
    let mut s = guest(app);
    s.say(json!({"type": "spectate", "game_id": game_id}));
    let _ = s.next("spectate_state");
    s
}

fn skill(c: &Client, skill: SkillId, target: SkillTarget) {
    c.send(ClientMsg::Action {
        action: Action::Skill { skill, target },
    });
}

fn events<'a>(view: &'a Value, ty: &str) -> Vec<&'a Value> {
    view["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == ty)
        .collect()
}

fn square_of(view: &Value, name: &str) -> Value {
    view["board"][sq(name) as usize].clone()
}

fn effect_kinds(view: &Value) -> Vec<String> {
    view["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

fn live(app: &Arc<App>) -> Vec<Value> {
    app.live_games(100)
        .into_iter()
        .map(|g| serde_json::to_value(g).unwrap())
        .collect()
}

// ---- the live list -------------------------------------------------------------

#[tokio::test]
async fn the_live_list_shows_running_games_but_not_deck_selection() {
    let (app, _) = new_app(cfg());
    let api = Api::new(&app);
    let (_, body) = api.get("/api/live", None).await;
    assert_eq!(body, json!({"games": []}));

    // A duel still choosing skills is not listed.
    let mut a = guest(&app);
    let mut b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let _ = a.next("deck_select");
    let _ = b.next("deck_select");
    assert!(live(&app).is_empty());
    a.pick_nothing();
    assert!(live(&app).is_empty(), "one pick is not enough");
    b.pick_nothing();

    let (status, body) = api.get("/api/live", None).await;
    assert_eq!(status, 200);
    let games = body["games"].as_array().unwrap();
    assert_eq!(games.len(), 1);
    let g = &games[0];
    assert_eq!(g["kind"], "duel");
    assert_eq!(g["rated"], false);
    assert_eq!(g["ply"], 0);
    assert_eq!(g["spectators"], 0);
    assert_eq!(
        g["white"],
        json!({"username": null, "elo": null, "bot": false})
    );
    let at = g["started_at"].as_str().unwrap();
    assert!(
        at.len() == 20 && at.ends_with('Z') && at.contains('T'),
        "{at}"
    );
    assert!(g["game_id"].as_str().unwrap().starts_with('g'));
}

#[tokio::test]
async fn solo_games_are_listed_with_the_bot_and_their_ply_moves_on() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.say(json!({"type": "solo_start", "elo": 900, "color": "white"}));
    let _ = g.next("deck_select");
    assert!(live(&app).is_empty());
    g.pick_nothing();
    let listed = live(&app);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["kind"], "solo");
    assert_eq!(listed[0]["rated"], false);
    assert_eq!(
        listed[0]["black"],
        json!({"username": "Sage", "elo": 900, "bot": true})
    );
    g.mv("e2", "e4");
    assert_eq!(live(&app)[0]["ply"], 1, "the real ply, not a delayed one");
}

#[tokio::test]
async fn the_list_is_sorted_by_average_elo_then_age_and_limited() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), cfg());
    let mut names = Vec::new();
    for (name, elo) in [
        ("Low1", 1200),
        ("Low2", 1200),
        ("High1", 1700),
        ("High2", 1700),
    ] {
        let c = account(&app, &store, name);
        set_rating(&db, &c.id, elo, 40);
        names.push(c);
    }
    let [low1, low2, high1, high2]: [Client; 4] = names.try_into().ok().unwrap();
    // The weaker pair starts first.
    let _low = ranked_match(low1, low2);
    let _high = ranked_match(high1, high2);
    let listed = live(&app);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["white"]["elo"].as_i64().unwrap(), 1700);
    assert_eq!(listed[0]["black"]["elo"].as_i64().unwrap(), 1700);
    assert_eq!(listed[0]["rated"], true);
    assert_eq!(listed[1]["white"]["elo"].as_i64().unwrap(), 1200);
    assert_eq!(app.live_games(1).len(), 1);

    let api = Api::new(&app);
    let (_, body) = api.get("/api/live?limit=1", None).await;
    assert_eq!(body["games"].as_array().unwrap().len(), 1);
    let (status, body) = api.get("/api/live?limit=abc", None).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn the_kind_tells_duels_rooms_challenges_and_rematches_apart() {
    let (app, store) = new_app(cfg());
    // A private room.
    let mut host = guest(&app);
    let other = guest(&app);
    host.send(ClientMsg::CreateRoom { time: None });
    let code = host.last("lobby")["status"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    other.send(ClientMsg::JoinRoom { code });
    let (mut white, mut black) = into_game(host, other);
    assert_eq!(live(&app)[0]["kind"], "room");
    // The rematch is a room game too.
    white.send(ClientMsg::Resign);
    white.send(ClientMsg::RematchRequest);
    black.send(ClientMsg::RematchRequest);
    let _ = white.next("deck_select");
    assert_eq!(live(&app).len(), 0, "deck selection again");
    let _ = black.next("deck_select");
    white.pick_nothing();
    black.pick_nothing();
    assert_eq!(live(&app)[0]["kind"], "room");

    // A friend challenge.
    let mut alice = account(&app, &store, "Alice");
    let mut bob = account(&app, &store, "Bob");
    alice.say(json!({"type": "friend_request", "username": "Bob"}));
    bob.say(json!({"type": "friend_respond", "username": "Alice", "accept": true}));
    alice.say(json!({"type": "challenge", "username": "Bob"}));
    bob.say(json!({"type": "challenge_respond", "username": "Alice", "accept": true}));
    let _ = alice.next("deck_select");
    let _ = bob.next("deck_select");
    alice.pick_nothing();
    bob.pick_nothing();
    let kinds: Vec<Value> = live(&app).iter().map(|g| g["kind"].clone()).collect();
    assert!(kinds.contains(&json!("challenge")), "{kinds:?}");
    assert!(kinds.contains(&json!("room")), "{kinds:?}");
}

// ---- watching ------------------------------------------------------------------

#[tokio::test]
async fn a_spectator_gets_the_position_then_one_view_per_action() {
    let (app, _) = new_app(cfg());
    let (white, black, game_id) = plain_game(&app);
    let mut s = guest(&app);
    s.say(json!({"type": "spectate", "game_id": game_id}));
    let v = s.next("spectate_state")["view"].clone();
    assert_eq!(v["game_id"], game_id.as_str());
    assert_eq!(v["kind"], "duel");
    assert_eq!(v["rated"], false);
    assert_eq!(v["ply"], 0);
    assert_eq!(v["to_move"], "white");
    assert_eq!(v["in_check"], false);
    assert_eq!(v["board"].as_array().unwrap().len(), 64);
    assert_eq!(square_of(&v, "e2")["kind"], "pawn");
    assert_eq!(v["outcome"]["type"], "ongoing");
    assert_eq!(v["clock_enabled"], true);
    assert_eq!(v["clock"]["running"], "white");
    assert_eq!(v["used"], json!({"white": [], "black": []}));
    assert_eq!(v["events"], json!([]));
    assert_eq!(v["spectators"], 1);
    assert_eq!(v["delay_ms"], 0);
    assert_eq!(
        v["white"],
        json!({"username": null, "elo": null, "bot": false})
    );
    for key in ["traps", "benched", "moves", "skill_options", "my_skills"] {
        assert!(
            v.get(key).is_none(),
            "`{key}` must not be shown to spectators"
        );
    }

    white.mv("e2", "e4");
    black.mv("e7", "e5");
    let v1 = s.next("spectate_state")["view"].clone();
    let v2 = s.next("spectate_state")["view"].clone();
    assert_eq!((v1["ply"].as_i64(), v2["ply"].as_i64()), (Some(1), Some(2)));
    assert_eq!(events(&v1, "moved").len(), 1);
    assert_eq!(square_of(&v1, "e4")["kind"], "pawn");
    assert_eq!(v1["to_move"], "black");
    assert_eq!(v2["to_move"], "white");
    assert!(s.try_next("spectate_state").is_none());
}

#[tokio::test]
async fn used_skills_show_for_both_sides() {
    let (app, store) = new_app(cfg());
    let (white, _black, game_id) = start_game(&app, &store, &[SkillId::Freeze], &[SkillId::Freeze]);
    let mut s = watcher(&app, &game_id);
    skill(
        &white,
        SkillId::Freeze,
        SkillTarget::Piece { square: sq("e7") },
    );
    let v = s.next("spectate_state")["view"].clone();
    assert_eq!(v["used"]["white"], json!(["freeze"]));
    assert_eq!(v["used"]["black"], json!([]));
    assert_eq!(events(&v, "skill_used")[0]["target"]["square"], sq("e7"));
}

#[tokio::test]
async fn unspectating_stops_the_flow_and_is_harmless_twice() {
    let (app, _) = new_app(cfg());
    let (white, black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    s.send(ClientMsg::Unspectate);
    s.send(ClientMsg::Unspectate);
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    assert!(s.try_next("spectate_state").is_none());
    assert!(s.try_next("error").is_none());
    assert_eq!(live(&app)[0]["spectators"], 0);
    // And they can come back.
    s.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(s.next("spectate_state")["view"]["ply"], 2);
}

#[tokio::test]
async fn watching_again_or_elsewhere_never_counts_twice() {
    let (app, _) = new_app(cfg());
    let (_w1, _b1, game1) = plain_game(&app);
    let (_w2, _b2, game2) = plain_game(&app);
    let mut s = watcher(&app, &game1);
    s.say(json!({"type": "spectate", "game_id": game1}));
    let v = s.next("spectate_state")["view"].clone();
    assert_eq!(v["spectators"], 1);
    let count = |game: &str| {
        live(&app).iter().find(|g| g["game_id"] == game).unwrap()["spectators"].clone()
    };
    assert_eq!(count(&game1), 1);
    s.say(json!({"type": "spectate", "game_id": game2}));
    assert_eq!(s.last("spectate_state")["view"]["game_id"], game2.as_str());
    assert_eq!(count(&game1), 0);
    assert_eq!(count(&game2), 1);
}

#[tokio::test]
async fn the_game_end_reaches_spectators_as_spectate_over_and_frees_them() {
    let (app, _) = new_app(cfg());
    let (white, black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    fools_mate(&white, &black);
    for ply in 1..=3 {
        assert_eq!(s.next("spectate_state")["view"]["ply"], ply);
    }
    let over = s.next("spectate_over")["view"].clone();
    assert_eq!(over["ply"], 4);
    assert_eq!(over["outcome"]["type"], "checkmate");
    assert_eq!(over["outcome"]["winner"], "black");
    assert_eq!(over["clock"]["running"], Value::Null);
    assert!(s.try_next("spectate_state").is_none());
    assert!(live(&app).is_empty(), "a finished game is not listed");

    // Released: more actions or a rematch do not reach them; they can watch again.
    black.send(ClientMsg::RematchRequest);
    white.send(ClientMsg::RematchRequest);
    assert!(s.try_next("spectate_state").is_none());
    s.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(s.error_code(), "no_such_game");
}

#[tokio::test]
async fn resignation_and_agreed_draws_end_the_watch_too() {
    let (app, _) = new_app(cfg());
    let (white, black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    white.mv("e2", "e4");
    black.send(ClientMsg::Resign);
    let _ = s.next("spectate_state");
    let over = s.next("spectate_over")["view"].clone();
    assert_eq!(over["outcome"]["type"], "resignation");

    let (white, black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    white.send(ClientMsg::OfferDraw);
    black.send(ClientMsg::RespondDraw { accept: true });
    let over = s.next("spectate_over")["view"].clone();
    assert_eq!(over["outcome"]["type"], "draw_agreed");
}

// ---- the delay -----------------------------------------------------------------

#[tokio::test]
async fn views_arrive_late_and_in_order() {
    let (app, _) = new_app(delayed(400));
    let (white, mut black, game_id) = plain_game(&app);
    let mut s = guest(&app);
    s.say(json!({"type": "spectate", "game_id": game_id}));
    let first = s.next("spectate_state")["view"].clone();
    assert_eq!(first["ply"], 0);
    assert_eq!(first["delay_ms"], 400);

    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.mv("g1", "f3");
    assert!(
        s.try_next("spectate_state").is_none(),
        "nothing before the delay is over"
    );
    // The real game is already at ply 3, the picture is not.
    assert_eq!(live(&app)[0]["ply"], 3);
    // Someone arriving now sees the delayed picture, not the real one.
    let mut late = guest(&app);
    late.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(late.next("spectate_state")["view"]["ply"], 0);
    s.clear(); // the counter update caused by the newcomer

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(s.try_next("spectate_state").is_none(), "still too early");
    for ply in 1..=3 {
        let v = s.wait_for("spectate_state").await["view"].clone();
        assert_eq!(v["ply"], ply, "order is kept");
        assert_eq!(events(&v, "moved").len(), 1);
    }
    // Players are never delayed.
    assert_eq!(black.last("state")["ply"], 3);
}

#[tokio::test]
async fn the_end_of_the_game_is_delayed_as_well() {
    let (app, _) = new_app(delayed(300));
    let (white, mut black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    fools_mate(&white, &black);
    assert_eq!(
        black.next("game_over")["outcome"]["type"],
        "checkmate",
        "the players know at once"
    );
    assert!(s.try_next("spectate_over").is_none());
    assert!(s.try_next("spectate_state").is_none());
    // The game is gone for newcomers while the old picture is still on its way.
    let mut late = guest(&app);
    late.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(late.error_code(), "no_such_game");
    for ply in 1..=3 {
        assert_eq!(s.wait_for("spectate_state").await["view"]["ply"], ply);
    }
    let over = s.wait_for("spectate_over").await["view"].clone();
    assert_eq!(over["outcome"]["type"], "checkmate");
    // Released only now: the spectator may watch another game.
    let (_w, _b, other) = plain_game(&app);
    s.say(json!({"type": "spectate", "game_id": other}));
    assert_eq!(s.next("spectate_state")["view"]["ply"], 0);
}

#[tokio::test]
async fn solo_games_are_shown_live_whatever_the_delay() {
    let (app, _) = new_app(delayed(60_000));
    let mut g = guest(&app);
    g.say(json!({"type": "solo_start", "elo": 400, "color": "white"}));
    let _ = g.next("deck_select");
    g.pick_nothing();
    let game_id = g.last("state")["game_id"].as_str().unwrap().to_string();
    let mut s = watcher(&app, &game_id);
    g.mv("e2", "e4");
    let v = s.wait_for("spectate_state").await["view"].clone();
    assert_eq!(v["ply"], 1);
    assert_eq!(v["delay_ms"], 0);
    assert_eq!(v["kind"], "solo");
    assert_eq!(v["clock_enabled"], false);
    assert_eq!(
        v["black"],
        json!({"username": "Sage", "elo": 400, "bot": true})
    );
    let reply = s.wait_for("spectate_state").await["view"].clone();
    assert_eq!(reply["ply"], 2, "the bot's answer follows");
    assert_eq!(reply["to_move"], "white");
}

// ---- hidden information --------------------------------------------------------

#[tokio::test]
async fn invisible_pieces_of_both_sides_are_hidden_from_spectators() {
    let (app, store) = new_app(cfg());
    let (mut white, black, game_id) = start_game(
        &app,
        &store,
        &[SkillId::Invisibility],
        &[SkillId::Invisibility, SkillId::Freeze],
    );
    let mut s = watcher(&app, &game_id);

    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("e2") },
    );
    let v = s.next("spectate_state")["view"].clone();
    assert!(square_of(&v, "e2").is_null(), "hidden from the spectator");
    assert!(!effect_kinds(&v).contains(&"invisible".to_string()));
    assert!(events(&v, "effect_added").is_empty());
    let used = events(&v, "skill_used");
    assert_eq!(used.len(), 1);
    assert_eq!(used[0]["target"], json!({"kind": "none"}));
    assert_eq!(v["used"]["white"], json!(["invisibility"]));

    // Black hides a piece of their own: hidden as well.
    skill(
        &black,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("a7") },
    );
    let v = s.next("spectate_state")["view"].clone();
    assert!(square_of(&v, "a7").is_null());
    assert!(square_of(&v, "e2").is_null());
    assert!(!effect_kinds(&v).contains(&"invisible".to_string()));
    assert!(events(&v, "effect_added").is_empty());
    assert_eq!(
        events(&v, "skill_used")[0]["target"],
        json!({"kind": "none"})
    );

    // The hidden pawn moves: no `moved` event, nothing on the destination.
    white.mv("e2", "e4");
    let v = s.next("spectate_state")["view"].clone();
    assert!(events(&v, "moved").is_empty(), "{v}");
    assert!(square_of(&v, "e4").is_null());
    assert_eq!(v["ply"], 3);
    // The players still see their own piece.
    assert_eq!(
        white.last("state")["board"][sq("e4") as usize]["kind"],
        "pawn"
    );

    // Spectators never get a skill aimed at a hidden piece either.
    black.mv("b7", "b6");
    let _ = s.next("spectate_state");
    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("e4") },
    );
    // (already used: refused, so nothing reaches the spectator)
    assert!(s.try_next("spectate_state").is_none());

    // Four plies after her skill the first pawn shows again.
    white.mv("a2", "a3");
    let v = s.next("spectate_state")["view"].clone();
    assert_eq!(v["ply"], 5);
    assert_eq!(square_of(&v, "e4")["kind"], "pawn");
}

#[tokio::test]
async fn traps_and_the_bench_are_never_revealed() {
    let (app, store) = new_app(cfg());
    let (white, black, game_id) = start_game(
        &app,
        &store,
        &[SkillId::Bench],
        &[SkillId::Trap, SkillId::Freeze],
    );
    let mut s = watcher(&app, &game_id);

    skill(
        &white,
        SkillId::Bench,
        SkillTarget::Piece { square: sq("b1") },
    );
    let v = s.next("spectate_state")["view"].clone();
    assert!(square_of(&v, "b1").is_null());
    assert!(v.get("benched").is_none(), "{v}");
    assert!(v.get("traps").is_none());

    skill(
        &black,
        SkillId::Trap,
        SkillTarget::Square { square: sq("e5") },
    );
    let v = s.next("spectate_state")["view"].clone();
    assert!(v.get("traps").is_none());
    assert!(events(&v, "trap_set").is_empty(), "{v}");
    assert_eq!(
        events(&v, "skill_used")[0]["target"],
        json!({"kind": "none"})
    );
    // The serialised view mentions no trap square anywhere but the events.
    assert!(!v.to_string().contains("trap_set"));

    // The knight came back with the second action; a sprung trap is public.
    assert_eq!(square_of(&v, "b1")["kind"], "knight");
    assert_eq!(events(&v, "unbenched").len(), 1);
    white.mv("e2", "e4");
    black.mv("a7", "a6");
    let _ = s.next("spectate_state");
    let _ = s.next("spectate_state");
    white.mv("e4", "e5");
    let v = s.next("spectate_state")["view"].clone();
    assert_eq!(events(&v, "trap_sprung").len(), 1);
    assert_eq!(events(&v, "trap_sprung")[0]["square"], sq("e5"));
    assert!(effect_kinds(&v).contains(&"frozen".to_string()));
}

#[tokio::test]
async fn mind_reading_hints_stay_with_the_player() {
    let (app, store) = new_app(cfg());
    let (mut white, _black, game_id) = start_game(
        &app,
        &store,
        &[SkillId::Mind, SkillId::Freeze],
        &[SkillId::Freeze],
    );
    let mut s = watcher(&app, &game_id);
    skill(&white, SkillId::Mind, SkillTarget::None);
    assert_eq!(events(&white.last("state"), "best_move").len(), 1);
    let v = s.next("spectate_state")["view"].clone();
    assert!(events(&v, "best_move").is_empty(), "{v}");
    assert_eq!(v["ply"], 0, "Mind Reading does not pass the turn");
    assert_eq!(v["to_move"], "white");
}

// ---- limits and errors ---------------------------------------------------------

#[tokio::test]
async fn players_cannot_watch_and_unknown_games_do_not_exist() {
    let (app, _) = new_app(cfg());
    let (mut white, _black, game_id) = plain_game(&app);
    white.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(white.error_code(), "already_in_game");
    let (_w, _b, other) = plain_game(&app);
    white.say(json!({"type": "spectate", "game_id": other}));
    assert_eq!(white.error_code(), "already_in_game");

    let mut s = guest(&app);
    s.say(json!({"type": "spectate", "game_id": "g99-nope"}));
    assert_eq!(s.error_code(), "no_such_game");

    // A game still in deck selection is not watchable.
    let mut a = guest(&app);
    let b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let id = a.next("deck_select")["game_id"]
        .as_str()
        .unwrap()
        .to_string();
    s.say(json!({"type": "spectate", "game_id": id}));
    assert_eq!(s.error_code(), "no_such_game");
    // A refused spectator is not counted as one.
    assert!(live(&app).iter().all(|g| g["spectators"] == 0));
    let _ = b;
}

#[tokio::test]
async fn at_most_fifty_people_can_watch_one_game() {
    let (app, _) = new_app(cfg());
    let (_white, _black, game_id) = plain_game(&app);
    let mut watchers = Vec::new();
    for _ in 0..50 {
        watchers.push(watcher(&app, &game_id));
    }
    assert_eq!(live(&app)[0]["spectators"], 50);
    let mut extra = guest(&app);
    extra.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(extra.error_code(), "spectate_full");
    assert!(extra.try_next("spectate_state").is_none());
    // Someone already in can ask again without being refused.
    watchers[0].say(json!({"type": "spectate", "game_id": game_id}));
    assert!(watchers[0].try_next("error").is_none());
    // A seat frees up.
    watchers[1].send(ClientMsg::Unspectate);
    extra.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(extra.next("spectate_state")["view"]["spectators"], 50);
}

#[tokio::test]
async fn spectators_cannot_play_or_chat() {
    let (app, _) = new_app(cfg());
    let (_white, _black, game_id) = plain_game(&app);
    let mut s = watcher(&app, &game_id);
    s.mv("e2", "e4");
    assert_eq!(s.error_code(), "not_in_game");
    s.say(json!({"type": "chat", "text": "hello"}));
    assert_eq!(s.error_code(), "not_in_game");
    s.send(ClientMsg::Resign);
    assert_eq!(s.error_code(), "not_in_game");
    s.send(ClientMsg::OfferDraw);
    assert_eq!(s.error_code(), "not_in_game");
    assert_eq!(live(&app)[0]["ply"], 0);
}

// ---- counters and presence -----------------------------------------------------

#[tokio::test]
async fn players_hear_about_the_spectator_count() {
    let (app, _) = new_app(cfg());
    let (mut white, mut black, game_id) = plain_game(&app);
    assert!(white.try_next("state").is_none(), "nothing sent yet");
    let s1 = watcher(&app, &game_id);
    assert_eq!(white.last("state")["spectators"], 1);
    assert_eq!(black.last("state")["spectators"], 1);
    let mut s2 = watcher(&app, &game_id);
    assert_eq!(white.last("state")["spectators"], 2);
    // The other spectator's counter follows too, without events.
    let again = s2.try_next("spectate_state");
    assert!(again.is_none(), "the newcomer is not told twice");
    app.disconnect(&s1.id, s1.conn);
    assert_eq!(white.last("state")["spectators"], 1);
    assert_eq!(s2.last("spectate_state")["view"]["spectators"], 1);
    s2.send(ClientMsg::Unspectate);
    assert_eq!(black.last("state")["spectators"], 0);

    // Each new state carries the current count.
    let _s3 = watcher(&app, &game_id);
    let _ = white.last("state");
    white.mv("e2", "e4");
    let state = black.last("state");
    assert_eq!(state["spectators"], 1);
    assert_eq!(state["ply"], 1);
}

#[tokio::test]
async fn disconnecting_unsubscribes_and_a_reconnect_resumes_the_view() {
    let (app, _) = new_app(cfg());
    let (white, black, game_id) = plain_game(&app);
    let s = watcher(&app, &game_id);
    assert_eq!(live(&app)[0]["spectators"], 1);
    // Same account, new socket: the picture is sent again, still one spectator.
    let token = s.token.clone();
    let mut s2 = Client::connect(&app, Some(token));
    let v = s2.next("spectate_state")["view"].clone();
    assert_eq!(v["spectators"], 1);
    assert_eq!(live(&app)[0]["spectators"], 1);
    white.mv("e2", "e4");
    assert_eq!(s2.next("spectate_state")["view"]["ply"], 1);
    // Leaving the socket for good frees the seat.
    app.disconnect(&s2.id, s2.conn);
    assert_eq!(live(&app)[0]["spectators"], 0);
    black.mv("e7", "e5");
    assert!(s2.try_next("spectate_state").is_none());
}

#[tokio::test]
async fn a_spectator_who_starts_a_game_stops_watching() {
    let (app, _) = new_app(cfg());
    let (white, _black, game_id) = plain_game(&app);
    let count = || {
        live(&app)
            .iter()
            .find(|g| g["game_id"] == game_id.as_str())
            .unwrap()["spectators"]
            .clone()
    };

    // Solo.
    let mut s = watcher(&app, &game_id);
    assert_eq!(count(), 1);
    s.say(json!({"type": "solo_start", "elo": 400, "color": "white"}));
    let _ = s.next("deck_select");
    assert_eq!(count(), 0);
    s.pick_nothing();
    white.mv("e2", "e4");
    assert!(s.try_next("spectate_state").is_none());

    // The queue.
    let s = watcher(&app, &game_id);
    assert_eq!(count(), 1);
    s.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(count(), 0);

    // A room.
    let s = watcher(&app, &game_id);
    s.send(ClientMsg::CreateRoom { time: None });
    assert_eq!(count(), 0);
}

#[tokio::test]
async fn an_accepted_challenge_takes_a_spectator_out_of_the_stands() {
    let (app, store) = new_app(cfg());
    let (_white, _black, game_id) = plain_game(&app);
    let mut alice = account(&app, &store, "Alice");
    let bob = account(&app, &store, "Bob");
    alice.say(json!({"type": "friend_request", "username": "Bob"}));
    bob.say(json!({"type": "friend_respond", "username": "Alice", "accept": true}));
    alice.say(json!({"type": "spectate", "game_id": game_id}));
    let _ = alice.next("spectate_state");
    bob.say(json!({"type": "challenge", "username": "Alice"}));
    alice.say(json!({"type": "challenge_respond", "username": "Bob", "accept": true}));
    let _ = alice.next("deck_select");
    let listed = live(&app);
    assert!(listed.iter().all(|g| g["spectators"] == 0), "{listed:?}");
}

#[tokio::test]
async fn friends_see_the_game_they_can_watch() {
    let (app, store) = new_app(cfg());
    let mut alice = account(&app, &store, "Alice");
    let mut bob = account(&app, &store, "Bob");
    let mut carol = account(&app, &store, "Carol");
    alice.say(json!({"type": "friend_request", "username": "Bob"}));
    bob.say(json!({"type": "friend_respond", "username": "Alice", "accept": true}));
    assert_eq!(bob.last("friends")["friends"][0]["game_id"], Value::Null);

    alice.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    carol.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let _ = alice.next("deck_select");
    let _ = carol.next("deck_select");
    // Choosing skills: in_game, but nothing to watch yet.
    let f = bob.last("friends");
    assert_eq!(f["friends"][0]["presence"], "in_game");
    assert_eq!(f["friends"][0]["game_id"], Value::Null);

    alice.pick_nothing();
    carol.pick_nothing();
    let game_id = alice.last("state")["game_id"].as_str().unwrap().to_string();
    let f = bob.last("friends");
    assert_eq!(f["friends"][0]["presence"], "in_game");
    assert_eq!(f["friends"][0]["game_id"], game_id.as_str());

    // "Regarder": Bob follows it.
    bob.say(json!({"type": "spectate", "game_id": game_id}));
    assert_eq!(
        bob.next("spectate_state")["view"]["game_id"],
        game_id.as_str()
    );
    // A spectating friend is merely online.
    assert_eq!(alice.last("friends")["friends"][0]["game_id"], Value::Null);

    carol.send(ClientMsg::Resign);
    let f = bob.last("friends");
    assert_eq!(f["friends"][0]["presence"], "online");
    assert_eq!(f["friends"][0]["game_id"], Value::Null);
    assert_eq!(
        bob.next("spectate_over")["view"]["outcome"]["type"],
        "resignation"
    );
}

#[tokio::test]
async fn the_game_id_of_a_friend_is_null_for_guests_and_offline_friends() {
    let (app, store) = new_app(cfg());
    let mut alice = account(&app, &store, "Alice");
    let bob = account(&app, &store, "Bob");
    alice.say(json!({"type": "friend_request", "username": "Bob"}));
    bob.say(json!({"type": "friend_respond", "username": "Alice", "accept": true}));
    app.disconnect(&bob.id, bob.conn);
    alice.say(json!({"type": "friends_list"}));
    let f = alice.last("friends");
    assert_eq!(f["friends"][0]["presence"], "offline");
    assert!(f["friends"][0].as_object().unwrap().contains_key("game_id"));
    assert_eq!(f["friends"][0]["game_id"], Value::Null);
}
