//! Boss rewards. A boss game is ended by the bot resigning: its skills escape
//! any scripted mate, and the reward rules do not depend on how the game ends.

use chessy_engine::forge::Rarity;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use super::{Hub, HubConfig};
use crate::campaign::{LevelRef, BOSS_LEVEL, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use crate::protocol::{RewardOffer, ServerMsg};
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
        store.record_campaign(&id, LevelRef { chapter: 0, level }, all).unwrap();
    }
    Player { hub, store, id, rx }
}

impl Player {
    /// Starts the boss and has the bot resign: the human wins.
    fn beat_boss(&mut self) -> Option<RewardOffer> {
        self.hub.campaign_start(&self.id.clone(), BOSS);
        let game_id = self.hub.player_game[&self.id].clone();
        let bot = self.hub.games[&game_id].solo.as_ref().unwrap().bot;
        self.hub.end_by_resignation(&game_id, bot, "resignation");
        std::iter::from_fn(|| self.rx.try_recv().ok()).find_map(|m| match m {
            ServerMsg::GameOver { reward, .. } => Some(reward),
            _ => None,
        })?
    }
}

#[test]
fn the_boss_pays_a_forge_once() {
    let mut p = player(true);
    let offer = p.beat_boss().expect("first boss win is rewarded");
    assert!(offer.steal_options.is_empty());

    let job = p.hub.begin_forge(&p.id.clone(), None).expect("a forge job");
    assert_eq!(job.range, Rarity::Uncommon..=Rarity::Epic);
    assert!(job.range.contains(&job.target));

    assert!(p.beat_boss().is_none(), "no second reward");
    let rows = p.store.campaign_rows(&p.id).unwrap();
    assert!(rows.iter().any(|r| r.at.is_boss() && r.rewarded));
}

#[test]
fn a_guest_beats_the_boss_without_a_forge() {
    let mut p = player(false);
    assert!(p.beat_boss().is_none());
    let rows = p.store.campaign_rows(&p.id).unwrap();
    assert!(rows.iter().any(|r| r.at.is_boss() && !r.rewarded));
}
