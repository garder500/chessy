//! The twenty skills added in v3 as seen through the hub: usage counters, free
//! actions, hidden information and the starter / reward pools.

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chessy_engine::{parse_square, Action, Color, SkillId, SkillKind, SkillTarget};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, RewardChoice};
use chessy_server::store::{classic_skills, Store};
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

fn sq(name: &str) -> u8 {
    parse_square(name).unwrap()
}

/// Two accounts matched in the ranked queue, white and black getting the given
/// decks and picking the given skills (unique ones come along on their own).
/// Returns `(white, black)` with the opening messages thrown away.
fn start_with(
    app: &Arc<App>,
    store: &Store,
    decks: [&[SkillId]; 2],
    picks: [&[SkillId]; 2],
) -> (Client, Client) {
    // Only rated games reward a skill: two accounts in the ranked queue.
    let mut a = account(app, store, "alice");
    let mut b = account(app, store, "bob");
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
    // The deck is read when the picks arrive, so it can be set after the match.
    store.set_deck(&white.id, decks[0]).unwrap();
    store.set_deck(&black.id, decks[1]).unwrap();
    white.pick(picks[0]);
    black.pick(picks[1]);
    white.clear();
    black.clear();
    (white, black)
}

/// Like `start_with`, bringing the first three classic skills of each deck.
fn start(
    app: &Arc<App>,
    store: &Store,
    white_deck: &[SkillId],
    black_deck: &[SkillId],
) -> (Client, Client) {
    let classic = |deck: &[SkillId]| -> Vec<SkillId> {
        deck.iter()
            .copied()
            .filter(|s| s.kind() == SkillKind::Classic)
            .take(3)
            .collect()
    };
    let (w, b) = (classic(white_deck), classic(black_deck));
    start_with(app, store, [white_deck, black_deck], [&w, &b])
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

fn board_piece(state: &Value, name: &str) -> Value {
    state["board"][sq(name) as usize].clone()
}

fn effect_kinds(state: &Value) -> Vec<String> {
    state["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

fn skill_slot(state: &Value, id: &str) -> Value {
    state["my_skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["skill"] == id)
        .unwrap_or_else(|| panic!("no {id} in {}", state["my_skills"]))
        .clone()
}

// ---- pools -----------------------------------------------------------------

#[test]
fn the_catalogue_has_seven_unique_and_twenty_classic_skills() {
    assert_eq!(SkillId::ALL.len(), 27);
    assert_eq!(classic_skills().len(), 20);
    assert_eq!(
        SkillId::ALL
            .into_iter()
            .filter(|s| s.kind() == SkillKind::Unique)
            .count(),
        7
    );
}

#[test]
fn starter_decks_are_three_distinct_classic_skills_from_the_whole_pool() {
    let store = Store::open(":memory:").unwrap();
    let mut seen = HashSet::new();
    for _ in 0..60 {
        let (id, _) = store.create_player().unwrap();
        let deck = store.deck(&id).unwrap();
        assert_eq!(deck.len(), 3);
        assert_eq!(deck.iter().collect::<HashSet<_>>().len(), 3, "distinct");
        assert!(deck.iter().all(|s| s.kind() == SkillKind::Classic));
        seen.extend(deck);
    }
    assert!(
        seen.len() > 12,
        "the new classic skills are handed out too: {seen:?}"
    );
}

#[tokio::test]
async fn new_classic_skills_can_be_picked_and_unique_ones_still_cannot() {
    let (app, store) = new_app(HubConfig::default());
    let mut a = guest(&app);
    let b = guest(&app);
    store
        .set_deck(
            &a.id,
            &[
                SkillId::Tornado,
                SkillId::Bench,
                SkillId::Godhelp,
                SkillId::Mirage,
            ],
        )
        .unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let _ = a.next("deck_select");
    a.pick(&[SkillId::Mirage]);
    assert_eq!(a.error_code(), "invalid_deck");
    a.pick(&[SkillId::Tornado, SkillId::Bench, SkillId::Godhelp]);
    assert!(a.try_next("error").is_none());
    b.pick_nothing();
    let state = a.last("state");
    let mine: Vec<&str> = state["my_skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["skill"].as_str().unwrap())
        .collect();
    assert_eq!(mine, ["tornado", "bench", "godhelp", "mirage"]);
}

#[tokio::test]
async fn random_rewards_forge_skills_that_are_stored_and_owned() {
    let mut gained = HashSet::new();
    for _ in 0..5 {
        let (app, store) = new_app(HubConfig::default());
        // Nobody brings skills, so the fool's mate is final.
        let (white, mut black) = start_with(
            &app,
            &store,
            [&[SkillId::Teleportation], &[SkillId::Imune]],
            [&[], &[]],
        );
        fools_mate(&white, &black);
        let _ = black.next("game_over");
        black.send(ClientMsg::RewardChoice {
            choice: RewardChoice::Random { replace: None },
        });
        let update = black.wait_for("deck_update").await;
        let skill: SkillId = serde_json::from_value(update["gained"].clone()).unwrap();
        let SkillId::Forged(n) = skill else {
            panic!("a random reward is a forged skill, got {skill:?}");
        };
        let view = store.forged_views(&[n]).unwrap().pop().expect("stored");
        // A legendary skill is unique: it belongs to its forger alone.
        assert_eq!(view.unique, skill.kind() == SkillKind::Unique);
        if view.unique {
            assert_eq!(
                store.unique_owner(skill).unwrap().as_deref(),
                Some(black.id.as_str())
            );
        }
        gained.insert(view.rarity);
    }
    assert!(!gained.is_empty());
}

#[tokio::test]
async fn forged_skills_are_listed_by_the_rest_api_and_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!("chessy-forged-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("db.sqlite");
    let path = path.to_str().unwrap();
    let id = {
        let store = chessy_server::store::Store::open(path).unwrap();
        let mut rng = chessy_engine::ai::Rng::new(77);
        let forged = chessy_engine::forge::generate::forge(
            &mut rng,
            chessy_engine::forge::Rarity::Common,
            &HashSet::new(),
            chessy_engine::forge::generate::Budget {
                attempts: 5,
                positions: 4,
            },
        );
        let n = store.insert_forged(&forged.def, &forged.graded).unwrap();
        // The same definition keeps its id.
        assert_eq!(store.insert_forged(&forged.def, &forged.graded).unwrap(), n);
        assert!(store
            .forged_signatures()
            .unwrap()
            .contains(&forged.def.signature()));
        n
    };
    // Reopening registers the stored definitions again.
    let store = chessy_server::store::Store::open(path).unwrap();
    assert!(chessy_engine::forge::registry::is_registered(id));
    let views = store.forged_views(&[id, 9_999_999]).unwrap();
    assert_eq!(views.len(), 1);
    // The client animates a forged skill from its bricks.
    let json = serde_json::to_value(&views[0]).unwrap();
    assert_eq!(json["bricks"]["action"], views[0].bricks.action);
    assert!(json["bricks"]["permanent"].is_boolean());
    // The client says the name and the description in its own language, from their parts.
    assert!(json["name_parts"]["noun"].is_u64());
    assert!(json["name_parts"]["proper"]
        .as_str()
        .is_some_and(|p| !p.is_empty()));
    assert!(json["bricks"]["selector_kinds"].is_array());
    assert!(json["bricks"]["side"].is_string());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- uses and free actions ---------------------------------------------------

#[tokio::test]
async fn my_skills_report_uses_and_max_uses() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, _black) = start(
        &app,
        &store,
        &[SkillId::Mind, SkillId::Freeze],
        &[SkillId::Imune],
    );
    white.mv("e2", "e4");
    let state = white.last("state");
    assert_eq!(
        skill_slot(&state, "mind"),
        json!({"skill": "mind", "used": false, "uses": 0, "max_uses": 3})
    );
    assert_eq!(
        skill_slot(&state, "freeze"),
        json!({"skill": "freeze", "used": false, "uses": 0, "max_uses": 1})
    );
}

#[tokio::test]
async fn mind_reading_keeps_the_turn_and_counts_its_uses() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Mind], &[SkillId::Freeze]);
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.clear();
    black.clear();

    for n in 1..=3u64 {
        skill(&white, SkillId::Mind, SkillTarget::None);
        let state = white.last("state");
        assert_eq!(state["to_move"], "white", "no turn passed");
        assert_eq!(state["ply"], 2);
        assert_eq!(skill_slot(&state, "mind")["uses"], n);
        assert_eq!(skill_slot(&state, "mind")["used"], n == 3);
        assert_eq!(
            events(&state, "best_move").len(),
            1,
            "the player gets the hint"
        );
        assert_eq!(state["outcome"], json!({"type": "ongoing"}));
        // The opponent learns that the skill was used but not the move.
        let theirs = black.last("state");
        assert!(events(&theirs, "best_move").is_empty());
        assert_eq!(events(&theirs, "skill_used").len(), 1);
    }
    // A fourth use is refused, a move still goes through.
    skill(&white, SkillId::Mind, SkillTarget::None);
    assert_eq!(white.error_code(), "illegal_action");
    white.mv("g1", "f3");
    assert_eq!(white.last("state")["to_move"], "black");
}

#[tokio::test]
async fn free_actions_leave_the_clock_running_for_the_same_player() {
    let (app, store) = new_app(HubConfig {
        clock_initial: Duration::from_secs(60),
        clock_increment: Duration::from_secs(5),
        ..HubConfig::default()
    });
    let (mut white, _black) = start(&app, &store, &[SkillId::Mind], &[SkillId::Freeze]);
    skill(&white, SkillId::Mind, SkillTarget::None);
    let state = white.last("state");
    assert_eq!(state["clock"]["running"], "white");
    assert!(
        state["clock"]["white_ms"].as_u64().unwrap() <= 60_000,
        "no increment for a free action: {}",
        state["clock"]
    );
    white.mv("e2", "e4");
    let state = white.last("state");
    assert_eq!(state["clock"]["running"], "black");
    assert!(state["clock"]["white_ms"].as_u64().unwrap() > 64_000);
}

#[tokio::test]
async fn mind_control_borrows_a_piece_for_the_turn() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Control], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Control,
        SkillTarget::Piece { square: sq("a7") },
    );
    let state = white.last("state");
    assert_eq!(state["to_move"], "white");
    assert_eq!(state["ply"], 0);
    assert_eq!(board_piece(&state, "a7")["color"], "white", "ours for now");
    assert!(state["effects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "color_loan" && e["orig_color"] == "black"));
    assert_eq!(board_piece(&black.last("state"), "a7")["color"], "white");

    white.mv("e2", "e4");
    let state = white.last("state");
    assert_eq!(board_piece(&state, "a7")["color"], "black", "it goes back");
    assert_eq!(events(&state, "loan_ended").len(), 1);
    assert_eq!(state["to_move"], "black");
}

#[tokio::test]
async fn mind_control_can_answer_a_would_be_checkmate() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Control], &[SkillId::Freeze]);
    fools_mate(&white, &black);
    let state = white.last("state");
    assert_eq!(state["outcome"], json!({"type": "ongoing"}));
    assert_eq!(state["in_check"], true);
    assert!(
        state["moves"].as_array().unwrap().is_empty(),
        "no plain move saves it"
    );
    let options = state["skill_options"].as_array().unwrap();
    let control = options.iter().find(|o| o["skill"] == "control").unwrap();
    assert!(control["targets"]
        .as_array()
        .unwrap()
        .contains(&json!({"kind": "piece", "square": sq("h4")})));
    let _ = black.last("state");

    skill(
        &white,
        SkillId::Control,
        SkillTarget::Piece { square: sq("h4") },
    );
    let state = white.last("state");
    assert_eq!(state["in_check"], false);
    assert!(!state["moves"].as_array().unwrap().is_empty());
}

// ---- hidden information -----------------------------------------------------

#[tokio::test]
async fn invisible_pieces_are_hidden_from_the_opponent_only() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(
        &app,
        &store,
        &[SkillId::Invisibility, SkillId::Forcefield],
        &[SkillId::Freeze],
    );
    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("e2") },
    );
    let mine = white.last("state");
    let theirs = black.last("state");
    assert_eq!(board_piece(&mine, "e2")["kind"], "pawn");
    assert!(board_piece(&theirs, "e2").is_null(), "hidden from black");
    assert!(effect_kinds(&mine).contains(&"invisible".to_string()));
    assert!(!effect_kinds(&theirs).contains(&"invisible".to_string()));
    // Black still learns that a skill was used, but not on what.
    let used = events(&theirs, "skill_used");
    assert_eq!(used.len(), 1);
    assert_eq!(used[0]["target"], json!({"kind": "none"}));
    assert!(events(&theirs, "effect_added").is_empty());
    assert_eq!(events(&mine, "effect_added").len(), 1);
    assert_eq!(events(&mine, "skill_used")[0]["target"]["kind"], "piece");

    // Other effects on the hidden piece stay hidden as well.
    black.mv("e7", "e5");
    skill(
        &white,
        SkillId::Forcefield,
        SkillTarget::Piece { square: sq("e2") },
    );
    let theirs = black.last("state");
    assert!(!effect_kinds(&theirs).contains(&"forcefield".to_string()));
    assert!(effect_kinds(&white.last("state")).contains(&"forcefield".to_string()));
}

#[tokio::test]
async fn the_moves_of_an_invisible_piece_are_not_reported_and_it_reappears_later() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Invisibility], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Invisibility,
        SkillTarget::Piece { square: sq("e2") },
    );
    black.mv("a7", "a6");
    white.mv("e2", "e4");
    let theirs = black.last("state");
    assert!(events(&theirs, "moved").is_empty(), "{theirs}");
    assert!(board_piece(&theirs, "e4").is_null());
    assert_eq!(events(&white.last("state"), "moved").len(), 1);
    // Four plies after the skill the pawn shows again.
    black.mv("a6", "a5");
    let theirs = black.last("state");
    assert_eq!(board_piece(&theirs, "e4")["kind"], "pawn");
    assert!(!effect_kinds(&theirs).contains(&"invisible".to_string()));
}

#[tokio::test]
async fn traps_are_secret_until_they_spring() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Trap], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Trap,
        SkillTarget::Square { square: sq("e5") },
    );
    let mine = white.last("state");
    let theirs = black.last("state");
    assert_eq!(mine["traps"], json!([sq("e5")]));
    assert_eq!(theirs["traps"], json!([]));
    assert!(events(&theirs, "trap_set").is_empty());
    assert_eq!(events(&mine, "trap_set").len(), 1);
    assert_eq!(
        events(&theirs, "skill_used")[0]["target"],
        json!({"kind": "none"})
    );

    black.mv("e7", "e5");
    let mine = white.last("state");
    let theirs = black.last("state");
    for state in [&mine, &theirs] {
        assert_eq!(events(state, "trap_sprung").len(), 1);
        assert_eq!(events(state, "trap_sprung")[0]["square"], sq("e5"));
        assert_eq!(state["traps"], json!([]), "the trap is used up");
    }
    // The pawn is frozen on the trapped square, visible to everybody.
    assert!(effect_kinds(&theirs).contains(&"frozen".to_string()));
}

#[tokio::test]
async fn the_bench_is_private_and_the_piece_comes_back() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Bench], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Bench,
        SkillTarget::Piece { square: sq("b1") },
    );
    let mine = white.last("state");
    let theirs = black.last("state");
    assert!(board_piece(&mine, "b1").is_null());
    assert!(board_piece(&theirs, "b1").is_null());
    assert_eq!(mine["benched"].as_array().unwrap().len(), 1);
    assert_eq!(mine["benched"][0]["kind"], "knight");
    assert_eq!(theirs["benched"], json!([]));

    black.mv("e7", "e5");
    let mine = white.last("state");
    assert_eq!(board_piece(&mine, "b1")["kind"], "knight");
    assert_eq!(mine["benched"], json!([]));
    assert_eq!(events(&mine, "unbenched").len(), 1);
}

#[tokio::test]
async fn terrain_is_public_and_not_listed_among_the_piece_effects() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(&app, &store, &[SkillId::Geomancy], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Geomancy,
        SkillTarget::Square { square: sq("e4") },
    );
    for state in [white.last("state"), black.last("state")] {
        let terrain = state["terrain"].as_array().unwrap();
        assert_eq!(terrain.len(), 3);
        let squares: HashSet<u64> = terrain
            .iter()
            .map(|t| t["square"].as_u64().unwrap())
            .collect();
        assert_eq!(
            squares,
            HashSet::from([sq("d4") as u64, sq("e4") as u64, sq("f4") as u64])
        );
        assert!(terrain
            .iter()
            .all(|t| t["owner"] == "white" && t["expires_at"] == 6));
        assert!(!effect_kinds(&state).contains(&"terrain".to_string()));
        assert_eq!(events(&state, "terrain").len(), 1);
    }
}

// ---- actions over the wire ---------------------------------------------------

#[tokio::test]
async fn new_target_shapes_travel_as_json() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, _black) = start(&app, &store, &[SkillId::Morph], &[SkillId::Freeze]);
    skill(
        &white,
        SkillId::Morph,
        SkillTarget::Piece { square: sq("e7") },
    );
    assert_eq!(
        white.error_code(),
        "illegal_action",
        "wrong shape for this skill"
    );
    // `kind` is the tag of a target, so the piece type of a Spawn is called `piece`.
    white.say(json!({
        "type": "action",
        "action": {
            "type": "skill", "skill": "morph",
            "target": {"kind": "spawn", "square": sq("e7"), "piece": "knight"}
        }
    }));
    let state = white.last("state");
    assert_eq!(board_piece(&state, "e7")["kind"], "knight");
    assert_eq!(events(&state, "transformed")[0]["kind"], "knight");
}

#[tokio::test]
async fn skill_options_list_the_new_target_shapes() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start(
        &app,
        &store,
        &[SkillId::Trap, SkillId::Morph],
        &[SkillId::Freeze],
    );
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    let state = white.last("state");
    let options = state["skill_options"].as_array().unwrap();
    let trap = options.iter().find(|o| o["skill"] == "trap").unwrap();
    assert!(trap["targets"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["kind"] == "square"));
    let morph = options.iter().find(|o| o["skill"] == "morph").unwrap();
    assert!(morph["targets"].as_array().unwrap().contains(&json!(
        {"kind": "spawn", "square": sq("e5"), "piece": "queen"}
    )));
    assert!(black.last("state")["skill_options"]
        .as_array()
        .unwrap()
        .is_empty());
}

// ---- fog ---------------------------------------------------------------------

fn pieces_on_board(state: &Value, color: &str) -> usize {
    state["board"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["color"] == color)
        .count()
}

#[tokio::test]
async fn fog_hides_the_far_enemy_pieces_from_both_players() {
    use chessy_engine::forge::{Effect, SkillDef};
    let (app, store) = new_app(HubConfig::default());
    let graded = chessy_engine::forge::Graded {
        rarity: chessy_engine::forge::Rarity::Common,
        score: 1.0,
        cost: 1.0,
        tone: 1.0,
        redundant: false,
    };
    let id = SkillId::Forged(
        store
            .insert_forged(&SkillDef::new(Effect::Fog { plies: 6 }), &graded)
            .unwrap(),
    );
    let (mut white, mut black) = start(&app, &store, &[id], &[SkillId::Imune]);
    skill(&white, id, SkillTarget::None);
    let w = white.last("state");
    let b = black.last("state");
    // Everything starts far away: each side sees only its own army.
    assert_eq!(pieces_on_board(&w, "white"), 16, "{w}");
    assert_eq!(
        pieces_on_board(&w, "black"),
        0,
        "white still sees black: {w}"
    );
    assert_eq!(pieces_on_board(&b, "black"), 16, "{b}");
    assert_eq!(
        pieces_on_board(&b, "white"),
        0,
        "black still sees white: {b}"
    );
}
