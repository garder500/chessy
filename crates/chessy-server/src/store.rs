//! SQLite persistence: accounts and sessions, skill decks, unique-skill
//! ownership, ratings and a log of finished games. Friendships live in
//! [`crate::social`].

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chessy_engine::{Action, Color, Outcome, SkillId, SkillKind};
use rand::seq::IndexedRandom;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use thiserror::Error;

use crate::elo::{self, START_ELO};
use crate::games_store::GameKind;
use crate::history_store::{self as history, Change, Source};
use crate::moderation::chat_muted;
use crate::protocol::{Me, PlayerId};

pub const STARTER_DECK_SIZE: usize = 3;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Invalid(&'static str),
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Timestamps are written and read as ISO 8601 UTC (`2026-01-02T03:04:05Z`).
pub(crate) const ISO_NOW: &str = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')";
const ISO_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

/// A SQLite time modifier for "`d` ago", to pass to `strftime(.., 'now', ?)`.
fn seconds_ago(d: Duration) -> String {
    format!("-{} seconds", d.as_secs())
}

fn skill_name(skill: SkillId) -> String {
    serde_json::to_value(skill)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .expect("skills serialize to strings")
}

fn parse_skill(name: &str) -> Option<SkillId> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

fn random_hex(bytes: usize) -> String {
    (0..bytes)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect()
}

pub fn classic_skills() -> Vec<SkillId> {
    SkillId::ALL
        .into_iter()
        .filter(|s| s.kind() == SkillKind::Classic)
        .collect()
}

/// A player's account fields, as stored.
#[derive(Clone, Debug)]
pub struct PlayerRow {
    pub id: PlayerId,
    /// `None` for a guest.
    pub username: Option<String>,
    pub elo: i32,
    pub peak_elo: i32,
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub created_at: String,
    pub last_seen: Option<String>,
    /// Chapter of the campaign title the player chose to display.
    pub title_active: Option<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub username: String,
    pub elo: i32,
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Leaderboard {
    pub total: u32,
    pub entries: Vec<LeaderboardEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryPoint {
    pub elo: i32,
    pub at: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameResult {
    Win,
    Loss,
    Draw,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecentGame {
    pub game_id: String,
    pub opponent: Option<String>,
    pub result: GameResult,
    pub color: Color,
    pub rated: bool,
    pub elo_delta: Option<i32>,
    pub reason: String,
    pub at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicProfile {
    pub username: String,
    pub elo: i32,
    pub peak_elo: i32,
    pub rank: u32,
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    /// Positive: wins in a row; negative: losses in a row (rated games).
    pub streak: i32,
    pub created_at: String,
    pub history: Vec<HistoryPoint>,
    pub recent: Vec<RecentGame>,
    /// Best campaign title (docs/spec-campagne.md).
    pub title: Option<&'static str>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegisterError {
    #[error("username taken")]
    UsernameTaken,
    #[error("database error")]
    Db,
}

/// Everything the store needs to log a finished game and settle ratings.
pub struct GameRecord<'a> {
    pub id: &'a str,
    pub white: &'a str,
    pub black: &'a str,
    pub outcome: &'a Outcome,
    pub reason: &'a str,
    pub plies: u32,
    /// Whether ratings change; the caller has already checked the rules.
    pub rated: bool,
    /// Unix time the game started (after deck selection).
    pub started_unix: i64,
    /// How the game came about.
    pub kind: GameKind,
    /// The skills each side brought, in the order the game was created with.
    pub loadouts: &'a [Vec<SkillId>; 2],
    /// FEN the game started from, when not the standard position.
    pub start_fen: Option<&'a str>,
    /// Every action played, in order (skills that keep the turn included).
    pub actions: &'a [Action],
    /// Solo: the level of the bot, whose seat (`white` or `black`) is not a player.
    pub solo_elo: Option<i32>,
    /// The length the players asked for; `None` for the default clock and Solo.
    pub time_control: Option<crate::protocol::TimeControl>,
}

/// Rating movement of one game, by colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EloChange {
    pub white_before: i32,
    pub white_after: i32,
    pub black_before: i32,
    pub black_after: i32,
}

/// The reason a finished game ended, derived from its outcome alone.
pub fn reason_of(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Ongoing => "",
        Outcome::Checkmate { .. } => "checkmate",
        Outcome::Resignation { .. } => "resignation",
        Outcome::Timeout { .. } => "timeout",
        Outcome::DrawAgreed => "agreed_draw",
        Outcome::Stalemate => "stalemate",
        Outcome::FiftyMoves => "fifty_moves",
        Outcome::Repetition => "repetition",
        Outcome::InsufficientMaterial => "insufficient_material",
    }
}

/// Cheap to clone: clones share one connection.
#[derive(Clone)]
pub struct Store {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

/// Schema migrations, applied in order; the database remembers how many ran
/// in `PRAGMA user_version`. Version 1 is the original schema, so a database
/// from before versioning (user_version 0) upgrades in place.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS players (
         id TEXT PRIMARY KEY,
         token TEXT NOT NULL UNIQUE,
         created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
     );
     CREATE TABLE IF NOT EXISTS player_skills (
         player_id TEXT NOT NULL REFERENCES players(id),
         skill TEXT NOT NULL,
         PRIMARY KEY (player_id, skill)
     );
     CREATE TABLE IF NOT EXISTS unique_skill_owner (
         skill TEXT PRIMARY KEY,
         player_id TEXT NOT NULL REFERENCES players(id)
     );
     CREATE TABLE IF NOT EXISTS games (
         id TEXT PRIMARY KEY,
         white TEXT NOT NULL REFERENCES players(id),
         black TEXT NOT NULL REFERENCES players(id),
         outcome TEXT NOT NULL,
         finished_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
     );",
    // Accounts, sessions, ratings and friendships. `players` is rebuilt to
    // drop its NOT NULL `token` column; tokens move to `sessions`.
    "CREATE TABLE sessions (
         token TEXT PRIMARY KEY,
         player_id TEXT NOT NULL REFERENCES players(id),
         created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
     );
     CREATE INDEX sessions_player ON sessions(player_id);
     INSERT INTO sessions (token, player_id, created_at)
         SELECT token, id, created_at FROM players;
     CREATE TABLE players_v2 (
         id TEXT PRIMARY KEY,
         created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
         username TEXT,
         username_lower TEXT UNIQUE,
         password_hash TEXT,
         elo INTEGER NOT NULL DEFAULT 1200,
         peak_elo INTEGER NOT NULL DEFAULT 1200,
         games INTEGER NOT NULL DEFAULT 0,
         wins INTEGER NOT NULL DEFAULT 0,
         losses INTEGER NOT NULL DEFAULT 0,
         draws INTEGER NOT NULL DEFAULT 0,
         last_seen TEXT
     );
     INSERT INTO players_v2 (id, created_at) SELECT id, created_at FROM players;
     DROP TABLE players;
     ALTER TABLE players_v2 RENAME TO players;
     CREATE INDEX players_ranking ON players(elo DESC, wins DESC);
     ALTER TABLE games ADD COLUMN rated INTEGER NOT NULL DEFAULT 0;
     ALTER TABLE games ADD COLUMN reason TEXT NOT NULL DEFAULT '';
     ALTER TABLE games ADD COLUMN plies INTEGER NOT NULL DEFAULT 0;
     ALTER TABLE games ADD COLUMN white_elo_before INTEGER;
     ALTER TABLE games ADD COLUMN white_elo_after INTEGER;
     ALTER TABLE games ADD COLUMN black_elo_before INTEGER;
     ALTER TABLE games ADD COLUMN black_elo_after INTEGER;
     ALTER TABLE games ADD COLUMN started_at TEXT;
     CREATE TABLE rating_history (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         player_id TEXT NOT NULL REFERENCES players(id),
         game_id TEXT NOT NULL,
         elo INTEGER NOT NULL,
         at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );
     CREATE INDEX rating_history_player ON rating_history(player_id, id);
     CREATE INDEX games_white ON games(white);
     CREATE INDEX games_black ON games(black);
     CREATE TABLE friendships (
         user_a TEXT NOT NULL REFERENCES players(id),
         user_b TEXT NOT NULL REFERENCES players(id),
         requester TEXT NOT NULL REFERENCES players(id),
         status TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
         created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
         PRIMARY KEY (user_a, user_b),
         CHECK (user_a < user_b)
     );
     CREATE INDEX friendships_b ON friendships(user_b);",
    // Replays (docs/spec-v4.md §1): every game keeps its loadouts and actions,
    // and Solo games are recorded too, so `games` is rebuilt with nullable
    // seats (the bot has no player row). Rows from before have no `actions`:
    // their replay is unavailable. Analyses are cached per (game, depth).
    "CREATE TABLE games_v3 (
         id TEXT PRIMARY KEY,
         white TEXT REFERENCES players(id),
         black TEXT REFERENCES players(id),
         outcome TEXT NOT NULL,
         finished_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
         rated INTEGER NOT NULL DEFAULT 0,
         reason TEXT NOT NULL DEFAULT '',
         plies INTEGER NOT NULL DEFAULT 0,
         white_elo_before INTEGER,
         white_elo_after INTEGER,
         black_elo_before INTEGER,
         black_elo_after INTEGER,
         started_at TEXT,
         kind TEXT NOT NULL DEFAULT 'duel',
         loadouts TEXT,
         actions TEXT,
         solo_elo INTEGER
     );
     INSERT INTO games_v3 (id, white, black, outcome, finished_at, rated, reason, plies,
             white_elo_before, white_elo_after, black_elo_before, black_elo_after, started_at)
         SELECT id, white, black, outcome, finished_at, rated, reason, plies,
             white_elo_before, white_elo_after, black_elo_before, black_elo_after, started_at
         FROM games;
     DROP TABLE games;
     ALTER TABLE games_v3 RENAME TO games;
     CREATE INDEX games_white ON games(white);
     CREATE INDEX games_black ON games(black);
     CREATE TABLE game_analysis (
         game_id TEXT NOT NULL,
         depth INTEGER NOT NULL,
         result TEXT NOT NULL,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
         PRIMARY KEY (game_id, depth)
     );",
    // Forged skills (docs/spec-forge.md): a skill made up by the forge is a
    // definition stored here and known as `forged_<id>` everywhere else. The
    // fingerprint keeps the same definition from being stored twice (and its
    // low 32 bits are the id, so ids never clash between databases in one
    // process); the signature is what makes a later skill redundant with this one.
    "CREATE TABLE forged_skill (
         id INTEGER PRIMARY KEY,
         fingerprint TEXT NOT NULL UNIQUE,
         signature TEXT NOT NULL,
         def_json TEXT NOT NULL,
         rarity TEXT NOT NULL,
         score REAL NOT NULL,
         cost REAL NOT NULL,
         tone REAL NOT NULL,
         redundant INTEGER NOT NULL DEFAULT 0,
         gen_version INTEGER NOT NULL,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
         retired INTEGER NOT NULL DEFAULT 0
     );
     CREATE INDEX forged_signature ON forged_skill(signature);",
    // The history of each player's skills (what the Collection screen shows):
    // every gain and loss, with how it happened. Skills owned before the
    // journal existed are entered once, dated from the account's creation.
    "CREATE TABLE skill_history (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         player_id TEXT NOT NULL REFERENCES players(id),
         skill TEXT NOT NULL,
         change TEXT NOT NULL,
         source TEXT NOT NULL,
         other TEXT,
         at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );
     CREATE INDEX skill_history_player ON skill_history(player_id, id);
     INSERT INTO skill_history (player_id, skill, change, source, at)
         SELECT ps.player_id, ps.skill, 'gained', 'earlier',
                strftime('%Y-%m-%dT%H:%M:%SZ', p.created_at)
         FROM player_skills ps JOIN players p ON p.id = ps.player_id
         ORDER BY ps.rowid;",
    // Sessions expire after a period of inactivity (docs/spec-v4.md, Sessions):
    // `last_used` is ISO 8601 UTC like every other timestamp compared in SQL.
    // Existing sessions are backfilled with the migration time, so a deploy
    // neither logs everybody out nor leaves old sessions immortal. The table is
    // rebuilt rather than altered (SQLite has no ADD COLUMN IF NOT EXISTS), so the
    // step still runs if a test rewinds `user_version` over a newer schema.
    "CREATE TABLE sessions_v2 (
         token TEXT PRIMARY KEY,
         player_id TEXT NOT NULL REFERENCES players(id),
         created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
         last_used TEXT NOT NULL
     );
     INSERT INTO sessions_v2 (token, player_id, created_at, last_used)
         SELECT token, player_id, created_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         FROM sessions;
     DROP TABLE sessions;
     ALTER TABLE sessions_v2 RENAME TO sessions;
     CREATE INDEX sessions_player ON sessions(player_id);
     CREATE INDEX sessions_last_used ON sessions(last_used);",
    // Account recovery (docs/spec-v2.md, section 1): one recovery code per account,
    // stored only as an argon2id hash. A new table rather than a column on `players`,
    // so the step is idempotent if a test rewinds `user_version`. Accounts created
    // before this step simply have no row until they ask for a code.
    "CREATE TABLE IF NOT EXISTS recovery_codes (
         player_id TEXT PRIMARY KEY REFERENCES players(id),
         code_hash TEXT NOT NULL,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );",
    // Chat moderation (docs/spec-v2.md §4, "Modération"): block lists, the
    // per-account "mute all chat" setting and the reports players file. New
    // tables only, and every statement is idempotent, so the step is safe to
    // run again over a schema a test rewound.
    "CREATE TABLE IF NOT EXISTS blocks (
         blocker TEXT NOT NULL REFERENCES players(id),
         blocked TEXT NOT NULL REFERENCES players(id),
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
         PRIMARY KEY (blocker, blocked)
     );
     CREATE INDEX IF NOT EXISTS blocks_blocked ON blocks(blocked);
     CREATE TABLE IF NOT EXISTS chat_settings (
         player_id TEXT PRIMARY KEY REFERENCES players(id),
         chat_muted INTEGER NOT NULL DEFAULT 0
     );
     CREATE TABLE IF NOT EXISTS reports (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         reporter TEXT NOT NULL REFERENCES players(id),
         target TEXT NOT NULL REFERENCES players(id),
         reason TEXT NOT NULL,
         game_id TEXT,
         context TEXT,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );
     CREATE INDEX IF NOT EXISTS reports_pair ON reports(reporter, target, created_at);
     CREATE INDEX IF NOT EXISTS reports_reporter ON reports(reporter, created_at);",
    // Game length asked for (`short`/`medium`/`long`); NULL = default clock or old game.
    "ALTER TABLE games ADD COLUMN time_control TEXT;",
    // Campaign: `stars` is a bitmask (1 win, 2 objective, 4 challenge) cumulated
    // over attempts; `rewarded` marks a boss forge already granted.
    "CREATE TABLE IF NOT EXISTS campaign_progress (
         player_id TEXT NOT NULL REFERENCES players(id),
         chapter INTEGER NOT NULL,
         level INTEGER NOT NULL,
         stars INTEGER NOT NULL DEFAULT 0,
         rewarded INTEGER NOT NULL DEFAULT 0,
         PRIMARY KEY (player_id, chapter, level)
     );",
    // Campaign v2: consecutive defeats (hint unlock), the chosen title (chapter,
    // NULL = none), the boss forge state machine and the rewards waiting for a
    // winner's choice (kept across restarts). The columns are guarded by `migrate`.
    "ALTER TABLE campaign_progress ADD COLUMN defeats INTEGER NOT NULL DEFAULT 0;
     ALTER TABLE players ADD COLUMN title_active INTEGER;
     CREATE TABLE IF NOT EXISTS campaign_boss_forges (
         player_id TEXT NOT NULL REFERENCES players(id),
         chapter INTEGER NOT NULL,
         skill_id INTEGER,
         state TEXT NOT NULL CHECK (state IN ('forging', 'pending', 'placed')),
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
         PRIMARY KEY (player_id, chapter)
     );
     CREATE TABLE IF NOT EXISTS pending_rewards (
         winner_id TEXT PRIMARY KEY REFERENCES players(id),
         loser_id TEXT NOT NULL,
         loser_skills TEXT NOT NULL,
         expires_at TEXT NOT NULL,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );",
    // Reward announcements for a loser who was offline when the winner chose;
    // `skill`/`refilled` are JSON skill ids (wire names), delivered then deleted.
    "CREATE TABLE IF NOT EXISTS reward_outcomes (
         id INTEGER PRIMARY KEY,
         player_id TEXT NOT NULL REFERENCES players(id),
         by_name TEXT NOT NULL,
         kind TEXT NOT NULL CHECK (kind IN ('stolen', 'forged', 'spared')),
         skill TEXT,
         refilled TEXT,
         created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     );
     CREATE INDEX IF NOT EXISTS reward_outcomes_player ON reward_outcomes(player_id);",
];

/// SQLite has no ADD COLUMN IF NOT EXISTS: drops the `ALTER TABLE .. ADD COLUMN`
/// statements whose column is already there (a test rewound `user_version`
/// over a newer schema).
fn without_existing_columns(conn: &Connection, sql: &str) -> StoreResult<String> {
    let mut kept = Vec::new();
    for statement in sql.split(';') {
        let words: Vec<&str> = statement.split_whitespace().collect();
        if let ["ALTER", "TABLE", table, "ADD", "COLUMN", column, ..] = words[..] {
            let exists = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                params![table, column],
                |r| r.get::<_, i64>(0),
            )? > 0;
            if exists {
                continue;
            }
        }
        kept.push(statement);
    }
    Ok(kept.join(";"))
}

fn migrate(conn: &mut Connection) -> StoreResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= MIGRATIONS.len() as i64 {
        return Ok(());
    }
    // Rebuilding a table referenced by others needs enforcement off, and the
    // pragma is a no-op inside a transaction.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = (|| -> StoreResult<()> {
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(&without_existing_columns(&tx, sql)?)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}

impl Store {
    /// The database connection. A panic in another thread while it held the lock
    /// poisons the mutex; rather than turn every later request into a panic, take
    /// the guard back (same policy as the hub's mutex in `app.rs`). A transaction
    /// that was in flight rolls back when its guard drops, so the connection stays usable.
    pub(crate) fn db(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Opens (creating if needed) the database at `path`; `":memory:"` works for tests.
    pub fn open(path: &str) -> StoreResult<Self> {
        let mut conn = Connection::open(path)?;
        migrate(&mut conn)?;
        let store = Store {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.load_forged()?;
        Ok(store)
    }

    /// Creates a guest holding a starter deck of random classic skills,
    /// with a first session. Returns the player id and the session token.
    pub fn create_player(&self) -> StoreResult<(PlayerId, String)> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let created = insert_guest(&tx)?;
        tx.commit()?;
        Ok(created)
    }

    /// Opens a new session for an existing player.
    pub fn create_session(&self, player: &str) -> StoreResult<String> {
        let conn = self.db();
        insert_session(&conn, player)
    }

    pub fn delete_session(&self, token: &str) -> StoreResult<()> {
        let conn = self.db();
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    /// Deletes every session of `player` and returns their tokens (so the
    /// caller can close the WebSockets that used them).
    pub fn delete_sessions_of(&self, player: &str) -> StoreResult<Vec<String>> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let tokens = {
            let mut stmt = tx.prepare("SELECT token FROM sessions WHERE player_id = ?1")?;
            let rows = stmt.query_map(params![player], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        tx.execute("DELETE FROM sessions WHERE player_id = ?1", params![player])?;
        tx.commit()?;
        Ok(tokens)
    }

    /// Like [`Self::player_by_token`], but a session unused for more than `ttl`
    /// is expired and does not resolve. A live one has its `last_used`
    /// refreshed, at most once per `touch_interval` (no write otherwise).
    pub fn session_player(
        &self,
        token: &str,
        ttl: Duration,
        touch_interval: Duration,
    ) -> StoreResult<Option<PlayerId>> {
        let conn = self.db();
        let found: Option<(PlayerId, bool)> = conn
            .query_row(
                &format!(
                    "SELECT player_id, last_used < strftime('{ISO_FORMAT}', 'now', ?3)
                     FROM sessions
                     WHERE token = ?1 AND last_used >= strftime('{ISO_FORMAT}', 'now', ?2)"
                ),
                params![token, seconds_ago(ttl), seconds_ago(touch_interval)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((player, stale)) = found else {
            return Ok(None);
        };
        if stale {
            conn.execute(
                &format!("UPDATE sessions SET last_used = {ISO_NOW} WHERE token = ?1"),
                params![token],
            )?;
        }
        Ok(Some(player))
    }

    /// Deletes the sessions unused for more than `ttl`; returns how many.
    pub fn purge_expired_sessions(&self, ttl: Duration) -> StoreResult<usize> {
        let conn = self.db();
        Ok(conn.execute(
            &format!("DELETE FROM sessions WHERE last_used < strftime('{ISO_FORMAT}', 'now', ?1)"),
            params![seconds_ago(ttl)],
        )?)
    }

    pub fn player_by_token(&self, token: &str) -> StoreResult<Option<PlayerId>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT player_id FROM sessions WHERE token = ?1",
                params![token],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Turns a guest into an account, or creates a fresh account when
    /// `guest_token` is missing, unknown or already belongs to an account.
    /// A promoted guest keeps its id, deck and session; returns the id and
    /// the session token to use.
    pub fn register(
        &self,
        username: &str,
        password_hash: &str,
        guest_token: Option<&str>,
    ) -> Result<(PlayerId, String), RegisterError> {
        self.register_with_recovery(username, password_hash, guest_token, None)
    }

    /// Like [`Self::register`], and stores the hash of the account's recovery
    /// code in the same transaction.
    pub fn register_with_recovery(
        &self,
        username: &str,
        password_hash: &str,
        guest_token: Option<&str>,
        recovery_hash: Option<&str>,
    ) -> Result<(PlayerId, String), RegisterError> {
        let mut conn = self.db();
        let tx = conn.transaction().map_err(|_| RegisterError::Db)?;
        let lower = username.to_ascii_lowercase();
        let taken: bool = tx
            .query_row(
                "SELECT 1 FROM players WHERE username_lower = ?1",
                params![lower],
                |_| Ok(true),
            )
            .optional()
            .map_err(|_| RegisterError::Db)?
            .unwrap_or(false);
        if taken {
            return Err(RegisterError::UsernameTaken);
        }
        let guest = match guest_token {
            Some(t) => tx
                .query_row(
                    "SELECT p.id FROM sessions s JOIN players p ON p.id = s.player_id
                     WHERE s.token = ?1 AND p.username IS NULL",
                    params![t],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(|_| RegisterError::Db)?
                .map(|id| (id, t.to_string())),
            None => None,
        };
        let (id, token) = match guest {
            Some(found) => found,
            None => insert_guest(&tx).map_err(|_| RegisterError::Db)?,
        };
        tx.execute(
            "UPDATE players SET username = ?2, username_lower = ?3, password_hash = ?4
             WHERE id = ?1",
            params![id, username, lower, password_hash],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                RegisterError::UsernameTaken
            }
            _ => RegisterError::Db,
        })?;
        if let Some(hash) = recovery_hash {
            put_recovery_hash(&tx, &id, hash).map_err(|_| RegisterError::Db)?;
        }
        tx.commit().map_err(|_| RegisterError::Db)?;
        Ok((id, token))
    }

    /// The id and password hash of the account named `username` (any case).
    pub fn credentials(&self, username: &str) -> StoreResult<Option<(PlayerId, String)>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT id, password_hash FROM players
                 WHERE username_lower = ?1 AND password_hash IS NOT NULL",
                params![username.to_ascii_lowercase()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// The password hash of `player`; `None` for a guest (or an unknown id).
    pub fn password_hash_of(&self, player: &str) -> StoreResult<Option<String>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT password_hash FROM players WHERE id = ?1",
                params![player],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// The registered account named `username` (any case) with the hash of its
    /// recovery code, which is `None` for an account that never asked for one.
    pub fn recovery_credentials(
        &self,
        username: &str,
    ) -> StoreResult<Option<(PlayerId, Option<String>)>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT p.id, r.code_hash FROM players p
                 LEFT JOIN recovery_codes r ON r.player_id = p.id
                 WHERE p.username_lower = ?1 AND p.password_hash IS NOT NULL",
                params![username.to_ascii_lowercase()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// Creates or replaces the recovery code hash of `player`.
    pub fn set_recovery_hash(&self, player: &str, code_hash: &str) -> StoreResult<()> {
        let conn = self.db();
        put_recovery_hash(&conn, player, code_hash)
    }

    /// Redeems a recovery code: in one transaction, swaps its hash for
    /// `new_code_hash` (only if it still is `old_code_hash`, so two concurrent
    /// requests cannot both redeem one code), sets the new password hash and
    /// deletes every session of the account. Returns the deleted tokens, or
    /// `None` if the code had already been replaced.
    pub fn recover_account(
        &self,
        player: &str,
        old_code_hash: &str,
        new_code_hash: &str,
        new_password_hash: &str,
    ) -> StoreResult<Option<Vec<String>>> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let swapped = tx.execute(
            &format!(
                "UPDATE recovery_codes SET code_hash = ?3, created_at = {ISO_NOW}
                 WHERE player_id = ?1 AND code_hash = ?2"
            ),
            params![player, old_code_hash, new_code_hash],
        )?;
        if swapped != 1 {
            return Ok(None);
        }
        tx.execute(
            "UPDATE players SET password_hash = ?2 WHERE id = ?1",
            params![player, new_password_hash],
        )?;
        let tokens = {
            let mut stmt = tx.prepare("SELECT token FROM sessions WHERE player_id = ?1")?;
            let rows = stmt.query_map(params![player], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        tx.execute("DELETE FROM sessions WHERE player_id = ?1", params![player])?;
        tx.commit()?;
        Ok(Some(tokens))
    }

    pub fn player_row(&self, player: &str) -> StoreResult<Option<PlayerRow>> {
        let conn = self.db();
        player_row(&conn, "id", player)
    }

    /// The account named `username` (any case), if there is one.
    pub fn account_by_name(&self, username: &str) -> StoreResult<Option<PlayerRow>> {
        let conn = self.db();
        player_row(&conn, "username_lower", &username.to_ascii_lowercase())
    }

    /// Consecutive defeats of a player on each campaign level they lost.
    pub fn campaign_defeats(
        &self,
        player: &str,
    ) -> StoreResult<Vec<(crate::campaign::LevelRef, u32)>> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT chapter, level, defeats FROM campaign_progress
             WHERE player_id = ?1 AND defeats > 0",
        )?;
        let rows = stmt.query_map(params![player], |r| {
            let at = crate::campaign::LevelRef {
                chapter: r.get(0)?,
                level: r.get(1)?,
            };
            Ok((at, r.get(2)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Chooses the campaign title shown on the profile (`None` shows none).
    pub fn set_title_active(&self, player: &str, chapter: Option<u8>) -> StoreResult<()> {
        self.db().execute(
            "UPDATE players SET title_active = ?2 WHERE id = ?1",
            params![player, chapter],
        )?;
        Ok(())
    }

    pub fn me(&self, player: &str) -> StoreResult<Option<Me>> {
        let conn = self.db();
        let Some(row) = player_row(&conn, "id", player)? else {
            return Ok(None);
        };
        let rank = rank_of(&conn, &row)?;
        let chat_muted = chat_muted(&conn, &row.id)?;
        Ok(Some(Me {
            player_id: row.id,
            guest: row.username.is_none(),
            username: row.username,
            elo: row.elo,
            rank,
            games: row.games,
            wins: row.wins,
            draws: row.draws,
            losses: row.losses,
            chat_muted,
        }))
    }

    pub fn touch_last_seen(&self, player: &str) -> StoreResult<()> {
        let conn = self.db();
        conn.execute(
            &format!("UPDATE players SET last_seen = {ISO_NOW} WHERE id = ?1"),
            params![player],
        )?;
        Ok(())
    }

    /// Registered accounts only, best first.
    pub fn leaderboard(&self, limit: u32, offset: u32) -> StoreResult<Leaderboard> {
        let conn = self.db();
        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM players WHERE username IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT username, elo, games, wins, draws, losses FROM players
             WHERE username IS NOT NULL
             ORDER BY elo DESC, wins DESC, username_lower
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |r| {
            Ok(LeaderboardEntry {
                rank: 0,
                username: r.get(0)?,
                elo: r.get(1)?,
                games: r.get(2)?,
                wins: r.get(3)?,
                draws: r.get(4)?,
                losses: r.get(5)?,
            })
        })?;
        let mut entries = Vec::new();
        for (i, row) in rows.enumerate() {
            let mut entry = row?;
            entry.rank = offset + i as u32 + 1;
            entries.push(entry);
        }
        Ok(Leaderboard { total, entries })
    }

    pub fn public_profile(&self, username: &str) -> StoreResult<Option<PublicProfile>> {
        let conn = self.db();
        let Some(row) = player_row(&conn, "username_lower", &username.to_ascii_lowercase())? else {
            return Ok(None);
        };
        let Some(rank) = rank_of(&conn, &row)? else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            "SELECT elo, at FROM rating_history WHERE player_id = ?1 ORDER BY id DESC LIMIT 30",
        )?;
        let mut history = stmt
            .query_map(params![row.id], |r| {
                Ok(HistoryPoint {
                    elo: r.get(0)?,
                    at: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        history.reverse();
        if history.len() < 30 {
            history.insert(
                0,
                HistoryPoint {
                    elo: START_ELO,
                    at: row.created_at.clone(),
                },
            );
        }

        let recent = player_games(&conn, &row.id, false, 10)?
            .into_iter()
            .map(|g| RecentGame {
                game_id: g.id,
                opponent: g.opponent,
                result: g.result,
                color: g.color,
                rated: g.rated,
                elo_delta: g.elo_delta,
                reason: g.reason,
                at: g.at,
            })
            .collect();

        let mut streak = 0i32;
        for g in player_games(&conn, &row.id, true, 200)? {
            let step = match g.result {
                GameResult::Win => 1,
                GameResult::Loss => -1,
                GameResult::Draw => break,
            };
            if streak != 0 && streak.signum() != step {
                break;
            }
            streak += step;
        }

        Ok(Some(PublicProfile {
            username: row.username.clone().unwrap_or_default(),
            elo: row.elo,
            peak_elo: row.peak_elo,
            rank,
            games: row.games,
            wins: row.wins,
            draws: row.draws,
            losses: row.losses,
            streak,
            created_at: row.created_at,
            history,
            recent,
            title: displayed_title(
                &crate::campaign_store::rows_of(&conn, &row.id)?,
                row.title_active,
            ),
        }))
    }

    pub fn deck(&self, player: &str) -> StoreResult<Vec<SkillId>> {
        let conn = self.db();
        deck_of(&conn, player)
    }

    pub fn unique_owner(&self, skill: SkillId) -> StoreResult<Option<PlayerId>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT player_id FROM unique_skill_owner WHERE skill = ?1",
                params![skill_name(skill)],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Logs a finished game. When it is rated, ratings and counters change in
    /// the same transaction and the movement is returned. Every game keeps its
    /// loadouts and actions so it can be replayed; a Solo game has one seat
    /// that is the bot (stored as NULL).
    pub fn record_game(&self, rec: &GameRecord) -> StoreResult<Option<EloChange>> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let change = if rec.rated {
            Some(settle_ratings(&tx, rec)?)
        } else {
            None
        };
        let seat = |id: &str| (!crate::bot::is_bot_id(id)).then(|| id.to_string());
        let (white, black) = (seat(rec.white), seat(rec.black));
        // The rating each account had going in, also for games that move no
        // rating (the replay list shows it). Guests have none.
        let elo_before = |id: &Option<String>| -> StoreResult<Option<i32>> {
            let Some(id) = id else { return Ok(None) };
            Ok(tx
                .query_row(
                    "SELECT elo FROM players WHERE id = ?1 AND username IS NOT NULL",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?)
        };
        let (white_before, black_before) = match change {
            Some(c) => (Some(c.white_before), Some(c.black_before)),
            None => (elo_before(&white)?, elo_before(&black)?),
        };
        let loadouts = serde_json::json!({
            "white": rec.loadouts[0],
            "black": rec.loadouts[1],
            "start": rec.start_fen,
        });
        tx.execute(
            "INSERT INTO games (id, white, black, outcome, finished_at, rated, reason, plies,
                 white_elo_before, white_elo_after, black_elo_before, black_elo_after, started_at,
                 kind, loadouts, actions, solo_elo, time_control)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?5, ?6, ?7,
                 ?8, ?9, ?10, ?11, strftime('%Y-%m-%dT%H:%M:%SZ', ?12, 'unixepoch'),
                 ?13, ?14, ?15, ?16, ?17)",
            params![
                rec.id,
                white,
                black,
                serde_json::to_string(rec.outcome).unwrap(),
                rec.rated,
                rec.reason,
                rec.plies,
                white_before,
                change.map(|c| c.white_after),
                black_before,
                change.map(|c| c.black_after),
                rec.started_unix,
                rec.kind.as_str(),
                loadouts.to_string(),
                serde_json::to_string(rec.actions).unwrap(),
                rec.solo_elo,
                rec.time_control.map(|t| t.as_str()),
            ],
        )?;
        tx.commit()?;
        Ok(change)
    }

    /// How many rated games `a` and `b` finished against each other (either
    /// colour) within the last `window_secs` seconds.
    pub fn rated_games_between(&self, a: &str, b: &str, window_secs: u64) -> StoreResult<u32> {
        let conn = self.db();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM games
             WHERE rated = 1
               AND ((white = ?1 AND black = ?2) OR (white = ?2 AND black = ?1))
               AND finished_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?3)",
            params![a, b, format!("-{window_secs} seconds")],
            |r| r.get(0),
        )?)
    }

    /// Atomically changes decks after a game: the loser may lose a skill, the
    /// winner may drop one (to make room) and gain one. Unique-skill ownership
    /// follows the skills; a unique skill can only be gained if it is unowned
    /// or being taken from the loser.
    pub fn apply_reward(
        &self,
        winner: &str,
        loser: &str,
        gain: Option<SkillId>,
        loser_loses: Option<SkillId>,
        winner_drops: Option<SkillId>,
    ) -> StoreResult<()> {
        self.apply_deck_change(winner, Some(loser), gain, loser_loses, winner_drops)
    }

    /// [`Self::apply_reward`] where the loser may be absent (a campaign boss
    /// reward takes from nobody).
    pub fn apply_deck_change(
        &self,
        winner: &str,
        loser: Option<&str>,
        gain: Option<SkillId>,
        loser_loses: Option<SkillId>,
        winner_drops: Option<SkillId>,
    ) -> StoreResult<()> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        let remove = |player: &str, skill: SkillId| -> StoreResult<()> {
            let name = skill_name(skill);
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
            Ok(())
        };
        if let (Some(loser), Some(skill)) = (loser, loser_loses) {
            remove(loser, skill)?;
        }
        if let Some(skill) = winner_drops {
            remove(winner, skill)?;
        }
        if let Some(skill) = gain {
            let name = skill_name(skill);
            tx.execute(
                "INSERT INTO player_skills (player_id, skill) VALUES (?1, ?2)",
                params![winner, name],
            )?;
            if skill.kind() == SkillKind::Unique {
                // Fails on the primary key if someone else still owns it.
                tx.execute(
                    "INSERT INTO unique_skill_owner (skill, player_id) VALUES (?1, ?2)",
                    params![name, winner],
                )?;
            }
        }
        // The journal: who lost what, and how the winner came by the new one.
        let winner_name = history::username_of(&tx, winner);
        let loser_name = loser.and_then(|l| history::username_of(&tx, l));
        if let (Some(loser), Some(skill)) = (loser, loser_loses) {
            history::log(
                &tx,
                loser,
                skill,
                Change::Lost,
                Source::Taken,
                winner_name.as_deref(),
            )?;
        }
        if let Some(skill) = winner_drops {
            history::log(&tx, winner, skill, Change::Lost, Source::Replaced, None)?;
        }
        if let Some(skill) = gain {
            let (source, other) = if skill.is_forged() {
                (Source::Forged, None)
            } else if loser_loses == Some(skill) {
                (Source::Stolen, loser_name.as_deref())
            } else {
                (Source::Won, None)
            };
            history::log(&tx, winner, skill, Change::Gained, source, other)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Replaces a player's whole deck. Fails, changing nothing, if a unique
    /// skill in `skills` belongs to someone else.
    pub fn set_deck(&self, player: &str, skills: &[SkillId]) -> StoreResult<()> {
        let mut conn = self.db();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM player_skills WHERE player_id = ?1",
            params![player],
        )?;
        tx.execute(
            "DELETE FROM unique_skill_owner WHERE player_id = ?1",
            params![player],
        )?;
        for &skill in skills {
            let name = skill_name(skill);
            tx.execute(
                "INSERT INTO player_skills (player_id, skill) VALUES (?1, ?2)",
                params![player, name],
            )?;
            if skill.kind() == SkillKind::Unique {
                tx.execute(
                    "INSERT INTO unique_skill_owner (skill, player_id) VALUES (?1, ?2)",
                    params![name, player],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// A player left with no skills gets one random classic skill.
    pub fn refill_if_empty(&self, player: &str) -> StoreResult<()> {
        let conn = self.db();
        if !deck_of(&conn, player)?.is_empty() {
            return Ok(());
        }
        let pool = classic_skills();
        let skill = pool
            .choose(&mut rand::rng())
            .copied()
            .ok_or(StoreError::Invalid("no classic skills"))?;
        conn.execute(
            "INSERT INTO player_skills (player_id, skill) VALUES (?1, ?2)",
            params![player, skill_name(skill)],
        )?;
        history::log(&conn, player, skill, Change::Gained, Source::Refill, None)?;
        Ok(())
    }
}

fn deck_of(conn: &Connection, player: &str) -> StoreResult<Vec<SkillId>> {
    let mut stmt =
        conn.prepare("SELECT skill FROM player_skills WHERE player_id = ?1 ORDER BY rowid")?;
    let names = stmt.query_map(params![player], |r| r.get::<_, String>(0))?;
    let mut deck = Vec::new();
    for name in names {
        if let Some(skill) = parse_skill(&name?) {
            deck.push(skill);
        }
    }
    Ok(deck)
}

fn put_recovery_hash(conn: &Connection, player: &str, code_hash: &str) -> StoreResult<()> {
    conn.execute(
        &format!(
            "INSERT INTO recovery_codes (player_id, code_hash) VALUES (?1, ?2)
             ON CONFLICT(player_id) DO UPDATE
             SET code_hash = excluded.code_hash, created_at = {ISO_NOW}"
        ),
        params![player, code_hash],
    )?;
    Ok(())
}

fn insert_session(conn: &Connection, player: &str) -> StoreResult<String> {
    let token = random_hex(16);
    conn.execute(
        &format!("INSERT INTO sessions (token, player_id, last_used) VALUES (?1, ?2, {ISO_NOW})"),
        params![token, player],
    )?;
    Ok(token)
}

/// A new guest with a random starter deck and a session.
fn insert_guest(tx: &rusqlite::Transaction) -> StoreResult<(PlayerId, String)> {
    let id = random_hex(8);
    tx.execute("INSERT INTO players (id) VALUES (?1)", params![id])?;
    let pool = classic_skills();
    let mut rng = rand::rng();
    for skill in pool.sample(&mut rng, STARTER_DECK_SIZE) {
        tx.execute(
            "INSERT INTO player_skills (player_id, skill) VALUES (?1, ?2)",
            params![id, skill_name(*skill)],
        )?;
        history::log(tx, &id, *skill, Change::Gained, Source::Starter, None)?;
    }
    let token = insert_session(tx, &id)?;
    Ok((id, token))
}

fn player_row(conn: &Connection, column: &str, value: &str) -> StoreResult<Option<PlayerRow>> {
    // `column` is one of two literals chosen by this module, never user input.
    let sql = format!(
        "SELECT id, username, elo, peak_elo, games, wins, draws, losses,
                strftime('%Y-%m-%dT%H:%M:%SZ', created_at), last_seen, title_active
         FROM players WHERE {column} = ?1"
    );
    Ok(conn
        .query_row(&sql, params![value], |r| {
            Ok(PlayerRow {
                id: r.get(0)?,
                username: r.get(1)?,
                elo: r.get(2)?,
                peak_elo: r.get(3)?,
                games: r.get(4)?,
                wins: r.get(5)?,
                draws: r.get(6)?,
                losses: r.get(7)?,
                created_at: r.get(8)?,
                last_seen: r.get(9)?,
                title_active: r.get(10)?,
            })
        })
        .optional()?)
}

/// The chosen title if it is earned, else the best earned one.
fn displayed_title(
    rows: &[crate::campaign_store::CampaignRow],
    active: Option<u8>,
) -> Option<&'static str> {
    let earned = crate::campaign::titles_earned(rows);
    active
        .and_then(|chapter| earned.iter().find(|(c, _)| *c == chapter))
        .or(earned.last())
        .map(|&(_, title)| title)
}

/// 1-based rank among registered accounts (`elo DESC, wins DESC, username`).
fn rank_of(conn: &Connection, row: &PlayerRow) -> StoreResult<Option<u32>> {
    let Some(name) = &row.username else {
        return Ok(None);
    };
    let ahead: u32 = conn.query_row(
        "SELECT COUNT(*) FROM players
         WHERE username IS NOT NULL
           AND (elo > ?1 OR (elo = ?1 AND wins > ?2)
                OR (elo = ?1 AND wins = ?2 AND username_lower < ?3))",
        params![row.elo, row.wins, name.to_ascii_lowercase()],
        |r| r.get(0),
    )?;
    Ok(Some(ahead + 1))
}

/// Applies Elo and counters for a rated game inside `tx`.
fn settle_ratings(tx: &rusqlite::Transaction, rec: &GameRecord) -> StoreResult<EloChange> {
    let load = |id: &str| -> StoreResult<(i32, u32)> {
        Ok(tx.query_row(
            "SELECT elo, games FROM players WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    };
    let (white_before, white_games) = load(rec.white)?;
    let (black_before, black_games) = load(rec.black)?;
    let white_score = match rec.outcome.winner() {
        Some(Color::White) => 1.0,
        Some(Color::Black) => 0.0,
        None => 0.5,
    };
    let white_after = elo::new_rating(white_before, black_before, white_score, white_games);
    let black_after = elo::new_rating(black_before, white_before, 1.0 - white_score, black_games);
    for (player, after, score) in [
        (rec.white, white_after, white_score),
        (rec.black, black_after, 1.0 - white_score),
    ] {
        let (win, draw, loss) = match score {
            s if s > 0.75 => (1, 0, 0),
            s if s < 0.25 => (0, 0, 1),
            _ => (0, 1, 0),
        };
        tx.execute(
            "UPDATE players SET elo = ?2, peak_elo = MAX(peak_elo, ?2), games = games + 1,
                 wins = wins + ?3, draws = draws + ?4, losses = losses + ?5
             WHERE id = ?1",
            params![player, after, win, draw, loss],
        )?;
        tx.execute(
            "INSERT INTO rating_history (player_id, game_id, elo) VALUES (?1, ?2, ?3)",
            params![player, rec.id, after],
        )?;
    }
    Ok(EloChange {
        white_before,
        white_after,
        black_before,
        black_after,
    })
}

struct GameRow {
    id: String,
    opponent: Option<String>,
    result: GameResult,
    color: Color,
    rated: bool,
    elo_delta: Option<i32>,
    reason: String,
    at: String,
}

/// A player's finished games, newest first.
fn player_games(
    conn: &Connection,
    player: &str,
    rated_only: bool,
    limit: u32,
) -> StoreResult<Vec<GameRow>> {
    let sql = format!(
        "SELECT g.id, g.white = ?1, g.outcome, g.rated, g.reason, opp.username,
                CASE WHEN g.white = ?1 THEN g.white_elo_after - g.white_elo_before
                     ELSE g.black_elo_after - g.black_elo_before END,
                strftime('%Y-%m-%dT%H:%M:%SZ', g.finished_at)
         FROM games g JOIN players opp ON opp.id = CASE WHEN g.white = ?1 THEN g.black ELSE g.white END
         WHERE (g.white = ?1 OR g.black = ?1) AND g.kind != 'solo' {}
         ORDER BY g.rowid DESC LIMIT ?2",
        if rated_only { "AND g.rated = 1" } else { "" }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![player, limit], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, bool>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<i32>>(6)?,
            r.get::<_, String>(7)?,
        ))
    })?;
    let mut games = Vec::new();
    for row in rows {
        let (id, is_white, outcome, rated, reason, opponent, elo_delta, at) = row?;
        let outcome: Outcome = serde_json::from_str(&outcome).unwrap_or(Outcome::Ongoing);
        let color = if is_white { Color::White } else { Color::Black };
        let result = match outcome.winner() {
            Some(w) if w == color => GameResult::Win,
            Some(_) => GameResult::Loss,
            None => GameResult::Draw,
        };
        games.push(GameRow {
            id,
            opponent,
            result,
            color,
            rated,
            elo_delta: if rated { elo_delta } else { None },
            reason: if reason.is_empty() {
                reason_of(&outcome).to_string()
            } else {
                reason
            },
            at,
        });
    }
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn the_campaign_step_runs_again_over_a_rewound_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let latest = user_version(&conn);
        conn.pragma_update(None, "user_version", latest - 1)
            .unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn), latest);
        let columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('campaign_progress') WHERE name = 'defeats'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(columns, 1);
    }

    #[test]
    fn the_active_title_is_stored_and_cleared() {
        let store = Store::open(":memory:").unwrap();
        let (player, _) = store.create_player().unwrap();
        assert_eq!(
            store.player_row(&player).unwrap().unwrap().title_active,
            None
        );
        store.set_title_active(&player, Some(3)).unwrap();
        assert_eq!(
            store.player_row(&player).unwrap().unwrap().title_active,
            Some(3)
        );
        store.set_title_active(&player, None).unwrap();
        assert_eq!(
            store.player_row(&player).unwrap().unwrap().title_active,
            None
        );
    }

    /// A panic while another thread holds the connection poisons the mutex. Later
    /// requests must keep working (the guard is taken back), not panic in turn.
    #[test]
    fn a_poisoned_connection_lock_does_not_break_later_requests() {
        let store = Store::open(":memory:").expect("open in-memory database");
        let held = store.clone();
        let result = std::thread::spawn(move || {
            let _guard = held.conn.lock().unwrap();
            panic!("simulated panic while holding the connection");
        })
        .join();
        assert!(result.is_err(), "the helper thread must have panicked");
        assert!(store.conn.is_poisoned(), "the mutex must be poisoned");
        assert_eq!(store.player_by_token("no-such-token").unwrap(), None);
    }
}
