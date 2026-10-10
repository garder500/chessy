//! Campaign mode (docs/spec-v6.md): levels against Sage with fixed hands,
//! stars kept per player, and the forge a beaten boss pays.

mod common;

use std::time::Duration;

use chessy_engine::forge::generate::{forge_at_least, Budget};
use chessy_engine::{parse_square, Action, SkillId};
use chessy_server::campaign::{STAR_ALL, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use chessy_server::hub::{Hub, HubConfig};
use chessy_server::protocol::{ClientMsg, ServerMsg};
use chessy_server::store::Store;
use common::*;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

fn cfg() -> HubConfig {
    HubConfig {
        bot_delay_min: Duration::from_millis(5),
        bot_delay_max: Duration::from_millis(10),
        bot_think_max: Duration::from_millis(100),
        msg_rate: 10_000.0,
        msg_burst: 10_000,
        ..HubConfig::default()
    }
}

fn level_of(campaign: &Value, id: u64) -> Value {
    campaign["levels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id)
        .unwrap_or_else(|| panic!("no level {id}"))
        .clone()
}

/// Gives every ordinary level of `chapter` all three stars, so its boss opens.
fn open_boss(store: &Store, player: &str, chapter: u8) {
    for i in 1..=6 {
        store
            .record_campaign(player, chapter * 10 + i, STAR_ALL)
            .unwrap();
    }
}

// ---- over the wire --------------------------------------------------------------

#[tokio::test]
async fn the_campaign_lists_every_level_and_opens_only_the_first() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.send(ClientMsg::CampaignGet);
    let campaign = g.next("campaign");
    let levels = campaign["levels"].as_array().unwrap();
    assert_eq!(levels.len(), 35);
    let open: Vec<u64> = levels
        .iter()
        .filter(|l| l["unlocked"] == true)
        .map(|l| l["id"].as_u64().unwrap())
        .collect();
    assert_eq!(open, vec![11]);
    assert!(levels.iter().all(|l| l["stars"] == 0));
    let l = level_of(&campaign, 25);
    assert_eq!(l["elo"], 900 + 100);
    assert_eq!(l["hand"], json!(["imune", "forcefield", "rollback"]));
    assert_eq!(l["choose"], false);
    assert_eq!(l["objective"]["kind"], "lose_fewer");
    let boss = level_of(&campaign, 27);
    assert_eq!(
        (boss["boss"] == true, boss["elo"].clone()),
        (true, json!(1150))
    );
    assert_eq!(boss["forge_min"], "rare");
    assert_eq!(level_of(&campaign, 57)["forge_min"], "legendary");
}

#[tokio::test]
async fn a_level_starts_at_once_with_the_hands_the_level_fixes() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.say(json!({"type": "campaign_start", "level": 11}));
    // No deck selection: the first thing the player gets is the game.
    let state = g.next("state");
    assert!(g.try_next("deck_select").is_none());
    assert_eq!(state["you"], "white");
    assert_eq!(state["clock_enabled"], false);
    assert_eq!(state["rated"], false);
    assert_eq!(
        state["opponent"],
        json!({"username": "Sage", "elo": 400, "guest": true, "bot": true})
    );
    assert_eq!(state["my_skills"][0]["skill"], "terminator");
    assert_eq!(state["my_skills"].as_array().unwrap().len(), 1);
    assert_eq!(state["opponent_skills"]["total"], 0);
    assert_eq!(state["campaign"]["level"], 11);
    assert_eq!(
        state["campaign"]["objective"],
        json!({"kind": "mate_before", "moves": 40})
    );
    assert_eq!(
        state["campaign"]["challenge"],
        json!({"kind": "keep_queen"})
    );
}

#[tokio::test]
async fn locked_levels_and_bad_hands_are_refused() {
    let (app, store) = new_app(cfg());
    let mut c = account(&app, &store, "alice");
    let id = c.welcome["account"]["player_id"]
        .as_str()
        .unwrap()
        .to_string();
    for (level, code) in [
        (12, "level_locked"),
        (17, "level_locked"),
        (21, "level_locked"),
        (99, "no_such_level"),
        (0, "no_such_level"),
    ] {
        c.say(json!({"type": "campaign_start", "level": level}));
        assert_eq!(c.error_code(), code, "level {level}");
    }
    // Chapter 3 lets the player choose: one to three skills of their deck.
    for i in 1..=6 {
        store.record_campaign(&id, 10 + i, STAR_ALL).unwrap();
        store.record_campaign(&id, 20 + i, STAR_ALL).unwrap();
    }
    store.record_campaign(&id, 17, STAR_WIN).unwrap();
    store.record_campaign(&id, 27, STAR_WIN).unwrap();
    store
        .set_deck(
            &id,
            &[
                SkillId::Freeze,
                SkillId::Imune,
                SkillId::Clone,
                SkillId::Mind,
            ],
        )
        .unwrap();
    for skills in [
        json!([]),
        json!(["tornado"]),
        json!(["freeze", "freeze"]),
        json!(["freeze", "imune", "clone", "rollback"]),
        json!(["mind"]),
    ] {
        c.say(json!({"type": "campaign_start", "level": 31, "skills": skills}));
        assert_eq!(c.error_code(), "invalid_deck", "{skills}");
    }
    // A hand on a level that imposes its own is refused too.
    c.say(json!({"type": "campaign_start", "level": 22, "skills": ["freeze"]}));
    assert_eq!(c.error_code(), "invalid_deck");

    c.say(json!({"type": "campaign_start", "level": 31, "skills": ["freeze", "clone"]}));
    let state = c.next("state");
    let mine: Vec<&str> = state["my_skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["skill"].as_str().unwrap())
        .collect();
    assert_eq!(
        mine,
        vec!["freeze", "clone"],
        "unique skills of the deck stay out"
    );
    // Busy: nothing else can start meanwhile.
    c.say(json!({"type": "campaign_start", "level": 31, "skills": ["freeze"]}));
    assert_eq!(c.error_code(), "already_in_game");
}

#[tokio::test]
async fn giving_up_earns_nothing_and_there_is_no_rematch() {
    let (app, store) = new_app(cfg());
    let mut c = account(&app, &store, "alice");
    let id = c.welcome["account"]["player_id"]
        .as_str()
        .unwrap()
        .to_string();
    c.say(json!({"type": "campaign_start", "level": 11}));
    c.next("state");
    c.send(ClientMsg::Resign);
    let over = c.next("game_over");
    assert_eq!(over["campaign"]["level"], 11);
    assert_eq!(over["campaign"]["earned"], 0);
    assert_eq!(over["campaign"]["total"], 0);
    assert_eq!(over["rated"], false);
    assert_eq!(over["reward"], Value::Null);
    // Nothing is stored, the next level stays shut, and the elo did not move.
    let campaign = c.next("campaign");
    assert_eq!(level_of(&campaign, 12)["unlocked"], false);
    assert!(store.campaign_progress(&id).unwrap().is_empty());
    assert_eq!(store.me(&id).unwrap().unwrap().elo, 1200);
    c.send(ClientMsg::RematchRequest);
    assert_eq!(c.error_code(), "no_rematch");
    // The level can be started again at once.
    c.say(json!({"type": "campaign_start", "level": 11}));
    c.next("state");
}

#[tokio::test]
async fn a_guest_keeps_their_progress_with_their_token() {
    let (app, store) = new_app(cfg());
    let g = guest(&app);
    store.record_campaign(&g.id, 11, STAR_WIN).unwrap();
    let mut back = Client::connect(&app, Some(g.token.clone()));
    back.send(ClientMsg::CampaignGet);
    let campaign = back.next("campaign");
    assert_eq!(level_of(&campaign, 11)["stars"], 1);
    assert_eq!(level_of(&campaign, 12)["unlocked"], true);
    // Someone else has their own campaign.
    let mut other = guest(&app);
    other.send(ClientMsg::CampaignGet);
    assert_eq!(level_of(&other.next("campaign"), 11)["stars"], 0);
}

// ---- the store -------------------------------------------------------------------

#[test]
fn stars_are_kept_at_their_best_and_never_lost() {
    let store = Store::open(":memory:").unwrap();
    let (id, _) = store.register("alice", "unused-hash", None).unwrap();
    let r = store.record_campaign(&id, 12, STAR_WIN).unwrap();
    assert_eq!((r.before, r.after, r.forge_due), (0, STAR_WIN, false));
    let r = store.record_campaign(&id, 12, STAR_CHALLENGE).unwrap();
    assert_eq!((r.before, r.after), (STAR_WIN, STAR_WIN | STAR_CHALLENGE));
    // A worse game afterwards changes nothing.
    let r = store.record_campaign(&id, 12, STAR_WIN).unwrap();
    assert_eq!(r.after, STAR_WIN | STAR_CHALLENGE);
    let p = store.campaign_progress(&id).unwrap();
    assert_eq!(p[&12].stars, STAR_WIN | STAR_CHALLENGE);
    // Only a boss owes a forge, and only the first time it is beaten.
    assert!(!store.record_campaign(&id, 11, STAR_ALL).unwrap().forge_due);
    assert!(store.record_campaign(&id, 17, STAR_WIN).unwrap().forge_due);
    assert!(!store.record_campaign(&id, 17, STAR_ALL).unwrap().forge_due);
    assert!(store.campaign_progress(&id).unwrap()[&17].forge_pending);
}

// ---- whole games, with the bot's moves put in by hand ------------------------------

struct Direct {
    hub: Hub,
    store: Store,
    id: String,
    rx: UnboundedReceiver<ServerMsg>,
    game: Option<String>,
}

fn direct() -> Direct {
    let store = Store::open(":memory:").unwrap();
    let mut hub = Hub::new(store.clone(), cfg());
    let (tx, rx) = unbounded_channel();
    let (id, _) = hub.connect(None, tx).unwrap();
    Direct {
        hub,
        store,
        id,
        rx,
        game: None,
    }
}

impl Direct {
    fn take(&mut self) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            if let ServerMsg::State(s) = &m {
                self.game = Some(s.game_id.clone());
            }
            out.push(serde_json::to_value(&m).unwrap());
        }
        out
    }

    fn mv(&mut self, from: &str, to: &str) {
        let (from, to) = (parse_square(from).unwrap(), parse_square(to).unwrap());
        self.hub.action(
            &self.id,
            Action::Move {
                from,
                to,
                promo: None,
            },
        );
    }

    fn bot(&mut self, ply: u32, from: &str, to: &str) {
        let game = self.game.clone().expect("a game is running");
        let (from, to) = (parse_square(from).unwrap(), parse_square(to).unwrap());
        self.hub.apply_bot_move(
            &game,
            ply,
            Some(Action::Move {
                from,
                to,
                promo: None,
            }),
        );
    }

    /// Fool's mate: Sage (Black) walks into it, the player mates on move 3.
    fn win_in_three(&mut self) -> Vec<Value> {
        self.take();
        self.mv("e2", "e4");
        self.take();
        self.bot(1, "f7", "f6");
        self.mv("d2", "d4");
        self.bot(3, "g7", "g5");
        self.mv("d1", "h5");
        self.take()
    }
}

fn of_type<'a>(msgs: &'a [Value], ty: &str) -> Vec<&'a Value> {
    msgs.iter().filter(|m| m["type"] == ty).collect()
}

#[tokio::test]
async fn winning_earns_the_stars_the_game_showed_and_opens_the_next_level() {
    let mut d = direct();
    d.hub.campaign_start(&d.id.clone(), 11, vec![]);
    let msgs = d.win_in_three();
    let over = of_type(&msgs, "game_over")[0];
    // Mate on move 3 (before 40) with the queen still there: all three stars.
    assert_eq!(over["campaign"]["earned"], STAR_ALL);
    assert_eq!(over["campaign"]["gained"], 3);
    assert_eq!(over["campaign"]["total"], 3);
    assert_eq!(over["campaign"]["chapter_stars"], 3);
    assert_eq!(over["campaign"]["boss_opened"], false);
    assert!(over["campaign"].get("forge").is_none());
    assert_eq!(over["rated"], false);
    assert_eq!(over["reward"], Value::Null);
    let campaign = of_type(&msgs, "campaign")[0];
    assert_eq!(level_of(campaign, 11)["stars"], STAR_ALL);
    assert_eq!(level_of(campaign, 12)["unlocked"], true);
    // The game counts for nothing else.
    let me = d.store.me(&d.id).unwrap().unwrap();
    assert_eq!((me.elo, me.games, me.wins), (1200, 0, 0));

    // Playing it again and losing keeps every star.
    d.hub.campaign_start(&d.id.clone(), 11, vec![]);
    d.take();
    d.hub.resign(&d.id.clone());
    let msgs = d.take();
    assert_eq!(of_type(&msgs, "game_over")[0]["campaign"]["earned"], 0);
    assert_eq!(
        level_of(of_type(&msgs, "campaign")[0], 11)["stars"],
        STAR_ALL
    );
}

#[tokio::test]
async fn the_objective_and_the_challenge_are_judged_apart() {
    let mut d = direct();
    // Level 12 asks for a rook; the win takes none: two stars short of three.
    d.store.record_campaign(&d.id, 11, STAR_WIN).unwrap();
    d.hub.campaign_start(&d.id.clone(), 12, vec![]);
    let msgs = d.win_in_three();
    let over = of_type(&msgs, "game_over")[0];
    assert_eq!(over["campaign"]["earned"], STAR_WIN | STAR_CHALLENGE);
    assert_eq!(
        over["campaign"]["earned"].as_u64().unwrap() & u64::from(STAR_OBJECTIVE),
        0
    );
    assert_eq!(over["campaign"]["gained"], 2);
}

#[tokio::test]
async fn the_win_that_reaches_twelve_stars_opens_the_boss() {
    let mut d = direct();
    // Five levels at all stars is 15 for the chapter already: the sixth win tips a chapter at 11.
    for i in 1..=5 {
        let stars = if i == 1 {
            STAR_WIN
        } else {
            STAR_WIN | STAR_OBJECTIVE
        };
        d.store.record_campaign(&d.id, 10 + i, stars).unwrap();
    }
    // 1 + 4 * 2 = 9 stars: level 6 is open (5 was won) and takes the chapter to 12.
    d.hub.campaign_start(&d.id.clone(), 16, vec![]);
    let msgs = d.win_in_three();
    let over = of_type(&msgs, "game_over")[0];
    assert_eq!(over["campaign"]["chapter_stars"], 9 + 3);
    assert_eq!(over["campaign"]["boss_opened"], true);
    let campaign = of_type(&msgs, "campaign")[0];
    assert_eq!(level_of(campaign, 17)["unlocked"], true);
    assert_eq!(level_of(campaign, 21)["unlocked"], false);
}

#[tokio::test]
async fn beating_a_boss_pays_a_forge_once_and_opens_the_next_chapter() {
    let mut d = direct();
    let player = d.id.clone();
    // Chapter 1 is done (its boss beaten, forge still owed); chapter 2's boss is open.
    open_boss(&d.store, &player, 1);
    d.store.record_campaign(&player, 17, STAR_WIN).unwrap();
    open_boss(&d.store, &player, 2);
    d.hub.campaign_start(&player, 27, vec![]);
    let state = d.take();
    assert!(!of_type(&state, "state").is_empty(), "{state:?}");
    let first = of_type(&state, "state")[0];
    assert_eq!(
        first["campaign"]["objective"],
        json!({"kind": "capture", "piece": "queen"})
    );
    // The boss brings two signatures and Sage plays at 1150.
    assert_eq!(first["opponent_skills"]["total"], 2);
    assert_eq!(first["opponent"]["elo"], 1150);
    // The player was handed the three skills of the chapter.
    assert_eq!(first["my_skills"].as_array().unwrap().len(), 3);

    let msgs = d.win_in_three();
    let over = of_type(&msgs, "game_over")[0];
    // No queen taken: the objective star is missing; no queen lost: the challenge holds.
    assert_eq!(over["campaign"]["earned"], STAR_WIN | STAR_CHALLENGE);
    assert_eq!(
        over["campaign"]["forge"],
        json!({"chapter": 2, "min": "rare"})
    );
    let campaign = of_type(&msgs, "campaign")[0];
    assert_eq!(level_of(campaign, 27)["forge_pending"], true);
    assert_eq!(level_of(campaign, 31)["unlocked"], true);

    // Each boss owes one forge, the lowest chapter first, of at least its floor.
    for (chapter, min) in [
        (1, chessy_engine::forge::Rarity::Uncommon),
        (2, chessy_engine::forge::Rarity::Rare),
    ] {
        let before = d.store.deck(&player).unwrap();
        let job = d
            .hub
            .begin_campaign_forge(&player, None)
            .expect("a forge is owed");
        assert_eq!(job.campaign, Some((chapter, min)));
        assert!(job.target >= min);
        // A second claim while it is being made is refused.
        assert!(d.hub.begin_campaign_forge(&player, None).is_none());
        assert!(of_type(&d.take(), "error")
            .iter()
            .any(|e| e["code"] == "forging"));
        let budget = Budget {
            attempts: 6,
            positions: Budget::live().positions,
        };
        let forged = forge_at_least(
            &mut chessy_engine::ai::Rng::new(job.seed),
            job.target,
            min,
            &job.known,
            budget,
        );
        let skill = SkillId::Forged(
            job.store
                .insert_forged(&forged.def, &forged.graded)
                .unwrap(),
        );
        d.hub
            .finish_campaign_forge(&player, chapter, None, Some(skill));
        let msgs = d.take();
        let update = of_type(&msgs, "deck_update")[0];
        assert_eq!(update["gained"], skill.to_string());
        let deck = d.store.deck(&player).unwrap();
        assert_eq!(deck.len(), before.len() + 1);
        assert!(deck.contains(&skill));
        let pending = level_of(of_type(&msgs, "campaign")[0], chapter as u64 * 10 + 7)
            ["forge_pending"]
            .clone();
        assert_eq!(pending, false);
    }

    // Nothing more is owed, even after winning the boss again.
    assert!(d.hub.begin_campaign_forge(&player, None).is_none());
    assert!(of_type(&d.take(), "error")
        .iter()
        .any(|e| e["code"] == "no_forge"));
    d.hub.campaign_start(&player, 27, vec![]);
    let msgs = d.win_in_three();
    assert!(of_type(&msgs, "game_over")[0]["campaign"]
        .get("forge")
        .is_none());
}

#[tokio::test]
async fn a_full_deck_must_say_what_the_forge_replaces() {
    let mut d = direct();
    let player = d.id.clone();
    open_boss(&d.store, &player, 1);
    d.store.record_campaign(&player, 17, STAR_WIN).unwrap();
    let full = [
        SkillId::Freeze,
        SkillId::Imune,
        SkillId::Clone,
        SkillId::Tornado,
        SkillId::Rollback,
        SkillId::Morph,
        SkillId::Bench,
    ];
    d.store.set_deck(&player, &full).unwrap();
    d.take();
    assert!(d.hub.begin_campaign_forge(&player, None).is_none());
    assert!(of_type(&d.take(), "error")
        .iter()
        .any(|e| e["code"] == "invalid_reward"));
    // A skill that is not in the deck does not do either.
    assert!(d
        .hub
        .begin_campaign_forge(&player, Some(SkillId::Wall))
        .is_none());
    assert!(of_type(&d.take(), "error")
        .iter()
        .any(|e| e["code"] == "invalid_reward"));
    // Naming one starts the forge; the skill is swapped in the same step.
    let job = d
        .hub
        .begin_campaign_forge(&player, Some(SkillId::Bench))
        .unwrap();
    assert_eq!(job.replace, Some(SkillId::Bench));
    d.hub.finish_campaign_forge(
        &player,
        1,
        Some(SkillId::Bench),
        Some(SkillId::Forged(4242)),
    );
    let deck = d.store.deck(&player).unwrap();
    assert_eq!(deck.len(), 7);
    assert!(deck.contains(&SkillId::Forged(4242)) && !deck.contains(&SkillId::Bench));
    let msgs = d.take();
    assert_eq!(of_type(&msgs, "deck_update")[0]["lost"], "bench");
}
