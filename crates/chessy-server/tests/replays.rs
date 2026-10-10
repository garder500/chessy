//! Recording of every game and the replay endpoints (docs/spec-v4.md §1, §2):
//! `GET /api/me/games`, `GET /api/games/{id}`, `.../analysis`, `.../explore`.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use chessy_engine::{Action, Outcome, SkillId};
use chessy_server::games_store::GameKind;
use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use chessy_server::store::{GameRecord, Store};
use chessy_server::App;
use common::*;
use serde_json::{json, Value};

fn setup() -> (Arc<App>, Store, Api) {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    (app, store, api)
}

/// Two guests matched in the friendly queue, as `(white, black)`.
fn friendly_game(app: &Arc<App>) -> (Client, Client) {
    let a = guest(app);
    let b = guest(app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    into_game(a, b)
}

/// Plays `moves` (alternating, white first) as `(from, to)` pairs.
fn play(white: &Client, black: &Client, moves: &[(&str, &str)]) {
    for (i, (from, to)) in moves.iter().enumerate() {
        let mover = if i % 2 == 0 { white } else { black };
        mover.mv(from, to);
    }
}

/// The id of `c`'s most recent game.
async fn latest_game(api: &Api, c: &Client) -> String {
    let (status, list) = api.get("/api/me/games", Some(&c.token)).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list["games"][0]["game_id"].as_str().unwrap().to_string()
}

async fn replay_of(api: &Api, id: &str) -> Value {
    let (status, v) = api.get(&format!("/api/games/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v
}

fn notations(replay: &Value) -> Vec<String> {
    replay["moves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["notation"].as_str().unwrap().to_string())
        .collect()
}

// ---- recording ---------------------------------------------------------------

#[tokio::test]
async fn a_ranked_duel_is_recorded_with_its_elo_movement() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (white, black) = into_game(a, b);
    fools_mate(&white, &black);

    let (status, list) = api.get("/api/me/games", Some(&black.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["total"], 1);
    let g = &list["games"][0];
    assert_eq!(g["kind"], "duel");
    assert_eq!(g["rated"], true);
    assert_eq!(g["color"], "black");
    assert_eq!(g["result"], "win");
    assert_eq!(g["reason"], "checkmate");
    assert_eq!(g["plies"], 4);
    assert_eq!(g["elo_delta"], 20);
    assert_eq!(g["white"]["bot"], false);
    assert_eq!(g["white"]["elo"], 1200, "the rating before the game");
    assert!(g["white"]["username"].is_string() && g["black"]["username"].is_string());
    assert!(g["at"].as_str().unwrap().ends_with('Z'));

    let (_, theirs) = api.get("/api/me/games", Some(&white.token)).await;
    assert_eq!(theirs["games"][0]["result"], "loss");
    assert_eq!(theirs["games"][0]["elo_delta"], -20);
    assert_eq!(theirs["games"][0]["color"], "white");
    assert_eq!(theirs["games"][0]["game_id"], g["game_id"]);

    // The public record is the same game.
    let replay = replay_of(&api, g["game_id"].as_str().unwrap()).await;
    assert_eq!(replay["kind"], "duel");
    assert_eq!(replay["result"]["reason"], "checkmate");
    assert_eq!(replay["rated"], true);
}

#[tokio::test]
async fn friendly_games_keep_account_ratings_but_move_none() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    a.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    let (white, black) = into_game(a, b);
    fools_mate(&white, &black);
    let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
    let g = &list["games"][0];
    assert_eq!(g["kind"], "duel");
    assert_eq!(g["rated"], false);
    assert_eq!(g["elo_delta"], Value::Null);
    assert_eq!(g["black"]["elo"], 1200);
    assert_eq!(store.me(&white.id).unwrap().unwrap().games, 0);
}

#[tokio::test]
async fn guests_are_recorded_without_name_or_rating() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
    let g = &list["games"][0];
    assert_eq!(
        g["white"],
        json!({"username": null, "elo": null, "bot": false})
    );
    assert_eq!(
        g["black"],
        json!({"username": null, "elo": null, "bot": false})
    );
}

#[tokio::test]
async fn rooms_and_challenges_have_their_own_kind() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    let mut a = a;
    a.send(ClientMsg::CreateRoom { time: None });
    let code = a.last("lobby")["status"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    b.send(ClientMsg::JoinRoom { code });
    let (white, black) = into_game(a, b);
    fools_mate(&white, &black);
    let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
    assert_eq!(list["games"][0]["kind"], "room");
    assert_eq!(list["games"][0]["rated"], false);

    // A challenge between friends.
    let (mut a, mut b) = (white, black);
    a.clear();
    b.clear();
    let an = store.me(&a.id).unwrap().unwrap().username.unwrap();
    let bn = store.me(&b.id).unwrap().unwrap().username.unwrap();
    a.say(json!({"type": "friend_request", "username": bn}));
    b.say(json!({"type": "friend_respond", "username": an, "accept": true}));
    a.clear();
    b.clear();
    a.say(json!({"type": "challenge", "username": bn}));
    b.say(json!({"type": "challenge_respond", "username": an, "accept": true}));
    let (white, black) = into_game(a, b);
    fools_mate(&white, &black);
    let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["games"][0]["kind"], "challenge");
    assert_eq!(list["games"][1]["kind"], "room");
}

#[tokio::test]
async fn a_rematch_keeps_the_kind_of_the_game() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    white.send(ClientMsg::RematchRequest);
    black.send(ClientMsg::RematchRequest);
    // Colours swap; the rematch is a new game of the same kind.
    let (w2, b2) = into_game(white, black);
    fools_mate(&w2, &b2);
    let (_, list) = api.get("/api/me/games", Some(&w2.token)).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["games"][0]["kind"], "duel");
    assert_ne!(list["games"][0]["game_id"], list["games"][1]["game_id"]);
}

#[tokio::test]
async fn a_solo_game_is_recorded_but_never_public() {
    let (app, store, api) = setup();
    let mut a = account(&app, &store, "alice");
    a.say(json!({"type": "solo_start", "elo": 900, "color": "white"}));
    a.next("deck_select");
    a.pick_nothing();
    a.mv("e2", "e4");
    a.send(ClientMsg::Resign);
    let _ = a.next("game_over");

    let (_, list) = api.get("/api/me/games", Some(&a.token)).await;
    assert_eq!(list["total"], 1);
    let g = &list["games"][0];
    assert_eq!(g["kind"], "solo");
    assert_eq!(g["rated"], false);
    assert_eq!(g["color"], "white");
    assert_eq!(g["result"], "loss");
    assert_eq!(g["reason"], "resignation");
    assert_eq!(g["elo_delta"], Value::Null);
    assert_eq!(g["white"]["username"], "alice");
    assert_eq!(g["white"]["bot"], false);
    assert_eq!(
        g["black"],
        json!({"username": "Sage", "elo": 900, "bot": true})
    );

    // Not in the public profile, the counters or the ranking.
    let (_, profile) = api.get("/api/players/alice", None).await;
    assert_eq!(profile["recent"], json!([]));
    assert_eq!(profile["games"], 0);
    let (_, board) = api.get("/api/leaderboard", None).await;
    assert_eq!(board["entries"][0]["games"], 0);
    assert_eq!(board["entries"][0]["elo"], 1200);

    // The player replays it; the replay has the bot's moves too.
    let id = g["game_id"].as_str().unwrap();
    let (status, replay) = api.get(&format!("/api/games/{id}"), Some(&a.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["kind"], "solo");
    assert!(replay["plies"].as_u64().unwrap() >= 1);
    assert_eq!(replay["moves"][0]["notation"], "e4");
}

#[tokio::test]
async fn a_game_cancelled_before_it_starts_is_not_recorded() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    a.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: Some(false),
        time: None,
    });
    // Nobody has chosen a deck yet: leaving cancels the game.
    a.send(ClientMsg::Resign);
    let (_, list) = api.get("/api/me/games", Some(&a.token)).await;
    assert_eq!(list["total"], 0);
    assert_eq!(list["games"], json!([]));
    let (_, list) = api.get("/api/me/games", Some(&b.token)).await;
    assert_eq!(list["total"], 0);
}

#[tokio::test]
async fn my_games_pages_newest_first_and_needs_a_token() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    let c = account(&app, &store, "carol");
    let none: Vec<Action> = Vec::new();
    for (i, (w, bl)) in [(&a, &b), (&b, &a), (&a, &c), (&c, &b), (&a, &b)]
        .iter()
        .enumerate()
    {
        store
            .record_game(&GameRecord {
                id: &format!("g{i}"),
                white: &w.id,
                black: &bl.id,
                outcome: &Outcome::Stalemate,
                reason: "stalemate",
                plies: 0,
                rated: false,
                started_unix: 0,
                kind: GameKind::Room,
                loadouts: &[vec![], vec![]],
                actions: &none,
                solo_elo: None,
                time_control: None,
            })
            .unwrap();
    }
    let (_, all) = api.get("/api/me/games", Some(&a.token)).await;
    assert_eq!(all["total"], 4);
    let ids = |v: &Value| -> Vec<String> {
        v["games"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["game_id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(ids(&all), ["g4", "g2", "g1", "g0"]);
    let (_, page) = api
        .get("/api/me/games?limit=2&offset=1", Some(&a.token))
        .await;
    assert_eq!(page["total"], 4);
    assert_eq!(ids(&page), ["g2", "g1"]);
    let (_, last) = api
        .get("/api/me/games?limit=2&offset=3", Some(&a.token))
        .await;
    assert_eq!(ids(&last), ["g0"]);
    let (_, beyond) = api
        .get("/api/me/games?limit=2&offset=40", Some(&a.token))
        .await;
    assert_eq!(beyond["games"], json!([]));
    // A draw is a draw for both.
    assert_eq!(all["games"][0]["result"], "draw");
    // Carol only sees hers.
    let (_, carol) = api.get("/api/me/games", Some(&c.token)).await;
    assert_eq!(ids(&carol), ["g3", "g2"]);

    let (status, v) = api.get("/api/me/games", None).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("unauthorized"))
    );
    let (status, _) = api.get("/api/me/games", Some("not-a-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, v) = api.get("/api/me/games?limit=abc", Some(&a.token)).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_request"))
    );
}

// ---- replay -------------------------------------------------------------------

#[tokio::test]
async fn a_replay_has_one_frame_more_than_it_has_plies_and_notation() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    let id = latest_game(&api, &white).await;
    let replay = replay_of(&api, &id).await;

    assert_eq!(replay["game_id"], id);
    assert_eq!(replay["plies"], 4);
    assert_eq!(notations(&replay), ["f3", "e5", "g4", "Dh4#"]);
    let moves = replay["moves"].as_array().unwrap();
    assert_eq!(
        moves
            .iter()
            .map(|m| m["ply"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(moves[0]["color"], "white");
    assert_eq!(moves[3]["color"], "black");
    assert_eq!(moves[3]["action"]["type"], "move");

    let frames = replay["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f["ply"], i);
        assert_eq!(f["board"].as_array().unwrap().len(), 64);
    }
    let first = &frames[0];
    assert_eq!(first["to_move"], "white");
    assert_eq!(first["in_check"], false);
    assert_eq!(first["events"], json!([]));
    assert_eq!(first["outcome"]["type"], "ongoing");
    assert_eq!(first["board"][4]["kind"], "king");
    assert_eq!(first["board"][20], Value::Null);
    let last = &frames[4];
    assert_eq!(last["to_move"], "white");
    assert_eq!(last["in_check"], true);
    assert_eq!(
        last["outcome"],
        json!({"type": "checkmate", "winner": "black"})
    );
    assert_eq!(replay["result"]["outcome"], last["outcome"]);
    assert_eq!(replay["result"]["reason"], "checkmate");
    assert!(last["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"] == "moved" && e["to"] == 31));
    assert_eq!(replay["loadouts"], json!({"white": [], "black": []}));
    assert_eq!(replay["white"]["bot"], false);
}

#[tokio::test]
async fn a_resignation_shows_in_the_last_frame_only() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    play(&white, &black, &[("e2", "e4"), ("e7", "e5")]);
    white.send(ClientMsg::Resign);
    let id = latest_game(&api, &white).await;
    let replay = replay_of(&api, &id).await;
    let frames = replay["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[1]["outcome"]["type"], "ongoing");
    assert_eq!(
        frames[2]["outcome"],
        json!({"type": "resignation", "winner": "black"})
    );
    assert_eq!(replay["result"]["reason"], "resignation");
    assert_eq!(notations(&replay), ["e4", "e5"]);
}

#[tokio::test]
async fn a_game_without_a_single_action_still_has_its_start_frame() {
    let (app, _, api) = setup();
    let (white, _black) = friendly_game(&app);
    white.send(ClientMsg::Resign);
    let id = latest_game(&api, &white).await;
    let replay = replay_of(&api, &id).await;
    assert_eq!(replay["plies"], 0);
    assert_eq!(replay["frames"].as_array().unwrap().len(), 1);
    assert_eq!(replay["frames"][0]["outcome"]["type"], "resignation");
    assert_eq!(replay["moves"], json!([]));
}

#[tokio::test]
async fn real_notation_covers_captures_castling_and_disambiguation() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    play(
        &white,
        &black,
        &[
            ("e2", "e4"),
            ("d7", "d5"),
            ("e4", "d5"), // exd5
            ("g8", "f6"),
            ("g1", "f3"),
            ("c8", "g4"),
            ("f1", "e2"),
            ("e7", "e6"),
            ("e1", "g1"), // O-O
            ("b8", "d7"),
            ("b1", "c3"),
            ("f8", "e7"),
            ("d5", "e6"), // dxe6
            ("g4", "f3"), // Fxf3
            ("e2", "f3"), // Fxf3
        ],
    );
    white.send(ClientMsg::Resign);
    let id = latest_game(&api, &white).await;
    let replay = replay_of(&api, &id).await;
    assert_eq!(
        notations(&replay),
        [
            "e4", "d5", "exd5", "Cf6", "Cf3", "Fg4", "Fe2", "e6", "O-O", "Cbd7", "Cc3", "Fe7",
            "dxe6", "Fxf3", "Fxf3"
        ]
    );
}

#[tokio::test]
async fn an_unknown_game_is_not_found() {
    let (_, _, api) = setup();
    for path in ["/api/games/nope", "/api/games/nope/analysis"] {
        let (status, v) = api.get(path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(v["error"], "not_found");
    }
    let (status, v) = api
        .post("/api/games/nope/explore", json!({"ply": 0, "line": []}))
        .await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
}

// ---- faithful replay of a real game with skills ------------------------------

/// Picks the next action from the mover's `state`: a skill now and then,
/// otherwise a move, all as a function of `n` so runs are repeatable.
fn choose(state: &Value, n: usize) -> Value {
    let skills = state["skill_options"].as_array().unwrap();
    if n % 3 == 1 && !skills.is_empty() {
        let option = &skills[(n / 3) % skills.len()];
        let targets = option["targets"].as_array().unwrap();
        let target = &targets[(n * 7) % targets.len()];
        return json!({"type": "skill", "skill": option["skill"], "target": target});
    }
    // Promotions are avoided when there is a choice: positions crowded with
    // queens make for a very slow analysis, which is not what these tests probe.
    let all = state["moves"].as_array().unwrap();
    let quiet: Vec<&Value> = all.iter().filter(|m| m["promo"].is_null()).collect();
    let moves: Vec<&Value> = if quiet.is_empty() {
        all.iter().collect()
    } else {
        quiet
    };
    let m = moves[(n * 5 + 3) % moves.len()];
    json!({"type": "move", "from": m["from"], "to": m["to"], "promo": m["promo"]})
}

/// The newest `state` the client has received, or `current` if none.
fn newest(c: &mut Client, current: Value) -> Value {
    let mut latest = current;
    while let Some(state) = c.try_next("state") {
        latest = state;
    }
    latest
}

struct Played {
    /// Each side's `state` after every action.
    white: Vec<Value>,
    black: Vec<Value>,
    /// Who acted.
    actors: Vec<String>,
    actions: Vec<Value>,
}

/// Plays up to `actions` scripted actions between `white` and `black` (their
/// opening `state`s still waiting in their queues).
fn play_script(white: &mut Client, black: &mut Client, actions: usize) -> Played {
    let mut played = Played {
        white: Vec::new(),
        black: Vec::new(),
        actors: Vec::new(),
        actions: Vec::new(),
    };
    let mut cur_w = white.last("state");
    let mut cur_b = black.last("state");
    for n in 0..actions {
        if cur_w["outcome"]["type"] != "ongoing" {
            break;
        }
        let white_moves = cur_w["to_move"] == "white";
        let action = choose(if white_moves { &cur_w } else { &cur_b }, n);
        let mover: &Client = if white_moves { white } else { black };
        mover.say(json!({"type": "action", "action": action}));
        // An offered action can be refused when a hidden piece is in the way
        // (the only state sent then is the same position with a lower clock):
        // nothing was played, try something else.
        let refused = if white_moves {
            &mut *white
        } else {
            &mut *black
        }
        .try_next("error")
        .is_some();
        cur_w = newest(white, cur_w);
        cur_b = newest(black, cur_b);
        if refused {
            continue;
        }
        played
            .actors
            .push(if white_moves { "white" } else { "black" }.into());
        played.actions.push(action);
        played.white.push(cur_w.clone());
        played.black.push(cur_b.clone());
    }
    played
}

fn used_skills(state: &Value) -> Vec<Value> {
    state["my_skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["uses"].as_u64().unwrap() > 0)
        .map(|s| s["skill"].clone())
        .collect()
}

/// `view` is what one player was sent after an action, `frame` what the replay
/// says: they must agree, except for what the player was not allowed to see.
fn assert_frame_matches_view(frame: &Value, view: &Value, viewer: &str, n: usize) {
    assert_eq!(frame["to_move"], view["to_move"], "action {n}");
    assert_eq!(frame["in_check"], view["in_check"], "action {n}");
    assert_eq!(frame["terrain"], view["terrain"], "action {n}");
    for sq in 0..64 {
        let seen = &view["board"][sq];
        let real = &frame["board"][sq];
        if seen.is_null() {
            // Only the opponent's invisible pieces may be missing from a view.
            assert!(
                real.is_null() || real["color"] != viewer,
                "action {n}, square {sq}: the viewer's own piece is missing"
            );
        } else {
            assert_eq!(real, seen, "action {n}, square {sq}");
        }
    }
    for effect in view["effects"].as_array().unwrap() {
        assert!(
            frame["effects"].as_array().unwrap().contains(effect),
            "action {n}: effect {effect} is not in the frame"
        );
    }
    // The viewer's own traps and bench are in the frame (it also has the other side's).
    for square in view["traps"].as_array().unwrap() {
        assert!(
            frame["traps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| &t["square"] == square && t["owner"] == viewer),
            "action {n}: trap {square}"
        );
    }
    for piece in view["benched"].as_array().unwrap() {
        assert!(
            frame["benched"]
                .as_array()
                .unwrap()
                .iter()
                .any(|b| &b["piece"] == piece && b["owner"] == viewer),
            "action {n}: benched {piece}"
        );
    }
    assert_eq!(
        frame["used"][viewer],
        json!(used_skills(view)),
        "action {n}"
    );
}

#[tokio::test]
async fn a_game_with_skills_replays_exactly_what_the_players_saw() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    // The uniques come along without being picked: Mind Reading keeps the
    // turn, Mind Control lends a piece.
    let a_deck = [
        SkillId::Teleportation,
        SkillId::Freeze,
        SkillId::Tornado,
        SkillId::Mind,
    ];
    let b_deck = [
        SkillId::Invisibility,
        SkillId::Trap,
        SkillId::DestinySwapper,
        SkillId::Control,
    ];
    store.set_deck(&a.id, &a_deck).unwrap();
    store.set_deck(&b.id, &b_deck).unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, mut b) = (a, b);
    for (c, deck) in [(&mut a, &a_deck), (&mut b, &b_deck)] {
        let ds = c.next("deck_select");
        c.color = serde_json::from_value(ds["you"].clone()).ok();
        c.pick(&deck[..3]);
    }
    let a_is_white = a.color == Some(chessy_engine::Color::White);
    let (mut white, mut black) = if a_is_white { (a, b) } else { (b, a) };

    let played = play_script(&mut white, &mut black, 70);
    assert!(played.actions.len() >= 30, "the game went on for a while");
    let skill_plies = played
        .actions
        .iter()
        .filter(|a| a["type"] == "skill")
        .count();
    assert!(skill_plies >= 8, "skills were played ({skill_plies})");
    let kinds: std::collections::HashSet<String> = played
        .actions
        .iter()
        .filter(|a| a["type"] == "skill")
        .map(|a| a["skill"].as_str().unwrap().to_string())
        .collect();
    assert!(kinds.len() >= 4, "several different skills: {kinds:?}");
    white.send(ClientMsg::Resign);

    let id = latest_game(&api, &white).await;
    let replay = replay_of(&api, &id).await;
    let n_actions = played.actions.len();
    assert_eq!(replay["plies"], n_actions);
    let frames = replay["frames"].as_array().unwrap();
    let moves = replay["moves"].as_array().unwrap();
    assert_eq!(frames.len(), n_actions + 1);
    assert_eq!(moves.len(), n_actions);

    // Loadouts: the picks first, then the uniques of the deck.
    let (white_deck, black_deck) = if a_is_white {
        (&a_deck, &b_deck)
    } else {
        (&b_deck, &a_deck)
    };
    let expect = |deck: &[SkillId; 4]| {
        let mut skills: Vec<SkillId> = deck[..3].to_vec();
        skills.push(deck[3]);
        serde_json::to_value(skills).unwrap()
    };
    assert_eq!(replay["loadouts"]["white"], expect(white_deck));
    assert_eq!(replay["loadouts"]["black"], expect(black_deck));

    for n in 0..n_actions {
        let frame = &frames[n + 1];
        assert_eq!(frame["ply"], n + 1);
        assert_eq!(moves[n]["ply"], n + 1);
        assert_eq!(moves[n]["color"], played.actors[n]);
        assert_eq!(moves[n]["action"]["type"], played.actions[n]["type"]);
        if played.actions[n]["type"] == "skill" {
            let notation = moves[n]["notation"].as_str().unwrap();
            assert!(
                !notation.is_empty() && !notation.contains("e2e4"),
                "{notation}"
            );
        }
        if n + 1 < n_actions {
            assert_eq!(frame["outcome"], played.white[n]["outcome"], "action {n}");
        } else {
            // The last frame says how the game really ended.
            assert_eq!(frame["outcome"]["type"], "resignation");
        }
        assert_frame_matches_view(frame, &played.white[n], "white", n);
        assert_frame_matches_view(frame, &played.black[n], "black", n);
        // The actor's events are all in the frame.
        let actor_view = if played.actors[n] == "white" {
            &played.white[n]
        } else {
            &played.black[n]
        };
        // (the frame has all of them, even those hidden from the actor)
        for event in actor_view["events"].as_array().unwrap() {
            assert!(
                frame["events"].as_array().unwrap().contains(event),
                "event {event} of action {n}"
            );
        }
    }

    // Determinism: asking again gives the very same bytes.
    let again = replay_of(&api, &id).await;
    assert_eq!(again, replay);
    // And so does a fresh replay of the stored actions.
    let stored = store.stored_game(&id).unwrap().unwrap();
    let (loadouts, actions) = stored.replayable().unwrap();
    let rebuilt = chessy_server::replay::replay(loadouts, actions).unwrap();
    let rebuilt_board = serde_json::to_value(
        &rebuilt
            .steps
            .last()
            .unwrap()
            .after
            .frame(0, vec![], Outcome::Ongoing)
            .board,
    )
    .unwrap();
    assert_eq!(rebuilt_board, frames[n_actions]["board"]);
    // The Mind Reading uses (turn kept) are real actions of the record.
    let mind = played
        .actions
        .iter()
        .filter(|a| a["skill"] == "mind" || a["skill"] == "control")
        .count();
    if mind > 0 {
        assert!(replay["moves"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["notation"] == "Mind Reading"
                || m["notation"].as_str().unwrap().starts_with("Mind Control")));
    }
}

// ---- access control -------------------------------------------------------------

#[tokio::test]
async fn solo_games_are_only_readable_by_their_player() {
    let (app, store, api) = setup();
    let mut a = account(&app, &store, "alice");
    let other = account(&app, &store, "bobby");
    a.say(json!({"type": "solo_start", "elo": 500, "color": "white"}));
    a.next("deck_select");
    a.pick_nothing();
    a.mv("e2", "e4");
    a.send(ClientMsg::Resign);
    let id = latest_game(&api, &a).await;

    for path in [
        format!("/api/games/{id}"),
        format!("/api/games/{id}/analysis?depth=1"),
    ] {
        let (status, _) = api.get(&path, Some(&a.token)).await;
        assert_eq!(status, StatusCode::OK, "{path} for the player");
        for token in [None, Some(other.token.as_str()), Some("bad-token")] {
            let (status, v) = api.get(&path, token).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path} with {token:?}");
            assert_eq!(v["error"], "not_found");
        }
    }
    let explore = format!("/api/games/{id}/explore");
    let body = json!({"ply": 0, "line": []});
    let (status, _) = api
        .call("POST", &explore, Some(&a.token), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    for token in [None, Some(other.token.as_str())] {
        let (status, v) = api.call("POST", &explore, token, Some(body.clone())).await;
        assert_eq!(
            (status, v["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found"))
        );
    }
}

#[tokio::test]
async fn other_games_are_public_and_a_bad_token_does_not_matter() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    let id = latest_game(&api, &white).await;
    for token in [None, Some("bad-token"), Some(white.token.as_str())] {
        let (status, _) = api.get(&format!("/api/games/{id}"), token).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = api
            .get(&format!("/api/games/{id}/analysis?depth=1"), token)
            .await;
        assert_eq!(status, StatusCode::OK);
    }
    let stranger = guest(&app);
    let (status, _) = api
        .get(&format!("/api/games/{id}"), Some(&stranger.token))
        .await;
    assert_eq!(status, StatusCode::OK);
}

// ---- analysis ----------------------------------------------------------------

/// 1.e4 e5 2.Dh5 Cc6 3.Dxe5?? Cxe5: white throws its queen away.
async fn blunder_game(app: &Arc<App>, api: &Api) -> String {
    let (white, black) = friendly_game(app);
    play(
        &white,
        &black,
        &[
            ("e2", "e4"),
            ("e7", "e5"),
            ("d1", "h5"),
            ("b8", "c6"),
            ("h5", "e5"),
            ("c6", "e5"),
        ],
    );
    white.send(ClientMsg::Resign);
    latest_game(api, &white).await
}

#[tokio::test]
async fn the_analysis_flags_an_obvious_blunder() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let (status, an) = api.get(&format!("/api/games/{id}/analysis"), None).await;
    assert_eq!(status, StatusCode::OK, "{an}");
    assert_eq!(an["depth"], 3, "the default depth");
    let plies = an["plies"].as_array().unwrap();
    assert_eq!(plies.len(), 6);
    for (i, p) in plies.iter().enumerate() {
        assert_eq!(p["ply"], i + 1);
        let eval = p["eval_cp"].as_i64().unwrap();
        assert!((-2000..=2000).contains(&eval));
        assert!(p["loss_cp"].as_i64().unwrap() >= 0);
        assert!(["best", "good", "inaccuracy", "mistake", "blunder"]
            .contains(&p["label"].as_str().unwrap()));
    }
    // 3.Dxe5 gives the queen for a pawn.
    let queen_grab = &plies[4];
    assert_eq!(queen_grab["label"], "blunder");
    assert!(
        queen_grab["loss_cp"].as_i64().unwrap() > 300,
        "{queen_grab}"
    );
    assert!(
        queen_grab["eval_cp"].as_i64().unwrap() < -400,
        "{queen_grab}"
    );
    let best = &queen_grab["best"];
    assert!(best["notation"].is_string());
    assert_ne!(best["notation"], "Dxe5");
    assert_eq!(best["action"]["type"], "move");
    assert!(best["eval_cp"].as_i64().unwrap() > queen_grab["eval_cp"].as_i64().unwrap());
    // Taking the queen back is the best answer.
    assert_eq!(plies[5]["label"], "best");
    assert_eq!(plies[5]["best"]["notation"], "Cxe5");
    assert_eq!(plies[5]["loss_cp"], 0);

    let white = &an["summary"]["white"];
    assert!(white["blunder"].as_u64().unwrap() >= 1);
    let sum = |side: &str| {
        ["best", "good", "inaccuracy", "mistake", "blunder"]
            .iter()
            .map(|k| an["summary"][side][k].as_u64().unwrap())
            .sum::<u64>()
    };
    assert_eq!(
        (sum("white"), sum("black")),
        (3, 3),
        "every action is counted once"
    );
    let (aw, ab) = (
        an["accuracy"]["white"].as_u64().unwrap(),
        an["accuracy"]["black"].as_u64().unwrap(),
    );
    assert!(aw < ab && aw < 80 && ab >= 80, "accuracy {aw} vs {ab}");
    assert!(
        an.get("reduced_from_ply").is_none(),
        "no time pressure here"
    );
}

#[tokio::test]
async fn a_checkmate_is_evaluated_at_the_cap() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    let id = latest_game(&api, &white).await;
    let (_, an) = api
        .get(&format!("/api/games/{id}/analysis?depth=2"), None)
        .await;
    assert_eq!(an["depth"], 2);
    let plies = an["plies"].as_array().unwrap();
    assert_eq!(plies.len(), 4);
    assert_eq!(
        plies[3]["eval_cp"], -2000,
        "Black mates: White's point of view"
    );
    assert_eq!(plies[3]["label"], "best");
    assert_eq!(plies[3]["loss_cp"], 0);
    assert_eq!(plies[3]["best"]["notation"], "Dh4#");
    // 2.g4 walks into the mate.
    assert_eq!(plies[2]["label"], "blunder");
    assert!(plies[2]["eval_cp"].as_i64().unwrap() <= -1000);
    assert!(an["accuracy"]["white"].as_u64().unwrap() < 50);
    assert_eq!(an["summary"]["black"]["blunder"], 0);
}

#[tokio::test]
async fn analyses_are_cached_per_game_and_depth() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let path = format!("/api/games/{id}/analysis?depth=2");
    let (_, first) = api.get(&path, None).await;
    let (_, second) = api.get(&path, None).await;
    assert_eq!(first, second);

    // The cache is what answers the second time: tamper with it and see.
    let cached = app.store().analysis_get(&id, 2).unwrap();
    assert!(cached.is_some(), "stored after the first request");
    assert!(
        app.store().analysis_get(&id, 3).unwrap().is_none(),
        "per depth"
    );
    app.store()
        .analysis_put(&id, 2, r#"{"depth":2,"from_cache":true}"#)
        .unwrap();
    let (_, tampered) = api.get(&path, None).await;
    assert_eq!(tampered["from_cache"], true);
    // Another depth is computed on its own.
    let (_, deeper) = api
        .get(&format!("/api/games/{id}/analysis?depth=3"), None)
        .await;
    assert_eq!(deeper["depth"], 3);
    assert!(app.store().analysis_get(&id, 3).unwrap().is_some());
    // The default depth is 3: that very cache entry.
    let (_, default) = api.get(&format!("/api/games/{id}/analysis"), None).await;
    assert_eq!(default, deeper);
}

#[tokio::test]
async fn simultaneous_requests_get_the_same_analysis() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let path = format!("/api/games/{id}/analysis?depth=3");
    let (a, b, c) = tokio::join!(
        api.get(&path, None),
        api.get(&path, None),
        api.get(&path, None)
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[tokio::test]
async fn an_analysis_is_repeatable() {
    // The same moves in two separate servers give the very same analysis.
    let mut results = Vec::new();
    for _ in 0..2 {
        let (app, _, api) = setup();
        let id = blunder_game(&app, &api).await;
        let (status, an) = api
            .get(&format!("/api/games/{id}/analysis?depth=3"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        results.push(an);
    }
    assert_eq!(results[0], results[1]);
}

#[tokio::test]
async fn the_analysis_gets_shallower_when_time_runs_out() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let stored = app.store().stored_game(&id).unwrap().unwrap();
    let (loadouts, actions) = stored.replayable().unwrap();
    let replayed = chessy_server::replay::replay(loadouts, actions).unwrap();
    let started = std::time::Instant::now();
    let an = chessy_server::analysis::analyze(&replayed, 5, std::time::Duration::from_millis(1));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(an.depth, 5, "the requested depth is reported");
    assert_eq!(an.plies.len(), 6, "every ply is analysed all the same");
    assert!(an.reduced_from_ply.is_some_and(|p| (1..=7).contains(&p)));
    let json = serde_json::to_value(&an).unwrap();
    assert!(json["reduced_from_ply"].is_number());
    // With time to spare nothing is reduced.
    let full = chessy_server::analysis::analyze(&replayed, 2, std::time::Duration::from_secs(25));
    assert_eq!(full.reduced_from_ply, None);
}

#[tokio::test]
async fn the_depth_is_checked() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    for depth in ["0", "6", "99", "-1", "abc"] {
        let (status, v) = api
            .get(&format!("/api/games/{id}/analysis?depth={depth}"), None)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "depth {depth}");
        assert!(v["error"].is_string());
    }
    for depth in 1..=5 {
        let (status, v) = api
            .get(&format!("/api/games/{id}/analysis?depth={depth}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "depth {depth}");
        assert_eq!(v["depth"], depth);
    }
}

// ---- exploration --------------------------------------------------------------

fn mv_json(from: &str, to: &str) -> Value {
    json!({
        "type": "move",
        "from": chessy_engine::parse_square(from).unwrap(),
        "to": chessy_engine::parse_square(to).unwrap(),
    })
}

async fn explore(api: &Api, id: &str, body: Value) -> (StatusCode, Value) {
    api.post(&format!("/api/games/{id}/explore"), body).await
}

#[tokio::test]
async fn a_valid_variation_is_played_and_described() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    // Back to the position after 2...Cc6 and try 3.Fc4 instead.
    let (status, v) = explore(
        &api,
        &id,
        json!({"ply": 4, "line": [mv_json("f1", "c4"), mv_json("g8", "f6")], "depth": 2}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["valid"], true);
    assert!(v.get("error").is_none());
    assert_eq!(v["at"], 2);
    assert_eq!(v["notation"], json!(["Fc4", "Cf6"]));
    assert_eq!(v["frame"]["ply"], 6);
    assert_eq!(v["frame"]["to_move"], "white");
    assert_eq!(v["frame"]["board"].as_array().unwrap().len(), 64);
    assert_eq!(
        v["frame"]["board"][26]["kind"], "bishop",
        "the bishop is on c4"
    );
    assert!(!v["moves"].as_array().unwrap().is_empty());
    assert!(v["moves"][0]["from"].is_number());
    assert_eq!(v["skill_options"], json!([]), "no skills in this game");
    let eval = v["eval_cp"].as_i64().unwrap();
    assert!((-2000..=2000).contains(&eval));
    assert_eq!(v["best"]["action"]["type"], "move");
    assert!(v["best"]["notation"].is_string());
    assert!(v["best"]["eval_cp"].is_number());

    // The game itself is untouched (stateless).
    let (_, again) = api.get(&format!("/api/games/{id}"), None).await;
    assert_eq!(again["plies"], 6);

    // Playing the game's own actions back reaches the recorded position.
    let replay = replay_of(&api, &id).await;
    let line: Vec<Value> = replay["moves"].as_array().unwrap()[4..]
        .iter()
        .map(|m| m["action"].clone())
        .collect();
    let (_, same) = explore(&api, &id, json!({"ply": 4, "line": line, "depth": 1})).await;
    assert_eq!(same["valid"], true);
    assert_eq!(same["frame"]["board"], replay["frames"][6]["board"]);
    assert_eq!(same["notation"], json!(["Dxe5+", "Cxe5"]));
    // ...and "ply" alone is the game at that point.
    let (_, start) = explore(&api, &id, json!({"ply": 0})).await;
    assert_eq!(start["at"], 0);
    assert_eq!(start["frame"]["ply"], 0);
    assert_eq!(start["moves"].as_array().unwrap().len(), 20);
}

#[tokio::test]
async fn the_variation_stops_at_the_first_illegal_action() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let (status, v) = explore(
        &api,
        &id,
        json!({"ply": 0, "line": [mv_json("e2", "e4"), mv_json("e2", "e4"), mv_json("e7", "e5")]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["valid"], false);
    assert_eq!(v["error"], "illegal_action");
    assert_eq!(v["at"], 1);
    assert_eq!(v["notation"], json!(["e4"]));
    assert_eq!(
        v["frame"]["ply"], 1,
        "the position reached before the bad action"
    );
    assert_eq!(v["frame"]["to_move"], "black");
    assert!(!v["moves"].as_array().unwrap().is_empty());
    // A skill nobody has is illegal too.
    let (_, v) = explore(
        &api,
        &id,
        json!({"ply": 0, "line": [{"type": "skill", "skill": "freeze", "target": {"kind": "piece", "square": 52}}]}),
    )
    .await;
    assert_eq!(
        (v["valid"].clone(), v["at"].clone()),
        (json!(false), json!(0))
    );
    assert_eq!(v["error"], "illegal_action");
    assert_eq!(v["frame"]["ply"], 0);
}

#[tokio::test]
async fn an_impossible_ply_is_reported_not_played() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    for ply in [7, 100, -1] {
        let (status, v) = explore(&api, &id, json!({"ply": ply, "line": []})).await;
        assert_eq!(status, StatusCode::OK, "ply {ply}");
        assert_eq!(v["valid"], false);
        assert_eq!(v["error"], "bad_ply");
        assert_eq!(v["frame"], Value::Null);
        assert_eq!(v["at"], 0);
        assert_eq!(v["moves"], json!([]));
    }
    // The last ply is fine.
    let (_, v) = explore(&api, &id, json!({"ply": 6, "line": []})).await;
    assert_eq!(v["valid"], true);
    assert_eq!(v["frame"]["ply"], 6);
}

#[tokio::test]
async fn the_variation_length_and_the_request_are_limited() {
    let (app, _, api) = setup();
    let id = blunder_game(&app, &api).await;
    let nonsense = vec![mv_json("e2", "e4"); 200];
    let (status, v) = explore(&api, &id, json!({"ply": 0, "line": nonsense, "depth": 1})).await;
    assert_eq!(status, StatusCode::OK, "200 actions are accepted");
    assert_eq!(v["error"], "illegal_action");
    let too_many = vec![mv_json("e2", "e4"); 201];
    let (status, v) = explore(&api, &id, json!({"ply": 0, "line": too_many})).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("line_too_long"))
    );

    let (status, v) = explore(&api, &id, json!({"line": []})).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_request"))
    );
    let (status, _) = explore(&api, &id, json!({"ply": 0, "line": [{"type": "dance"}]})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for depth in [0, 6] {
        let (status, v) = explore(&api, &id, json!({"ply": 0, "depth": depth})).await;
        assert_eq!(
            (status, v["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("invalid_depth"))
        );
    }
    let huge = "x".repeat(80 * 1024);
    let (status, v) = explore(&api, &id, json!({"ply": 0, "pad": huge})).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("payload_too_large"))
    );
}

#[tokio::test]
async fn exploring_a_finished_position_offers_nothing() {
    let (app, _, api) = setup();
    let (white, black) = friendly_game(&app);
    fools_mate(&white, &black);
    let id = latest_game(&api, &white).await;
    let (_, v) = explore(&api, &id, json!({"ply": 4, "line": []})).await;
    assert_eq!(v["valid"], true);
    assert_eq!(
        v["frame"]["outcome"],
        json!({"type": "checkmate", "winner": "black"})
    );
    assert_eq!(v["moves"], json!([]));
    assert_eq!(v["best"], Value::Null);
    assert_eq!(v["eval_cp"], -2000);
    // Nothing can be played after a mate.
    let (_, v) = explore(&api, &id, json!({"ply": 4, "line": [mv_json("a2", "a3")]})).await;
    assert_eq!(
        (v["valid"].clone(), v["error"].clone()),
        (json!(false), json!("illegal_action"))
    );
    // The mating move itself reads as mate.
    let (_, v) = explore(&api, &id, json!({"ply": 3, "line": [mv_json("d8", "h4")]})).await;
    assert_eq!(v["notation"], json!(["Dh4#"]));
    assert_eq!(v["eval_cp"], -2000);
}

#[tokio::test]
async fn exploration_offers_the_skills_of_the_loadouts() {
    let (app, store, api) = setup();
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    store
        .set_deck(&a.id, &[SkillId::Teleportation, SkillId::Freeze])
        .unwrap();
    store
        .set_deck(&b.id, &[SkillId::Imune, SkillId::Trap])
        .unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, mut b) = (a, b);
    for (c, deck) in [
        (&mut a, vec![SkillId::Teleportation, SkillId::Freeze]),
        (&mut b, vec![SkillId::Imune, SkillId::Trap]),
    ] {
        let ds = c.next("deck_select");
        c.color = serde_json::from_value(ds["you"].clone()).ok();
        c.pick(&deck);
    }
    let (white, black) = if a.color == Some(chessy_engine::Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    white.mv("e2", "e4");
    white.send(ClientMsg::Resign);
    let id = latest_game(&api, &white).await;

    let (_, v) = explore(&api, &id, json!({"ply": 0, "line": []})).await;
    let options = v["skill_options"].as_array().unwrap();
    assert_eq!(options.len(), 2, "white's two skills: {v}");
    assert!(options
        .iter()
        .all(|o| !o["targets"].as_array().unwrap().is_empty()));
    // Use one: the answer's options are the other side's.
    let first = &options[0];
    let skill_action =
        json!({"type": "skill", "skill": first["skill"], "target": first["targets"][0]});
    let (_, v) = explore(
        &api,
        &id,
        json!({"ply": 0, "line": [skill_action], "depth": 2}),
    )
    .await;
    assert_eq!(v["valid"], true, "{v}");
    assert_eq!(v["at"], 1);
    assert_eq!(v["frame"]["to_move"], "black");
    assert_eq!(v["frame"]["used"]["white"], json!([first["skill"]]));
    let name_of = |id: &str| match id {
        "freeze" => "Freeze",
        "teleportation" => "Teleportation",
        "imune" => "Imune",
        "trap" => "Trap Card",
        other => panic!("unexpected {other}"),
    };
    assert!(v["notation"][0]
        .as_str()
        .unwrap()
        .starts_with(name_of(first["skill"].as_str().unwrap())));
    let names: Vec<&str> = v["skill_options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["skill"].as_str().unwrap())
        .collect();
    let white_names: Vec<&str> = options
        .iter()
        .map(|o| o["skill"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 2, "black's two skills");
    assert!(
        names.iter().all(|n| !white_names.contains(n)),
        "{names:?} vs {white_names:?}"
    );
    drop(black);
}

// ---- old databases ---------------------------------------------------------------

/// The schema before accounts existed (see `accounts.rs`), with one game.
const LEGACY_SCHEMA: &str = "
    CREATE TABLE players (id TEXT PRIMARY KEY, token TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
    CREATE TABLE player_skills (player_id TEXT NOT NULL REFERENCES players(id),
        skill TEXT NOT NULL, PRIMARY KEY (player_id, skill));
    CREATE TABLE unique_skill_owner (skill TEXT PRIMARY KEY,
        player_id TEXT NOT NULL REFERENCES players(id));
    CREATE TABLE games (id TEXT PRIMARY KEY, white TEXT NOT NULL REFERENCES players(id),
        black TEXT NOT NULL REFERENCES players(id), outcome TEXT NOT NULL,
        finished_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
    INSERT INTO players (id, token) VALUES ('aaaaaaaa', 'old-token-a'), ('bbbbbbbb', 'old-token-b');
    INSERT INTO games (id, white, black, outcome)
        VALUES ('g1-000001', 'aaaaaaaa', 'bbbbbbbb', '{\"type\":\"stalemate\"}');
";

#[tokio::test]
async fn games_from_before_replays_are_listed_but_cannot_be_replayed() {
    let db = TempDb::new();
    db.raw().execute_batch(LEGACY_SCHEMA).unwrap();
    for _ in 0..2 {
        // Opening twice proves the migration is idempotent.
        let store = Store::open(db.path_str()).unwrap();
        let app = App::new(store, HubConfig::default());
        let api = Api::new(&app);

        let (status, list) = api.get("/api/me/games", Some("old-token-a")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["total"], 1);
        let g = &list["games"][0];
        assert_eq!(g["game_id"], "g1-000001");
        assert_eq!(g["kind"], "duel");
        assert_eq!(g["result"], "draw");
        assert_eq!(g["reason"], "stalemate");
        assert_eq!(g["color"], "white");

        for path in ["/api/games/g1-000001", "/api/games/g1-000001/analysis"] {
            let (status, v) = api.get(path, None).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(v["error"], "no_replay");
        }
        let (status, v) = api
            .post(
                "/api/games/g1-000001/explore",
                json!({"ply": 0, "line": []}),
            )
            .await;
        assert_eq!(
            (status, v["error"].as_str()),
            (StatusCode::NOT_FOUND, Some("no_replay"))
        );
    }
    let raw = db.raw();
    let version: i64 = raw
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert!(version >= 3);
    let columns: Vec<String> = raw
        .prepare("SELECT name FROM pragma_table_info('games')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for wanted in ["kind", "loadouts", "actions", "solo_elo", "plies"] {
        assert!(
            columns.iter().any(|c| c == wanted),
            "{wanted} in {columns:?}"
        );
    }
}

#[tokio::test]
async fn new_games_are_recorded_next_to_old_ones() {
    let db = TempDb::new();
    db.raw().execute_batch(LEGACY_SCHEMA).unwrap();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), HubConfig::default());
    let api = Api::new(&app);
    let a = Client::connect(&app, Some("old-token-a".into()));
    let b = Client::connect(&app, Some("old-token-b".into()));
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (white, black) = into_game(a, b);
    fools_mate(&white, &black);
    let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
    assert_eq!(list["total"], 2);
    assert_eq!(list["games"][1]["game_id"], "g1-000001");
    let id = list["games"][0]["game_id"].as_str().unwrap().to_string();
    assert_eq!(replay_of(&api, &id).await["plies"], 4);
}

// ---- speed ---------------------------------------------------------------------------

#[tokio::test]
async fn a_long_game_is_analysed_within_the_budget() {
    // The script plays 120 plies in a blink: far above what a person sends.
    let (app, store) = new_app(HubConfig {
        msg_burst: 1_000,
        ..HubConfig::default()
    });
    let api = Api::new(&app);
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    let a_deck = [SkillId::Teleportation, SkillId::Freeze, SkillId::Tornado];
    let b_deck = [SkillId::Imune, SkillId::Trap, SkillId::Bench];
    store.set_deck(&a.id, &a_deck).unwrap();
    store.set_deck(&b.id, &b_deck).unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let (mut a, mut b) = (a, b);
    for (c, deck) in [(&mut a, a_deck), (&mut b, b_deck)] {
        let ds = c.next("deck_select");
        c.color = serde_json::from_value(ds["you"].clone()).ok();
        c.pick(&deck);
    }
    let (mut white, mut black) = if a.color == Some(chessy_engine::Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    let played = play_script(&mut white, &mut black, 120);
    white.send(ClientMsg::Resign);
    let id = latest_game(&api, &white).await;
    let started = std::time::Instant::now();
    let (_, replay) = api.get(&format!("/api/games/{id}"), None).await;
    eprintln!(
        "replay of {} frames: {:?}, {} bytes",
        replay["frames"].as_array().unwrap().len(),
        started.elapsed(),
        replay.to_string().len()
    );
    let started = std::time::Instant::now();
    let (status, an) = api
        .get(&format!("/api/games/{id}/analysis?depth=3"), None)
        .await;
    let took = started.elapsed();
    eprintln!(
        "{} actions analysed at depth 3 in {took:?}",
        played.actions.len()
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(an["plies"].as_array().unwrap().len(), played.actions.len());
    assert!(took < std::time::Duration::from_secs(30), "took {took:?}");
}
