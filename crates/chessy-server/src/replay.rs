//! Replaying recorded games (docs/spec-v4.md §2): the engine is deterministic,
//! so `loadouts + actions` rebuild every position of a game. This module
//! turns them into [`Frame`]s (with *all* the information: invisible pieces
//! and traps included, the game being over), and answers the stateless
//! exploration of a variation. Pure and synchronous: callers run it in
//! `spawn_blocking`.

use std::time::{Duration, Instant};

use chessy_engine::analysis::{cap, search_position, terminal_score, white_pov};
use chessy_engine::notation::{move_notation, simulated_move_notation, skill_notation};
use chessy_engine::{
    Action, ActiveEffect, Color, EffectKind, Event, Game, Move, Outcome, Piece, Position,
    RuleError, SkillId, Square, Trap,
};
use serde::{Deserialize, Serialize};

use crate::games_store::{GameKind, Loadouts, Seat, StoredGame};
use crate::protocol::{SkillOptions, TerrainView, TimeControl};

/// Skills each side has used so far (Mind Reading counts from its first use).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UsedView {
    pub white: Vec<SkillId>,
    pub black: Vec<SkillId>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchedView {
    pub piece: Piece,
    pub square: Square,
    pub owner: Color,
    pub back_at: u32,
}

/// One position of a game. `ply` 0 is the initial position; `events` is what
/// happened to get here.
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub ply: u32,
    pub to_move: Color,
    pub in_check: bool,
    pub board: Vec<Option<Piece>>,
    pub effects: Vec<ActiveEffect>,
    pub traps: Vec<Trap>,
    pub terrain: Vec<TerrainView>,
    pub benched: Vec<BenchedView>,
    pub events: Vec<Event>,
    pub used: UsedView,
    pub outcome: Outcome,
}

/// A position of the game with the facts a [`Frame`] needs.
#[derive(Clone, Debug)]
pub struct Point {
    pub pos: Position,
    pub outcome: Outcome,
    pub used: [Vec<SkillId>; 2],
}

impl Point {
    fn of(game: &Game) -> Point {
        let used = |color: Color| {
            game.loadout(color)
                .slots
                .iter()
                .filter(|s| s.uses > 0)
                .map(|s| s.skill)
                .collect()
        };
        Point {
            pos: game.pos.clone(),
            outcome: game.outcome(),
            used: [used(Color::White), used(Color::Black)],
        }
    }

    /// The frame of this position; `events` is what led here.
    pub fn frame(&self, ply: u32, events: Vec<Event>, outcome: Outcome) -> Frame {
        let pos = &self.pos;
        Frame {
            ply,
            to_move: pos.side,
            in_check: pos.in_check(pos.side),
            board: pos.board.to_vec(),
            effects: pos
                .effects
                .iter()
                .filter(|e| e.kind != EffectKind::Terrain)
                .copied()
                .collect(),
            traps: pos.traps.clone(),
            terrain: pos
                .effects
                .iter()
                .filter(|e| e.kind == EffectKind::Terrain)
                .filter_map(|e| {
                    Some(TerrainView {
                        square: e.square?,
                        owner: e.owner?,
                        expires_at: e.expires_at,
                    })
                })
                .collect(),
            benched: pos
                .benched
                .iter()
                .map(|b| BenchedView {
                    piece: b.piece,
                    square: b.square,
                    owner: b.piece.color,
                    back_at: b.back_at,
                })
                .collect(),
            events,
            used: UsedView {
                white: self.used[0].clone(),
                black: self.used[1].clone(),
            },
            outcome,
        }
    }
}

/// One action of the game and the position it led to.
#[derive(Clone, Debug)]
pub struct Step {
    pub mover: Color,
    pub action: Action,
    pub events: Vec<Event>,
    pub notation: String,
    pub after: Point,
}

/// A whole game replayed.
pub struct Replay {
    pub initial: Point,
    pub steps: Vec<Step>,
}

impl Replay {
    /// The position before action `i` (0-based); `i == steps.len()` is the last one.
    pub fn point(&self, i: usize) -> &Point {
        if i == 0 {
            &self.initial
        } else {
            &self.steps[i - 1].after
        }
    }
}

pub fn new_game(loadouts: &Loadouts) -> Game {
    game_at(loadouts.start.as_deref(), &loadouts.white, &loadouts.black)
}

/// A game from the standard position, or from `start` (a FEN) when given.
pub fn game_at(start: Option<&str>, white: &[SkillId], black: &[SkillId]) -> Game {
    match start.map(Position::from_fen) {
        Some(Ok(pos)) => Game::from_position(pos, white, black),
        _ => Game::new(white, black),
    }
}

/// Plays `action` and describes it (notation, events, resulting position).
pub fn advance(game: &mut Game, action: Action) -> Result<Step, RuleError> {
    let before = game.pos.clone();
    let mover = game.side_to_move();
    let events = game.apply(action)?;
    let notation = match action {
        Action::Move { from, to, promo } => move_notation(
            &before,
            Move { from, to, promo },
            &events,
            &game.pos,
            matches!(game.outcome(), Outcome::Checkmate { .. }),
        ),
        Action::Skill { skill, target } => skill_notation(skill, target),
    };
    Ok(Step {
        mover,
        action,
        events,
        notation,
        after: Point::of(game),
    })
}

/// Replays `actions` from the start. On failure, the index of the action the
/// engine refused (which means the record is corrupt or the rules changed).
pub fn replay(loadouts: &Loadouts, actions: &[Action]) -> Result<Replay, usize> {
    let mut game = new_game(loadouts);
    let initial = Point::of(&game);
    let mut steps = Vec::with_capacity(actions.len());
    for (i, &action) in actions.iter().enumerate() {
        steps.push(advance(&mut game, action).map_err(|_| i)?);
    }
    Ok(Replay { initial, steps })
}

#[derive(Clone, Debug, Serialize)]
pub struct MoveInfo {
    pub ply: u32,
    pub color: Color,
    pub action: Action,
    pub notation: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultView {
    pub outcome: Outcome,
    pub reason: String,
}

/// `GET /api/games/{id}`.
#[derive(Clone, Debug, Serialize)]
pub struct ReplayView {
    pub game_id: String,
    pub kind: GameKind,
    pub rated: bool,
    pub white: Seat,
    pub black: Seat,
    pub result: ResultView,
    pub plies: u32,
    pub at: String,
    /// Game length asked for; null for the default clock, Solo and old games.
    pub time_control: Option<TimeControl>,
    pub loadouts: Loadouts,
    pub moves: Vec<MoveInfo>,
    pub frames: Vec<Frame>,
}

/// Everything the replay endpoint returns for a stored game, or the index of
/// the action that could not be replayed. `None` when the game has no record
/// of its actions.
pub fn view(game: &StoredGame) -> Option<Result<ReplayView, usize>> {
    let (loadouts, actions) = game.replayable()?;
    let replayed = match replay(loadouts, actions) {
        Ok(r) => r,
        Err(at) => return Some(Err(at)),
    };
    let last = replayed.steps.len();
    let mut frames = Vec::with_capacity(last + 1);
    frames.push(
        replayed
            .initial
            .frame(0, Vec::new(), replayed.initial.outcome),
    );
    let mut moves = Vec::with_capacity(last);
    for (i, step) in replayed.steps.iter().enumerate() {
        let ply = i as u32 + 1;
        // The last frame carries how the game really ended (resignation,
        // timeout...), which the rules alone cannot know.
        let outcome = if i + 1 == last {
            game.outcome
        } else {
            step.after.outcome
        };
        frames.push(step.after.frame(ply, step.events.clone(), outcome));
        moves.push(MoveInfo {
            ply,
            color: step.mover,
            action: step.action,
            notation: step.notation.clone(),
        });
    }
    if last == 0 {
        frames[0].outcome = game.outcome;
    }
    Some(Ok(ReplayView {
        game_id: game.id.clone(),
        kind: game.kind,
        rated: game.rated,
        white: game.white.clone(),
        black: game.black.clone(),
        result: ResultView {
            outcome: game.outcome,
            reason: game.reason.clone(),
        },
        plies: last as u32,
        at: game.at.clone(),
        time_control: game.time_control,
        loadouts: loadouts.clone(),
        moves,
        frames,
    }))
}

// ---- exploration -------------------------------------------------------------

/// The most actions a variation may hold.
pub const MAX_LINE: usize = 200;

#[derive(Debug, Deserialize)]
pub struct ExploreRequest {
    /// How many actions of the game to play before the variation.
    pub ply: i64,
    #[serde(default)]
    pub line: Vec<Action>,
    #[serde(default)]
    pub depth: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BestView {
    pub action: Action,
    pub notation: String,
    pub eval_cp: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExploreResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'static str>,
    /// How many actions of `line` were applied.
    pub at: u32,
    pub frame: Option<Frame>,
    pub moves: Vec<Move>,
    pub skill_options: Vec<SkillOptions>,
    pub eval_cp: i32,
    pub best: Option<BestView>,
    /// Notation of the applied actions of `line`.
    pub notation: Vec<String>,
}

impl ExploreResponse {
    fn bad_ply() -> Self {
        ExploreResponse {
            valid: false,
            error: Some("bad_ply"),
            at: 0,
            frame: None,
            moves: Vec::new(),
            skill_options: Vec::new(),
            eval_cp: 0,
            best: None,
            notation: Vec::new(),
        }
    }
}

fn push_target(
    options: &mut Vec<SkillOptions>,
    skill: SkillId,
    target: chessy_engine::SkillTarget,
) {
    match options.iter_mut().find(|o| o.skill == skill) {
        Some(o) => o.targets.push(target),
        None => options.push(SkillOptions {
            skill,
            targets: vec![target],
        }),
    }
}

/// Plays `ply` actions of the game, then the variation `line` up to the first
/// action the rules refuse, and describes the position reached for the side
/// to move. `Err(i)` when the stored game itself cannot be replayed.
/// `budget` caps the search for the best move.
pub fn explore(
    loadouts: &Loadouts,
    actions: &[Action],
    req: &ExploreRequest,
    depth: u32,
    budget: Duration,
) -> Result<ExploreResponse, usize> {
    let Ok(ply) = usize::try_from(req.ply) else {
        return Ok(ExploreResponse::bad_ply());
    };
    if ply > actions.len() {
        return Ok(ExploreResponse::bad_ply());
    }
    let mut game = new_game(loadouts);
    let mut events = Vec::new();
    for (i, &action) in actions[..ply].iter().enumerate() {
        events = game.apply(action).map_err(|_| i)?;
    }
    let mut notation = Vec::new();
    let mut error = None;
    for &action in &req.line {
        match advance(&mut game, action) {
            Ok(step) => {
                notation.push(step.notation);
                events = step.events;
            }
            Err(_) => {
                error = Some("illegal_action");
                break;
            }
        }
    }
    let at = notation.len() as u32;

    let point = Point::of(&game);
    let outcome = point.outcome;
    let side = game.side_to_move();
    let mut moves = Vec::new();
    let mut skill_options: Vec<SkillOptions> = Vec::new();
    for action in game.legal_actions() {
        match action {
            Action::Move { from, to, promo } => moves.push(Move { from, to, promo }),
            Action::Skill { skill, target } => push_target(&mut skill_options, skill, target),
        }
    }
    let (eval_cp, best) = match terminal_score(outcome, side) {
        Some(score) => (cap(white_pov(score, side)), None),
        None => {
            let deadline = Instant::now() + budget;
            let found = search_position(&game.pos, depth, &|| Instant::now() >= deadline);
            let eval = cap(white_pov(found.score, side));
            let best = found.best.map(|mv| BestView {
                action: mv.into(),
                notation: simulated_move_notation(&game.pos, mv),
                eval_cp: eval,
            });
            (eval, best)
        }
    };
    Ok(ExploreResponse {
        valid: error.is_none(),
        error,
        at,
        frame: Some(point.frame(ply as u32 + at, events, outcome)),
        moves,
        skill_options,
        eval_cp,
        best,
        notation,
    })
}
