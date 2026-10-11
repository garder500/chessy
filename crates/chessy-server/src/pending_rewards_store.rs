//! Rewards waiting for the winner's choice, kept in the database so they
//! survive a restart (docs/spec-campagne.md).

use chessy_engine::SkillId;
use rusqlite::params;

use crate::protocol::PlayerId;
use crate::store::{Store, StoreError, StoreResult, ISO_NOW};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRewardRow {
    pub winner_id: PlayerId,
    pub loser_id: PlayerId,
    pub loser_skills: Vec<SkillId>,
    /// ISO 8601 UTC, like every timestamp compared in SQL.
    pub expires_at: String,
}

fn skills_from_json(json: &str) -> StoreResult<Vec<SkillId>> {
    serde_json::from_str(json).map_err(|_| StoreError::Invalid("unreadable pending reward"))
}

impl Store {
    /// Saves (or replaces) the reward waiting for `winner_id`.
    pub fn save_pending_reward(&self, reward: &PendingRewardRow) -> StoreResult<()> {
        let skills = serde_json::to_string(&reward.loser_skills)
            .map_err(|_| StoreError::Invalid("unwritable pending reward"))?;
        self.db().execute(
            "INSERT OR REPLACE INTO pending_rewards (winner_id, loser_id, loser_skills, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![reward.winner_id, reward.loser_id, skills, reward.expires_at],
        )?;
        Ok(())
    }

    pub fn delete_pending_reward(&self, winner: &str) -> StoreResult<()> {
        self.db().execute(
            "DELETE FROM pending_rewards WHERE winner_id = ?1",
            params![winner],
        )?;
        Ok(())
    }

    /// The rewards still open; the expired ones are deleted.
    pub fn load_pending_rewards(&self) -> StoreResult<Vec<PendingRewardRow>> {
        let conn = self.db();
        conn.execute(
            &format!("DELETE FROM pending_rewards WHERE expires_at <= {ISO_NOW}"),
            [],
        )?;
        let mut stmt = conn.prepare(
            "SELECT winner_id, loser_id, loser_skills, expires_at FROM pending_rewards
             ORDER BY created_at, winner_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?, r.get(3)?))
        })?;
        rows.map(|row| {
            let (winner_id, loser_id, skills, expires_at) = row?;
            Ok(PendingRewardRow {
                winner_id,
                loser_id,
                loser_skills: skills_from_json(&skills)?,
                expires_at,
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reward(winner: &str, expires_at: &str) -> PendingRewardRow {
        PendingRewardRow {
            winner_id: winner.to_owned(),
            loser_id: "loser".to_owned(),
            loser_skills: vec![SkillId::ALL[0], SkillId::ALL[1]],
            expires_at: expires_at.to_owned(),
        }
    }

    fn store_with_player() -> (Store, String) {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        (store, player)
    }

    #[test]
    fn a_saved_reward_is_loaded_back_then_deleted() {
        let (store, winner) = store_with_player();
        let saved = reward(&winner, "2999-01-01T00:00:00Z");
        store.save_pending_reward(&saved).unwrap();
        assert_eq!(store.load_pending_rewards().unwrap(), vec![saved]);
        store.delete_pending_reward(&winner).unwrap();
        assert!(store.load_pending_rewards().unwrap().is_empty());
    }

    #[test]
    fn saving_again_replaces_the_reward_of_the_winner() {
        let (store, winner) = store_with_player();
        store
            .save_pending_reward(&reward(&winner, "2999-01-01T00:00:00Z"))
            .unwrap();
        let newer = reward(&winner, "2999-06-01T00:00:00Z");
        store.save_pending_reward(&newer).unwrap();
        assert_eq!(store.load_pending_rewards().unwrap(), vec![newer]);
    }

    #[test]
    fn expired_rewards_are_dropped_on_load() {
        let (store, winner) = store_with_player();
        store
            .save_pending_reward(&reward(&winner, "2000-01-01T00:00:00Z"))
            .unwrap();
        assert!(store.load_pending_rewards().unwrap().is_empty());
        let count: i64 = store
            .db()
            .query_row("SELECT COUNT(*) FROM pending_rewards", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
