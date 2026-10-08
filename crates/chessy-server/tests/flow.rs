//! Drives the hub through its public `App` API with in-process channels.

use std::sync::Arc;
use std::time::Duration;

use chessy_engine::{parse_square, Action, Color, SkillId};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, RewardChoice, ServerMsg};
use chessy_server::store::Store;
use chessy_server::App;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

struct Client {
    app: Arc<App>,
    rx: UnboundedReceiver<ServerMsg>,
    id: String,
    token: String,
    conn: u64,
    color: Option<Color>,
    deck: Vec<Value>,
    buffered: Vec<Value>,
}

impl Client {
    fn connect(app: &Arc<App>, token: Option<String>) -> Client {
        let (tx, rx) = unbounded_channel();
        let (id, conn) = app.connect(token, tx).unwrap();
        let mut c = Client {
            app: app.clone(),
            rx,
            id,
            token: String::new(),
            conn,
            color: None,
            deck: Vec::new(),
            buffered: Vec::new(),
        };
        let welcome = c.next("welcome");
        c.token = welcome["token"].as_str().unwrap().to_string();
        c.deck = welcome["deck"].as_array().unwrap().clone();
        let _ = c.try_next("lobby"); // initial idle status
        c
    }

    fn send(&self, msg: ClientMsg) {
        self.app.handle(&self.id, self.conn, msg);
    }

    fn mv(&self, from: &str, to: &str) {
        self.send(ClientMsg::Action {
            action: Action::Move {
                from: parse_square(from).unwrap(),
                to: parse_square(to).unwrap(),
                promo: None,
            },
        });
    }

    /// Removes and returns the oldest message of the given type; other
    /// messages stay queued.
    fn next(&mut self, ty: &str) -> Value {
        self.try_next(ty)
            .unwrap_or_else(|| panic!("no `{ty}` message arrived"))
    }

    fn try_next(&mut self, ty: &str) -> Option<Value> {
        while let Ok(msg) = self.rx.try_recv() {
            self.buffered.push(serde_json::to_value(&msg).unwrap());
        }
        let at = self.buffered.iter().position(|v| v["type"] == ty)?;
        Some(self.buffered.remove(at))
    }

    /// The most recent queued message of the given type; drains everything.
    fn last(&mut self, ty: &str) -> Value {
        let mut found = None;
        while let Some(v) = self.try_next(ty) {
            found = Some(v);
        }
        found.unwrap_or_else(|| panic!("no `{ty}` message arrived"))
    }

    /// Waits (briefly) for a message that is produced by a timer.
    async fn wait_for(&mut self, ty: &str) -> Value {
        for _ in 0..400 {
            if let Some(v) = self.try_next(ty) {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out waiting for `{ty}`");
    }

    fn pick(&self, skills: &[SkillId]) {
        self.send(ClientMsg::SelectDeck {
            skills: skills.to_vec(),
        });
    }
}

fn new_app(config: HubConfig) -> (Arc<App>, Store) {
    let store = Store::open(":memory:").unwrap();
    (App::new(store.clone(), config), store)
}

/// Two registered accounts matched in the ranked queue (only rated games pay
/// a reward), with fixed decks and the given loadouts, both already past deck
/// selection. Returns (white, black).
fn start_game_with(
    app: &Arc<App>,
    store: &Store,
    decks: [&[SkillId]; 2],
    picks: [&[SkillId]; 2],
) -> (Client, Client) {
    let (_, token_a) = store.register("alice", "unused-hash", None).unwrap();
    let (_, token_b) = store.register("bob", "unused-hash", None).unwrap();
    let mut a = Client::connect(app, Some(token_a));
    let mut b = Client::connect(app, Some(token_b));
    store.set_deck(&a.id, decks[0]).unwrap();
    store.set_deck(&b.id, decks[1]).unwrap();
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
    assert_ne!(a.color, b.color);
    a.pick(picks[0]);
    b.pick(picks[1]);
    if a.color == Some(Color::White) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Like `start_game_with`, but nobody brings skills, so checkmate is final
/// (a skill that could escape the mate would keep the game going).
fn start_game(
    app: &Arc<App>,
    store: &Store,
    deck_a: &[SkillId],
    deck_b: &[SkillId],
) -> (Client, Client) {
    start_game_with(app, store, [deck_a, deck_b], [&[], &[]])
}

fn fools_mate(white: &Client, black: &Client) {
    white.mv("f2", "f3");
    black.mv("e7", "e5");
    white.mv("g2", "g4");
    black.mv("d8", "h4");
}

const DECK_A: &[SkillId] = &[SkillId::Teleportation, SkillId::Imune, SkillId::Freeze];
const DECK_B: &[SkillId] = &[SkillId::Rollback, SkillId::Clone, SkillId::DestinySwapper];

#[tokio::test]
async fn new_players_get_a_starter_deck_and_tokens_persist() {
    let (app, _) = new_app(HubConfig::default());
    let mut first = Client::connect(&app, None);
    assert_eq!(first.deck.len(), 3);
    let again = Client::connect(&app, Some(first.token.clone()));
    assert_eq!(again.id, first.id, "the token identifies the same player");
    assert_eq!(again.deck, first.deck);
    let other = Client::connect(&app, Some("not-a-real-token".into()));
    assert_ne!(other.id, first.id, "an unknown token starts a fresh player");
    assert_eq!(first.next("error")["code"], "replaced");
}

#[tokio::test]
async fn matchmaking_and_deck_selection_start_a_game() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game_with(&app, &store, [DECK_A, DECK_B], [DECK_A, DECK_B]);

    let ws = white.next("state");
    let bs = black.next("state");
    assert_eq!(ws["you"], "white");
    assert_eq!(bs["you"], "black");
    assert_eq!(ws["moves"].as_array().unwrap().len(), 20);
    assert!(
        bs["moves"].as_array().unwrap().is_empty(),
        "only the mover gets moves"
    );
    assert_eq!(ws["my_skills"].as_array().unwrap().len(), 3);
    assert_eq!(ws["opponent_skills"]["total"], 3);
    assert_eq!(
        ws["opponent_skills"]["used"],
        json!([]),
        "the opponent's skills stay hidden"
    );
}

#[tokio::test]
async fn a_full_game_ends_with_a_reward_for_the_winner() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    fools_mate(&white, &black);
    let last = black.next("game_over");
    assert_eq!(
        last["outcome"],
        json!({"type": "checkmate", "winner": "black"})
    );
    assert!(last["reward"].is_object(), "the winner is offered a reward");
    assert!(white.next("game_over")["reward"].is_null());
}

#[tokio::test]
async fn a_skill_in_hand_means_no_checkmate_yet() {
    let (app, store) = new_app(HubConfig::default());
    let (white, mut black) = start_game_with(&app, &store, [DECK_A, DECK_B], [DECK_A, DECK_B]);
    fools_mate(&white, &black);
    assert!(black.try_next("game_over").is_none());
    let mut white = white;
    let state = white.last("state");
    assert_eq!(state["in_check"], true);
    assert!(state["moves"].as_array().unwrap().is_empty());
    assert!(
        !state["skill_options"].as_array().unwrap().is_empty(),
        "a skill can still help"
    );
}

#[tokio::test]
async fn deck_selection_is_validated() {
    let (app, store) = new_app(HubConfig::default());
    let mut a = Client::connect(&app, None);
    let b = Client::connect(&app, None);
    store
        .set_deck(
            &a.id,
            &[
                SkillId::Teleportation,
                SkillId::Imune,
                SkillId::Freeze,
                SkillId::Remover,
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

    a.pick(&[SkillId::Rollback]);
    assert_eq!(a.next("error")["code"], "invalid_deck", "not in the deck");
    a.pick(&[SkillId::Remover]);
    assert_eq!(
        a.next("error")["code"],
        "invalid_deck",
        "unique skills are not picked"
    );
    a.pick(&[SkillId::Imune, SkillId::Imune]);
    assert_eq!(a.next("error")["code"], "invalid_deck", "duplicates");
    a.pick(&[SkillId::Teleportation, SkillId::Imune, SkillId::Freeze]);
    assert!(
        a.try_next("error").is_none(),
        "three classic picks are fine"
    );
    a.pick(&[SkillId::Imune]);
    assert_eq!(a.next("error")["code"], "already_selected");
}

#[tokio::test]
async fn unique_skills_join_the_loadout_beyond_the_three() {
    let (app, store) = new_app(HubConfig::default());
    let mut a = Client::connect(&app, None);
    let b = Client::connect(&app, None);
    store
        .set_deck(
            &a.id,
            &[
                SkillId::Teleportation,
                SkillId::Imune,
                SkillId::Freeze,
                SkillId::Remover,
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
    a.pick(&[SkillId::Teleportation, SkillId::Imune, SkillId::Freeze]);
    b.pick(&[]);
    let state = a.next("state");
    let skills: Vec<_> = state["my_skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["skill"].clone())
        .collect();
    assert_eq!(skills.len(), 4);
    assert!(skills.contains(&json!("remover")));
}

#[tokio::test]
async fn moves_are_validated_and_turn_ordered() {
    let (app, store) = new_app(HubConfig::default());
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    black.mv("e7", "e5");
    assert_eq!(black.next("error")["code"], "not_your_turn");
    white.mv("e2", "e5");
    let mut white = white;
    assert_eq!(white.next("error")["code"], "illegal_action");
    white.mv("e2", "e4");
    assert_eq!(white.last("state")["ply"], 1);
}

#[tokio::test]
async fn skills_work_over_the_protocol() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game_with(
        &app,
        &store,
        [&[SkillId::Teleportation], &[SkillId::Teleportation]],
        [&[SkillId::Teleportation], &[SkillId::Teleportation]],
    );
    let state = white.last("state");
    let opts = state["skill_options"].as_array().unwrap();
    assert_eq!(opts.len(), 1);
    assert_eq!(opts[0]["skill"], "teleportation");
    white.send(ClientMsg::Action {
        action: serde_json::from_value(json!({
            "type": "skill", "skill": "teleportation",
            "target": {"kind": "piece_to", "from": 0, "to": 28}
        }))
        .unwrap(),
    });
    let after = black.last("state");
    assert_eq!(after["board"][28]["kind"], "rook");
    assert_eq!(after["opponent_skills"]["used"], json!(["teleportation"]));
}

fn skill_of(v: &Value) -> SkillId {
    serde_json::from_value(v.clone()).unwrap()
}

#[tokio::test]
async fn winner_steals_a_skill() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    fools_mate(&white, &black);
    let offer = black.next("game_over")["reward"].clone();
    // The decks are disjoint, so everything the loser holds can be stolen.
    let options = offer["steal_options"].as_array().unwrap().clone();
    assert_eq!(options.len(), 3);
    let skill = skill_of(&options[0]);
    black.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill,
            replace: None,
        },
    });
    let update = black.next("deck_update");
    assert_eq!(skill_of(&update["gained"]), skill);
    assert_eq!(update["deck"].as_array().unwrap().len(), 4);
    assert_eq!(skill_of(&white.next("deck_update")["lost"]), skill);
    assert!(!store.deck(&white.id).unwrap().contains(&skill));
    assert!(store.deck(&black.id).unwrap().contains(&skill));

    // The reward is spent, so claiming again is refused.
    black.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Skip,
    });
    assert_eq!(black.next("error")["code"], "no_reward");
}

#[tokio::test]
async fn steal_requests_are_checked() {
    let (app, store) = new_app(HubConfig::default());
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    fools_mate(&white, &black);
    let offer = black.next("game_over")["reward"].clone();
    let owned = skill_of(&offer["deck"][0]);
    let stealable = skill_of(&offer["steal_options"][0]);

    black.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill: owned,
            replace: None,
        },
    });
    assert_eq!(
        black.next("error")["code"],
        "invalid_reward",
        "already owned"
    );
    black.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill: stealable,
            replace: None,
        },
    });
    assert!(
        black.try_next("error").is_none(),
        "the claim stayed open after the mistake"
    );
    assert_eq!(skill_of(&black.next("deck_update")["gained"]), stealable);
}

#[tokio::test]
async fn unique_skills_move_between_owners() {
    let (app, store) = new_app(HubConfig::default());
    // White's deck holds Remover; black wins and takes it.
    let (white, black) = start_game(
        &app,
        &store,
        &[SkillId::Teleportation, SkillId::Remover],
        DECK_B,
    );
    // Make the Remover holder the loser whichever colour it drew.
    let (loser, winner) = if store.deck(&white.id).unwrap().contains(&SkillId::Remover) {
        (white, black)
    } else {
        (black, white)
    };
    assert_eq!(
        store.unique_owner(SkillId::Remover).unwrap().as_deref(),
        Some(loser.id.as_str())
    );
    let mut winner = winner;
    let loser_is_white = loser.color == Some(Color::White);
    if loser_is_white {
        loser.mv("f2", "f3");
        winner.mv("e7", "e5");
        loser.mv("g2", "g4");
        winner.mv("d8", "h4");
    } else {
        winner.mv("e2", "e4");
        loser.mv("f7", "f6");
        winner.mv("d2", "d4");
        loser.mv("g7", "g5");
        winner.mv("d1", "h5");
    }
    let offer = winner.next("game_over")["reward"].clone();
    assert!(offer["steal_options"]
        .as_array()
        .unwrap()
        .contains(&json!("remover")));
    winner.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill: SkillId::Remover,
            replace: None,
        },
    });
    assert_eq!(winner.next("deck_update")["gained"], "remover");
    assert_eq!(
        store.unique_owner(SkillId::Remover).unwrap().as_deref(),
        Some(winner.id.as_str())
    );
    assert!(!store.deck(&loser.id).unwrap().contains(&SkillId::Remover));
}

#[test]
fn unique_skills_cannot_have_two_owners() {
    let store = Store::open(":memory:").unwrap();
    let (a, _) = store.create_player().unwrap();
    let (b, _) = store.create_player().unwrap();
    store.set_deck(&a, &[SkillId::Remover]).unwrap();
    let starter = store.deck(&b).unwrap();
    assert!(store.set_deck(&b, &[SkillId::Remover]).is_err());
    assert_eq!(
        store.deck(&b).unwrap(),
        starter,
        "the failed change rolled back"
    );
    // A failed reward changes nothing.
    store.set_deck(&b, &[SkillId::Teleportation]).unwrap();
    let before = store.deck(&b).unwrap();
    assert!(store
        .apply_reward(
            &b,
            &a,
            Some(SkillId::Remover),
            Some(SkillId::Teleportation),
            None
        )
        .is_err());
    assert_eq!(store.deck(&b).unwrap(), before);
}

#[tokio::test]
async fn a_random_reward_forges_a_new_skill() {
    let (app, store) = new_app(HubConfig::default());
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    fools_mate(&white, &black);
    let _ = black.next("game_over");
    black.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Random { replace: None },
    });
    let update = black.wait_for("deck_update").await;
    let gained = update["gained"].as_str().expect("a skill was gained");
    assert!(gained.starts_with("forged_"), "{update}");
    let id = SkillId::parse(gained).unwrap();
    assert!(store.deck(&black.id).unwrap().contains(&id));
    let SkillId::Forged(n) = id else {
        unreachable!()
    };
    let views = store.forged_views(&[n]).unwrap();
    assert_eq!(views.len(), 1, "the definition is stored");
    assert!(!views[0].name.is_empty() && !views[0].description.is_empty());
    let mut white = white;
    assert!(
        !white.wait_for("deck_update").await["lost"].is_null(),
        "the loser still loses a skill"
    );
}

#[tokio::test]
async fn a_second_random_choice_while_forging_is_refused() {
    let (app, store) = new_app(HubConfig::default());
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    fools_mate(&white, &black);
    let _ = black.next("game_over");
    for _ in 0..2 {
        black.send(ClientMsg::RewardChoice {
            choice: RewardChoice::Random { replace: None },
        });
    }
    let _ = black.wait_for("deck_update").await;
    // Only one skill was forged and handed over.
    assert_eq!(store.deck(&black.id).unwrap().len(), DECK_B.len() + 1);
}

#[tokio::test]
async fn resigning_ends_the_game() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    white.send(ClientMsg::Resign);
    assert_eq!(
        black.next("game_over")["outcome"],
        json!({"type": "resignation", "winner": "black"})
    );
    let _ = white.next("game_over");
}

#[tokio::test]
async fn disconnecting_forfeits_after_the_grace_period() {
    let (app, store) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(100),
        ..HubConfig::default()
    });
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    app.disconnect(&white.id, white.conn);
    assert_eq!(black.next("opponent_status")["connected"], false);
    let over = black.wait_for("game_over").await;
    assert_eq!(
        over["outcome"],
        json!({"type": "resignation", "winner": "black"})
    );
}

#[tokio::test]
async fn reconnecting_in_time_resumes_the_game() {
    let (app, store) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(150),
        ..HubConfig::default()
    });
    let (white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    white.mv("e2", "e4");
    app.disconnect(&white.id, white.conn);
    assert_eq!(black.next("opponent_status")["connected"], false);

    let mut back = Client::connect(&app, Some(white.token.clone()));
    assert_eq!(back.id, white.id);
    let state = back.next("state");
    assert_eq!(state["you"], "white");
    assert_eq!(state["ply"], 1);
    assert_eq!(black.next("opponent_status")["connected"], true);

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        black.try_next("game_over").is_none(),
        "the forfeit timer was defused"
    );
    black.mv("e7", "e5");
    assert_eq!(back.next("state")["ply"], 2);
}

#[tokio::test]
async fn resuming_a_game_replays_the_moves_already_played() {
    let (app, store) = new_app(HubConfig::default());
    let (mut white, mut black) = start_game(&app, &store, DECK_A, DECK_B);
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.mv("g1", "f3");
    let _ = white.last("state");
    let _ = black.last("state");

    // A page reload: the old socket goes away, the same token comes back.
    app.disconnect(&white.id, white.conn);
    let mut back = Client::connect(&app, Some(white.token.clone()));
    let state = back.next("state");
    assert_eq!(state["ply"], 3);
    assert!(state["events"].as_array().unwrap().is_empty());
    let history = state["history"]
        .as_array()
        .expect("history is sent on resume");
    assert_eq!(history.len(), 3, "one entry per action: {history:?}");
    for (i, entry) in history.iter().enumerate() {
        assert_eq!(entry["ply"], i as u64 + 1);
        assert_eq!(entry["events"][0]["type"], "moved");
    }
    assert_eq!(history[0]["events"][0]["from"], 12);
    assert_eq!(history[0]["events"][0]["to"], 28);
    assert_eq!(history[0]["to_move"], "black");
    // The kind of the piece that moved travels with the entry (the board is gone by then).
    assert_eq!(
        history[2]["landed"],
        json!([{"square": 21, "kind": "knight"}])
    );

    // The opponent gets the same history; live states do not carry it.
    app.disconnect(&black.id, black.conn);
    let mut black_back = Client::connect(&app, Some(black.token.clone()));
    assert_eq!(
        black_back.next("state")["history"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    black_back.mv("b8", "c6");
    let live = back.last("state");
    assert!(live.get("history").is_none());
    assert_eq!(live["ply"], 4);
    drop(black);
}

#[tokio::test]
async fn a_second_connection_replaces_the_first() {
    let (app, _) = new_app(HubConfig::default());
    let mut first = Client::connect(&app, None);
    let _second = Client::connect(&app, Some(first.token.clone()));
    assert_eq!(first.next("error")["code"], "replaced");
    // The stale connection can no longer act, and its later disconnect is ignored.
    first.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    app.disconnect(&first.id, first.conn);
}

#[tokio::test]
async fn private_rooms() {
    let (app, store) = new_app(HubConfig::default());
    let mut host = Client::connect(&app, None);
    let mut guest = Client::connect(&app, None);
    store.set_deck(&host.id, DECK_A).unwrap();
    host.send(ClientMsg::CreateRoom { time: None });
    let code = host.next("lobby")["status"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    guest.send(ClientMsg::JoinRoom {
        code: "NOPE1".into(),
    });
    assert_eq!(guest.next("error")["code"], "no_such_room");
    host.send(ClientMsg::JoinRoom { code: code.clone() });
    assert_eq!(host.next("error")["code"], "own_room");

    guest.send(ClientMsg::JoinRoom {
        code: code.to_lowercase(),
    });
    let _ = host.next("deck_select");
    let _ = guest.next("deck_select");
    guest.send(ClientMsg::JoinRoom { code });
    assert_eq!(guest.next("error")["code"], "already_in_game");
}

#[tokio::test]
async fn leaving_the_queue_and_dropping_out_of_it() {
    let (app, _) = new_app(HubConfig::default());
    let mut a = Client::connect(&app, None);
    let mut b = Client::connect(&app, None);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(a.next("lobby")["status"]["type"], "queued");
    a.send(ClientMsg::LeaveLobby);
    assert_eq!(a.next("lobby")["status"]["type"], "idle");
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(
        b.next("lobby")["status"]["type"],
        "queued",
        "a left, so b waits"
    );
    app.disconnect(&b.id, b.conn);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert_eq!(
        a.next("lobby")["status"]["type"],
        "queued",
        "b disconnected, so a waits"
    );
}

#[tokio::test]
async fn deck_selection_times_out_with_automatic_picks() {
    let (app, store) = new_app(HubConfig {
        deck_select_time: Duration::from_millis(100),
        ..HubConfig::default()
    });
    let mut a = Client::connect(&app, None);
    let mut b = Client::connect(&app, None);
    store.set_deck(&a.id, DECK_A).unwrap();
    store.set_deck(&b.id, DECK_B).unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    let state = a.wait_for("state").await;
    assert_eq!(state["my_skills"].as_array().unwrap().len(), 3);
    let _ = b.wait_for("state").await;
}
