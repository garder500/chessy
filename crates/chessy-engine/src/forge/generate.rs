//! Making skills up: random definitions, graded and kept when they land on
//! the rarity that was asked for.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::bricks::{self, Condition, Zone};
use super::def::{Constraint, Effect, Side, SkillDef, SwapScope};
use super::measure;
use super::rarity::{grade, Graded, Rarity, Thresholds};
use crate::ai::Rng;
use crate::types::PieceKind;

/// The thresholds the forge grades with, calibrated on the generator's own
/// output by `cargo run -p chessy-forge -- calibrate`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub version: u8,
    /// Skills the thresholds were computed from.
    pub samples: u32,
    /// Positions each of them was measured on.
    pub positions: u32,
    pub thresholds: Thresholds,
}

pub const CALIBRATION_JSON: &str = include_str!("calibration.json");

pub fn calibration() -> &'static Calibration {
    static CALIBRATION: OnceLock<Calibration> = OnceLock::new();
    CALIBRATION
        .get_or_init(|| serde_json::from_str(CALIBRATION_JSON).expect("calibration.json is valid"))
}

/// Share of the forges aimed at each tier, in percent, Common first.
pub const DROP_WEIGHTS: [u64; 5] = [55, 25, 13, 6, 1];

pub fn roll_rarity(rng: &mut Rng) -> Rarity {
    roll_rarity_in(rng, Rarity::Common, Rarity::Legendary)
}

/// Like [`roll_rarity`], but only among the tiers `min..=max`, the drop weights
/// renormalised over that range.
pub fn roll_rarity_in(rng: &mut Rng, min: Rarity, max: Rarity) -> Rarity {
    let (min, max) = (min.min(max), min.max(max));
    let tiers = || {
        Rarity::ALL
            .iter()
            .zip(&DROP_WEIGHTS)
            .filter(|(tier, _)| (min..=max).contains(tier))
    };
    let mut n = rng.below(tiers().map(|(_, w)| w).sum());
    for (tier, &w) in tiers() {
        if n < w {
            return *tier;
        }
        n -= w;
    }
    min
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ranged_roll_stays_in_range() {
        let mut rng = Rng::new(7);
        for (min, max) in [
            (Rarity::Uncommon, Rarity::Epic),
            (Rarity::Rare, Rarity::Legendary),
            (Rarity::Epic, Rarity::Legendary),
            (Rarity::Rare, Rarity::Rare),
            (Rarity::Epic, Rarity::Uncommon),
        ] {
            let (min, max) = (min.min(max), min.max(max));
            for _ in 0..500 {
                assert!((min..=max).contains(&roll_rarity_in(&mut rng, min, max)));
            }
        }
    }
}

fn pick<T: Copy>(rng: &mut Rng, items: &[T]) -> T {
    items[rng.below(items.len() as u64) as usize]
}

fn some_kinds(rng: &mut Rng, pool: &[PieceKind]) -> Vec<PieceKind> {
    let mut out: Vec<PieceKind> = pool.iter().copied().filter(|_| rng.chance(500)).collect();
    if out.is_empty() {
        out.push(pick(rng, pool));
    }
    out
}

const PIECES: [PieceKind; 5] = [
    PieceKind::Pawn,
    PieceKind::Knight,
    PieceKind::Bishop,
    PieceKind::Rook,
    PieceKind::Queen,
];
const SPAWNABLE: [PieceKind; 4] = [
    PieceKind::Knight,
    PieceKind::Bishop,
    PieceKind::Rook,
    PieceKind::Queen,
];

/// A random, valid, canonical definition. Every effect has a weight: the
/// rewriting ones are rarer to draw than the adjusting ones.
pub fn random_def(rng: &mut Rng) -> SkillDef {
    const WEIGHTS: [u64; 17] = [8, 8, 5, 8, 5, 6, 4, 6, 4, 6, 7, 5, 3, 2, 3, 3, 2];
    let mut n = rng.below(WEIGHTS.iter().sum());
    let mut which = 0;
    for (i, &w) in WEIGHTS.iter().enumerate() {
        if n < w {
            which = i;
            break;
        }
        n -= w;
    }
    let plies = 2 + rng.below(7) as u8;
    let effect = match which {
        0 => Effect::Freeze { plies },
        1 => Effect::Shield { plies },
        2 => Effect::Cloak { plies },
        3 => Effect::Morph {
            side: pick(rng, &[Side::Own, Side::Enemy]),
            into: pick(rng, &PieceKind::PROMOTIONS),
            plies,
        },
        4 => Effect::Promote,
        5 => Effect::Remove {
            kinds: some_kinds(rng, &PIECES),
        },
        6 => Effect::Convert,
        7 => Effect::Teleport,
        8 => Effect::Duplicate,
        9 => Effect::Swap {
            scope: pick(rng, &[SwapScope::Own, SwapScope::Any]),
        },
        10 => Effect::Spawn {
            kinds: some_kinds(rng, &SPAWNABLE),
            plies,
        },
        11 => Effect::Revive {
            kinds: some_kinds(rng, &PIECES),
        },
        12 => Effect::Truce { plies },
        13 => Effect::Mirror,
        14 => Effect::Fog { plies },
        15 => Effect::Silence { plies },
        _ => Effect::Ambush { plies },
    };
    let mut def = SkillDef::new(effect);
    // Removing or converting a piece must not be a way to mate.
    let strong = matches!(def.effect, Effect::Remove { .. } | Effect::Convert);
    if strong || rng.chance(150) {
        def.constraints.push(Constraint::ForbidMate);
    }
    if rng.chance(100) {
        def.constraints.push(Constraint::ForbidCheck);
    }
    if rng.chance(100) {
        def.constraints.push(Constraint::OnlyInCheck);
    }
    if rng.chance(150) {
        def.max_uses = 2 + rng.below(2) as u8;
    }
    if rng.chance(80) {
        def.free_action = true;
    }
    // The newer bricks are drawn last, so the draws above are unchanged.
    let takes = bricks::takes(&def.effect);
    if takes.kinds && rng.chance(150) {
        def.selector.kinds = Some(some_kinds(rng, &PIECES));
    }
    if takes.zone && rng.chance(200) {
        def.selector.zone = pick(rng, &Zone::ALL[1..]);
    }
    if rng.chance(120) {
        def.condition = Some(pick(rng, &Condition::ALL));
    }
    def.canonical()
}

/// How hard to look for a skill of the rarity asked for.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Candidates tried before settling for the closest one.
    pub attempts: u32,
    /// Positions each candidate is measured on.
    pub positions: usize,
}

impl Budget {
    /// What the server spends on a reward: measured on as many positions as
    /// the calibration was, or the thresholds would not fit.
    pub fn live() -> Budget {
        Budget {
            attempts: 24,
            positions: calibration().positions as usize,
        }
    }

    /// What a campaign boss forge spends: more candidates, since the floor
    /// and the family reject many of them.
    pub fn campaign() -> Budget {
        Budget {
            attempts: 96,
            ..Budget::live()
        }
    }
}

/// A skill with a narrowing brick must be usable on at least this share of
/// the measured positions to be forged.
pub const MIN_AVAILABILITY: f64 = 0.15;

#[derive(Clone, Debug, PartialEq)]
pub struct Forged {
    /// The definition, with `unique` set for a Legendary.
    pub def: SkillDef,
    pub graded: Graded,
}

/// Grades `def` (measuring it) against the signatures already in the world.
pub fn grade_def(
    def: &SkillDef,
    known: &HashSet<String>,
    thresholds: &Thresholds,
    positions: usize,
) -> Graded {
    let measurement = measure::measure(def, positions);
    grade(
        def,
        &measurement,
        thresholds,
        known.contains(&def.signature()),
    )
}

fn distance(a: Rarity, b: Rarity) -> usize {
    a.index().abs_diff(b.index())
}

/// Draws definitions until one grades as `target`; after `budget.attempts`
/// returns the one that came closest (a fresh skill is preferred to a
/// redundant one at equal distance). Deterministic for a given `rng`.
pub fn forge(rng: &mut Rng, target: Rarity, known: &HashSet<String>, budget: Budget) -> Forged {
    let thresholds = &calibration().thresholds;
    let mut best: Option<(Forged, (usize, bool))> = None;
    for _ in 0..budget.attempts.max(1) {
        let mut def = random_def(rng);
        let measurement = measure::measure(&def, budget.positions);
        let graded = grade(
            &def,
            &measurement,
            thresholds,
            known.contains(&def.signature()),
        );
        def.unique = graded.rarity.is_unique();
        // Bricks can combine into a skill that is almost never playable
        // ("only on light squares, when you have no queen, a rook...").
        // Those are kept only as a last resort.
        let dead = def.is_narrowed() && measurement.availability < MIN_AVAILABILITY;
        let candidate = Forged { def, graded };
        if graded.rarity == target && !dead {
            return candidate;
        }
        let key = (
            distance(graded.rarity, target) + if dead { Rarity::ALL.len() } else { 0 },
            graded.redundant,
        );
        if best.as_ref().is_none_or(|(_, k)| key < *k) {
            best = Some((candidate, key));
        }
    }
    best.expect("at least one attempt").0
}

/// Recomputes the thresholds: the score of `samples` random skills, cut at
/// the quantiles that make Uncommon the top half, Rare the top quarter, Epic
/// the top tenth and Legendary the top few percent.
pub fn calibrate(samples: u32, seed: u64, positions: usize) -> Calibration {
    let mut rng = Rng::new(seed);
    let none = HashSet::new();
    let neutral = Thresholds {
        uncommon: 0.0,
        rare: 0.0,
        epic: 0.0,
        legendary: f64::MAX,
    };
    let mut scores: Vec<f64> = (0..samples)
        .map(|_| grade_def(&random_def(&mut rng), &none, &neutral, positions).score)
        .collect();
    scores.sort_by(f64::total_cmp);
    let at = |q: f64| scores[((scores.len() - 1) as f64 * q).round() as usize];
    Calibration {
        version: 1,
        samples,
        positions: positions as u32,
        thresholds: Thresholds {
            uncommon: round1(at(0.50)),
            rare: round1(at(0.75)),
            epic: round1(at(0.90)),
            legendary: round1(at(0.97)),
        },
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
