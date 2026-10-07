//! How rare a forged skill is, from numbers.
//!
//! Every forged skill is technically one of a kind (it has its own id), but
//! many combinations amount to the same thing. So rarity has two parts:
//!
//! * a **score**, from what the skill costs on paper ([`SkillDef::cost`]) and
//!   from what it does to measured positions ([`super::measure`]), turned into
//!   a tier by thresholds calibrated on the generator's own output;
//! * **redundancy**: a skill whose [`SkillDef::signature`] is already taken by
//!   another skill in the world is a duplicate in all but name, and falls to
//!   [`Rarity::Common`] whatever its score.
//!
//! Only a skill with a tone atom ([`Effect::is_tone`]) can reach Legendary,
//! and a Legendary is a unique skill: one deck in the world.

use serde::{Deserialize, Serialize};

use super::def::{Constraint, Effect, Side, SkillDef, SwapScope};
use super::measure::Measurement;
use crate::types::PieceKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    /// A unique skill: it exists in a single deck.
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn is_unique(self) -> bool {
        self == Rarity::Legendary
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        }
    }

    pub fn parse(name: &str) -> Option<Rarity> {
        Rarity::ALL.into_iter().find(|r| r.as_str() == name)
    }
}

/// The score a skill needs for each tier.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    pub uncommon: f64,
    pub rare: f64,
    pub epic: f64,
    pub legendary: f64,
}

impl Thresholds {
    pub fn tier(&self, score: f64) -> Rarity {
        if score >= self.legendary {
            Rarity::Legendary
        } else if score >= self.epic {
            Rarity::Epic
        } else if score >= self.rare {
            Rarity::Rare
        } else if score >= self.uncommon {
            Rarity::Uncommon
        } else {
            Rarity::Common
        }
    }
}

/// The verdict on a skill, with the numbers behind it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graded {
    pub rarity: Rarity,
    /// 0 to 100: what the thresholds are compared with.
    pub score: f64,
    /// The paper cost, 0 to 100.
    pub cost: f64,
    /// The measured tone index, 0 to 100.
    pub tone: f64,
    /// Another skill already has this signature.
    pub redundant: bool,
}

/// Costs above this count as 100.
const MAX_COST: f64 = 16.0;

fn value(kind: PieceKind) -> f64 {
    match kind {
        PieceKind::Pawn => 1.0,
        PieceKind::Knight | PieceKind::Bishop => 2.5,
        PieceKind::Rook => 3.5,
        PieceKind::Queen => 5.0,
        PieceKind::King => 0.0,
    }
}

fn best_value(kinds: &[PieceKind]) -> f64 {
    kinds.iter().map(|&k| value(k)).fold(0.0, f64::max)
}

/// `p` pawns, `m` minor pieces, `M` major pieces.
fn class(kind: PieceKind) -> char {
    match kind {
        PieceKind::Pawn => 'p',
        PieceKind::Knight | PieceKind::Bishop => 'm',
        PieceKind::Rook | PieceKind::Queen => 'M',
        PieceKind::King => 'k',
    }
}

fn classes(kinds: &[PieceKind]) -> String {
    let mut c: Vec<char> = kinds.iter().map(|&k| class(k)).collect();
    c.sort_unstable();
    c.dedup();
    c.into_iter().collect()
}

fn bucket(plies: u8) -> char {
    match plies {
        0..=3 => 's',
        4..=5 => 'm',
        _ => 'l',
    }
}

impl SkillDef {
    /// What a skill is on paper, 0 to 100: bigger scope, longer effect, more
    /// uses and free actions cost more; restrictions refund some.
    pub fn cost(&self) -> f64 {
        let plies = |p: u8| f64::from(p);
        let base = match &self.effect {
            Effect::Freeze { plies: p } => 3.0 + 0.5 * plies(*p),
            Effect::Shield { plies: p } => 2.0 + 0.4 * plies(*p),
            Effect::Cloak { plies: p } => 1.0 + 0.3 * plies(*p),
            Effect::Morph {
                side,
                into,
                plies: p,
            } => {
                let side = if *side == Side::Enemy { 3.0 } else { 2.0 };
                side + 0.3 * plies(*p) + value(*into) * 0.4
            }
            Effect::Promote => 5.0,
            Effect::Remove { kinds } => 2.0 + best_value(kinds) + 0.3 * kinds.len() as f64,
            Effect::Convert => 6.0,
            Effect::Teleport => 3.0,
            Effect::Duplicate => 5.0,
            Effect::Swap { scope } => match scope {
                SwapScope::Own => 2.0,
                SwapScope::Any => 4.0,
            },
            Effect::Spawn { kinds, plies: p } => 2.0 + best_value(kinds) + 0.3 * plies(*p),
            Effect::Revive { kinds } => 4.0 + 1.2 * best_value(kinds),
            Effect::Truce { plies: p } => 4.0 + 0.5 * plies(*p),
            Effect::Mirror => 9.0,
            Effect::Fog { plies: p } => 4.0 + 0.5 * plies(*p),
            Effect::Silence { plies: p } => 4.0 + 0.6 * plies(*p),
            Effect::Ambush { plies: p } => 5.0 + 0.5 * plies(*p),
        };
        let mut cost = base * (1.0 + 0.5 * f64::from(self.max_uses.saturating_sub(1)));
        if self.free_action {
            cost += 4.0;
        }
        for c in &self.constraints {
            cost -= match c {
                Constraint::OnlyInCheck => 2.5,
                Constraint::ForbidMate | Constraint::ForbidCheck => 0.5,
            };
        }
        (cost.max(0.5) / MAX_COST * 100.0).min(100.0)
    }

    /// What the skill amounts to, ignoring details that do not change how it
    /// plays: the effect, its duration and the pieces it touches in coarse
    /// buckets, and the rules around it. Two skills with the same signature
    /// are redundant with each other.
    pub fn signature(&self) -> String {
        let (op, params) = match &self.effect {
            Effect::Freeze { plies } => ("freeze", bucket(*plies).to_string()),
            Effect::Shield { plies } => ("shield", bucket(*plies).to_string()),
            Effect::Cloak { plies } => ("cloak", bucket(*plies).to_string()),
            Effect::Morph { side, into, plies } => (
                "morph",
                format!("{:?}{}{}", side, class(*into), bucket(*plies)),
            ),
            Effect::Promote => ("promote", String::new()),
            Effect::Remove { kinds } => ("remove", classes(kinds)),
            Effect::Convert => ("convert", String::new()),
            Effect::Teleport => ("teleport", String::new()),
            Effect::Duplicate => ("duplicate", String::new()),
            Effect::Swap { scope } => ("swap", format!("{scope:?}")),
            Effect::Spawn { kinds, plies } => {
                ("spawn", format!("{}{}", classes(kinds), bucket(*plies)))
            }
            Effect::Revive { kinds } => ("revive", classes(kinds)),
            Effect::Truce { plies } => ("truce", bucket(*plies).to_string()),
            Effect::Mirror => ("mirror", String::new()),
            Effect::Fog { plies } => ("fog", bucket(*plies).to_string()),
            Effect::Silence { plies } => ("silence", bucket(*plies).to_string()),
            Effect::Ambush { plies } => ("ambush", bucket(*plies).to_string()),
        };
        let mut constraints = self.constraints.clone();
        constraints.sort_by_key(|c| *c as u8);
        constraints.dedup();
        format!(
            "{op}:{params}|{constraints:?}|{}|{}",
            self.max_uses, self.free_action
        )
    }
}

/// Grades a skill from its measurement. `redundant` says that another skill
/// in the world already has the same signature.
pub fn grade(
    def: &SkillDef,
    measurement: &Measurement,
    thresholds: &Thresholds,
    redundant: bool,
) -> Graded {
    let cost = def.cost();
    let tone = measurement.tone_index(def.irreversible());
    let score = 0.6 * tone + 0.4 * cost;
    let mut rarity = thresholds.tier(score);
    if rarity == Rarity::Legendary && !def.effect.is_tone() {
        rarity = Rarity::Epic;
    }
    if redundant {
        rarity = Rarity::Common;
    }
    Graded {
        rarity,
        score,
        cost,
        tone,
        redundant,
    }
}
