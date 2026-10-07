use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::skills::{SkillId, SkillTarget};

/// Board square index: `a1 = 0`, `b1 = 1`, ..., `h8 = 63` (`rank * 8 + file`).
pub type Square = u8;
/// Stable identity of a piece for the whole game, so effects can follow it.
pub type PieceId = u16;

pub fn sq(file: u8, rank: u8) -> Square {
    rank * 8 + file
}

pub fn file_of(s: Square) -> u8 {
    s % 8
}

pub fn rank_of(s: Square) -> u8 {
    s / 8
}

pub fn square_name(s: Square) -> String {
    format!("{}{}", (b'a' + file_of(s)) as char, rank_of(s) + 1)
}

pub fn parse_square(name: &str) -> Option<Square> {
    let b = name.as_bytes();
    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return None;
    }
    Some(sq(b[0] - b'a', b[1] - b'1'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const BOTH: [Color; 2] = [Color::White, Color::Black];

    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }

    /// Rank direction pawns of this color advance in.
    pub fn forward(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    pub fn home_rank(self) -> u8 {
        match self {
            Color::White => 0,
            Color::Black => 7,
        }
    }

    pub fn pawn_start_rank(self) -> u8 {
        match self {
            Color::White => 1,
            Color::Black => 6,
        }
    }

    pub fn promotion_rank(self) -> u8 {
        self.opposite().home_rank()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    pub const PROMOTIONS: [PieceKind; 4] = [
        PieceKind::Queen,
        PieceKind::Rook,
        PieceKind::Bishop,
        PieceKind::Knight,
    ];

    pub fn from_fen_char(c: char) -> Option<(Color, PieceKind)> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let kind = match c.to_ascii_lowercase() {
            'p' => PieceKind::Pawn,
            'n' => PieceKind::Knight,
            'b' => PieceKind::Bishop,
            'r' => PieceKind::Rook,
            'q' => PieceKind::Queen,
            'k' => PieceKind::King,
            _ => return None,
        };
        Some((color, kind))
    }

    pub fn fen_char(self, color: Color) -> char {
        let c = match self {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n',
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        };
        match color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub kind: PieceKind,
    pub color: Color,
    /// Square this piece last reached by a real move (used by Rollback).
    #[serde(skip)]
    pub prev: Option<Square>,
    /// The square the piece started on (Celestial Intervention sends it back there).
    #[serde(default)]
    pub home: Square,
    /// Summoned by Mirage: never captures, attacks nothing, vanishes when taken.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirage: bool,
    /// Brought back by Wall: its death is not recorded in the graveyard.
    #[serde(default, skip_serializing_if = "is_false")]
    pub wall: bool,
    /// A temporary piece (Terminator copy, God Help): it has no graveyard entry.
    #[serde(default, skip_serializing_if = "is_false")]
    pub temp: bool,
}

impl Piece {
    pub fn new(id: PieceId, kind: PieceKind, color: Color, home: Square) -> Self {
        Piece {
            id,
            kind,
            color,
            prev: None,
            home,
            mirage: false,
            wall: false,
            temp: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promo: Option<PieceKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    /// The piece cannot be captured.
    Immune,
    /// The piece cannot move (and therefore gives no check).
    Frozen,
    /// The opponent cannot see the piece (the server hides it).
    Invisible,
    /// When the piece is captured, the capturer is pushed back.
    Forcefield,
    /// When the piece is captured, it goes back to its starting square instead (once).
    Celestial,
    /// Wall pawns cannot move for a turn.
    Locked,
    /// The piece temporarily has another type (`orig_kind`).
    Morphed,
    /// The piece temporarily fights for the other side (`orig_color`).
    ColorLoan,
    /// The piece disappears when the effect expires.
    Vanish,
    /// Geomancy terrain: not tied to a piece; see `square` and `owner`.
    Terrain,
    /// Armistice (forged): nothing attacks anything, so there are no captures
    /// and no check. Global: not tied to a piece.
    Truce,
    /// Fog (forged): each player only sees the enemy pieces within two squares
    /// of one of their own. Global; the server does the hiding.
    Fog,
    /// Silence (forged): the player in `owner` cannot use skills. Global.
    Silenced,
    /// Domain (forged): an ambush armed for `owner`. The next time the other
    /// side puts the owner in check, bishops strike the checking pieces.
    /// Global; the board "expanding" is only a show for the clients.
    Domain,
}

impl EffectKind {
    /// Effects that belong to the whole game, not to a piece (their
    /// `ActiveEffect::piece` is [`NO_PIECE`]).
    pub fn is_global(self) -> bool {
        matches!(
            self,
            EffectKind::Terrain
                | EffectKind::Truce
                | EffectKind::Fog
                | EffectKind::Silenced
                | EffectKind::Domain
        )
    }
}

/// `ActiveEffect::piece` for effects that are not about a piece (terrain).
pub const NO_PIECE: PieceId = PieceId::MAX;
/// `expires_at` of effects that last until something happens to the piece.
pub const NEVER: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub kind: EffectKind,
    pub piece: PieceId,
    /// The effect is active while `Position::ply < expires_at`.
    pub expires_at: u32,
    /// Terrain: the covered square.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub square: Option<Square>,
    /// Terrain: the player who created it (their pieces are not hindered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<Color>,
    /// Morph: the type to go back to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig_kind: Option<PieceKind>,
    /// Color loan: the side the piece goes back to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig_color: Option<Color>,
}

impl ActiveEffect {
    pub fn new(kind: EffectKind, piece: PieceId, expires_at: u32) -> Self {
        ActiveEffect {
            kind,
            piece,
            expires_at,
            square: None,
            owner: None,
            orig_kind: None,
            orig_color: None,
        }
    }
}

/// A Trap Card laid by `owner`: it catches the first enemy piece to cross `square`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trap {
    pub square: Square,
    pub owner: Color,
}

/// A piece set aside by The Bench; it returns at `back_at` (a `Position::ply`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchedPiece {
    pub piece: Piece,
    /// Where it stood when benched.
    pub square: Square,
    pub back_at: u32,
}

/// One piece going from `from` to `to` (Tornado).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shift {
    pub from: Square,
    pub to: Square,
}

/// Something a player can do with their turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Move {
        from: Square,
        to: Square,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        promo: Option<PieceKind>,
    },
    Skill {
        skill: SkillId,
        target: SkillTarget,
    },
}

impl From<Move> for Action {
    fn from(m: Move) -> Self {
        Action::Move {
            from: m.from,
            to: m.to,
            promo: m.promo,
        }
    }
}

/// Facts about what happened, for clients to animate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Moved {
        from: Square,
        to: Square,
        piece: PieceId,
    },
    Captured {
        square: Square,
        piece: Piece,
    },
    Promoted {
        square: Square,
        to: PieceKind,
    },
    Castled {
        rook_from: Square,
        rook_to: Square,
    },
    SkillUsed {
        color: Color,
        skill: SkillId,
        target: SkillTarget,
    },
    Teleported {
        from: Square,
        to: Square,
    },
    Cloned {
        from: Square,
        to: Square,
        piece: Piece,
    },
    Swapped {
        a: Square,
        b: Square,
    },
    Removed {
        square: Square,
        piece: Piece,
    },
    RolledBack {
        from: Square,
        to: Square,
    },
    EffectAdded {
        piece: PieceId,
        effect: EffectKind,
        expires_at: u32,
    },
    /// A piece appeared (Wall, Mirage, Terminator, God Help).
    Spawned {
        square: Square,
        piece: Piece,
    },
    /// The piece on `square` changed type (Evolve, Morph, or a Morph wearing off).
    Transformed {
        square: Square,
        kind: PieceKind,
    },
    /// The piece on `square` changed sides for good (Switch Sides).
    Switched {
        square: Square,
        piece: Piece,
    },
    Rotated {
        moves: Vec<Shift>,
    },
    TrapSet {
        square: Square,
    },
    TrapSprung {
        square: Square,
        piece: PieceId,
    },
    Benched {
        square: Square,
        piece: Piece,
    },
    Unbenched {
        square: Square,
        piece: Piece,
    },
    /// A Force Field pushed the capturer back.
    Pushed {
        piece: PieceId,
        from: Square,
        to: Square,
    },
    /// Celestial Intervention saved a captured piece and sent it home.
    Saved {
        piece: PieceId,
        from: Square,
        to: Square,
    },
    BestMove {
        from: Square,
        to: Square,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        promo: Option<PieceKind>,
    },
    /// The last skill of the opponent was undone.
    Cancelled {
        skill: SkillId,
    },
    Terrain {
        squares: Vec<Square>,
    },
    /// A temporary piece disappeared.
    Vanished {
        square: Square,
        piece: Piece,
    },
    /// A game-wide effect began (Armistice, Fog, Silence). `owner` is the
    /// player it is aimed at, when it is aimed at one.
    GlobalEffect {
        effect: EffectKind,
        expires_at: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<Color>,
    },
    /// A Domain ambush: bishops from beyond the board strike the `piece` on
    /// `square` (a [`Event::Captured`] follows) and the domain collapses.
    Ambushed {
        square: Square,
        piece: Piece,
    },
    /// A piece lent by Mind Control went back to its side.
    LoanEnded {
        square: Square,
        piece: Piece,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    Ongoing,
    Checkmate {
        winner: Color,
    },
    Resignation {
        winner: Color,
    },
    Stalemate,
    FiftyMoves,
    Repetition,
    InsufficientMaterial,
    /// `winner` still had time on the clock when the other side ran out.
    Timeout {
        winner: Color,
    },
    /// Both players accepted a draw offer.
    DrawAgreed,
}

impl Outcome {
    pub fn is_over(&self) -> bool {
        !matches!(self, Outcome::Ongoing)
    }

    pub fn winner(&self) -> Option<Color> {
        match self {
            Outcome::Checkmate { winner }
            | Outcome::Resignation { winner }
            | Outcome::Timeout { winner } => Some(*winner),
            _ => None,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("the game is over")]
    GameOver,
    #[error("illegal action")]
    IllegalAction,
    #[error("invalid FEN: {0}")]
    InvalidFen(String),
}
