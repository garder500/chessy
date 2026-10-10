//! Campaign mode: levels against the bot, stars and the boss reward.

mod common;

use axum::http::StatusCode;
use chessy_engine::{parse_square, Action, Color};
use chessy_server::campaign::{LevelRef, BOSS_LEVEL, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use chessy_server::hub::{Hub, HubConfig};
use chessy_server::protocol::{CampaignInfo, RewardOffer, ServerMsg};
use chessy_server::store::Store;
use common::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

const ATTACK: u8 = 0;

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
        self.hub.campaign_start(&self.id.clone(), level);
        self.messages()
            .into_iter()
            .find_map(|m| match m {
                ServerMsg::State(s) => Some((s.game_id, s.you)),
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
        for level in 0..4 {
            let all = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;
            self.store.record_campaign(&self.id, at(level), all).unwrap();
        }
    }
}

#[test]
fn unknown_and_locked_levels_are_refused() {
    let mut p = player(true);
    for level in [
        LevelRef { chapter: 1, level: 0 },
        LevelRef { chapter: 9, level: 0 },
        at(BOSS_LEVEL + 1),
    ] {
        p.hub.campaign_start(&p.id.clone(), level);
        assert_eq!(p.error_code().as_deref(), Some("unknown_level"));
    }
    p.hub.campaign_start(&p.id.clone(), at(BOSS_LEVEL));
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
    store.record_campaign(&id, at(2), STAR_WIN | STAR_OBJECTIVE).unwrap();

    let (status, v) = api.get("/api/campaign", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(v.is_null() || v.get("chapters").is_none());

    let (status, v) = api.get("/api/campaign", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let chapters = v["chapters"].as_array().unwrap();
    assert_eq!(chapters.len(), 5);
    assert_eq!(chapters[1]["available"], false);
    assert_eq!(chapters[1]["levels"].as_array().unwrap().len(), 0);

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
    assert_eq!(levels[6]["elo"], 750);
    assert_eq!(levels[6]["boss"], true);
    assert_eq!(levels[2]["best"], serde_json::json!([true, true, false]));
    assert_eq!(levels[2]["rewarded"], false);
}
