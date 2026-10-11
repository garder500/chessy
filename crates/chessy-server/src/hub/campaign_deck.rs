//! The hand a player brings to a campaign level.

use chessy_engine::{SkillId, SkillKind};

use super::Hub;
use crate::campaign::Level;

const MAX_CHOSEN_SKILLS: usize = 3;

impl Hub {
    /// The skills played in `level`, or `None` (the player is told why).
    /// An imposed level ignores `chosen`. A choosing level takes up to 3 distinct
    /// classics among the player's deck and the skills lent for the level, then
    /// the deck's uniques, as in a ranked game. A loan lives in this hand only.
    pub(super) fn campaign_hand(
        &mut self,
        player: &str,
        level: &Level,
        chosen: Vec<SkillId>,
    ) -> Option<Vec<SkillId>> {
        if !level.deck_choice {
            return Some(level.player_deck.to_vec());
        }
        let owned = match self.deck_of(player) {
            Ok(owned) => owned,
            Err(e) => {
                self.internal_error(player, e);
                return None;
            }
        };
        let mut seen = Vec::new();
        let valid = chosen.len() <= MAX_CHOSEN_SKILLS
            && chosen.iter().all(|skill| {
                let fresh = !seen.contains(skill);
                seen.push(*skill);
                let available = owned.contains(skill) || level.lent.contains(skill);
                fresh && available && skill.kind() == SkillKind::Classic
            });
        if !valid {
            self.fail(
                player,
                "bad_deck",
                "pick up to three distinct classic skills from your deck or lent for this level",
            );
            return None;
        }
        let mut hand = chosen;
        hand.extend(owned.iter().filter(|s| s.kind() == SkillKind::Unique));
        Some(hand)
    }
}
