//! Solo mode: games against the bot.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chessy_engine::{parse_square, Action, Color};
use chessy_server::hub::{Hub, HubConfig, Timer};
use chessy_server::protocol::{ClientMsg, ServerMsg, SoloColor};
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::{json, Value};
use tokio::sync::mpsc::unbounded_channel;

/// Quick bot: short delays and a short search so debug builds keep up. The
/// test clients answer the bot as fast as it plays, far faster than a person
/// could, so the per-connection message quota (tested in `hardening.rs`) is
/// lifted here: it would otherwise drop moves now and then.
fn cfg() -> HubConfig {
    HubConfig {
        bot_delay_min: Duration::from_millis(5),
        bot_delay_max: Duration::from_millis(10),
        bot_think_max: Duration::from_millis(150),
        msg_rate: 10_000.0,
        msg_burst: 10_000,
        ..HubConfig::default()
    }
}

/// Starts a solo game and returns the `deck_select` message.
fn solo(c: &mut Client, elo: i64, color: &str) -> Value {
    c.say(json!({"type": "solo_start", "elo": elo, "color": color}));
    let ds = c.next("deck_select");
    c.color = serde_json::from_value(ds["you"].clone()).ok();
    ds
}

/// Waits until it is `c`'s turn in an ongoing game and returns that `state`;
/// `Err` carries the `game_over` if the game ended first.
async fn my_turn(c: &mut Client) -> Result<Value, Value> {
    let me = serde_json::to_value(c.color.unwrap()).unwrap();
    for _ in 0..600 {
        if let Some(over) = c.try_next("game_over") {
            return Err(over);
        }
        if let Some(state) = c.try_next("state") {
            if state["to_move"] == me && state["outcome"]["type"] == "ongoing" {
                return Ok(state);
            }
            continue;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let error = c.try_next("error");
    panic!("no turn arrived; have {:?}; error {error:?}", c.types());
}

fn move_json(m: &Value) -> Value {
    json!({"type": "action", "action": {"type": "move", "from": m["from"], "to": m["to"], "promo": m["promo"]}})
}

fn resign_and_wait(c: &mut Client) -> Value {
    c.send(ClientMsg::Resign);
    c.next("game_over")
}

// ---- starting ------------------------------------------------------------------

#[tokio::test]
async fn a_guest_starts_a_solo_game_against_sage() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    let ds = solo(&mut g, 1400, "white");
    assert_eq!(ds["you"], "white");
    assert_eq!(ds["rated"], false);
    assert_eq!(
        ds["opponent"],
        json!({"username": "Sage", "elo": 1400, "guest": true, "bot": true})
    );
    assert_eq!(ds["submitted"], false);
}

#[tokio::test]
async fn an_account_can_play_solo_too() {
    let (app, store) = new_app(cfg());
    let mut a = account(&app, &store, "alice");
    let ds = solo(&mut a, 800, "black");
    assert_eq!(ds["you"], "black");
    assert_eq!(ds["opponent"]["bot"], true);
    assert_eq!(ds["opponent"]["username"], "Sage");
}

#[tokio::test]
async fn people_are_not_marked_as_bots() {
    let (app, _) = new_app(cfg());
    let a = guest(&app);
    let b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, _b) = matched_keep(a, b);
    let s = a.last("state");
    assert!(s["opponent"].get("bot").is_none() || s["opponent"]["bot"] == false);
}

/// `matched` without discarding the opening states.
fn matched_keep(mut a: Client, mut b: Client) -> (Client, Client) {
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    b.color = serde_json::from_value(b.next("deck_select")["you"].clone()).ok();
    a.pick_nothing();
    b.pick_nothing();
    (a, b)
}

#[tokio::test]
async fn color_defaults_to_random_and_random_gives_both_sides() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.say(json!({"type": "solo_start", "elo": 1000}));
    let ds = g.next("deck_select");
    assert!(ds["you"] == "white" || ds["you"] == "black");

    let mut seen = std::collections::HashSet::new();
    for _ in 0..24 {
        let mut c = guest(&app);
        let ds = solo(&mut c, 1000, "random");
        seen.insert(ds["you"].as_str().unwrap().to_string());
    }
    assert_eq!(seen.len(), 2, "random never picked both colours: {seen:?}");
}

#[tokio::test]
async fn solo_start_is_refused_when_busy_or_out_of_range() {
    let (app, _) = new_app(cfg());
    let mut a = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.clear();
    a.say(json!({"type": "solo_start", "elo": 1000, "color": "white"}));
    assert_eq!(a.error_code(), "already_in_game", "while queued");
    a.send(ClientMsg::LeaveLobby);

    a.send(ClientMsg::CreateRoom { time: None });
    a.clear();
    a.say(json!({"type": "solo_start", "elo": 1000, "color": "white"}));
    assert_eq!(a.error_code(), "already_in_game", "while hosting a room");
    a.send(ClientMsg::LeaveLobby);

    a.clear();
    solo(&mut a, 1000, "white");
    a.say(json!({"type": "solo_start", "elo": 1000, "color": "white"}));
    assert_eq!(a.error_code(), "already_in_game", "while in a solo game");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(a.error_code(), "already_in_game", "queueing during solo");

    let b = guest(&app);
    let c = guest(&app);
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    c.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut b, _c) = matched(b, c);
    b.say(json!({"type": "solo_start", "elo": 1000, "color": "white"}));
    assert_eq!(b.error_code(), "already_in_game", "while in a duel");

    let mut d = guest(&app);
    for elo in [399, 2801, 0, -5, 100_000] {
        d.say(json!({"type": "solo_start", "elo": elo, "color": "white"}));
        assert_eq!(d.error_code(), "invalid_elo", "elo {elo}");
    }
    // The bounds themselves are fine.
    for elo in [400, 2800] {
        let mut e = guest(&app);
        solo(&mut e, elo, "white");
    }
}

// ---- playing -------------------------------------------------------------------

#[tokio::test]
async fn the_game_has_no_clock_and_the_bot_has_three_classic_skills() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 600, "white");
    g.pick_nothing();
    let s = g.last("state");
    assert_eq!(s["clock_enabled"], false);
    assert_eq!(s["clock"]["running"], Value::Null);
    assert_eq!(s["rated"], false);
    assert_eq!(s["opponent"]["bot"], true);
    assert_eq!(s["opponent_skills"]["total"], 3);
    assert_eq!(s["opponent_skills"]["used"], json!([]));
    assert_eq!(s["ply"], 0);
    assert_eq!(s["opponent_connected"], true);
    // A normal game keeps its clock.
    let a = guest(&app);
    let b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, _b) = matched_keep(a, b);
    let s = a.last("state");
    assert_eq!(s["clock_enabled"], true);
    assert_ne!(s["clock"]["running"], Value::Null);
}

#[tokio::test]
async fn the_bot_answers_a_move() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 600, "white");
    g.pick_nothing();
    let s = my_turn(&mut g).await.unwrap();
    assert_eq!(s["ply"], 0);
    g.mv("e2", "e4");
    let s = my_turn(&mut g).await.unwrap();
    assert_eq!(s["ply"], 2, "the human moved, then the bot");
    assert_eq!(s["to_move"], "white");
    assert!(!s["moves"].as_array().unwrap().is_empty());
    // The bot's move is in the events of the state that announced it.
}

#[tokio::test]
async fn the_bot_opens_when_the_player_has_black() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 600, "black");
    g.pick_nothing();
    let s = my_turn(&mut g).await.unwrap();
    assert_eq!(s["ply"], 1, "white (the bot) already moved");
    assert_eq!(s["you"], "black");
    assert_eq!(s["clock_enabled"], false);
}

#[tokio::test]
async fn skills_work_for_the_player_and_do_not_break_the_bot() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    let deck: Vec<chessy_engine::SkillId> =
        serde_json::from_value(g.welcome["deck"].clone()).unwrap();
    solo(&mut g, 700, "white");
    g.pick(&deck[..deck.len().min(3)]);
    let mut s = my_turn(&mut g).await.unwrap();
    for _ in 0..6 {
        let m = s["moves"][0].clone();
        g.say(move_json(&m));
        match my_turn(&mut g).await {
            Ok(next) => s = next,
            Err(_) => return,
        }
    }
    assert!(s["ply"].as_u64().unwrap() >= 12);
}

#[tokio::test]
async fn a_whole_game_ends_with_no_reward_no_elo_and_no_public_trace() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), cfg());
    let mut a = account(&app, &store, "alice");
    solo(&mut a, 400, "white");
    a.pick_nothing();
    let mut over = None;
    for n in 0..260 {
        match my_turn(&mut a).await {
            Ok(s) => {
                let moves = s["moves"].as_array().unwrap();
                // Wander about with legal moves; the 400 bot should win or the cap resigns.
                let m = &moves[(n * 7 + 3) % moves.len()];
                a.say(move_json(m));
            }
            Err(o) => {
                over = Some(o);
                break;
            }
        }
    }
    let over = match over {
        Some(o) => o,
        None => resign_and_wait(&mut a),
    };
    assert_eq!(over["rated"], false);
    assert_eq!(over["elo"], Value::Null);
    assert_eq!(over["reward"], Value::Null);
    assert!(over["reason"].is_string());

    let me = store.me(&a.id).unwrap().unwrap();
    assert_eq!(
        (me.elo, me.games, me.wins, me.losses, me.draws),
        (1200, 0, 0, 0, 0)
    );
    let profile = store.public_profile("alice").unwrap().unwrap();
    assert!(
        profile.recent.is_empty(),
        "solo games stay out of the history"
    );
    // The game is recorded for replay (docs/spec-v4.md §1), as a solo game
    // whose bot seat is not a player.
    let (rows, kind, seats): (i64, String, i64) = db
        .raw()
        .query_row(
            "SELECT COUNT(*), MIN(kind), SUM(white IS NULL) + SUM(black IS NULL) FROM games",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((rows, kind.as_str(), seats), (1, "solo", 1));
    // Back in the lobby and free to play again.
    assert_eq!(a.last("lobby")["status"]["type"], "idle");
    a.say(json!({"type": "solo_start", "elo": 500, "color": "white"}));
    a.next("deck_select");
}

#[tokio::test]
async fn resigning_ends_the_game_and_the_bot_never_resigns() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 600, "white");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    g.mv("e2", "e4");
    my_turn(&mut g).await.unwrap();
    let over = resign_and_wait(&mut g);
    assert_eq!(over["reason"], "resignation");
    assert_eq!(
        over["outcome"],
        json!({"type": "resignation", "winner": "black"})
    );
    assert_eq!(over["rated"], false);
}

#[tokio::test]
async fn a_game_against_a_strong_bot_ends_cleanly() {
    // The player (white) walks into a fool's mate; whatever the bot plays,
    // the game must still end cleanly through the normal path if it does mate.
    let (app, _) = new_app(HubConfig {
        bot_think_max: Duration::from_millis(300),
        ..cfg()
    });
    let mut g = guest(&app);
    solo(&mut g, 2000, "white");
    g.pick_nothing();
    let mut s = my_turn(&mut g).await.unwrap();
    let mut ended = None;
    // f3/g4 style moves: pick moves that weaken the king until mated or capped.
    for step in 0..40 {
        let moves = s["moves"].as_array().unwrap().clone();
        let weakening = ["f2f3", "g2g4", "e2e3", "h2h3"];
        let pick = moves
            .iter()
            .find(|m| {
                let name = format!(
                    "{}{}",
                    chessy_engine::square_name(m["from"].as_u64().unwrap() as u8),
                    chessy_engine::square_name(m["to"].as_u64().unwrap() as u8)
                );
                weakening.contains(&name.as_str())
            })
            .unwrap_or(&moves[step % moves.len()]);
        g.say(move_json(pick));
        match my_turn(&mut g).await {
            Ok(next) => s = next,
            Err(over) => {
                ended = Some(over);
                break;
            }
        }
    }
    if let Some(over) = ended {
        assert_eq!(over["rated"], false);
        assert_eq!(over["reward"], Value::Null);
        assert!([
            "checkmate",
            "stalemate",
            "repetition",
            "fifty_moves",
            "insufficient_material"
        ]
        .contains(&over["reason"].as_str().unwrap()));
    }
}

// ---- rematch and draws ---------------------------------------------------------

#[tokio::test]
async fn the_bot_accepts_a_rematch_with_colours_swapped() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    let first = solo(&mut g, 1100, "white");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    resign_and_wait(&mut g);
    g.clear();
    g.send(ClientMsg::RematchRequest);
    let ds = g.next("deck_select");
    assert_eq!(first["you"], "white");
    assert_eq!(ds["you"], "black", "colours swap");
    assert_eq!(ds["opponent"]["elo"], 1100);
    assert_eq!(ds["opponent"]["bot"], true);
    g.color = Some(Color::Black);
    g.pick_nothing();
    // The bot (white) opens the second game.
    let s = my_turn(&mut g).await.unwrap();
    assert_eq!(s["ply"], 1);
    resign_and_wait(&mut g);
    g.clear();
    g.send(ClientMsg::RematchRequest);
    assert_eq!(g.next("deck_select")["you"], "white");
}

#[tokio::test]
async fn no_rematch_without_a_finished_solo_game_or_after_leaving() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.send(ClientMsg::RematchRequest);
    assert_eq!(g.error_code(), "no_rematch");

    solo(&mut g, 1100, "white");
    g.pick_nothing();
    g.send(ClientMsg::RematchRequest);
    assert_eq!(g.error_code(), "no_rematch", "mid-game");
    resign_and_wait(&mut g);
    // Starting something else drops the chance.
    g.say(json!({"type": "solo_start", "elo": 900, "color": "white"}));
    g.next("deck_select");
    g.pick_nothing();
    resign_and_wait(&mut g);
    g.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    g.clear();
    g.send(ClientMsg::RematchRequest);
    assert_eq!(g.error_code(), "no_rematch");
}

#[tokio::test]
async fn the_bot_declines_an_early_draw() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 1000, "white");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    g.send(ClientMsg::OfferDraw);
    g.next("draw_declined");
    assert!(g.try_next("game_over").is_none());
    // One offer per ply, as in a duel.
    g.send(ClientMsg::OfferDraw);
    assert_eq!(g.error_code(), "draw_already_offered");
    // After a move the player may ask again.
    g.mv("e2", "e4");
    my_turn(&mut g).await.unwrap();
    g.send(ClientMsg::OfferDraw);
    g.next("draw_declined");
}

#[tokio::test]
async fn the_bot_accepts_a_draw_when_the_position_is_level_enough() {
    let (app, _) = new_app(HubConfig {
        bot_draw_min_plies: 0,
        bot_draw_window: 100_000,
        ..cfg()
    });
    let mut g = guest(&app);
    solo(&mut g, 1000, "white");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    g.send(ClientMsg::OfferDraw);
    let over = g.next("game_over");
    assert_eq!(over["reason"], "agreed_draw");
    assert_eq!(over["outcome"], json!({"type": "draw_agreed"}));
    assert_eq!(over["rated"], false);
    assert_eq!(over["reward"], Value::Null);
}

#[tokio::test]
async fn the_bot_refuses_a_draw_when_it_is_winning_or_losing() {
    use chessy_server::bot::accepts_draw;
    let mut pos = chessy_engine::Position::from_fen(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    )
    .unwrap();
    pos.ply = 50;
    assert!(accepts_draw(&pos, Color::Black, 40, 30), "equal position");
    assert!(!accepts_draw(&pos, Color::Black, 60, 30), "too early");
    // White is a queen up: Black will not take a draw, White's bot would.
    let mut up = chessy_engine::Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
    up.ply = 80;
    assert!(
        !accepts_draw(&up, Color::White, 40, 30),
        "the bot is winning"
    );
    assert!(
        !accepts_draw(&up, Color::Black, 40, 30),
        "the bot is losing"
    );
}

#[tokio::test]
async fn chat_is_ignored_in_solo() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 1000, "white");
    g.pick_nothing();
    g.clear();
    g.send(ClientMsg::Chat {
        text: "hello bot".into(),
    });
    // Only the echo; the bot is not a person and says nothing.
    let echo = g.next("chat");
    assert_eq!(echo["mine"], true);
    assert!(g.try_next("chat").is_none());
    assert!(g.try_next("error").is_none());
}

// ---- disconnects ---------------------------------------------------------------

#[tokio::test]
async fn a_player_who_leaves_forfeits_after_the_grace_period() {
    let (app, _) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(80),
        ..cfg()
    });
    let mut g = guest(&app);
    solo(&mut g, 1000, "white");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    let token = g.token.clone();
    app.disconnect(&g.id, g.conn);
    tokio::time::sleep(Duration::from_millis(250)).await;
    let mut back = Client::connect(&app, Some(token));
    assert_eq!(back.next("lobby")["status"]["type"], "idle");
    assert!(back.try_next("state").is_none());
    // And the player is free to start again.
    back.say(json!({"type": "solo_start", "elo": 1000, "color": "white"}));
    back.next("deck_select");
}

#[tokio::test]
async fn coming_back_within_the_grace_period_resumes_the_game() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 1000, "black");
    g.pick_nothing();
    my_turn(&mut g).await.unwrap();
    let token = g.token.clone();
    app.disconnect(&g.id, g.conn);
    let mut back = Client::connect(&app, Some(token));
    back.color = Some(Color::Black);
    let s = back.next("state");
    assert_eq!(s["you"], "black");
    assert_eq!(s["clock_enabled"], false);
    assert_eq!(s["opponent"]["bot"], true);
    // The game goes on.
    let moves = s["moves"].as_array().unwrap();
    back.say(move_json(&moves[0]));
    let s = my_turn(&mut back).await.unwrap();
    assert_eq!(s["ply"], 3);
}

#[tokio::test]
async fn a_friend_cannot_challenge_someone_in_a_solo_game() {
    let (app, store) = new_app(cfg());
    let mut a = account(&app, &store, "alice");
    let mut b = account(&app, &store, "bob");
    a.say(json!({"type": "friend_request", "username": "bob"}));
    b.say(json!({"type": "friend_respond", "username": "alice", "accept": true}));
    solo(&mut b, 1000, "white");
    a.clear();
    a.say(json!({"type": "challenge", "username": "bob"}));
    assert!(a.has_notice("friend_busy"));
}

// ---- the hub's own view: timers, stale results, levels --------------------------

struct Direct {
    hub: Hub,
    id: String,
    rx: tokio::sync::mpsc::UnboundedReceiver<ServerMsg>,
}

fn direct(config: HubConfig) -> Direct {
    let store = Store::open(":memory:").unwrap();
    let mut hub = Hub::new(store, config);
    let (tx, rx) = unbounded_channel();
    let (id, _) = hub.connect(None, tx).unwrap();
    Direct { hub, id, rx }
}

impl Direct {
    fn game_id(&mut self) -> String {
        let mut found = None;
        while let Ok(m) = self.rx.try_recv() {
            if let ServerMsg::DeckSelect { game_id, .. } = m {
                found = Some(game_id);
            }
        }
        found.expect("a deck_select was sent")
    }

    fn states(&mut self) -> Vec<u32> {
        let mut plies = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            if let ServerMsg::State(s) = m {
                plies.push(s.ply);
            }
        }
        plies
    }
}

fn bot_move_timer(hub: &mut Hub) -> Option<(Duration, String, u32)> {
    hub.take_timers().into_iter().find_map(|(d, t)| match t {
        Timer::BotMove { game_id, ply } => Some((d, game_id, ply)),
        _ => None,
    })
}

#[tokio::test]
async fn the_bot_move_timer_is_planned_with_a_delay_in_range_and_no_flag() {
    let mut d = direct(HubConfig::default());
    d.hub.solo_start(&d.id.clone(), 1000, SoloColor::White);
    let game = d.game_id();
    d.hub.select_deck(&d.id.clone(), vec![]);
    let timers = d.hub.take_timers();
    assert!(
        !timers.iter().any(|(_, t)| matches!(t, Timer::Flag { .. })),
        "no clock in solo"
    );
    assert!(
        !timers
            .iter()
            .any(|(_, t)| matches!(t, Timer::BotMove { .. })),
        "white to move: the human plays first"
    );
    let e2 = parse_square("e2").unwrap();
    let e4 = parse_square("e4").unwrap();
    d.hub.action(
        &d.id.clone(),
        Action::Move {
            from: e2,
            to: e4,
            promo: None,
        },
    );
    let (delay, id, ply) = bot_move_timer(&mut d.hub).expect("a bot move is planned");
    assert_eq!(id, game);
    assert_eq!(ply, 1);
    assert!(
        (Duration::from_millis(600)..=Duration::from_millis(1400)).contains(&delay),
        "delay {delay:?}"
    );
}

#[tokio::test]
async fn a_stale_bot_result_is_dropped() {
    let mut d = direct(cfg());
    let me = d.id.clone();
    d.hub.solo_start(&me, 800, SoloColor::White);
    let game = d.game_id();
    d.hub.select_deck(&me, vec![]);
    d.hub.action(
        &me,
        Action::Move {
            from: parse_square("e2").unwrap(),
            to: parse_square("e4").unwrap(),
            promo: None,
        },
    );
    let (_, _, ply) = bot_move_timer(&mut d.hub).unwrap();
    d.states();

    // A snapshot only exists for the ply it was planned for.
    assert!(d.hub.bot_job(&game, ply + 1).is_none());
    assert!(d.hub.bot_job(&game, ply - 1).is_none());
    let job = d.hub.bot_job(&game, ply).expect("current ply");
    let action = chessy_server::bot::think(&job);
    assert!(action.is_some());

    // Applied for the wrong ply: ignored, nothing is sent.
    d.hub.apply_bot_move(&game, ply - 1, action);
    d.hub.apply_bot_move(&game, ply + 5, action);
    assert!(d.states().is_empty(), "stale results change nothing");

    // Applied for the right ply: the bot moves once...
    d.hub.apply_bot_move(&game, ply, action);
    assert_eq!(d.states(), vec![2]);
    // ...and a duplicate of the same result is stale now.
    d.hub.apply_bot_move(&game, ply, action);
    assert!(d.states().is_empty());
}

#[tokio::test]
async fn a_bot_result_for_a_finished_game_is_dropped() {
    let mut d = direct(cfg());
    let me = d.id.clone();
    d.hub.solo_start(&me, 800, SoloColor::White);
    let game = d.game_id();
    d.hub.select_deck(&me, vec![]);
    d.hub.action(
        &me,
        Action::Move {
            from: parse_square("d2").unwrap(),
            to: parse_square("d4").unwrap(),
            promo: None,
        },
    );
    let (_, _, ply) = bot_move_timer(&mut d.hub).unwrap();
    let job = d.hub.bot_job(&game, ply).unwrap();
    let action = chessy_server::bot::think(&job);
    d.hub.resign(&me);
    d.states();
    d.hub.apply_bot_move(&game, ply, action);
    assert!(d.states().is_empty());
    assert!(d.hub.bot_job(&game, ply).is_none());
}

#[tokio::test]
async fn the_level_chosen_sets_the_search_strength() {
    let mut depths = Vec::new();
    for elo in [450, 900, 1300, 1700, 2100, 2700] {
        let mut d = direct(HubConfig::default());
        let me = d.id.clone();
        d.hub.solo_start(&me, elo, SoloColor::Black);
        let game = d.game_id();
        d.hub.select_deck(&me, vec![]);
        let (_, id, ply) = bot_move_timer(&mut d.hub).expect("the bot opens");
        assert_eq!((id.as_str(), ply), (game.as_str(), 0));
        let job = d.hub.bot_job(&game, 0).unwrap();
        assert_eq!(job.strength.elo, elo as i32);
        depths.push(job.strength.depth);
        assert!(job.max_think <= Duration::from_secs(3));
    }
    assert_eq!(depths, vec![1, 2, 3, 4, 5, 6]);
}

#[tokio::test]
async fn the_bot_search_does_not_hold_the_hub() {
    // A slow level still lets other players use the server while it thinks.
    let (app, _) = new_app(HubConfig {
        bot_delay_min: Duration::from_millis(1),
        bot_delay_max: Duration::from_millis(1),
        bot_think_max: Duration::from_millis(400),
        ..HubConfig::default()
    });
    let mut g = guest(&app);
    solo(&mut g, 2800, "black");
    g.pick_nothing();
    let mut other = guest(&app);
    let started = std::time::Instant::now();
    other.send(ClientMsg::CreateRoom { time: None });
    other.next("lobby");
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "the hub answered while the bot was thinking"
    );
    my_turn(&mut g).await.unwrap();
}

#[tokio::test]
async fn many_solo_games_at_once() {
    let (app, _): (Arc<App>, _) = new_app(cfg());
    let mut clients = Vec::new();
    for i in 0..6 {
        let mut c = guest(&app);
        solo(
            &mut c,
            400 + 300 * i,
            if i % 2 == 0 { "white" } else { "black" },
        );
        c.pick_nothing();
        clients.push(c);
    }
    for c in clients.iter_mut() {
        let s = my_turn(c).await.unwrap();
        let m = s["moves"][0].clone();
        c.say(move_json(&m));
        let s = my_turn(c).await.unwrap();
        assert!(s["ply"].as_u64().unwrap() >= 2);
    }
}

// ---- backing out of deck selection ---------------------------------------------

#[tokio::test]
async fn leaving_deck_selection_against_the_bot_returns_to_the_lobby() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    solo(&mut g, 1000, "white");
    g.send(ClientMsg::LeaveDeckSelect);
    assert_eq!(g.next("game_cancelled")["reason"], "you_left");
    assert_eq!(g.last("lobby")["status"]["type"], "idle");
    // The slot is free: another solo game can start straight away.
    solo(&mut g, 1000, "white");
}

#[tokio::test]
async fn leaving_deck_selection_puts_the_opponent_back_in_the_queue() {
    let (app, _) = new_app(cfg());
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
    a.next("deck_select");
    b.next("deck_select");
    a.send(ClientMsg::LeaveDeckSelect);
    assert_eq!(a.next("game_cancelled")["reason"], "you_left");
    assert_eq!(b.next("game_cancelled")["reason"], "opponent_left_requeued");
    assert_eq!(b.last("lobby")["status"]["type"], "queued");
    // A newcomer is matched with the waiting player.
    let mut c = guest(&app);
    c.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    c.next("deck_select");
    b.next("deck_select");
}

#[tokio::test]
async fn a_fog_skill_is_playable_in_solo_and_the_bot_plays_in_the_fog_too() {
    use chessy_engine::forge::{Effect, Graded, Rarity, SkillDef};
    use chessy_engine::SkillId;
    let store = Store::open(":memory:").unwrap();
    let graded = Graded {
        rarity: Rarity::Common,
        score: 1.0,
        cost: 1.0,
        tone: 1.0,
        redundant: false,
    };
    let fog = SkillId::Forged(
        store
            .insert_forged(&SkillDef::new(Effect::Fog { plies: 8 }), &graded)
            .unwrap(),
    );
    let mut hub = Hub::new(store.clone(), HubConfig::default());
    let (tx, mut rx) = unbounded_channel();
    let (me, _) = hub.connect(None, tx).unwrap();
    store.set_deck(&me, &[fog]).unwrap();
    hub.solo_start(&me, 800, SoloColor::White);
    hub.select_deck(&me, vec![fog]);
    while rx.try_recv().is_ok() {}

    hub.action(
        &me,
        Action::Skill {
            skill: fog,
            target: chessy_engine::SkillTarget::None,
        },
    );
    // The skill was not left out of the game: the fog is on.
    let mut fogged = false;
    while let Ok(m) = rx.try_recv() {
        if let ServerMsg::State(s) = m {
            fogged |= s
                .effects
                .iter()
                .any(|e| e.kind == chessy_engine::EffectKind::Fog);
            // The player no longer sees the bot's army, which is out of reach.
            assert!(
                s.board.iter().flatten().all(|p| p.color == Color::White),
                "the player still sees the bot"
            );
        }
    }
    assert!(fogged, "the fog skill did nothing");

    // The bot searches the board its own side sees: it does not see the player's army either.
    let (_, id, ply) = bot_move_timer(&mut hub).expect("the bot answers");
    let job = hub.bot_job(&id, ply).expect("a job");
    assert_eq!(
        job.game.pos.pieces(Color::White).count(),
        0,
        "the bot sees through the fog"
    );
    assert_eq!(job.game.pos.pieces(Color::Black).count(), 16);
}
