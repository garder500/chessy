//! Ranked rewards: persisted while waiting for the winner's choice, resolved
//! into a deck change, and announced to the loser.

use std::time::{Duration, Instant};

use chessy_engine::forge::Rarity;
use chessy_engine::SkillId;
use rand::seq::IndexedRandom;

use super::spectate::iso;
use super::{now_unix, ForgeJob, Hub, PendingReward, MAX_DECK};
use crate::pending_rewards_store::PendingRewardRow;
use crate::protocol::*;
use crate::reward_outcome_store::RewardOutcomeRow;
use crate::store::StoreError;

/// Unix seconds of `2026-10-07T12:00:00Z`.
fn unix_of_iso(iso: &str) -> Option<i64> {
    let field = |from: usize, to: usize| iso.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (field(0, 4)?, field(5, 7)?, field(8, 10)?);
    let (hour, minute, second) = (field(11, 13)?, field(14, 16)?, field(17, 19)?);
    // Days since 1970-01-01 from a civil date (Howard Hinnant's algorithm).
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

impl Hub {
    /// Writes a ranked reward to the database so it survives a restart. A
    /// reward taken from nobody is not kept.
    pub(super) fn save_pending_reward(&self, winner: &str, pending: &PendingReward) {
        let Some(loser) = &pending.loser else {
            return;
        };
        let remaining = self
            .config
            .reward_ttl
            .saturating_sub(pending.created.elapsed());
        let row = PendingRewardRow {
            winner_id: winner.to_string(),
            loser_id: loser.clone(),
            loser_skills: pending.loser_deck.clone(),
            expires_at: iso(now_unix() + remaining.as_secs() as i64),
        };
        if let Err(e) = self.store.save_pending_reward(&row) {
            tracing::error!("could not save the reward of {winner}: {e}");
        }
    }

    /// The reward is resolved or expired: forgets it for good.
    pub(super) fn delete_pending_reward(&self, winner: &str) {
        if let Err(e) = self.store.delete_pending_reward(winner) {
            tracing::error!("could not delete the reward of {winner}: {e}");
        }
    }

    /// Puts back the rewards that were waiting when the server stopped.
    pub fn restore_rewards(&mut self) {
        let rows = match self.store.load_pending_rewards() {
            Ok(rows) => rows,
            Err(e) => return tracing::error!("could not load the pending rewards: {e}"),
        };
        let now = now_unix();
        for row in rows {
            let Some(expires) = unix_of_iso(&row.expires_at) else {
                continue;
            };
            let remaining = Duration::from_secs((expires - now).max(0) as u64);
            let elapsed = self.config.reward_ttl.saturating_sub(remaining);
            self.rewards.insert(
                row.winner_id,
                PendingReward {
                    forging: false,
                    loser: Some(row.loser_id),
                    range: Rarity::Common..=Rarity::Legendary,
                    loser_deck: row.loser_skills,
                    created: Instant::now()
                        .checked_sub(elapsed)
                        .unwrap_or_else(Instant::now),
                },
            );
        }
    }

    /// Announcements for losers who were offline, for the async caller to
    /// store: the hub itself does no extra I/O for them.
    pub fn take_reward_outcome_pushes(&mut self) -> Vec<(PlayerId, RewardOutcomeRow)> {
        std::mem::take(&mut self.reward_outcome_pushes)
    }

    fn announce_outcome(
        &mut self,
        winner: &str,
        loser: &str,
        kind: RewardOutcomeKind,
        skill: Option<SkillId>,
        refilled: Option<SkillId>,
    ) {
        let Some(by) = self.name_of(winner) else {
            return;
        };
        if self.conns.contains_key(loser) {
            return self.send(
                loser,
                ServerMsg::RewardOutcome {
                    by,
                    kind,
                    skill,
                    refilled,
                },
            );
        }
        let row = RewardOutcomeRow {
            by,
            kind,
            skill,
            refilled,
        };
        self.reward_outcome_pushes.push((loser.to_string(), row));
    }

    pub fn reward_choice(&mut self, player: &str, choice: RewardChoice) {
        if matches!(choice, RewardChoice::Random { .. }) {
            // A random skill is forged: see `begin_forge`, which the app calls.
            return self.fail(
                player,
                "invalid_reward",
                "a random skill is forged, not drawn",
            );
        }
        let Some(pending) = self.rewards.remove(player) else {
            return self.fail(player, "no_reward", "you have no reward to claim");
        };
        if pending.created.elapsed() >= self.config.reward_ttl {
            self.delete_pending_reward(player);
            return self.fail(player, "no_reward", "that reward has expired");
        }
        if pending.forging {
            self.rewards.insert(player.to_string(), pending);
            return self.fail(player, "forging", "a skill is already being forged for you");
        }
        match self.resolve_reward(
            player,
            pending.loser.as_deref(),
            &pending.loser_deck,
            choice,
            None,
        ) {
            Ok(()) => {
                self.delete_pending_reward(player);
            }
            Err(msg) => {
                // Let the player try again with a corrected choice.
                self.rewards.insert(player.to_string(), pending);
                self.fail(player, "invalid_reward", msg);
            }
        }
    }

    /// The winner chose a random skill: checks the reward and returns what
    /// the forge needs. The forging itself takes a while (it measures the
    /// candidates on real positions), so the app does it off the lock and then
    /// calls [`Hub::finish_forge`].
    pub fn begin_forge(&mut self, player: &str, replace: Option<SkillId>) -> Option<ForgeJob> {
        let Some(pending) = self.rewards.get_mut(player) else {
            self.fail(player, "no_reward", "you have no reward to claim");
            return None;
        };
        if pending.created.elapsed() >= self.config.reward_ttl {
            self.rewards.remove(player);
            self.delete_pending_reward(player);
            self.fail(player, "no_reward", "that reward has expired");
            return None;
        }
        if pending.forging {
            self.fail(player, "forging", "a skill is already being forged for you");
            return None;
        }
        let known = match self.store.forged_signatures() {
            Ok(known) => known,
            Err(_) => {
                self.fail(player, "internal", "internal error");
                return None;
            }
        };
        let seed: u64 = rand::random();
        let pending = self.rewards.get_mut(player)?;
        pending.forging = true;
        let range = pending.range.clone();
        let target = chessy_engine::forge::generate::roll_rarity_in(
            &mut chessy_engine::ai::Rng::new(seed ^ 0xA5A5),
            *range.start(),
            *range.end(),
        );
        Some(ForgeJob {
            player: player.to_string(),
            replace,
            target,
            range,
            seed,
            known,
            store: self.store.clone(),
        })
    }

    /// The forge is done: `skill` is what it made, `None` if it failed.
    pub fn finish_forge(&mut self, player: &str, replace: Option<SkillId>, skill: Option<SkillId>) {
        let Some(mut pending) = self.rewards.remove(player) else {
            return;
        };
        pending.forging = false;
        let result = match skill {
            None => Err("the forge could not make a skill, try again"),
            Some(_) => self.resolve_reward(
                player,
                pending.loser.as_deref(),
                &pending.loser_deck,
                RewardChoice::Random { replace },
                skill,
            ),
        };
        match result {
            Ok(()) => {
                self.delete_pending_reward(player);
            }
            Err(msg) => {
                self.rewards.insert(player.to_string(), pending);
                self.fail(player, "invalid_reward", msg);
            }
        }
    }

    fn resolve_reward(
        &mut self,
        winner: &str,
        loser: Option<&str>,
        snapshot: &[SkillId],
        choice: RewardChoice,
        forged: Option<SkillId>,
    ) -> Result<(), &'static str> {
        let db = |_: StoreError| "internal error";
        let winner_deck = self.deck_of(winner).map_err(db)?;
        // Only what the loser had at the end of the game and still has now
        // (they may have won skills elsewhere since, or lost some).
        let loser_deck: Vec<SkillId> = match loser {
            Some(loser) => self
                .deck_of(loser)
                .map_err(db)?
                .into_iter()
                .filter(|s| snapshot.contains(s))
                .collect(),
            None => Vec::new(),
        };

        let (gain, loser_loses, replace, kind) = match choice {
            RewardChoice::Skip => {
                self.send(
                    winner,
                    ServerMsg::DeckUpdate {
                        deck: winner_deck,
                        gained: None,
                        lost: None,
                    },
                );
                if let Some(loser) = loser {
                    self.announce_outcome(winner, loser, RewardOutcomeKind::Spared, None, None);
                }
                return Ok(());
            }
            RewardChoice::Steal { skill, replace } => {
                if !loser_deck.contains(&skill) {
                    return Err("that skill is not available to take any more");
                }
                if winner_deck.contains(&skill) {
                    return Err("you already have that skill");
                }
                (Some(skill), Some(skill), replace, RewardOutcomeKind::Stolen)
            }
            RewardChoice::Random { replace } => {
                // The forge made the skill; the loser loses one of theirs at random.
                let skill = forged.ok_or("no skill was forged")?;
                if winner_deck.contains(&skill) {
                    return Err("you already have that skill");
                }
                let mut rng = rand::rng();
                (
                    Some(skill),
                    loser_deck.choose(&mut rng).copied(),
                    replace,
                    RewardOutcomeKind::Forged,
                )
            }
        };

        let winner_drops = if gain.is_some() && winner_deck.len() >= MAX_DECK {
            match replace {
                Some(r) if winner_deck.contains(&r) && Some(r) != gain => Some(r),
                _ => return Err("your deck is full: choose a skill to replace"),
            }
        } else {
            None
        };

        let loser_before = loser.and_then(|l| self.deck_of(l).ok()).unwrap_or_default();
        self.store
            .apply_deck_change(winner, loser, gain, loser_loses, winner_drops)
            .map_err(|_| "could not apply that reward")?;

        let winner_after = self.deck_of(winner).map_err(db)?;
        self.send(
            winner,
            ServerMsg::DeckUpdate {
                deck: winner_after,
                gained: gain,
                lost: winner_drops,
            },
        );
        if let Some(loser) = loser {
            if let Ok(loser_after) = self.deck_of(loser) {
                let refilled = loser_after
                    .iter()
                    .copied()
                    .find(|s| !loser_before.contains(s));
                self.send(
                    loser,
                    ServerMsg::DeckUpdate {
                        deck: loser_after,
                        gained: None,
                        lost: loser_loses,
                    },
                );
                self.announce_outcome(winner, loser, kind, loser_loses, refilled);
            }
        }
        Ok(())
    }
}
