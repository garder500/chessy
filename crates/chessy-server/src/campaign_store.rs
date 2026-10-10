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

    /// Adds `stars` to the ones the player already has on a level; returns
    /// the cumulated mask.
    pub fn record_campaign(&self, player: &str, at: LevelRef, stars: u8) -> StoreResult<u8> {
        Ok(self.db().query_row(
            "INSERT INTO campaign_progress (player_id, chapter, level, stars)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (player_id, chapter, level)
             DO UPDATE SET stars = stars | excluded.stars
             RETURNING stars",
            params![player, at.chapter, at.level, stars],
            |r| r.get(0),
        )?)
    }

    /// Marks the reward of a level as given.
    pub fn mark_campaign_rewarded(&self, player: &str, at: LevelRef) -> StoreResult<()> {
        self.db().execute(
            "UPDATE campaign_progress SET rewarded = 1
             WHERE player_id = ?1 AND chapter = ?2 AND level = ?3",
            params![player, at.chapter, at.level],
        )?;
        Ok(())
    }
}
