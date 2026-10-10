use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::bricks::{validate_selector, Condition, Selector};
use crate::types::PieceKind;

/// The only definition format understood so far.
pub const VERSION: u8 = 1;
/// Shortest and longest effect, in plies (two plies = one turn of each player).
pub const MIN_PLIES: u8 = 2;
pub const MAX_PLIES: u8 = 8;
pub const MAX_USES: u8 = 3;
const MAX_CONSTRAINTS: usize = 3;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DefError {
    #[error("invalid skill definition: {0}")]
    Invalid(String),
    #[error("forged skill {0} is already registered with another definition")]
    Conflict(u32),
}

fn invalid<T>(why: &str) -> Result<T, DefError> {
    Err(DefError::Invalid(why.to_string()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Own,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapScope {
    /// Two of the caster's own pieces.
    Own,
    /// Any two pieces (kings excepted), whoever they belong to.
    Any,
}

/// What a forged skill does. Every variant fixes the shape of its target, so a
/// definition can never ask for an impossible combination.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Effect {
    /// An enemy piece (not the king) cannot move for `plies`.
    Freeze {
        plies: u8,
    },
    /// One of your pieces (not the king) cannot be captured for `plies`.
    Shield {
        plies: u8,
    },
    /// One of your pieces (not the king) is hidden from the opponent for `plies`.
    Cloak {
        plies: u8,
    },
    /// A piece (not the king) of `side` becomes `into` for `plies`.
    Morph {
        side: Side,
        into: PieceKind,
        plies: u8,
    },
    /// One of your pieces (not the king or a queen) becomes a queen for good.
    Promote,
    /// An enemy piece of one of `kinds` is removed from the board.
    Remove {
        kinds: Vec<PieceKind>,
    },
    /// An enemy piece (not the king) that attacks none of yours changes sides for good.
    Convert,
    /// One of your pieces (not the king) goes to any empty square.
    Teleport,
    /// One of your pieces (not the king) is copied onto an adjacent empty square.
    Duplicate,
    Swap {
        scope: SwapScope,
    },
    /// A temporary piece of one of `kinds` appears on ranks 3 to 6 for `plies`.
    Spawn {
        kinds: Vec<PieceKind>,
        plies: u8,
    },
    /// One of your captured pieces, of one of `kinds`, comes back to the board.
    Revive {
        kinds: Vec<PieceKind>,
    },
    /// Armistice: for `plies` nothing attacks anything. No captures, no check.
    Truce {
        plies: u8,
    },
    /// Miroir: each player inherits the other's army and formation.
    Mirror,
    /// Brouillard: for `plies` each player only sees the enemy pieces within
    /// two squares of their own.
    Fog {
        plies: u8,
    },
    /// Silence: the opponent cannot use skills for `plies`.
    Silence {
        plies: u8,
    },
    /// Domain Expansion: for `plies` the caster is under an ambush. The next
    /// time the opponent puts their king in check, bishops strike the checking
    /// pieces down. (The board swelling around the fight is a show the
    /// clients put on.)
    Ambush {
        plies: u8,
    },
}

impl Effect {
    /// The effects that rewrite the nature of a game rather than adjust it:
    /// only a skill carrying one of them can be Legendary.
    pub fn is_tone(&self) -> bool {
        matches!(
            self,
            Effect::Revive { .. }
                | Effect::Truce { .. }
                | Effect::Mirror
                | Effect::Fog { .. }
                | Effect::Silence { .. }
                | Effect::Ambush { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Constraint {
    /// Only usable while your king is in check.
    OnlyInCheck,
    /// Refused when it would checkmate the opponent.
    ForbidMate,
    /// Refused when it would leave either king in check.
    ForbidCheck,
}

fn default_uses() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillDef {
    pub version: u8,
    pub effect: Effect,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// How many times a game it can be used.
    #[serde(default = "default_uses")]
    pub max_uses: u8,
    /// Using it does not pass the turn.
    #[serde(default)]
    pub free_action: bool,
    /// Legendary: exists in one deck only, and does not count toward the three picked.
    #[serde(default)]
    pub unique: bool,
    /// Which pieces and squares the effect may hit. The default hits whatever
    /// the effect allows, so it is left out of the JSON.
    #[serde(default, skip_serializing_if = "Selector::is_default")]
    pub selector: Selector,
    /// When it can be used at all (`None`: any time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
}

fn check_plies(plies: u8) -> Result<(), DefError> {
    if (MIN_PLIES..=MAX_PLIES).contains(&plies) {
        Ok(())
    } else {
        invalid("duration out of range")
    }
}

fn check_kinds(kinds: &[PieceKind]) -> Result<(), DefError> {
    if kinds.is_empty() {
        return invalid("empty list of piece types");
    }
    if kinds.contains(&PieceKind::King) {
        return invalid("a king cannot be listed");
    }
    Ok(())
}

impl SkillDef {
    pub fn new(effect: Effect) -> Self {
        SkillDef {
            version: VERSION,
            effect,
            constraints: Vec::new(),
            max_uses: 1,
            free_action: false,
            unique: false,
            selector: Selector::default(),
            condition: None,
        }
    }

    /// The effect's `op` name, as it appears in the JSON.
    pub fn action_name(&self) -> &'static str {
        match self.effect {
            Effect::Freeze { .. } => "freeze",
            Effect::Shield { .. } => "shield",
            Effect::Cloak { .. } => "cloak",
            Effect::Morph { .. } => "morph",
            Effect::Promote => "promote",
            Effect::Remove { .. } => "remove",
            Effect::Convert => "convert",
            Effect::Teleport => "teleport",
            Effect::Duplicate => "duplicate",
            Effect::Swap { .. } => "swap",
            Effect::Spawn { .. } => "spawn",
            Effect::Revive { .. } => "revive",
            Effect::Truce { .. } => "truce",
            Effect::Mirror => "mirror",
            Effect::Fog { .. } => "fog",
            Effect::Silence { .. } => "silence",
            Effect::Ambush { .. } => "ambush",
        }
    }

    /// How long the effect lasts, in plies, if it has a duration.
    pub fn plies(&self) -> Option<u8> {
        match self.effect {
            Effect::Freeze { plies }
            | Effect::Shield { plies }
            | Effect::Cloak { plies }
            | Effect::Morph { plies, .. }
            | Effect::Spawn { plies, .. }
            | Effect::Truce { plies }
            | Effect::Fog { plies }
            | Effect::Silence { plies }
            | Effect::Ambush { plies } => Some(plies),
            _ => None,
        }
    }

    /// Rejects anything the interpreter does not promise to handle. Definitions
    /// come from a database, so they are checked again on every load.
    pub fn validate(&self) -> Result<(), DefError> {
        if self.version != VERSION {
            return invalid("unsupported version");
        }
        if !(1..=MAX_USES).contains(&self.max_uses) {
            return invalid("uses out of range");
        }
        if self.constraints.len() > MAX_CONSTRAINTS {
            return invalid("too many constraints");
        }
        validate_selector(&self.effect, &self.selector)?;
        match &self.effect {
            Effect::Freeze { plies }
            | Effect::Shield { plies }
            | Effect::Cloak { plies }
            | Effect::Truce { plies }
            | Effect::Fog { plies }
            | Effect::Silence { plies }
            | Effect::Ambush { plies } => check_plies(*plies),
            Effect::Morph { into, plies, .. } => {
                if *into == PieceKind::King {
                    return invalid("cannot morph into a king");
                }
                check_plies(*plies)
            }
            Effect::Spawn { kinds, plies } => {
                check_kinds(kinds)?;
                check_plies(*plies)
            }
            Effect::Remove { kinds } | Effect::Revive { kinds } => check_kinds(kinds),
            Effect::Promote
            | Effect::Convert
            | Effect::Teleport
            | Effect::Duplicate
            | Effect::Mirror
            | Effect::Swap { .. } => Ok(()),
        }
    }

    /// The same definition with lists sorted and deduplicated, so two
    /// definitions that mean the same thing are equal.
    pub fn canonical(mut self) -> SkillDef {
        let kinds = match &mut self.effect {
            Effect::Remove { kinds } | Effect::Revive { kinds } | Effect::Spawn { kinds, .. } => {
                Some(kinds)
            }
            _ => None,
        };
        if let Some(kinds) = kinds {
            kinds.sort_by_key(|k| *k as u8);
            kinds.dedup();
        }
        if let Some(kinds) = &mut self.selector.kinds {
            kinds.sort_by_key(|k| *k as u8);
            kinds.dedup();
        }
        self.constraints.sort_by_key(|c| *c as u8);
        self.constraints.dedup();
        self
    }

    /// A stable 64-bit fingerprint of the canonical definition (FNV-1a over its
    /// JSON), used to avoid storing the same skill twice. Whether it is
    /// legendary is a verdict on the skill, not part of what it does, so it is
    /// left out.
    pub fn fingerprint(&self) -> u64 {
        let mut plain = self.clone().canonical();
        plain.unique = false;
        let json = serde_json::to_string(&plain).expect("a skill definition always serializes");
        json.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
}
