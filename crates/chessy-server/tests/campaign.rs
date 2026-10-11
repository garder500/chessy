//! Campaign mode: levels against the bot, stars and the boss reward.

mod common;

use axum::http::StatusCode;
use chessy_engine::{parse_square, Action, Color, SkillId};
use chessy_server::campaign::{LevelRef, BOSS_LEVEL, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use chessy_server::hub::Timer;
use chessy_server::hub::{Hub, HubConfig};
use chessy_server::protocol::{CampaignInfo, ClientMsg, RewardOffer, ServerMsg, StateView};
use chessy_server::store::Store;
use common::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

const ATTACK: u8 = 0;
const DEFENSE: u8 = 1;
const MOBILITY: u8 = 2;
const CREATE: u8 = 4;
const ALL_STARS: u8 = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;
const OWNED: [SkillId; 4] = [
    SkillId::Mirage,
    SkillId::Wall,
    SkillId::Freeze,
    SkillId::Morph,
];
const DEFENSE_BOSS_FEN: &str = "rnbqkbnr/pppppppp/2p2p2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn at(level: u8) -> LevelRef {
    LevelRef {
        chapter: ATTACK,
        level,
    }
}

struct Player {
    hub: Hub,
    store: Store,
    id: String,
    rx: UnboundedReceiver<ServerMsg>,
}

/// A connected player, an account or a guest, on a hub of its own.
fn player(account: bool) -> Player {
    let store = Store::open(":memory:").unwrap();
    let mut hub = Hub::new(store.clone(), HubConfig::default());
    let token = account.then(|| store.register("ana", "unused-hash", None).unwrap().1);
    let (tx, rx) = unbounded_channel();
    let (id, _) = hub.connect(token, tx).unwrap();
    Player { hub, store, id, rx }
}

struct Finished {
    campaign: Option<CampaignInfo>,
    reward: Option<RewardOffer>,
}

impl Player {
    fn messages(&mut self) -> Vec<ServerMsg> {
        std::iter::from_fn(|| self.rx.try_recv().ok()).collect()
    }

    fn error_code(&mut self) -> Option<String> {
        self.messages().into_iter().find_map(|m| match m {
            ServerMsg::Error { code, .. } => Some(code),
            _ => None,
        })
    }

    fn start(&mut self, level: LevelRef) -> (String, Color) {
        let state = self.start_with(level, None);
        (state.game_id, state.you)
    }

    fn start_with(&mut self, level: LevelRef, deck: Option<Vec<SkillId>>) -> StateView {
        self.hub.campaign_start(&self.id.clone(), level, deck);
        self.messages()
            .into_iter()
            .find_map(|m| match m {
                ServerMsg::State(s) => Some(*s),
                _ => None,
            })
            .expect("the game starts without deck selection")
    }

    fn play(&mut self, from: &str, to: &str) {
        let (from, to) = (parse_square(from).unwrap(), parse_square(to).unwrap());
        let action = Action::Move {
            from,
            to,
            promo: None,
        };
        self.hub.action(&self.id.clone(), action);
    }

    fn bot_plays(&mut self, game: &str, ply: u32, from: &str, to: &str) {
        let (from, to) = (parse_square(from).unwrap(), parse_square(to).unwrap());
        let action = Action::Move {
            from,
            to,
            promo: None,
        };
        self.hub.apply_bot_move(game, ply, Some(action));
    }

    /// Wins in a few plies against a bot that blunders into a mate.
    fn win(&mut self, level: LevelRef) -> Finished {
        let (game, color) = self.start(level);
        match color {
            Color::White => {
                self.play("e2", "e4");
                self.bot_plays(&game, 1, "f7", "f6");
                self.play("d2", "d4");
                self.bot_plays(&game, 3, "g7", "g5");
                self.play("d1", "h5");
            }
            Color::Black => {
                self.bot_plays(&game, 0, "f2", "f3");
                self.play("e7", "e5");
                self.bot_plays(&game, 2, "g2", "g4");
                self.play("d8", "h4");
            }
        }
        self.finished()
    }

    fn lose(&mut self, level: LevelRef) -> Finished {
        self.start(level);
        self.hub.resign(&self.id.clone());
        self.finished()
    }

    fn finished(&mut self) -> Finished {
        self.messages()
            .into_iter()
            .find_map(|m| match m {
                ServerMsg::GameOver {
                    campaign, reward, ..
                } => Some(Finished { campaign, reward }),
                _ => None,
            })
            .expect("the game is over")
    }

    fn open_boss(&mut self) {
        self.open_boss_of(ATTACK);
    }

    fn open_boss_of(&mut self, chapter: u8) {
        for level in 0..4 {
            let at = LevelRef { chapter, level };
            self.store.record_campaign(&self.id, at, ALL_STARS).unwrap();
        }
    }
}

#[test]
fn unknown_and_locked_levels_are_refused() {
    let mut p = player(true);
    for level in [
        LevelRef {
            chapter: 5,
            level: 0,
        },
        LevelRef {
            chapter: 9,
            level: 0,
        },
        at(BOSS_LEVEL + 1),
    ] {
        p.hub.campaign_start(&p.id.clone(), level, None);
        assert_eq!(p.error_code().as_deref(), Some("unknown_level"));
    }
    p.hub.campaign_start(&p.id.clone(), at(BOSS_LEVEL), None);
    assert_eq!(p.error_code().as_deref(), Some("boss_locked"));
}

#[test]
fn a_win_gives_stars_that_cumulate_over_attempts() {
    let mut p = player(true);
    let first = p.win(at(0)).campaign.expect("campaign result");
    assert_eq!(first.stars, [true, false, true]);
    assert_eq!(first.best, [true, false, true]);
    assert_eq!(first.chapter_stars, 2);
    assert!(!first.boss_unlocked);

    let lost = p.lose(at(0)).campaign.expect("campaign result");
    assert_eq!(lost.stars, [false; 3]);
    assert_eq!(lost.best, [true, false, true]);

    let rows = p.store.campaign_rows(&p.id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].stars, STAR_WIN | STAR_CHALLENGE);
}

#[test]
fn a_loss_earns_nothing_and_a_campaign_game_is_not_rated() {
    let mut p = player(true);
    let over = p.lose(at(0));
    assert_eq!(over.campaign.unwrap().stars, [false; 3]);
    assert!(over.reward.is_none());
    assert!(p.store.campaign_rows(&p.id).unwrap().is_empty());
}

#[test]
fn the_boss_opens_at_twelve_stars() {
    let mut p = player(true);
    p.open_boss();
    let over = p.lose(at(BOSS_LEVEL));
    let info = over.campaign.unwrap();
    assert_eq!(info.level, BOSS_LEVEL);
    assert!(info.boss_unlocked);
    assert!(over.reward.is_none());
}

#[tokio::test]
async fn the_rest_api_lists_the_levels_and_the_progress() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("ana").await;
    let id = store.player_by_token(&token).unwrap().unwrap();
    store
        .record_campaign(&id, at(2), STAR_WIN | STAR_OBJECTIVE)
        .unwrap();

    let (status, v) = api.get("/api/campaign", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(v.is_null() || v.get("chapters").is_none());

    let (status, v) = api.get("/api/campaign", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let chapters = v["chapters"].as_array().unwrap();
    assert_eq!(chapters.len(), 5);
    assert_eq!(chapters[1]["available"], true);
    assert_eq!(chapters[1]["levels"].as_array().unwrap().len(), 7);

    let attack = &chapters[0];
    assert_eq!(attack["family"], "attack");
    assert_eq!(attack["available"], true);
    assert_eq!(attack["stars"], 2);
    assert_eq!(attack["boss_stars_required"], 12);
    assert_eq!(attack["boss_unlocked"], false);
    let levels = attack["levels"].as_array().unwrap();
    assert_eq!(levels.len(), 7);
    assert_eq!(levels[0]["elo"], 400);
    assert_eq!(levels[5]["elo"], 650);
    assert_eq!(levels[6]["elo"], 800);
    assert_eq!(levels[6]["boss"], true);
    assert_eq!(levels[2]["best"], serde_json::json!([true, true, false]));
    assert_eq!(levels[2]["rewarded"], false);
}

#[tokio::test]
async fn a_guest_cannot_read_the_campaign_over_rest() {
    let (app, _) = new_app(HubConfig::default());
    let guest = guest(&app);

    let (status, _) = Api::new(&app)
        .get("/api/campaign", Some(&guest.token))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

const CHOICE: LevelRef = LevelRef {
    chapter: MOBILITY,
    level: 0,
};

fn player_with_deck() -> Player {
    let p = player(true);
    p.store.set_deck(&p.id, &OWNED).unwrap();
    p
}

fn skills_of(state: &StateView) -> Vec<SkillId> {
    state.my_skills.iter().map(|s| s.skill).collect()
}

#[test]
fn a_chosen_deck_is_played_and_ignored_on_imposed_levels() {
    let mut p = player_with_deck();
    let chosen = vec![SkillId::Wall, SkillId::Mirage];
    let state = p.start_with(CHOICE, Some(chosen.clone()));
    assert_eq!(skills_of(&state), chosen);
    p.hub.resign(&p.id.clone());
    p.messages();

    let state = p.start_with(at(0), Some(vec![SkillId::Wall]));
    let imposed = chessy_server::campaign::level(at(0)).unwrap().player_deck;
    assert_eq!(skills_of(&state), imposed);
}

#[test]
fn a_bad_deck_is_refused() {
    let mut p = player_with_deck();
    let not_owned = vec![SkillId::Godhelp];
    let too_many = OWNED.to_vec();
    let twice = vec![SkillId::Wall, SkillId::Wall];
    for deck in [
        Some(vec![]),
        Some(too_many),
        Some(twice),
        Some(not_owned),
        None,
    ] {
        p.hub.campaign_start(&p.id.clone(), CHOICE, deck);
        let sent = p.messages();
        assert!(sent
            .iter()
            .any(|m| matches!(m, ServerMsg::Error { code, .. } if code == "bad_deck")));
        assert!(!sent.iter().any(|m| matches!(m, ServerMsg::State(_))));
    }
}

#[test]
fn a_rematch_is_refused_when_the_chosen_deck_is_no_longer_owned() {
    let mut p = player_with_deck();
    p.start_with(CHOICE, Some(vec![SkillId::Freeze]));
    p.hub.resign(&p.id.clone());
    p.messages();
    p.store
        .set_deck(&p.id, &[SkillId::Wall, SkillId::Mirage])
        .unwrap();
    p.hub.rematch_request(&p.id.clone());
    let sent = p.messages();
    assert!(sent
        .iter()
        .any(|m| matches!(m, ServerMsg::Error { code, .. } if code == "bad_deck")));
    assert!(!sent.iter().any(|m| matches!(m, ServerMsg::State(_))));
}

#[test]
fn a_rematch_keeps_the_chosen_deck() {
    let mut p = player_with_deck();
    let chosen = vec![SkillId::Freeze];
    p.start_with(CHOICE, Some(chosen.clone()));
    p.hub.resign(&p.id.clone());
    p.messages();
    p.hub.rematch_request(&p.id.clone());
    let state = p
        .messages()
        .into_iter()
        .find_map(|m| match m {
            ServerMsg::State(s) => Some(*s),
            _ => None,
        })
        .expect("the rematch starts");
    assert_eq!(skills_of(&state), chosen);
}

#[test]
fn a_custom_start_boss_begins_on_its_position_and_the_bot_moves_first() {
    let mut p = player_with_deck();
    p.open_boss_of(CREATE);
    let boss = LevelRef {
        chapter: CREATE,
        level: BOSS_LEVEL,
    };
    p.hub.take_timers();
    let state = p.start_with(boss, Some(vec![SkillId::Wall]));
    assert_eq!(state.you, Color::Black);
    assert_eq!(state.to_move, Color::White);
    assert!(state.board[18].is_some() && state.board[21].is_some());
    let timers = p.hub.take_timers();
    assert!(timers
        .iter()
        .any(|(_, t)| matches!(t, Timer::BotMove { ply: 0, .. })));
}

#[tokio::test]
async fn the_replay_of_a_custom_start_boss_starts_from_its_position() {
    let (app, store) = new_app(HubConfig::default());
    let mut c = account(&app, &store, "ana");
    for level in 0..4 {
        let at = LevelRef {
            chapter: DEFENSE,
            level,
        };
        store.record_campaign(&c.id, at, ALL_STARS).unwrap();
    }
    c.send(ClientMsg::CampaignStart {
        chapter: DEFENSE,
        level: BOSS_LEVEL,
        deck: None,
    });
    let state = c.next("state");
    assert_eq!(state["you"], "white");
    assert!(!state["board"][42].is_null());
    c.mv("e2", "e4");
    c.send(ClientMsg::Resign);

    let id = state["game_id"].as_str().unwrap();
    let (status, replay) = Api::new(&app)
        .get(&format!("/api/games/{id}"), Some(&c.token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["loadouts"]["start"], DEFENSE_BOSS_FEN);
    assert!(!replay["frames"][0]["board"][42].is_null());
    assert_eq!(replay["moves"][0]["notation"], "e4");
}

#[test]
fn a_lost_boss_game_gives_no_title() {
    let mut p = player(true);
    p.open_boss();
    assert_eq!(p.lose(at(BOSS_LEVEL)).campaign.unwrap().title, None);
    assert_eq!(p.store.public_profile("ana").unwrap().unwrap().title, None);
}

#[test]
fn the_boss_unlock_is_announced_once() {
    let mut p = player(true);
    for level in 1..4 {
        p.store
            .record_campaign(&p.id, at(level), ALL_STARS)
            .unwrap();
    }
    p.store
        .record_campaign(&p.id, at(0), STAR_WIN | STAR_OBJECTIVE)
        .unwrap();
    let first = p.win(at(0)).campaign.unwrap();
    assert!(first.boss_unlocked && first.boss_just_unlocked);
    assert_eq!(first.boss_stars_required, 12);
    let second = p.win(at(0)).campaign.unwrap();
    assert!(second.boss_unlocked && !second.boss_just_unlocked);
}

#[tokio::test]
async fn the_rest_api_describes_titles_decks_and_starts() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("ana").await;
    let id = store.player_by_token(&token).unwrap().unwrap();
    for level in 0..4 {
        store.record_campaign(&id, at(level), ALL_STARS).unwrap();
    }
    store
        .record_campaign(&id, at(BOSS_LEVEL), ALL_STARS)
        .unwrap();

    let (_, v) = api.get("/api/campaign", Some(&token)).await;
    let chapters = v["chapters"].as_array().unwrap();
    assert_eq!(chapters[0]["title"], "Tombeur du Bélier");
    assert_eq!(chapters[0]["title_earned"], true);
    assert_eq!(chapters[1]["title_earned"], false);
    let boss = |c: usize| &chapters[c]["levels"][usize::from(BOSS_LEVEL)];
    assert_eq!(boss(0)["deck_choice"], false);
    assert!(boss(0)["start_fen"].is_null() && boss(0)["human_color"].is_null());
    assert_eq!(boss(1)["start_fen"], DEFENSE_BOSS_FEN);
    assert_eq!(boss(1)["human_color"], "white");
    assert_eq!(boss(4)["deck_choice"], true);
    assert_eq!(boss(4)["human_color"], "black");
    assert_eq!(chapters[2]["levels"][0]["deck_choice"], true);
}
