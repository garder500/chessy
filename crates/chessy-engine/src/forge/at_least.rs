//! A forge that keeps its promise: a rarity floor, an optional family, never
//! a skill the world already has.

use std::collections::HashSet;

use super::generate::{calibration, grade_def, random_def, Budget, Forged};
use super::identity::{family, Family};
use super::rarity::Rarity;
use crate::ai::Rng;

/// Only the measured candidates spend the budget; cheap rejections (known
/// signature, wrong family) are free. This caps the raw draws per expected
/// measured candidate so a saturated pool cannot loop forever.
const MAX_DRAWS_PER_MEASURED: u32 = 50;

#[derive(Clone, Debug, PartialEq)]
pub struct ForgeOutcome {
    /// `None` when no candidate reached the floor within the budget.
    pub forged: Option<Forged>,
    /// The target was Legendary and every tone-effect candidate drawn was
    /// already known.
    pub legendary_exhausted: bool,
}

/// Draws definitions until one measures exactly `target`; when the budget runs
/// out, keeps the closest candidate at or above `floor`, else nothing.
/// Deterministic for a given `rng`.
pub fn forge_at_least(
    rng: &mut Rng,
    target: Rarity,
    floor: Rarity,
    family_wanted: Option<Family>,
    known: &HashSet<String>,
    budget: Budget,
) -> ForgeOutcome {
    search(rng, target, floor, family_wanted, known, budget).0
}

/// The outcome plus the number of candidates actually measured.
fn search(
    rng: &mut Rng,
    target: Rarity,
    floor: Rarity,
    family_wanted: Option<Family>,
    known: &HashSet<String>,
    budget: Budget,
) -> (ForgeOutcome, u32) {
    let thresholds = &calibration().thresholds;
    let mut best: Option<(Forged, usize)> = None;
    let (mut tone_known, mut tone_fresh) = (0u32, 0u32);
    let (mut measured, mut draws) = (0, 0);
    let attempts = budget.attempts.max(1);
    while measured < attempts && draws < attempts * MAX_DRAWS_PER_MEASURED {
        draws += 1;
        let mut def = random_def(rng);
        if family_wanted.is_some_and(|f| family(&def.effect) != f) {
            continue;
        }
        let is_known = known.contains(&def.signature());
        if def.effect.is_tone() {
            if is_known {
                tone_known += 1;
            } else {
                tone_fresh += 1;
            }
        }
        if is_known {
            continue;
        }
        measured += 1;
        let graded = grade_def(&def, known, thresholds, budget.positions);
        if graded.redundant || graded.rarity < floor {
            continue;
        }
        def.unique = graded.rarity.is_unique();
        let candidate = Forged { def, graded };
        if graded.rarity == target {
            let outcome = ForgeOutcome {
                forged: Some(candidate),
                legendary_exhausted: false,
            };
            return (outcome, measured);
        }
        let distance = graded.rarity.index().abs_diff(target.index());
        if best.as_ref().is_none_or(|(_, d)| distance < *d) {
            best = Some((candidate, distance));
        }
    }
    let outcome = ForgeOutcome {
        forged: best.map(|(forged, _)| forged),
        legendary_exhausted: target == Rarity::Legendary && tone_known > 0 && tone_fresh == 0,
    };
    (outcome, measured)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: Budget = Budget {
        attempts: 12,
        positions: 4,
    };

    #[test]
    fn never_below_the_floor() {
        let mut rng = Rng::new(3);
        for floor in [Rarity::Uncommon, Rarity::Rare, Rarity::Epic] {
            let outcome = forge_at_least(&mut rng, floor, floor, None, &HashSet::new(), BUDGET);
            if let Some(forged) = outcome.forged {
                assert!(forged.graded.rarity >= floor);
                assert!(!forged.graded.redundant);
            }
        }
    }

    #[test]
    fn stays_in_the_family() {
        let mut rng = Rng::new(5);
        for wanted in [Family::Attack, Family::Defense, Family::Control] {
            let outcome = forge_at_least(
                &mut rng,
                Rarity::Common,
                Rarity::Common,
                Some(wanted),
                &HashSet::new(),
                BUDGET,
            );
            let forged = outcome.forged.expect("a common skill is easy to find");
            assert_eq!(family(&forged.def.effect), wanted);
        }
    }

    #[test]
    fn measures_the_whole_budget_when_the_floor_is_unreachable() {
        let (outcome, measured) = search(
            &mut Rng::new(7),
            Rarity::Legendary,
            Rarity::Legendary,
            Some(Family::Attack),
            &HashSet::new(),
            BUDGET,
        );
        assert!(outcome.forged.is_none());
        assert_eq!(measured, BUDGET.attempts);
    }

    #[test]
    fn nothing_when_everything_is_known() {
        let draws = BUDGET.attempts * MAX_DRAWS_PER_MEASURED;
        let mut replay = Rng::new(9);
        let known: HashSet<String> = (0..draws)
            .map(|_| random_def(&mut replay).signature())
            .collect();
        let outcome = forge_at_least(
            &mut Rng::new(9),
            Rarity::Common,
            Rarity::Common,
            None,
            &known,
            BUDGET,
        );
        assert!(outcome.forged.is_none());
    }

    #[test]
    fn legendary_runs_out_when_every_tone_candidate_is_known() {
        let draws = BUDGET.attempts * MAX_DRAWS_PER_MEASURED;
        let mut replay = Rng::new(11);
        let known: HashSet<String> = (0..draws)
            .map(|_| random_def(&mut replay).signature())
            .collect();
        let outcome = forge_at_least(
            &mut Rng::new(11),
            Rarity::Legendary,
            Rarity::Epic,
            Some(Family::Control),
            &known,
            BUDGET,
        );
        assert!(outcome.forged.is_none());
        assert!(outcome.legendary_exhausted);
    }
}
