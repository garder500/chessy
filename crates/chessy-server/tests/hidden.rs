//! Hidden information must not leak through what a player is *offered*: the
//! legal moves and skill targets of a view are those of the position the
//! player sees (hidden enemy pieces and traps removed), and an action is
//! judged against that view first, then against the real position.

mod common;

use std::sync::Arc;

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

fn start(
    app: &Arc<App>,
    store: &Store,
    white_deck: &[SkillId],
    black_deck: &[SkillId],
) -> (Client, Client) {
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
    white.clear();
    black.clear();
    (white, black)
}

fn skill(c: &Client, skill: SkillId, target: SkillTarget) {
    c.send(ClientMsg::Action {
        action: Action::Skill { skill, target },
    });
}

fn events<'a>(state: &'a Value, ty: &str) -> Vec<&'a Value> {
    state["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == ty)
        .collect()
}

fn offers_move(state: &Value, from: &str, to: &str) -> bool {
    state["moves"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["from"] == sq(from) && m["to"] == sq(to))
}

fn targets_of(state: &Value, skill: &str) -> Vec<Value> {
    state["skill_options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["skill"] == skill)
        .map(|o| o["targets"].as_array().unwrap().clone())
        .unwrap_or_default()
}

fn piece_target(name: &str) -> Value {
    json!({"kind": "piece", "square": sq(name)})
}

fn square_target(name: &str) -> Value {
    json!({"kind": "square", "square": sq(name)})
}

fn white_ms(state: &Value) -> u64 {
    state["clock"]["white_ms"].as_u64().unwrap()
}

/// 1. e4 d5 2. Bc4, then black hides the d5 pawn. Returns white's state.
fn black_hides_d5(white_deck: &[SkillId]) -> (Arc<App>, Client, Client, Value) {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, white_deck, &[SkillId::Invisibility]);
    white.mv("e2", "e4");
    black.mv("d7", "d5");
    white.mv("f1", "c4");
    skill(
        &black,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("d5") },
    );
    let state = white.last("state");
    assert_eq!(state["to_move"], "white");
    assert!(state["board"][sq("d5") as usize].is_null());
    black.clear();
    (app, white, black, state)
}

#[tokio::test]
async fn legal_moves_are_those_of_the_board_the_player_sees() {
    let (_app, _white, _black, state) = black_hides_d5(&[SkillId::Freeze]);
    // The pawn is not on white's board: no pawn capture of it is offered...
    assert!(
        !offers_move(&state, "e4", "d5"),
        "capture of a hidden piece"
    );
    // ...the bishop's ray is not cut short by it...
    assert!(offers_move(&state, "c4", "e6"), "ray cut by a hidden piece");
    assert!(offers_move(&state, "c4", "f7"));
    // ...and the square looks empty, so it can be landed on.
    assert!(offers_move(&state, "c4", "d5"));
}

#[tokio::test]
async fn skill_targets_follow_the_view_too() {
    let (_app, _white, _black, state) = black_hides_d5(&[SkillId::Freeze, SkillId::Trap]);
    assert!(
        !targets_of(&state, "freeze").contains(&piece_target("d5")),
        "a hidden piece cannot be frozen on purpose"
    );
    // The square looks empty, so a trap can be offered on it.
    assert!(targets_of(&state, "trap").contains(&square_target("d5")));
}

#[tokio::test]
async fn an_enemy_trap_does_not_change_the_squares_offered_for_a_trap() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Trap], &[SkillId::Trap]);
    skill(
        &white,
        SkillId::Trap,
        SkillTarget::Square { square: sq("e5") },
    );
    let state = black.last("state");
    assert!(
        targets_of(&state, "trap").contains(&square_target("e5")),
        "white's secret trap must not remove a square from black's list"
    );
    assert_eq!(targets_of(&state, "trap").len(), 32, "every free square");
    // Using it works: both traps share the square.
    skill(
        &black,
        SkillId::Trap,
        SkillTarget::Square { square: sq("e5") },
    );
    let state = black.last("state");
    assert_eq!(state["traps"], json!([sq("e5")]));
    assert_eq!(state["to_move"], "white");
    let _ = white.last("state");
}

#[tokio::test]
async fn an_offered_action_the_real_board_refuses_costs_time_but_says_nothing() {
    let (_app, mut white, _black, state) = black_hides_d5(&[SkillId::Freeze]);
    let before = white_ms(&state);

    // Never offered: refused, free, no new state.
    white.mv("e4", "d5");
    let plain = white.next("error");
    assert_eq!(plain["code"], "illegal_action");
    assert!(white.try_next("state").is_none());

    // Offered (the ray looks open) but the hidden pawn is in the way.
    white.mv("c4", "e6");
    let blocked = white.next("error");
    assert_eq!(blocked, plain, "one message for every refusal");
    let after = white.last("state");
    assert_eq!(after["to_move"], "white", "nothing happened");
    assert!(after["board"][sq("d5") as usize].is_null(), "nothing shown");
    assert!(
        white_ms(&after) + 9_000 <= before,
        "probing is charged: {before} -> {}",
        white_ms(&after)
    );

    // Landing on the square captures the pawn: that is how it is found.
    white.mv("c4", "d5");
    let state = white.last("state");
    assert_eq!(state["to_move"], "black");
    assert_eq!(events(&state, "captured").len(), 1);
}

#[tokio::test]
async fn a_hidden_checker_is_unmasked_so_check_stays_playable() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Invisibility], &[SkillId::Freeze]);
    white.mv("e2", "e4");
    black.mv("f7", "f6");
    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("d1") },
    );
    let state = black.last("state");
    assert!(state["board"][sq("d1") as usize].is_null());
    black.mv("a7", "a6");
    white.clear();
    white.mv("d1", "h5");
    let state = black.last("state");
    assert_eq!(state["in_check"], true);
    // The piece that gives the check is shown, with its move...
    assert_eq!(state["board"][sq("h5") as usize]["kind"], "queen");
    assert_eq!(events(&state, "moved").len(), 1);
    // ...and only moves that answer the check are offered.
    assert!(offers_move(&state, "g7", "g6"));
    assert!(
        !offers_move(&state, "e8", "f7"),
        "f7 is covered by the queen"
    );
    assert!(!offers_move(&state, "a6", "a5"));
}

#[tokio::test]
async fn a_hidden_piece_that_does_not_check_stays_hidden() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Invisibility], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("d1") },
    );
    let state = black.last("state");
    assert_eq!(state["in_check"], false);
    assert!(state["board"][sq("d1") as usize].is_null());
    black.mv("a7", "a6");
    let _ = white.last("state");
    white.mv("e2", "e4");
    let state = black.last("state");
    assert!(state["board"][sq("d1") as usize].is_null());
}

#[tokio::test]
async fn the_history_sent_on_resume_keeps_secrets_secret() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Trap], &[SkillId::Trap]);
    skill(
        &white,
        SkillId::Trap,
        SkillTarget::Square { square: sq("e5") },
    );
    white.clear();
    black.clear();

    // The trapper still sees their own trap in the history ...
    app.disconnect(&white.id, white.conn);
    let mut white_back = Client::connect(&app, Some(white.token.clone()));
    let mine = white_back.next("state");
    let entry = &mine["history"][0];
    assert!(entry["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"] == "trap_set"));

    // ... the victim learns that a skill was used, not where, and sees no trap.
    app.disconnect(&black.id, black.conn);
    let mut black_back = Client::connect(&app, Some(black.token.clone()));
    let theirs = black_back.next("state");
    let events = theirs["history"][0]["events"].as_array().unwrap().clone();
    assert!(events.iter().all(|e| e["type"] != "trap_set"), "{events:?}");
    let used = events.iter().find(|e| e["type"] == "skill_used").unwrap();
    assert_eq!(used["target"], json!({"kind": "none"}));
}
