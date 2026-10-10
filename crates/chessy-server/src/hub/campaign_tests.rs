//! Boss rewards. A boss game is ended by the bot resigning: its skills escape
//! any scripted mate, and the reward rules do not depend on how the game ends.

use chessy_engine::forge::Rarity;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use super::{Hub, HubConfig};
use crate::campaign::{LevelRef, BOSS_LEVEL, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use crate::protocol::{CampaignInfo, DevResult, RewardChoice, RewardOffer, ServerMsg, SoloColor};
use crate::store::Store;

const BOSS: LevelRef = LevelRef {
    chapter: 0,
    level: BOSS_LEVEL,
};

struct Player {
    hub: Hub,
    store: Store,
    id: String,
    rx: UnboundedReceiver<ServerMsg>,
}

/// A connected player on a hub of its own, with the boss open.
fn player(account: bool) -> Player {
    let store = Store::open(":memory:").unwrap();
    let mut hub = Hub::new(store.clone(), HubConfig::default());
    let token = account.then(|| store.register("ana", "unused-hash", None).unwrap().1);
    let (tx, rx) = unbounded_channel();
    let (id, _) = hub.connect(token, tx).unwrap();
    for level in 0..4 {
        let all = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;
        store
            .record_campaign(&id, LevelRef { chapter: 0, level }, all)
            .unwrap();
    }
    Player { hub, store, id, rx }
}

impl Player {
    /// Starts the boss and has the bot resign: the human wins.
    fn beat_boss(&mut self) -> Option<RewardOffer> {
        self.beat_boss_over().1
    }

    fn beat_boss_over(&mut self) -> (Option<CampaignInfo>, Option<RewardOffer>) {
        self.hub.campaign_start(&self.id.clone(), BOSS, None);
        let game_id = self.hub.player_game[&self.id].clone();
        let bot = self.hub.games[&game_id].solo.as_ref().unwrap().bot;
        self.hub.end_by_resignation(&game_id, bot, "resignation");
        std::iter::from_fn(|| self.rx.try_recv().ok())
            .find_map(|m| match m {
                ServerMsg::GameOver {
                    campaign, reward, ..
                } => Some((campaign, reward)),
                _ => None,
            })
            .expect("the game is over")
    }
}

impl Player {
    fn boss_rewarded(&self) -> bool {
        let rows = self.store.campaign_rows(&self.id).unwrap();
        rows.iter().any(|r| r.at.is_boss() && r.rewarded)
    }
}

const FRESH_LEVEL: LevelRef = LevelRef {
    chapter: 1,
    level: 0,
};

impl Player {
    fn fresh_level_stars(&self) -> u8 {
        let rows = self.store.campaign_rows(&self.id).unwrap();
        rows.iter()
            .find(|r| r.at == FRESH_LEVEL)
            .map_or(0, |r| r.stars)
    }
}

#[test]
fn dev_finish_win_records_the_victory_star() {
    let mut p = player(false);
    p.hub.campaign_start(&p.id.clone(), FRESH_LEVEL, None);
    p.hub.dev_finish(&p.id.clone(), DevResult::Win);
    assert_ne!(p.fresh_level_stars() & STAR_WIN, 0);
}

#[test]
fn dev_finish_all_stars_records_every_star_of_the_level() {
    let mut p = player(false);
    p.hub.campaign_start(&p.id.clone(), FRESH_LEVEL, None);
    p.hub.dev_finish(&p.id.clone(), DevResult::AllStars);
    assert_eq!(
        p.fresh_level_stars(),
        STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE
    );
}

#[test]
fn dev_finish_all_stars_on_a_boss_records_only_the_victory_star() {
    let mut p = player(true);
    p.hub.campaign_start(&p.id.clone(), BOSS, None);
    p.hub.dev_finish(&p.id.clone(), DevResult::AllStars);
    let rows = p.store.campaign_rows(&p.id).unwrap();
    let boss = rows.iter().find(|r| r.at == BOSS).unwrap();
    assert_eq!(boss.stars, STAR_WIN);
    assert!(
        p.hub.rewards.contains_key(&p.id),
        "the boss reward is offered"
    );
}

#[test]
fn dev_finish_loss_records_no_star() {
    let mut p = player(false);
    p.hub.campaign_start(&p.id.clone(), FRESH_LEVEL, None);
    p.hub.dev_finish(&p.id.clone(), DevResult::Loss);
    assert_eq!(p.fresh_level_stars(), 0);
}

#[test]
fn dev_finish_is_refused_outside_a_campaign_game() {
    let mut p = player(false);
    p.hub.solo_start(&p.id.clone(), 400, SoloColor::White);
    p.hub.dev_finish(&p.id.clone(), DevResult::Win);
    assert!(p.hub.player_game.contains_key(&p.id), "the game goes on");
}

#[test]
fn a_boss_win_gives_the_chapter_title_and_shows_on_the_profile() {
    let mut p = player(true);
    assert_eq!(p.store.public_profile("ana").unwrap().unwrap().title, None);
    let (info, _) = p.beat_boss_over();
    assert_eq!(info.unwrap().title.as_deref(), Some("Fer de Lance"));
    let profile = p.store.public_profile("ana").unwrap().unwrap();
    assert_eq!(profile.title, Some("Fer de Lance"));
}

#[test]
fn an_unresolved_boss_offer_is_made_again() {
    let mut p = player(true);
    let offer = p.beat_boss().expect("first boss win is rewarded");
    assert!(offer.steal_options.is_empty());

    let job = p.hub.begin_forge(&p.id.clone(), None).expect("a forge job");
    assert_eq!(job.range, Rarity::Uncommon..=Rarity::Epic);
    assert!(job.range.contains(&job.target));
    assert!(!p.boss_rewarded(), "the offer is not resolved yet");

    p.hub.finish_forge(&p.id.clone(), None, None);
    assert!(!p.boss_rewarded(), "a failed forge leaves the offer open");

    assert!(
        p.beat_boss().is_some(),
        "a lost offer comes back on the next win"
    );
}

#[test]
fn the_boss_pays_once_resolved() {
    let mut p = player(true);
    p.beat_boss().expect("first boss win is rewarded");
    p.hub.reward_choice(&p.id.clone(), RewardChoice::Skip);
    assert!(p.boss_rewarded());
    assert!(p.beat_boss().is_none(), "no second reward");
}

#[test]
fn a_guest_beats_the_boss_without_a_forge() {
    let mut p = player(false);
    assert!(p.beat_boss().is_none());
    let rows = p.store.campaign_rows(&p.id).unwrap();
    assert!(rows.iter().any(|r| r.at.is_boss() && !r.rewarded));
}
