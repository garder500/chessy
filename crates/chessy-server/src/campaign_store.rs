//! What the database keeps of the campaign (docs/spec-v6.md): the stars of each
//! level and the forge a boss still owes. Guests have their own rows, so a
//! guest's progress stays with their browser's token and follows the account
//! when the guest signs up.

use chessy_engine::{SkillId, SkillKind};
use rusqlite::params;

use crate::campaign::{self, Progress, Record, STAR_ALL, STAR_WIN};
use crate::history_store::{self as history, Change, Source};
use crate::store::{Store, StoreError, StoreResult};

/// What recording a game did to a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The stars the level had before (the best of the games so far).
    pub before: u8,
    /// And after: stars are never lost.
    pub after: u8,
    /// This win beat a boss for the first time: a forge is owed.
    pub forge_due: bool,
}

impl Store {
    pub fn campaign_progress(&self, player: &str) -> StoreResult<Progress> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT level, stars, forge_pending FROM campaign_levels WHERE player_id = ?1",
        )?;
        let rows = stmt.query_map(params![player], |r| {
            Ok((r.get::<_, u8>(0)?, r.get::<_, u8>(1)?, r.get::<_, bool>(2)?))
        })?;
        let mut out = Progress::new();
        for row in rows {
            let (level, stars, forge_pending) = row?;
            if campaign::level(level).is_some() {
                out.insert(
                    level,
                    Record {
                        stars: stars & STAR_ALL,
                        forge_pending,
                    },
                );
            }
        }
        Ok(out)
    }

    /// Adds the `stars` a finished game earned to the best of `level`. A boss
    /// won for the first time leaves a forge to claim.
    pub fn record_campaign(&self, player: &str, level: u8, stars: u8) -> StoreResult<Recorded> {
        let boss = campaign::level(level).is_some_and(|l| l.boss);
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let before: u8 = match tx.query_row(
            "SELECT stars FROM campaign_levels WHERE player_id = ?1 AND level = ?2",
            params![player, level],
            |r| r.get(0),
        ) {
            Ok(stars) => stars,
            Err(rusqlite::Error::QueryReturnedNoRows) => 0,
            Err(e) => return Err(e.into()),
        };
        let after = (before | stars) & STAR_ALL;
        let forge_due = boss && before & STAR_WIN == 0 && stars & STAR_WIN != 0;
        tx.execute(
            "INSERT INTO campaign_levels (player_id, level, stars, forge_pending)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(player_id, level) DO UPDATE SET
                 stars = campaign_levels.stars | excluded.stars,
                 forge_pending = campaign_levels.forge_pending OR excluded.forge_pending,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            params![player, level, after, forge_due],
        )?;
        tx.commit()?;
        Ok(Recorded {
            before,
            after,
            forge_due,
        })
    }

    /// Gives the forged skill a beaten boss owed and settles the forge, in one
    /// transaction: nothing changes if the player owns the skill already or the
    /// forge was claimed.
    pub fn apply_campaign_forge(
        &self,
        player: &str,
        chapter: u8,
        gain: SkillId,
        drop: Option<SkillId>,
    ) -> StoreResult<()> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let settled = tx.execute(
            "UPDATE campaign_levels SET forge_pending = 0
             WHERE player_id = ?1 AND level = ?2 AND forge_pending = 1",
            params![player, campaign::boss_id(chapter)],
        )?;
        if settled == 0 {
            return Err(StoreError::Invalid("no forge to claim"));
        }
        if let Some(skill) = drop {
            let name = skill.to_string();
            let removed = tx.execute(
                "DELETE FROM player_skills WHERE player_id = ?1 AND skill = ?2",
                params![player, name],
            )?;
            if removed == 0 {
                return Err(StoreError::Invalid("player does not own that skill"));
            }
            tx.execute(
                "DELETE FROM unique_skill_owner WHERE skill = ?1 AND player_id = ?2",
                params![name, player],
            )?;
            history::log(&tx, player, skill, Change::Lost, Source::Replaced, None)?;
        }
        let name = gain.to_string();
        tx.execute(
            "INSERT INTO player_skills (player_id, skill) VALUES (?1, ?2)",
            params![player, name],
        )?;
        if gain.kind() == SkillKind::Unique {
            tx.execute(
                "INSERT INTO unique_skill_owner (skill, player_id) VALUES (?1, ?2)",
                params![name, player],
            )?;
        }
        history::log(&tx, player, gain, Change::Gained, Source::Forged, None)?;
        tx.commit()?;
        Ok(())
    }
}
