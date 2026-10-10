//! Campaign boss forges in the database (docs/spec-campagne.md): one row per
//! player and chapter, moving `forging` -> `pending` -> `placed`.

use rusqlite::params;

use crate::store::{Store, StoreResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossForgeState {
    Forging,
    Pending,
    Placed,
}

impl BossForgeState {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "forging" => Some(Self::Forging),
            "pending" => Some(Self::Pending),
            "placed" => Some(Self::Placed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BossForgeRow {
    pub chapter: u8,
    /// Id of the forged skill in the forge registry, once registered.
    pub skill_id: Option<u32>,
    pub state: BossForgeState,
}

const SELECT_FORGES: &str = "SELECT chapter, skill_id, state FROM campaign_boss_forges";

fn row_of(r: &rusqlite::Row) -> rusqlite::Result<BossForgeRow> {
    let state: String = r.get(2)?;
    Ok(BossForgeRow {
        chapter: r.get(0)?,
        skill_id: r.get(1)?,
        state: BossForgeState::parse(&state).ok_or(rusqlite::Error::InvalidQuery)?,
    })
}

impl Store {
    /// Starts the forge of a chapter boss; true only if this call created it,
    /// so replaying the boss never forges twice.
    pub fn begin_boss_forge(&self, player: &str, chapter: u8) -> StoreResult<bool> {
        Ok(self.db().execute(
            "INSERT OR IGNORE INTO campaign_boss_forges (player_id, chapter, state)
             VALUES (?1, ?2, 'forging')",
            params![player, chapter],
        )? > 0)
    }

    pub fn boss_forge(&self, player: &str, chapter: u8) -> StoreResult<Option<BossForgeRow>> {
        let conn = self.db();
        let mut stmt = conn.prepare(&format!(
            "{SELECT_FORGES} WHERE player_id = ?1 AND chapter = ?2"
        ))?;
        let mut rows = stmt.query_map(params![player, chapter], row_of)?;
        Ok(rows.next().transpose()?)
    }

    pub fn boss_forges(&self, player: &str) -> StoreResult<Vec<BossForgeRow>> {
        let conn = self.db();
        let mut stmt = conn.prepare(&format!(
            "{SELECT_FORGES} WHERE player_id = ?1 ORDER BY chapter"
        ))?;
        let rows = stmt.query_map(params![player], row_of)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Records the forged skill of a `forging` row and makes it `pending`;
    /// false if the row was not forging.
    pub fn set_boss_forge_skill(
        &self,
        player: &str,
        chapter: u8,
        skill_id: u32,
    ) -> StoreResult<bool> {
        Ok(self.db().execute(
            "UPDATE campaign_boss_forges SET skill_id = ?3, state = 'pending'
             WHERE player_id = ?1 AND chapter = ?2 AND state = 'forging'",
            params![player, chapter, skill_id],
        )? > 0)
    }

    /// Marks a `pending` forge as placed in the deck; false if it was not pending.
    pub fn place_boss_forge(&self, player: &str, chapter: u8) -> StoreResult<bool> {
        Ok(self.db().execute(
            "UPDATE campaign_boss_forges SET state = 'placed'
             WHERE player_id = ?1 AND chapter = ?2 AND state = 'pending'",
            params![player, chapter],
        )? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_player() -> (Store, String) {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        (store, player)
    }

    #[test]
    fn a_forge_is_created_once_per_chapter() {
        let (store, p) = store_with_player();
        assert!(store.begin_boss_forge(&p, 1).unwrap());
        assert!(!store.begin_boss_forge(&p, 1).unwrap());
        assert!(store.begin_boss_forge(&p, 2).unwrap());
        let forge = store.boss_forge(&p, 1).unwrap().unwrap();
        assert_eq!(forge.state, BossForgeState::Forging);
        assert_eq!(forge.skill_id, None);
        assert_eq!(store.boss_forge(&p, 3).unwrap(), None);
        assert_eq!(store.boss_forges(&p).unwrap().len(), 2);
    }

    #[test]
    fn the_forge_follows_forging_pending_placed() {
        let (store, p) = store_with_player();
        assert!(!store.set_boss_forge_skill(&p, 1, 7).unwrap());
        store.begin_boss_forge(&p, 1).unwrap();
        assert!(!store.place_boss_forge(&p, 1).unwrap());
        assert!(store.set_boss_forge_skill(&p, 1, 7).unwrap());
        assert!(!store.set_boss_forge_skill(&p, 1, 8).unwrap());
        let pending = store.boss_forge(&p, 1).unwrap().unwrap();
        assert_eq!(
            (pending.state, pending.skill_id),
            (BossForgeState::Pending, Some(7))
        );
        assert!(store.place_boss_forge(&p, 1).unwrap());
        assert!(!store.place_boss_forge(&p, 1).unwrap());
        assert!(!store.begin_boss_forge(&p, 1).unwrap());
        assert_eq!(
            store.boss_forge(&p, 1).unwrap().unwrap().state,
            BossForgeState::Placed
        );
    }
}
