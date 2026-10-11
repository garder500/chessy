//! Reward announcements waiting for a loser who was offline when the winner
//! chose (docs/spec-campagne.md): delivered once on reconnection, then deleted.

use chessy_engine::SkillId;
use rusqlite::params;

use crate::protocol::RewardOutcomeKind;
use crate::store::{Store, StoreError, StoreResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardOutcomeRow {
    pub by: String,
    pub kind: RewardOutcomeKind,
    pub skill: Option<SkillId>,
    pub refilled: Option<SkillId>,
}

fn kind_name(kind: RewardOutcomeKind) -> &'static str {
    match kind {
        RewardOutcomeKind::Stolen => "stolen",
        RewardOutcomeKind::Forged => "forged",
        RewardOutcomeKind::Spared => "spared",
    }
}

fn kind_from_name(name: &str) -> StoreResult<RewardOutcomeKind> {
    match name {
        "stolen" => Ok(RewardOutcomeKind::Stolen),
        "forged" => Ok(RewardOutcomeKind::Forged),
        "spared" => Ok(RewardOutcomeKind::Spared),
        _ => Err(StoreError::Invalid("unreadable reward outcome")),
    }
}

fn skill_to_json(skill: Option<SkillId>) -> StoreResult<Option<String>> {
    skill
        .map(|s| serde_json::to_string(&s))
        .transpose()
        .map_err(|_| StoreError::Invalid("unwritable reward outcome"))
}

fn skill_from_json(json: Option<String>) -> StoreResult<Option<SkillId>> {
    json.map(|j| serde_json::from_str(&j))
        .transpose()
        .map_err(|_| StoreError::Invalid("unreadable reward outcome"))
}

impl Store {
    pub fn push_reward_outcome(&self, player: &str, outcome: &RewardOutcomeRow) -> StoreResult<()> {
        self.db().execute(
            "INSERT INTO reward_outcomes (player_id, by_name, kind, skill, refilled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                player,
                outcome.by,
                kind_name(outcome.kind),
                skill_to_json(outcome.skill)?,
                skill_to_json(outcome.refilled)?,
            ],
        )?;
        Ok(())
    }

    /// Reads then deletes the announcements of `player`, oldest first.
    pub fn take_reward_outcomes(&self, player: &str) -> StoreResult<Vec<RewardOutcomeRow>> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let rows = {
            let mut stmt = tx.prepare(
                "SELECT by_name, kind, skill, refilled FROM reward_outcomes
                 WHERE player_id = ?1 ORDER BY id",
            )?;
            let rows = stmt.query_map(params![player], |r| {
                Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        tx.execute(
            "DELETE FROM reward_outcomes WHERE player_id = ?1",
            params![player],
        )?;
        tx.commit()?;
        rows.into_iter()
            .map(|(by, kind, skill, refilled)| {
                Ok(RewardOutcomeRow {
                    by,
                    kind: kind_from_name(&kind)?,
                    skill: skill_from_json(skill)?,
                    refilled: skill_from_json(refilled)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(kind: RewardOutcomeKind, skill: Option<SkillId>) -> RewardOutcomeRow {
        RewardOutcomeRow {
            by: "winner".to_owned(),
            kind,
            skill,
            refilled: skill.map(|_| SkillId::ALL[1]),
        }
    }

    fn store_with_player() -> (Store, String) {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        (store, player)
    }

    #[test]
    fn a_pushed_outcome_is_taken_once_in_order() {
        let (store, player) = store_with_player();
        let first = outcome(RewardOutcomeKind::Stolen, Some(SkillId::ALL[0]));
        let second = outcome(RewardOutcomeKind::Spared, None);
        store.push_reward_outcome(&player, &first).unwrap();
        store.push_reward_outcome(&player, &second).unwrap();
        assert_eq!(
            store.take_reward_outcomes(&player).unwrap(),
            vec![first, second]
        );
        assert!(store.take_reward_outcomes(&player).unwrap().is_empty());
    }

    #[test]
    fn taking_without_outcomes_gives_nothing() {
        let (store, player) = store_with_player();
        assert!(store.take_reward_outcomes(&player).unwrap().is_empty());
    }

    #[test]
    fn taking_leaves_the_outcomes_of_other_players() {
        let (store, player) = store_with_player();
        let (other, _) = store.create_player().unwrap();
        let kept = outcome(RewardOutcomeKind::Forged, Some(SkillId::ALL[2]));
        store.push_reward_outcome(&other, &kept).unwrap();
        assert!(store.take_reward_outcomes(&player).unwrap().is_empty());
        assert_eq!(store.take_reward_outcomes(&other).unwrap(), vec![kept]);
    }
}
