//! Campaign progress in the database (docs/spec-campagne.md): the best stars
//! of each level and whether the boss reward was given.

use rusqlite::{params, Connection};

use crate::campaign::LevelRef;
use crate::store::{Store, StoreResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignRow {
    pub at: LevelRef,
    /// Stars mask, cumulated over every attempt.
    pub stars: u8,
    pub rewarded: bool,
}

/// The rows of a player on a connection the caller already holds.
pub(crate) fn rows_of(conn: &Connection, player: &str) -> StoreResult<Vec<CampaignRow>> {
    let mut stmt = conn.prepare(
        "SELECT chapter, level, stars, rewarded FROM campaign_progress
         WHERE player_id = ?1 ORDER BY chapter, level",
    )?;
    let rows = stmt.query_map(params![player], |r| {
        Ok(CampaignRow {
            at: LevelRef {
                chapter: r.get(0)?,
                level: r.get(1)?,
            },
            stars: r.get(2)?,
            rewarded: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

impl Store {
    pub fn campaign_rows(&self, player: &str) -> StoreResult<Vec<CampaignRow>> {
        rows_of(&self.db(), player)
    }

    /// Adds `stars` to the ones the player already has on a level and resets
    /// its defeat streak; returns the cumulated mask.
    pub fn record_campaign(&self, player: &str, at: LevelRef, stars: u8) -> StoreResult<u8> {
        Ok(self.db().query_row(
            "INSERT INTO campaign_progress (player_id, chapter, level, stars)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (player_id, chapter, level)
             DO UPDATE SET stars = stars | excluded.stars, defeats = 0
             RETURNING stars",
            params![player, at.chapter, at.level, stars],
            |r| r.get(0),
        )?)
    }

    /// Counts one more defeat in a row on a level; returns the streak.
    pub fn record_campaign_defeat(&self, player: &str, chapter: u8, level: u8) -> StoreResult<u32> {
        Ok(self.db().query_row(
            "INSERT INTO campaign_progress (player_id, chapter, level, defeats)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT (player_id, chapter, level)
             DO UPDATE SET defeats = defeats + 1
             RETURNING defeats",
            params![player, chapter, level],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL: LevelRef = LevelRef {
        chapter: 1,
        level: 2,
    };

    #[test]
    fn defeats_count_up_and_a_win_resets_them() {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        assert_eq!(store.record_campaign_defeat(&player, 1, 2).unwrap(), 1);
        assert_eq!(store.record_campaign_defeat(&player, 1, 2).unwrap(), 2);
        assert_eq!(store.record_campaign_defeat(&player, 1, 3).unwrap(), 1);
        store.record_campaign(&player, LEVEL, 1).unwrap();
        assert_eq!(store.record_campaign_defeat(&player, 1, 2).unwrap(), 1);
    }

    #[test]
    fn a_defeat_alone_earns_no_star() {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        store.record_campaign_defeat(&player, 1, 2).unwrap();
        assert_eq!(store.campaign_rows(&player).unwrap()[0].stars, 0);
    }
}
