//! Elo ratings: pure arithmetic, no I/O.

pub const START_ELO: i32 = 1200;
pub const ELO_FLOOR: i32 = 100;
/// Rated games during which a player is "provisional" and moves faster.
pub const PROVISIONAL_GAMES: u32 = 30;

/// K is 40 for a player's first 30 rated games, then 20.
/// `games_before` counts the rated games played before this one.
pub fn k_factor(games_before: u32) -> f64 {
    if games_before < PROVISIONAL_GAMES {
        40.0
    } else {
        20.0
    }
}

/// Expected score of a player rated `ra` against one rated `rb`.
pub fn expected_score(ra: i32, rb: i32) -> f64 {
    1.0 / (1.0 + 10f64.powf(f64::from(rb - ra) / 400.0))
}

/// New rating after one game; `score` is 1.0 (win), 0.5 (draw) or 0.0 (loss).
pub fn new_rating(ra: i32, rb: i32, score: f64, games_before: u32) -> i32 {
    let delta = k_factor(games_before) * (score - expected_score(ra, rb));
    ((f64::from(ra) + delta).round() as i32).max(ELO_FLOOR)
}

/// Levels of the bots met in the placement games (docs/spec-v5.md), in Elo.
pub const PLACEMENT_LEVELS: [i32; 5] = [400, 800, 1200, 1600, 2000];
/// Bounds of an estimate. Five games cannot tell a strong player from a very
/// strong one, so the starting rating stops at the default one: whoever
/// dominates climbs from there with the high K of the first rated games.
pub const PLACEMENT_MIN: i32 = 200;
pub const PLACEMENT_MAX: i32 = START_ELO;

/// The rating whose expected score against `results`' opponents equals the
/// score actually made (a performance rating, found by bisection since the
/// expected total grows with the rating). `results` is `(opponent Elo, score)`
/// with a score of 1.0, 0.5 or 0.0.
pub fn placement_estimate(results: &[(i32, f64)]) -> i32 {
    let total: f64 = results.iter().map(|&(_, s)| s).sum();
    let expected = |r: f64| -> f64 {
        results
            .iter()
            .map(|&(opp, _)| 1.0 / (1.0 + 10f64.powf((f64::from(opp) - r) / 400.0)))
            .sum()
    };
    let (mut lo, mut hi) = (f64::from(PLACEMENT_MIN), f64::from(PLACEMENT_MAX));
    if expected(lo) >= total {
        return PLACEMENT_MIN;
    }
    if expected(hi) <= total {
        return PLACEMENT_MAX;
    }
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if expected(mid) < total {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (((lo + hi) / 2.0).round() as i32).clamp(PLACEMENT_MIN, PLACEMENT_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_split_k_evenly() {
        assert_eq!(new_rating(1200, 1200, 1.0, 0), 1220);
        assert_eq!(new_rating(1200, 1200, 0.0, 0), 1180);
        assert_eq!(new_rating(1200, 1200, 0.5, 0), 1200);
        assert_eq!(new_rating(1200, 1200, 1.0, 30), 1210);
    }

    #[test]
    fn k_drops_after_thirty_games() {
        assert_eq!(k_factor(0), 40.0);
        assert_eq!(k_factor(29), 40.0);
        assert_eq!(k_factor(30), 20.0);
    }

    #[test]
    fn upsets_and_favourites() {
        // E = 0.7597: the favourite gains 40 * 0.2403 = 9.6 -> 10; the loser of 1200 v 1400 drops 40 * 0.2403 -> 10.
        assert_eq!(new_rating(1400, 1200, 1.0, 0), 1410);
        assert_eq!(new_rating(1200, 1400, 0.0, 0), 1200 - 10);
        // The underdog's win is worth more.
        assert_eq!(new_rating(1200, 1400, 1.0, 0), 1200 + 30);
    }

    #[test]
    fn floor_holds() {
        assert_eq!(new_rating(100, 1200, 0.0, 0), 100);
        assert_eq!(new_rating(105, 105, 0.0, 0), 100);
    }

    fn levels(scores: [f64; 5]) -> Vec<(i32, f64)> {
        PLACEMENT_LEVELS.into_iter().zip(scores).collect()
    }

    #[test]
    fn placement_extremes_are_clamped() {
        assert_eq!(placement_estimate(&levels([0.0; 5])), PLACEMENT_MIN);
        assert_eq!(placement_estimate(&levels([1.0; 5])), PLACEMENT_MAX);
        assert_eq!(PLACEMENT_MAX, 1200);
    }

    #[test]
    fn placement_follows_where_the_results_flip() {
        // Beat the 400 and 800 bots, lost to the rest: two points out of five.
        let mid = placement_estimate(&levels([1.0, 1.0, 0.0, 0.0, 0.0]));
        assert!((950..=1050).contains(&mid), "{mid}");
        // Beating more should never rate lower.
        let better = placement_estimate(&levels([1.0, 1.0, 1.0, 0.0, 0.0]));
        assert!(better > mid + 100, "{better} vs {mid}");
        // Beating the 1600 and 2000 bots too does not go above the default.
        assert_eq!(
            placement_estimate(&levels([1.0, 1.0, 1.0, 1.0, 0.0])),
            PLACEMENT_MAX
        );
        // Half a point against everyone is a draw-ish 1200.
        let even = placement_estimate(&levels([0.5; 5]));
        assert!((1190..=1210).contains(&even), "{even}");
    }
}
