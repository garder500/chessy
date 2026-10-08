//! The history of a player's skills (docs/spec-forge.md, « Collection »): every
//! skill they got or forged, and every one they lost, with how and from whom.
//! The Collection screen reads it back. It is a journal: `player_skills` stays
//! what the player owns right now.

use chessy_engine::SkillId;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::store::{Store, StoreResult};

/// Entries returned at most.
pub const HISTORY_LIMIT: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Gained,
    Lost,
}

impl Change {
    fn as_str(self) -> &'static str {
        match self {
            Change::Gained => "gained",
            Change::Lost => "lost",
        }
    }
}

/// How a skill came or went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The deck a new player starts with.
    Starter,
    /// A player left with no skill is given one.
    Refill,
    /// Made by the forge after a win.
    Forged,
    /// Taken from the loser of a game.
    Stolen,
    /// Won in another way (a reward that was neither stolen nor forged).
    Won,
    /// Taken from the player by the winner of a game.
    Taken,
    /// Given up to make room for a new one.
    Replaced,
    /// Already owned when the history began.
    Earlier,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Starter => "starter",
            Source::Refill => "refill",
            Source::Forged => "forged",
            Source::Stolen => "stolen",
            Source::Won => "won",
            Source::Taken => "taken",
            Source::Replaced => "replaced",
            Source::Earlier => "earlier",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub skill: SkillId,
    pub change: Change,
    pub source: String,
    /// The other player, when there was one and they have an account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
    pub at: String,
}

/// Adds a line to the journal. Used inside the transactions that change a deck.
pub(crate) fn log(
    conn: &Connection,
    player: &str,
    skill: SkillId,
    change: Change,
    source: Source,
    other: Option<&str>,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO skill_history (player_id, skill, change, source, other) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![player, skill.to_string(), change.as_str(), source.as_str(), other],
    )?;
    Ok(())
}

/// The account name of a player, if they have one.
pub(crate) fn username_of(conn: &Connection, player: &str) -> Option<String> {
    conn.query_row(
        "SELECT username FROM players WHERE id = ?1",
        params![player],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

impl Store {
    /// The newest entries first.
    pub fn skill_history(&self, player: &str) -> StoreResult<Vec<HistoryEntry>> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT id, skill, change, source, other, at FROM skill_history
             WHERE player_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![player, HISTORY_LIMIT], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, skill, change, source, other, at) = row?;
            let Some(skill) = SkillId::parse(&skill) else {
                continue;
            };
            out.push(HistoryEntry {
                id,
                skill,
                change: if change == "lost" {
                    Change::Lost
                } else {
                    Change::Gained
                },
                source,
                other,
                at,
            });
        }
        Ok(out)
    }
}
