//! Campaign mode: a Solo game against the bot with imposed decks, and what a
//! finished one earns (stars, boss reward). See `docs/spec-campagne.md`.

use std::time::Instant;

use chessy_engine::{Color, SkillId};

use super::{Hub, PendingReward, Phase, Session};
use crate::campaign::{self, LevelRef, BOSS_STARS, CHAPTERS, HINT_AFTER_DEFEATS};
use crate::protocol::{CampaignInfo, DevResult, RewardOffer};

/// Release builds (`make serve`, Docker) never end a game through `dev_finish`.
const DEV_SHORTCUTS_ENABLED: bool = cfg!(debug_assertions);

/// What `finish_game` sends the human of a finished campaign game.
pub(super) struct CampaignResult {
    pub player: String,
    pub info: CampaignInfo,
    pub reward: Option<RewardOffer>,
}

impl Hub {
    /// `chosen` is the deck brought to a `deck_choice` level; elsewhere the
    /// level imposes the deck and `chosen` is ignored.
    pub fn campaign_start(&mut self, player: &str, at: LevelRef, chosen: Option<Vec<SkillId>>) {
        if !self.ensure_idle(player) {
            return;
        }
        if !self.is_account(player) {
            return self.fail(
                player,
                "account_required",
                "la campagne demande un compte : connecte-toi ou inscris-toi",
            );
        }
        let Some(level) = campaign::level(at) else {
            return self.fail(player, "unknown_level", "this level does not exist yet");
        };
        let rows = match self.store.campaign_rows(player) {
            Ok(rows) => rows,
            Err(e) => return self.internal_error(player, e),
        };
        if at.is_boss() && !campaign::boss_unlocked(&rows, at.chapter) {
            return self.fail(player, "boss_locked", "the boss is still locked");
        }
        let Some(deck) = self.campaign_hand(player, level, chosen.unwrap_or_default()) else {
            return;
        };
        let human = level
            .start
            .as_ref()
            .map_or_else(Self::random_color, |start| start.human);
        self.start_solo(player, at.elo(), human, Some(at), deck);
    }

    fn is_account(&self, player: &str) -> bool {
        matches!(self.store.player_row(player), Ok(Some(row)) if row.username.is_some())
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
        let earned = if won && solo.dev_all_stars {
            campaign::all_stars(at)
        } else {
            campaign::stars_earned(at, game, human, won)
        };
        // A read error counts as already unlocked: never announce a false unlock.
        let rows_before = self.store.campaign_rows(&player).ok();
        let was_unlocked = rows_before
            .as_ref()
            .is_none_or(|rows| campaign::boss_unlocked(rows, at.chapter));
        let had_title = rows_before
            .as_ref()
            .is_none_or(|rows| campaign::title_earned(rows, at.chapter));
        // A defeat, a draw or a resignation never takes a star back.
        let hint_available = if won {
            if let Err(e) = self.store.record_campaign(&player, at, earned) {
                tracing::error!("could not record campaign progress: {e}");
            }
            false
        } else {
            self.record_defeat(&player, at) >= HINT_AFTER_DEFEATS
        };
        let rows = self.store.campaign_rows(&player).unwrap_or_default();
        let best = rows.iter().find(|r| r.at == at).map_or(0, |r| r.stars);
        let boss_unlocked = campaign::boss_unlocked(&rows, at.chapter);
        let already_rewarded = rows.iter().any(|r| r.at == at && r.rewarded);
        let reward = if won && at.is_boss() && !already_rewarded {
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
                boss_unlocked,
                boss_stars_required: BOSS_STARS,
                boss_just_unlocked: boss_unlocked && !was_unlocked,
                title: (!had_title && campaign::title_earned(&rows, at.chapter))
                    .then(|| CHAPTERS[usize::from(at.chapter)].title.to_string()),
                total_stars: campaign::total_stars(&rows),
                hint_available,
                boss_forge: None,
            },
            player,
            reward,
        })
    }

    /// Defeats in a row on the level; a store error counts as none.
    fn record_defeat(&self, player: &str, at: LevelRef) -> u32 {
        self.store
            .record_campaign_defeat(player, at.chapter, at.level)
            .unwrap_or_else(|e| {
                tracing::error!("could not record a campaign defeat: {e}");
                0
            })
    }

    /// Debug builds only: ends the campaign game in progress, won or lost by the human.
    pub fn dev_finish(&mut self, player: &str, result: DevResult) {
        if !DEV_SHORTCUTS_ENABLED {
            return self.fail(
                player,
                "dev_only",
                "this shortcut exists in debug builds only",
            );
        }
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return self.fail(player, "not_in_game", "you are not in a game");
        };
        let campaign_solo = match self.games.get_mut(&game_id) {
            Some(Session {
                solo: Some(solo),
                phase: Phase::Playing { .. },
                ..
            }) if solo.campaign.is_some() => Some(solo),
            _ => None,
        };
        let Some(solo) = campaign_solo else {
            return self.fail(player, "not_campaign", "you are not in a campaign game");
        };
        solo.dev_all_stars = result == DevResult::AllStars;
        let bot = solo.bot;
        let loser = if result == DevResult::Loss {
            bot.opposite()
        } else {
            bot
        };
        self.end_by_resignation(&game_id, loser, "resignation");
    }

    /// The forged skill of a boss, offered until the account has resolved it
    /// (a lost offer is made again on the next win).
    fn boss_reward(&mut self, player: &str, at: LevelRef) -> Option<RewardOffer> {
        let offer = self.offer_for(player, None, &[]).ok()?;
        self.rewards.insert(
            player.to_string(),
            PendingReward {
                forging: false,
                loser: None,
                range: campaign::rarity_range(at.chapter),
                loser_deck: Vec::new(),
                boss: Some(at),
                created: Instant::now(),
            },
        );
        Some(offer)
    }

    /// Called once a reward is resolved (taken, forged or skipped).
    pub(super) fn mark_boss_rewarded(&self, player: &str, pending: &PendingReward) {
        let Some(at) = pending.boss else {
            return;
        };
        if let Err(e) = self.store.mark_campaign_rewarded(player, at) {
            tracing::error!("could not mark the boss reward as given: {e}");
        }
    }
}
