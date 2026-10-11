//! Reading back recorded games (docs/spec-v4.md §1 and §2): the list of a
//! player's games, one game with its actions, and the analysis cache.
//! Writing a game is [`Store::record_game`].

use chessy_engine::{Action, Color, Outcome, SkillId};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::protocol::TimeControl;
use crate::store::{reason_of, GameResult, Store, StoreResult};

/// How a game came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameKind {
    /// Found through a queue (ranked or friendly).
    Duel,
    /// A friend's challenge.
    Challenge,
    /// A private room.
    Room,
    /// Against the bot.
    Solo,
}

impl GameKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GameKind::Duel => "duel",
            GameKind::Challenge => "challenge",
            GameKind::Room => "room",
            GameKind::Solo => "solo",
        }
    }

    fn parse(text: &str) -> GameKind {
        match text {
            "challenge" => GameKind::Challenge,
            "room" => GameKind::Room,
            "solo" => GameKind::Solo,
            _ => GameKind::Duel,
        }
    }
}

/// Who sat on one side: an account, a guest (`username: null`) or the bot.
/// `elo` is the rating before the game, or the bot's level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Seat {
    pub username: Option<String>,
    pub elo: Option<i32>,
    pub bot: bool,
}

/// The skills each side brought.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loadouts {
    pub white: Vec<SkillId>,
    pub black: Vec<SkillId>,
    /// FEN of a custom starting position (campaign bosses); standard when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

impl Loadouts {
    pub fn pair(&self) -> [Vec<SkillId>; 2] {
        [self.white.clone(), self.black.clone()]
    }
}

/// One row of `GET /api/me/games`.
#[derive(Clone, Debug, Serialize)]
pub struct GameSummary {
    pub game_id: String,
    pub kind: GameKind,
    pub rated: bool,
    pub white: Seat,
    pub black: Seat,
    /// The asker's side.
    pub color: Color,
    pub result: GameResult,
    pub reason: String,
    pub plies: u32,
    pub elo_delta: Option<i32>,
    pub at: String,
    /// Game length asked for; `None` for the default clock, Solo and old games.
    pub time_control: Option<TimeControl>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GameList {
    pub total: u32,
    pub games: Vec<GameSummary>,
}

/// A recorded game, with what is needed to replay it when it was kept.
#[derive(Clone, Debug)]
pub struct StoredGame {
    pub id: String,
    pub kind: GameKind,
    pub rated: bool,
    pub white_id: Option<String>,
    pub black_id: Option<String>,
    pub white: Seat,
    pub black: Seat,
    pub outcome: Outcome,
    pub reason: String,
    pub plies: u32,
    pub at: String,
    /// `None` for games recorded before replays existed.
    pub loadouts: Option<Loadouts>,
    pub actions: Option<Vec<Action>>,
    /// `None` for the default clock, Solo and games recorded before it was kept.
    pub time_control: Option<TimeControl>,
}

impl StoredGame {
    /// Whether `viewer` (a player id) may read the game: everything is public
    /// but Solo games, which only their player sees.
    pub fn readable_by(&self, viewer: Option<&str>) -> bool {
        self.kind != GameKind::Solo
            || viewer.is_some_and(|v| {
                self.white_id.as_deref() == Some(v) || self.black_id.as_deref() == Some(v)
            })
    }

    /// The loadouts and actions to replay, when the game has them.
    pub fn replayable(&self) -> Option<(&Loadouts, &[Action])> {
        Some((self.loadouts.as_ref()?, self.actions.as_deref()?))
    }
}

/// Columns shared by both queries; the full one adds `loadouts` and `actions`.
const SELECT: &str = "SELECT g.id, g.kind, g.rated, g.white, g.black, w.username, b.username,
        g.white_elo_before, g.black_elo_before, g.white_elo_after, g.black_elo_after,
        g.solo_elo, g.outcome, g.reason, g.plies,
        strftime('%Y-%m-%dT%H:%M:%SZ', g.finished_at), g.time_control";
const FROM: &str = "FROM games g
        LEFT JOIN players w ON w.id = g.white
        LEFT JOIN players b ON b.id = g.black";

struct RawRow {
    game: StoredGame,
    elo_after: [Option<i32>; 2],
}

fn read_row(r: &rusqlite::Row, full: bool) -> rusqlite::Result<RawRow> {
    let kind = GameKind::parse(&r.get::<_, String>(1)?);
    let white_id: Option<String> = r.get(3)?;
    let black_id: Option<String> = r.get(4)?;
    let (white_name, black_name): (Option<String>, Option<String>) = (r.get(5)?, r.get(6)?);
    let (white_elo, black_elo): (Option<i32>, Option<i32>) = (r.get(7)?, r.get(8)?);
    let solo_elo: Option<i32> = r.get(11)?;
    let outcome: Outcome =
        serde_json::from_str(&r.get::<_, String>(12)?).unwrap_or(Outcome::Ongoing);
    let reason: String = r.get(13)?;
    let seat = |id: &Option<String>, name: Option<String>, elo: Option<i32>| {
        if id.is_none() && kind == GameKind::Solo {
            Seat {
                username: Some(crate::bot::BOT_NAME.to_string()),
                elo: solo_elo,
                bot: true,
            }
        } else {
            Seat {
                username: name,
                elo,
                bot: false,
            }
        }
    };
    let (loadouts, actions) = if full {
        let loadouts: Option<String> = r.get(17)?;
        let actions: Option<String> = r.get(18)?;
        (
            loadouts.and_then(|t| serde_json::from_str(&t).ok()),
            actions.and_then(|t| serde_json::from_str(&t).ok()),
        )
    } else {
        (None, None)
    };
    Ok(RawRow {
        elo_after: [r.get(9)?, r.get(10)?],
        game: StoredGame {
            id: r.get(0)?,
            kind,
            rated: r.get(2)?,
            white: seat(&white_id, white_name.clone(), white_elo),
            black: seat(&black_id, black_name.clone(), black_elo),
            white_id,
            black_id,
            reason: if reason.is_empty() {
                reason_of(&outcome).to_string()
            } else {
                reason
            },
            outcome,
            plies: r.get(14)?,
            at: r.get(15)?,
            loadouts,
            actions,
            time_control: r
                .get::<_, Option<String>>(16)?
                .and_then(|t| TimeControl::parse(&t)),
        },
    })
}

fn summary(raw: RawRow, player: &str) -> GameSummary {
    let g = raw.game;
    let color = if g.white_id.as_deref() == Some(player) {
        Color::White
    } else {
        Color::Black
    };
    let result = match g.outcome.winner() {
        Some(w) if w == color => GameResult::Win,
        Some(_) => GameResult::Loss,
        None => GameResult::Draw,
    };
    let (before, after) = match color {
        Color::White => (g.white.elo, raw.elo_after[0]),
        Color::Black => (g.black.elo, raw.elo_after[1]),
    };
    GameSummary {
        game_id: g.id,
        kind: g.kind,
        rated: g.rated,
        white: g.white,
        black: g.black,
        color,
        result,
        reason: g.reason,
        plies: g.plies,
        elo_delta: if g.rated {
            before.zip(after).map(|(b, a)| a - b)
        } else {
            None
        },
        at: g.at,
        time_control: g.time_control,
    }
}

fn load(conn: &Connection, id: &str) -> StoreResult<Option<StoredGame>> {
    let sql = format!("{SELECT}, g.loadouts, g.actions {FROM} WHERE g.id = ?1");
    Ok(conn
        .query_row(&sql, params![id], |r| read_row(r, true))
        .optional()?
        .map(|raw| raw.game))
}

impl Store {
    /// A player's games of every kind (Solo included), newest first.
    pub fn games_of(&self, player: &str, limit: u32, offset: u32) -> StoreResult<GameList> {
        let conn = self.db();
        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM games WHERE white = ?1 OR black = ?1",
            params![player],
            |r| r.get(0),
        )?;
        let sql = format!(
            "{SELECT} {FROM} WHERE g.white = ?1 OR g.black = ?1
             ORDER BY g.rowid DESC LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![player, limit, offset], |r| read_row(r, false))?;
        let mut games = Vec::new();
        for row in rows {
            games.push(summary(row?, player));
        }
        Ok(GameList { total, games })
    }

    /// One recorded game by id.
    pub fn stored_game(&self, id: &str) -> StoreResult<Option<StoredGame>> {
        let conn = self.db();
        load(&conn, id)
    }

    /// The cached analysis (its JSON body) of a game at a depth.
    pub fn analysis_get(&self, game_id: &str, depth: u32) -> StoreResult<Option<String>> {
        let conn = self.db();
        Ok(conn
            .query_row(
                "SELECT result FROM game_analysis WHERE game_id = ?1 AND depth = ?2",
                params![game_id, depth],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn analysis_put(&self, game_id: &str, depth: u32, body: &str) -> StoreResult<()> {
        let conn = self.db();
        conn.execute(
            "INSERT OR REPLACE INTO game_analysis (game_id, depth, result) VALUES (?1, ?2, ?3)",
            params![game_id, depth, body],
        )?;
        Ok(())
    }
}
