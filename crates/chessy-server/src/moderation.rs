//! Chat moderation, database side: block lists, the "mute all chat" setting
//! and player reports (docs/spec-v2.md §4, "Modération"). Written
//! synchronously through the store like the friendships in `social`.

use rusqlite::{params, Connection, OptionalExtension};

use crate::protocol::PlayerId;
use crate::store::{Store, StoreResult};

/// Most accounts one player can block (keeps the table and the list bounded).
pub const MAX_BLOCKS: u32 = 200;

#[derive(Debug, PartialEq, Eq)]
pub enum BlockOutcome {
    UserNotFound,
    SelfBlock,
    /// The block list is full and `other` was not on it already.
    Full,
    /// `removed` says what happened to the friendship row between the two,
    /// whatever its status: `Some(true)` a friendship, `Some(false)` a pending
    /// request (in either direction), `None` nothing.
    Blocked {
        other: PlayerId,
        removed: Option<bool>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReportOutcome {
    Filed,
    /// The same reporter already reported the same target within the window.
    Duplicate,
    /// The reporter filed too many reports within the window.
    Limited,
}

/// What a report carries; every field is already bounded by the caller.
pub struct NewReport<'a> {
    pub reporter: &'a str,
    pub target: &'a str,
    pub reason: &'a str,
    pub game_id: Option<&'a str>,
    pub context: Option<&'a str>,
}

fn account_id(conn: &Connection, username: &str) -> StoreResult<Option<PlayerId>> {
    Ok(conn
        .query_row(
            "SELECT id FROM players WHERE username_lower = ?1",
            params![username.to_ascii_lowercase()],
            |r| r.get(0),
        )
        .optional()?)
}

pub(crate) fn chat_muted(conn: &Connection, player: &str) -> StoreResult<bool> {
    Ok(conn
        .query_row(
            "SELECT chat_muted FROM chat_settings WHERE player_id = ?1",
            params![player],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .is_some_and(|v| v != 0))
}

/// Whether `blocker` blocked `blocked`.
pub(crate) fn is_blocked(conn: &Connection, blocker: &str, blocked: &str) -> StoreResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM blocks WHERE blocker = ?1 AND blocked = ?2",
            params![blocker, blocked],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

impl Store {
    /// Blocks `username` for `me`; removes any friendship or request between
    /// them. Blocking someone already blocked changes nothing.
    pub fn block_user(&self, me: &str, username: &str) -> StoreResult<BlockOutcome> {
        let mut conn = self.conn.lock().unwrap();
        let Some(other) = account_id(&conn, username)? else {
            return Ok(BlockOutcome::UserNotFound);
        };
        if other == me {
            return Ok(BlockOutcome::SelfBlock);
        }
        let tx = conn.transaction()?;
        if !is_blocked(&tx, me, &other)? {
            let count: u32 = tx.query_row(
                "SELECT COUNT(*) FROM blocks WHERE blocker = ?1",
                params![me],
                |r| r.get(0),
            )?;
            if count >= MAX_BLOCKS {
                return Ok(BlockOutcome::Full);
            }
            tx.execute(
                "INSERT INTO blocks (blocker, blocked) VALUES (?1, ?2)",
                params![me, other],
            )?;
        }
        let (ua, ub) = if me < other.as_str() {
            (me, other.as_str())
        } else {
            (other.as_str(), me)
        };
        let removed = tx
            .query_row(
                "SELECT status FROM friendships WHERE user_a = ?1 AND user_b = ?2",
                params![ua, ub],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|status| status == "accepted");
        if removed.is_some() {
            tx.execute(
                "DELETE FROM friendships WHERE user_a = ?1 AND user_b = ?2",
                params![ua, ub],
            )?;
        }
        tx.commit()?;
        Ok(BlockOutcome::Blocked { other, removed })
    }

    /// Lifts a block. `None` when no such account exists; lifting a block
    /// that was not there is not an error.
    pub fn unblock_user(&self, me: &str, username: &str) -> StoreResult<Option<()>> {
        let conn = self.conn.lock().unwrap();
        let Some(other) = account_id(&conn, username)? else {
            return Ok(None);
        };
        conn.execute(
            "DELETE FROM blocks WHERE blocker = ?1 AND blocked = ?2",
            params![me, other],
        )?;
        Ok(Some(()))
    }

    /// The usernames `me` blocked, alphabetical.
    pub fn blocked_names(&self, me: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.username FROM blocks b JOIN players p ON p.id = b.blocked
             WHERE b.blocker = ?1 AND p.username IS NOT NULL
             ORDER BY p.username_lower",
        )?;
        let names = stmt.query_map(params![me], |r| r.get(0))?;
        Ok(names.collect::<Result<_, _>>()?)
    }

    pub fn set_chat_muted(&self, player: &str, muted: bool) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chat_settings (player_id, chat_muted) VALUES (?1, ?2)
             ON CONFLICT(player_id) DO UPDATE SET chat_muted = excluded.chat_muted",
            params![player, muted as i64],
        )?;
        Ok(())
    }

    /// Whether a chat message from `sender` must not reach `recipient`: the
    /// recipient muted all chat, or blocked the sender.
    pub fn drops_chat(&self, recipient: &str, sender: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(chat_muted(&conn, recipient)? || is_blocked(&conn, recipient, sender)?)
    }

    /// Records a report unless the reporter already reported `target` within
    /// `window_secs` (`Duplicate`, nothing written) or already filed `max`
    /// reports within it (`Limited`).
    pub fn file_report(
        &self,
        report: &NewReport<'_>,
        window_secs: u64,
        max: u32,
    ) -> StoreResult<ReportOutcome> {
        let conn = self.conn.lock().unwrap();
        let since = format!("-{window_secs} seconds");
        let same: u32 = conn.query_row(
            "SELECT COUNT(*) FROM reports WHERE reporter = ?1 AND target = ?2
               AND created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?3)",
            params![report.reporter, report.target, since],
            |r| r.get(0),
        )?;
        if same > 0 {
            return Ok(ReportOutcome::Duplicate);
        }
        let recent: u32 = conn.query_row(
            "SELECT COUNT(*) FROM reports WHERE reporter = ?1
               AND created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?2)",
            params![report.reporter, since],
            |r| r.get(0),
        )?;
        if recent >= max {
            return Ok(ReportOutcome::Limited);
        }
        conn.execute(
            "INSERT INTO reports (reporter, target, reason, game_id, context)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                report.reporter,
                report.target,
                report.reason,
                report.game_id,
                report.context
            ],
        )?;
        Ok(ReportOutcome::Filed)
    }
}
