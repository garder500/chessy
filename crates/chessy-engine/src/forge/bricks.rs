//! The bricks a forged skill is assembled from, beyond its [`Effect`].
//!
//! A skill is an **action** (the effect) plus orthogonal bricks that narrow
//! it down: a [`Selector`] says which pieces and squares the action may hit,
//! a [`Condition`] says when the skill can be used at all. Every brick has a
//! neutral value (`Selector::default()`, no condition) that leaves the skill
//! exactly as it was before the brick existed, so definitions stored earlier
//! keep their meaning, their fingerprint and their signature.
//!
//! [`SkillDef::bricks`] lays the whole assembly out as one flat, serializable
//! value for anything that wants to read a skill piece by piece (icons,
//! descriptions, tooling) without matching on [`Effect`] itself.

use serde::{Deserialize, Serialize};

use super::def::{Constraint, DefError, Effect, SkillDef};
use crate::position::Position;
use crate::skills::SkillTarget;
use crate::types::{Color, PieceKind, Square};

/// Where on the board the squares a skill affects must lie. Relative to the
/// caster, so "own half" is the caster's side whatever the colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    #[default]
    Anywhere,
    /// The four ranks nearest the caster.
    OwnHalf,
    /// The four ranks nearest the opponent.
    EnemyHalf,
    /// The central 4x4 (files c to f, ranks 3 to 6).
    Center,
    /// Files a, b, g and h.
    Wings,
    /// The outer ring of the board.
    Rim,
    /// Light squares only.
    Light,
    /// Dark squares only.
    Dark,
}

impl Zone {
    pub const ALL: [Zone; 8] = [
        Zone::Anywhere,
        Zone::OwnHalf,
        Zone::EnemyHalf,
        Zone::Center,
        Zone::Wings,
        Zone::Rim,
        Zone::Light,
        Zone::Dark,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Zone::Anywhere => "anywhere",
            Zone::OwnHalf => "own_half",
            Zone::EnemyHalf => "enemy_half",
            Zone::Center => "center",
            Zone::Wings => "wings",
            Zone::Rim => "rim",
            Zone::Light => "light",
            Zone::Dark => "dark",
        }
    }

    pub fn contains(self, caster: Color, square: Square) -> bool {
        let (file, rank) = (square % 8, square / 8);
        // Rank counted from the caster's own back rank, 0 to 7.
        let depth = match caster {
            Color::White => rank,
            Color::Black => 7 - rank,
        };
        match self {
            Zone::Anywhere => true,
            Zone::OwnHalf => depth < 4,
            Zone::EnemyHalf => depth >= 4,
            Zone::Center => (2..=5).contains(&file) && (2..=5).contains(&rank),
            Zone::Wings => file <= 1 || file >= 6,
            Zone::Rim => file == 0 || file == 7 || rank == 0 || rank == 7,
            // a1 (square 0) is dark.
            Zone::Light => (file + rank) % 2 == 1,
            Zone::Dark => (file + rank) % 2 == 0,
        }
    }
}

/// Which pieces and squares the action may hit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selector {
    /// Only pieces of these kinds (`None`: any the action allows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<PieceKind>>,
    /// Only squares in this zone: the piece hit, or where it lands.
    #[serde(default, skip_serializing_if = "is_anywhere")]
    pub zone: Zone,
}

fn is_anywhere(zone: &Zone) -> bool {
    *zone == Zone::Anywhere
}

impl Selector {
    pub fn is_default(&self) -> bool {
        self.kinds.is_none() && self.zone == Zone::Anywhere
    }

    /// Whether `target` is allowed. For a target that moves a piece, the
    /// zone is where it lands; for a swap, both squares.
    pub(crate) fn allows(&self, pos: &Position, caster: Color, target: SkillTarget) -> bool {
        let kind_ok = |square: Square| match (&self.kinds, pos.board[square as usize]) {
            (Some(kinds), Some(piece)) => kinds.contains(&piece.kind),
            _ => true,
        };
        let zone_ok = |square: Square| self.zone.contains(caster, square);
        match target {
            SkillTarget::Piece { square } => kind_ok(square) && zone_ok(square),
            SkillTarget::PieceTo { from, to } => kind_ok(from) && zone_ok(to),
            SkillTarget::Pair { a, b } => kind_ok(a) && kind_ok(b) && zone_ok(a) && zone_ok(b),
            SkillTarget::Spawn { square, .. } => kind_ok(square) && zone_ok(square),
            SkillTarget::Square { square } => zone_ok(square),
            SkillTarget::None => true,
        }
    }
}

/// When the skill can be used at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// You are behind on material.
    Behind,
    /// You are ahead on material.
    Ahead,
    /// Before the 10th move of each player.
    Early,
    /// From the 20th move of each player.
    Late,
    /// You no longer have a queen on the board.
    NoQueen,
    /// You have lost at least three pieces.
    Wounded,
}

impl Condition {
    pub const ALL: [Condition; 6] = [
        Condition::Behind,
        Condition::Ahead,
        Condition::Early,
        Condition::Late,
        Condition::NoQueen,
        Condition::Wounded,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Condition::Behind => "behind",
            Condition::Ahead => "ahead",
            Condition::Early => "early",
            Condition::Late => "late",
            Condition::NoQueen => "no_queen",
            Condition::Wounded => "wounded",
        }
    }

    pub fn holds(self, pos: &Position, color: Color) -> bool {
        let material = |c: Color| -> u32 {
            pos.pieces(c)
                .map(|(_, p)| match p.kind {
                    PieceKind::Pawn => 1,
                    PieceKind::Knight | PieceKind::Bishop => 3,
                    PieceKind::Rook => 5,
                    PieceKind::Queen => 9,
                    PieceKind::King => 0,
                })
                .sum()
        };
        match self {
            Condition::Behind => material(color) < material(color.opposite()),
            Condition::Ahead => material(color) > material(color.opposite()),
            Condition::Early => pos.ply < 20,
            Condition::Late => pos.ply >= 40,
            Condition::NoQueen => !pos.pieces(color).any(|(_, p)| p.kind == PieceKind::Queen),
            Condition::Wounded => pos.graveyard.iter().filter(|p| p.color == color).count() >= 3,
        }
    }
}

/// What a selector may be asked to do for a given effect: effects that name
/// their own pieces, or that hit no piece, take no kinds; effects with no
/// square take no zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Takes {
    pub kinds: bool,
    pub zone: bool,
}

pub(crate) fn takes(effect: &Effect) -> Takes {
    match effect {
        // Global effects and Mirror hit no square.
        Effect::Truce { .. }
        | Effect::Fog { .. }
        | Effect::Silence { .. }
        | Effect::Ambush { .. }
        | Effect::Mirror => Takes {
            kinds: false,
            zone: false,
        },
        // They choose the kind themselves; the square is still a choice.
        Effect::Remove { .. } | Effect::Spawn { .. } | Effect::Revive { .. } => Takes {
            kinds: false,
            zone: true,
        },
        _ => Takes {
            kinds: true,
            zone: true,
        },
    }
}

pub(crate) fn validate_selector(effect: &Effect, selector: &Selector) -> Result<(), DefError> {
    let takes = takes(effect);
    if let Some(kinds) = &selector.kinds {
        if !takes.kinds {
            return Err(DefError::Invalid(
                "this effect takes no piece selector".to_string(),
            ));
        }
        if kinds.is_empty() || kinds.contains(&PieceKind::King) {
            return Err(DefError::Invalid(
                "a selector needs piece types, and never the king".to_string(),
            ));
        }
    }
    if selector.zone != Zone::Anywhere && !takes.zone {
        return Err(DefError::Invalid("this effect takes no zone".to_string()));
    }
    Ok(())
}

/// A skill laid out brick by brick, for anything that reads skills piece by
/// piece. Built by [`SkillDef::bricks`]; every field is derived from the
/// definition, none is stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bricks {
    /// What it does: the effect's `op` name (`freeze`, `morph`…).
    pub action: &'static str,
    /// The key of the sign that stands for the action (one of `identity::GLYPHS`).
    /// Only the action picks it: no other brick changes the sign.
    pub sign: &'static str,
    /// Whom it touches: `own`, `enemy`, `any` (either side) or `none` (the whole game).
    pub side: &'static str,
    /// Which kinds of piece it is about, when it has a say in it: the
    /// selector's, or the effect's own list.
    pub kinds: Vec<PieceKind>,
    pub zone: Zone,
    /// How long it lasts, in plies; `None` for an instant effect.
    pub plies: Option<u8>,
    /// Whether what it does cannot be taken back.
    pub permanent: bool,
    pub condition: Option<Condition>,
    pub constraints: Vec<Constraint>,
    pub max_uses: u8,
    pub free_action: bool,
}

impl SkillDef {
    /// The key of the sign standing for the action, from the closed list
    /// `identity::GLYPHS`; a new action must be added to that list.
    pub fn sign(&self) -> &'static str {
        super::identity::GLYPHS[super::identity::effect_index(&self.effect)]
    }

    /// Whether a brick beyond the effect narrows down when or where it can be used.
    pub fn is_narrowed(&self) -> bool {
        !self.selector.is_default() || self.condition.is_some()
    }

    /// The skill as one flat list of bricks.
    pub fn bricks(&self) -> Bricks {
        let def = self.clone().canonical();
        let side = match &def.effect {
            Effect::Freeze { .. }
            | Effect::Remove { .. }
            | Effect::Convert
            | Effect::Silence { .. } => "enemy",
            Effect::Shield { .. }
            | Effect::Cloak { .. }
            | Effect::Promote
            | Effect::Teleport
            | Effect::Duplicate
            | Effect::Spawn { .. }
            | Effect::Revive { .. }
            | Effect::Ambush { .. } => "own",
            Effect::Morph { side, .. } => match side {
                super::def::Side::Own => "own",
                super::def::Side::Enemy => "enemy",
            },
            Effect::Swap { scope } => match scope {
                super::def::SwapScope::Own => "own",
                super::def::SwapScope::Any => "any",
            },
            Effect::Truce { .. } | Effect::Fog { .. } | Effect::Mirror => "none",
        };
        let kinds = match (&def.selector.kinds, &def.effect) {
            (Some(kinds), _) => kinds.clone(),
            (
                None,
                Effect::Remove { kinds } | Effect::Spawn { kinds, .. } | Effect::Revive { kinds },
            ) => kinds.clone(),
            (None, Effect::Morph { into, .. }) => vec![*into],
            (None, Effect::Promote) => vec![PieceKind::Queen],
            _ => Vec::new(),
        };
        Bricks {
            action: def.action_name(),
            sign: def.sign(),
            side,
            kinds,
            zone: def.selector.zone,
            plies: def.plies(),
            permanent: def.irreversible(),
            condition: def.condition,
            constraints: def.constraints.clone(),
            max_uses: def.max_uses,
            free_action: def.free_action,
        }
    }
}
