//! The skill a chapter boss gives: forged off the hub lock like a ranked
//! reward, but kept in its own table. The player gains it and nobody loses one.

use std::collections::HashSet;

use chessy_engine::ai::Rng;
use chessy_engine::forge::at_least::forge_at_least;
use chessy_engine::forge::generate::Budget;
use chessy_engine::forge::Rarity;
use chessy_engine::SkillId;

use super::{Hub, PlayerId, MAX_DECK};
use crate::boss_forge_store::{BossForgeRow, BossForgeState};
use crate::campaign::{self, ForgeTable};
use crate::protocol::{self, BossForgeInfo, ServerMsg};
use crate::store::Store;

/// What the forge of a boss needs to run off the hub lock.
pub struct BossForgeJob {
    pub player: PlayerId,
    pub chapter: u8,
    pub table: ForgeTable,
    /// Signatures already in the world: a skill that repeats one is never kept.
    pub known: HashSet<String>,
}

/// What the forge made; `skill` is `None` when it found nothing worth keeping.
pub struct BossForgeMade {
    pub skill: Option<SkillId>,
    pub legendary_exhausted: bool,
}

impl BossForgeJob {
    /// Slow (it measures candidates): never call it with the hub locked.
    pub fn forge(&self, store: &Store) -> BossForgeMade {
        let mut rng = Rng::new(rand::random());
        let target = roll_rarity(&mut rng, &self.table);
        let outcome = forge_at_least(
            &mut rng,
            target,
            self.table.floor,
            self.table.family,
            &self.known,
            Budget::campaign(),
        );
        let skill = outcome.forged.and_then(|forged| {
            store
                .insert_forged(&forged.def, &forged.graded)
                .inspect_err(|e| tracing::error!("could not store a boss skill: {e}"))
                .ok()
                .map(SkillId::Forged)
        });
        BossForgeMade {
            skill,
            legendary_exhausted: outcome.legendary_exhausted,
        }
    }
}

fn roll_rarity(rng: &mut Rng, table: &ForgeTable) -> Rarity {
    let total: u64 = table.weights.iter().map(|&(_, w)| u64::from(w)).sum();
    let mut roll = rng.below(total.max(1));
    for &(rarity, weight) in table.weights {
        if roll < u64::from(weight) {
            return rarity;
        }
        roll -= u64::from(weight);
    }
    table.floor
}

fn wire_state(state: BossForgeState) -> protocol::BossForgeState {
    match state {
        BossForgeState::Forging => protocol::BossForgeState::Forging,
        BossForgeState::Pending => protocol::BossForgeState::Pending,
        BossForgeState::Placed => protocol::BossForgeState::Placed,
    }
}

impl Hub {
    /// A boss was beaten: starts its forge unless the account already has one
    /// (replaying a boss never forges twice).
    pub(super) fn start_boss_forge(&mut self, player: &str, chapter: u8) -> Option<BossForgeInfo> {
        match self.store.begin_boss_forge(player, chapter) {
            Ok(true) => self.queue_boss_forge(player, chapter),
            Ok(false) => None,
            Err(e) => {
                tracing::error!("could not begin the boss forge of {player}: {e}");
                None
            }
        }
    }

    /// Jobs queued since the last call, for the app to run off the lock.
    pub fn take_boss_forge_jobs(&mut self) -> Vec<BossForgeJob> {
        std::mem::take(&mut self.boss_forge_jobs)
    }

    fn queue_boss_forge(&mut self, player: &str, chapter: u8) -> Option<BossForgeInfo> {
        let known = match self.store.forged_signatures() {
            Ok(known) => known,
            Err(e) => {
                self.internal_error(player, e);
                return None;
            }
        };
        self.boss_forges_in_flight
            .insert((player.to_string(), chapter));
        self.boss_forge_jobs.push(BossForgeJob {
            player: player.to_string(),
            chapter,
            table: campaign::forge_table(chapter),
            known,
        });
        let info = self.boss_forge_info(player, chapter, BossForgeState::Forging, None, false);
        self.send(player, ServerMsg::BossForge { info: info.clone() });
        Some(info)
    }

    fn boss_forge_info(
        &self,
        player: &str,
        chapter: u8,
        state: BossForgeState,
        skill: Option<SkillId>,
        legendary_unavailable: bool,
    ) -> BossForgeInfo {
        let deck_full = self
            .deck_of(player)
            .is_ok_and(|deck| deck.len() >= MAX_DECK);
        BossForgeInfo {
            chapter,
            state: wire_state(state),
            skill,
            deck_full,
            legendary_unavailable,
        }
    }

    fn send_boss_forge(&self, player: &str, row: BossForgeRow) {
        let skill = row.skill_id.map(SkillId::Forged);
        let info = self.boss_forge_info(player, row.chapter, row.state, skill, false);
        self.send(player, ServerMsg::BossForge { info });
    }

    /// The forge of `chapter` for an account, or the error already sent.
    fn account_boss_forge(&self, player: &str, chapter: u8) -> Option<BossForgeRow> {
        if !self.is_account(player) {
            self.fail(
                player,
                "account_required",
                "la campagne demande un compte : connecte-toi ou inscris-toi",
            );
            return None;
        }
        match self.store.boss_forge(player, chapter) {
            Ok(Some(row)) => Some(row),
            Ok(None) => {
                self.fail(player, "no_boss_forge", "no skill is forged for this boss");
                None
            }
            Err(e) => {
                self.internal_error(player, e);
                None
            }
        }
    }

    /// The player asks for a forge to (re)start, e.g. after a restart lost it.
    pub fn boss_forge_claim(&mut self, player: &str, chapter: u8) {
        let Some(row) = self.account_boss_forge(player, chapter) else {
            return;
        };
        if row.state != BossForgeState::Forging {
            return self.send_boss_forge(player, row);
        }
        if self
            .boss_forges_in_flight
            .contains(&(player.to_string(), chapter))
        {
            return self.fail(player, "forging", "this skill is already being forged");
        }
        self.queue_boss_forge(player, chapter);
    }

    /// The forge is over. On failure the row stays `forging`: a claim retries.
    pub fn finish_boss_forge(&mut self, player: &str, chapter: u8, made: BossForgeMade) {
        self.boss_forges_in_flight
            .remove(&(player.to_string(), chapter));
        let Some(SkillId::Forged(id)) = made.skill else {
            return self.send_boss_forge(
                player,
                BossForgeRow {
                    chapter,
                    skill_id: None,
                    state: BossForgeState::Forging,
                },
            );
        };
        match self.store.set_boss_forge_skill(player, chapter, id) {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => return self.internal_error(player, e),
        }
        let info = self.boss_forge_info(
            player,
            chapter,
            BossForgeState::Pending,
            made.skill,
            made.legendary_exhausted,
        );
        self.send(player, ServerMsg::BossForge { info });
    }

    pub fn boss_forge_place(&mut self, player: &str, chapter: u8, replace: Option<SkillId>) {
        let Some(row) = self.account_boss_forge(player, chapter) else {
            return;
        };
        let pending = row
            .skill_id
            .filter(|_| row.state == BossForgeState::Pending);
        let Some(skill) = pending.map(SkillId::Forged) else {
            return self.fail(
                player,
                "not_pending",
                "this skill is not waiting for a place",
            );
        };
        let deck = match self.deck_of(player) {
            Ok(deck) => deck,
            Err(e) => return self.internal_error(player, e),
        };
        let dropped = if deck.len() < MAX_DECK {
            None
        } else if let Some(r) = replace.filter(|r| deck.contains(r)) {
            Some(r)
        } else {
            return self.fail(
                player,
                "deck_full",
                "your deck is full: choose a skill to replace",
            );
        };
        let placed = self
            .store
            .apply_deck_change(player, None, Some(skill), None, dropped)
            .and_then(|()| self.store.place_boss_forge(player, chapter));
        if let Err(e) = placed {
            return self.internal_error(player, e);
        }
        match self.deck_of(player) {
            Ok(deck) => self.send(
                player,
                ServerMsg::DeckUpdate {
                    deck,
                    gained: Some(skill),
                    lost: dropped,
                },
            ),
            Err(e) => return self.internal_error(player, e),
        }
        self.send_boss_forge(
            player,
            BossForgeRow {
                state: BossForgeState::Placed,
                ..row
            },
        );
    }

    /// On connection: the forges still waiting on the player.
    pub(super) fn push_boss_forges(&self, player: &str) {
        match self.store.boss_forges(player) {
            Ok(rows) => rows
                .into_iter()
                .filter(|row| row.state != BossForgeState::Placed)
                .for_each(|row| self.send_boss_forge(player, row)),
            Err(e) => tracing::error!("could not read the boss forges of {player}: {e}"),
        }
    }
}
