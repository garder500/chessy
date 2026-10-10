//! Campaign mode: a Solo game against the bot with imposed decks, and what a
//! finished one earns (stars, boss reward). See `docs/spec-campagne.md`.

use std::time::Instant;

use chessy_engine::Color;

use super::{Hub, PendingReward, Phase, Session};
use crate::campaign::{self, LevelRef};
use crate::protocol::{CampaignInfo, RewardOffer};

/// What `finish_game` sends the human of a finished campaign game.
pub(super) struct CampaignResult {
    pub player: String,
    pub info: CampaignInfo,
    pub reward: Option<RewardOffer>,
}

impl Hub {
    pub fn campaign_start(&mut self, player: &str, at: LevelRef) {
        if !self.ensure_idle(player) {
            return;
        }
        if campaign::level(at).is_none() {
            return self.fail(player, "unknown_level", "this level does not exist yet");
        }
        let rows = match self.store.campaign_rows(player) {
            Ok(rows) => rows,
            Err(e) => return self.internal_error(player, e),
        };
        if at.is_boss() && !campaign::boss_unlocked(&rows, at.chapter) {
            return self.fail(player, "boss_locked", "the boss is still locked");
        }
        self.start_solo(player, at.elo(), Self::random_color(), Some(at));
    }

    /// Records the stars of a finished campaign game and prepares the boss
    /// reward; `None` when the game was not a campaign level.
    pub(super) fn finish_campaign(
        &mut self,
        session: &Session,
        winner: Option<Color>,
    ) -> Option<CampaignResult> {
        let solo = session.solo.as_ref()?;
        let at = solo.campaign?;
        let Phase::Playing { game } = &session.phase else {
            return None;
        };
        let human = solo.bot.opposite();
        let player = session.players[human.index()].clone();
        let won = winner == Some(human);
        let plies = session.recording.actions.len();
        let earned = campaign::stars_earned(at, game, human, won, plies);
        if earned != 0 {
            if let Err(e) = self.store.record_campaign(&player, at, earned) {
                tracing::error!("could not record campaign progress: {e}");
            }
        }
        let rows = self.store.campaign_rows(&player).unwrap_or_default();
        let best = rows.iter().find(|r| r.at == at).map_or(0, |r| r.stars);
        let reward = if won && at.is_boss() {
            self.boss_reward(&player, at)
        } else {
            None
        };
        Some(CampaignResult {
            info: CampaignInfo {
                chapter: at.chapter,
                level: at.level,
                stars: campaign::star_flags(earned),
                best: campaign::star_flags(best),
                chapter_stars: campaign::chapter_stars(&rows, at.chapter),
                boss_unlocked: campaign::boss_unlocked(&rows, at.chapter),
            },
            player,
            reward,
        })
    }

    /// The forged skill of a boss, offered once per account.
    fn boss_reward(&mut self, player: &str, at: LevelRef) -> Option<RewardOffer> {
        let is_account = matches!(self.store.player_row(player), Ok(Some(row)) if row.username.is_some());
        if !is_account || !self.store.claim_campaign_reward(player, at).unwrap_or(false) {
            return None;
        }
        let offer = self.offer_for(player, None, &[]).ok()?;
        self.rewards.insert(
            player.to_string(),
            PendingReward {
                forging: false,
                loser: None,
                range: campaign::rarity_range(at.chapter),
                loser_deck: Vec::new(),
                created: Instant::now(),
            },
        );
        Some(offer)
    }
}
