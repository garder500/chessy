//! Campaign mode (docs/spec-v6.md): five chapters against Sage, six levels and a
//! boss each. This module is pure data and rules: the levels, what earns a
//! star, and how a finished game is judged. The hub runs the games
//! (`hub::solo`) and the store keeps the stars.

use std::collections::HashMap;

use chessy_engine::forge::Rarity;
use chessy_engine::{Color, Event, Outcome, PieceKind, SkillId};
use serde::Serialize;

pub const CHAPTERS: u8 = 5;
/// Ordinary levels in a chapter; the boss comes after them.
pub const LEVELS: u8 = 6;
/// A level id is `chapter * 10 + index`, `index` 1..=6, the boss being 7.
pub const BOSS_INDEX: u8 = LEVELS + 1;
/// Stars (out of `LEVELS * 3`) that open the boss of a chapter.
pub const BOSS_GATE: u32 = 12;
/// Skills a player brings when they choose them (chapters 3 to 5).
pub const MAX_HAND: usize = 3;

/// The star bits stored for a level.
pub const STAR_WIN: u8 = 1;
pub const STAR_OBJECTIVE: u8 = 2;
pub const STAR_CHALLENGE: u8 = 4;
pub const STAR_ALL: u8 = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;

pub fn star_count(mask: u8) -> u32 {
    (mask & STAR_ALL).count_ones()
}

/// What the level asks for besides winning. Every objective is judged on a game
/// the player won.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Objective {
    /// Checkmate before move `moves`.
    MateBefore { moves: u32 },
    /// Capture a piece of this kind.
    Capture { piece: PieceKind },
    /// Capture at least `count` enemy pieces.
    Take { count: u32 },
    /// Promote a pawn.
    Promote,
    /// Lose fewer than `count` pieces (pawns not counted).
    LoseFewer { count: u32 },
}

/// A constraint the player chooses to play under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Challenge {
    /// Never lose the queen.
    KeepQueen,
    /// Never lose a knight or a bishop.
    NoMinorLoss,
    /// Do not lose your queen while taking theirs.
    NoQueenTrade,
    /// Play at least one skill.
    UseSkill,
    /// Play no skill at all.
    NoSkill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub id: u8,
    pub chapter: u8,
    pub index: u8,
    pub boss: bool,
    /// Sage's level.
    pub elo: i32,
    /// The skills the level imposes (chapters 1 and 2), empty when the player chooses.
    pub hand: Vec<SkillId>,
    /// The player brings up to [`MAX_HAND`] skills of their own.
    pub choose: bool,
    /// Sage's skills.
    pub enemy: Vec<SkillId>,
    pub objective: Objective,
    pub challenge: Challenge,
    /// The least rarity of the forge a boss pays (boss only).
    pub forge_min: Option<Rarity>,
}

/// The three skills each chapter teaches.
const TAUGHT: [[SkillId; 3]; CHAPTERS as usize] = [
    [SkillId::Terminator, SkillId::Canceller, SkillId::Queensac],
    [SkillId::Imune, SkillId::Forcefield, SkillId::Rollback],
    [
        SkillId::Teleportation,
        SkillId::Transposition,
        SkillId::Bench,
    ],
    [SkillId::Freeze, SkillId::Tornado, SkillId::Invisibility],
    [SkillId::Clone, SkillId::Morph, SkillId::Trap],
];

/// The two signature skills of each boss.
const SIGNATURE: [[SkillId; 2]; CHAPTERS as usize] = [
    [SkillId::Terminator, SkillId::Queensac],
    [SkillId::Forcefield, SkillId::Imune],
    [SkillId::Teleportation, SkillId::Transposition],
    [SkillId::Invisibility, SkillId::Trap],
    [SkillId::Clone, SkillId::Morph],
];

const FORGE_MIN: [Rarity; CHAPTERS as usize] = [
    Rarity::Uncommon,
    Rarity::Rare,
    Rarity::Rare,
    Rarity::Epic,
    Rarity::Legendary,
];

/// Skills Sage may bring on the levels where it plays any.
const ENEMY_POOL: [SkillId; 10] = [
    SkillId::Teleportation,
    SkillId::Imune,
    SkillId::Freeze,
    SkillId::Rollback,
    SkillId::Clone,
    SkillId::Tornado,
    SkillId::Invisibility,
    SkillId::Trap,
    SkillId::Bench,
    SkillId::Transposition,
];

pub fn level_id(chapter: u8, index: u8) -> u8 {
    chapter * 10 + index
}

pub fn boss_id(chapter: u8) -> u8 {
    level_id(chapter, BOSS_INDEX)
}

/// Every level id in play order.
pub fn all_ids() -> impl Iterator<Item = u8> {
    (1..=CHAPTERS).flat_map(|c| (1..=BOSS_INDEX).map(move |i| level_id(c, i)))
}

/// The level with this id, if there is one.
pub fn level(id: u8) -> Option<Level> {
    let (chapter, index) = (id / 10, id % 10);
    if !(1..=CHAPTERS).contains(&chapter) || !(1..=BOSS_INDEX).contains(&index) {
        return None;
    }
    let c = usize::from(chapter - 1);
    let boss = index == BOSS_INDEX;
    let start = 400 + 400 * i32::from(chapter - 1);
    let elo = if boss {
        start + 350
    } else {
        start + 50 * i32::from(index - 1)
    };
    // The first two chapters hand out the skills they teach, one at a time then
    // together; from the third on, the player brings their own.
    let choose = chapter >= 3;
    let taught = TAUGHT[c];
    let hand = if choose {
        Vec::new()
    } else if boss {
        taught.to_vec()
    } else {
        match index {
            1 => vec![taught[0]],
            2 => vec![taught[1]],
            3 => vec![taught[2]],
            4 => vec![taught[0], taught[1]],
            _ => taught.to_vec(),
        }
    };
    let enemy = if boss {
        SIGNATURE[c].to_vec()
    } else {
        // None on the first two levels, one skill, then two: never one the
        // player is handed.
        let count = usize::from((index - 1) / 2);
        ENEMY_POOL
            .iter()
            .cycle()
            .skip(usize::from(id) % ENEMY_POOL.len())
            .filter(|s| !hand.contains(s))
            .take(count)
            .copied()
            .collect()
    };
    let n = u32::from(chapter - 1);
    let objective = match index {
        1 => Objective::MateBefore { moves: 40 - 2 * n },
        2 => Objective::Capture {
            piece: [
                PieceKind::Rook,
                PieceKind::Bishop,
                PieceKind::Knight,
                PieceKind::Rook,
                PieceKind::Bishop,
            ][c],
        },
        3 => Objective::Take { count: 5 + n },
        4 => Objective::Promote,
        5 => Objective::LoseFewer { count: 3 },
        6 => Objective::MateBefore { moves: 32 - 2 * n },
        _ => Objective::Capture {
            piece: PieceKind::Queen,
        },
    };
    let challenge = match index {
        1 | 5 => Challenge::KeepQueen,
        2 | 7 => Challenge::NoQueenTrade,
        3 | 6 => Challenge::NoMinorLoss,
        _ if choose => Challenge::NoSkill,
        _ => Challenge::UseSkill,
    };
    Some(Level {
        id,
        chapter,
        index,
        boss,
        elo,
        hand,
        choose,
        enemy,
        objective,
        challenge,
        forge_min: boss.then_some(FORGE_MIN[c]),
    })
}

/// What a boss's forge can come out as: the tiers from `min` up in the
/// proportions of the forge's own drop weights, in percent (rounded, the first
/// tier takes the remainder so the list adds up to 100).
pub fn forge_odds(min: Rarity) -> Vec<crate::protocol::ForgeOdds> {
    use chessy_engine::forge::generate::DROP_WEIGHTS;
    let weights = &DROP_WEIGHTS[min.index()..];
    let sum: u64 = weights.iter().sum();
    let mut odds: Vec<crate::protocol::ForgeOdds> = Rarity::ALL[min.index()..]
        .iter()
        .zip(weights)
        .map(|(&rarity, &w)| crate::protocol::ForgeOdds {
            rarity,
            percent: (w * 100 / sum) as u32,
        })
        .collect();
    let rest = 100 - odds.iter().map(|o| o.percent).sum::<u32>();
    odds[0].percent += rest;
    odds
}

// ---- progress --------------------------------------------------------------

/// What a player has done on one level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub stars: u8,
    /// A boss won whose forge was not claimed yet.
    pub forge_pending: bool,
}

pub type Progress = HashMap<u8, Record>;

pub fn stars_of(progress: &Progress, id: u8) -> u8 {
    progress.get(&id).map_or(0, |r| r.stars & STAR_ALL)
}

/// Stars won on the six ordinary levels of `chapter`.
pub fn chapter_stars(progress: &Progress, chapter: u8) -> u32 {
    (1..=LEVELS)
        .map(|i| star_count(stars_of(progress, level_id(chapter, i))))
        .sum()
}

/// Whether the boss of `chapter` may be played: enough stars on its levels.
pub fn boss_open(progress: &Progress, chapter: u8) -> bool {
    chapter_stars(progress, chapter) >= BOSS_GATE
}

/// Whether `id` may be started: the first level, the level after one that was
/// won, the boss once the gate is passed. A chapter opens when the boss of the
/// one before is beaten.
pub fn unlocked(progress: &Progress, id: u8) -> bool {
    let Some(l) = level(id) else { return false };
    if l.chapter > 1 && stars_of(progress, boss_id(l.chapter - 1)) & STAR_WIN == 0 {
        return false;
    }
    if l.boss {
        return boss_open(progress, l.chapter);
    }
    l.index == 1 || stars_of(progress, id - 1) & STAR_WIN != 0
}

/// Whether `hand` is a valid choice for `level`: distinct classic skills
/// from `deck`, one to [`MAX_HAND`]. A level that imposes its hand takes none.
pub fn valid_hand(level: &Level, hand: &[SkillId], deck: &[SkillId]) -> bool {
    if !level.choose {
        return hand.is_empty();
    }
    let mut seen = Vec::new();
    !hand.is_empty()
        && hand.len() <= MAX_HAND
        && hand.iter().all(|s| {
            let fresh = !seen.contains(s);
            seen.push(*s);
            fresh && deck.contains(s) && s.kind() == chessy_engine::SkillKind::Classic
        })
}

// ---- judging a game ----------------------------------------------------------

/// What a game showed of the player's side, collected action by action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Enemy pieces the player took.
    pub taken: Vec<PieceKind>,
    /// The player's pieces that were taken.
    pub lost: Vec<PieceKind>,
    pub promoted: bool,
    pub skills_used: u32,
}

impl Tally {
    /// Notes what `events`, the result of an action by `actor`, did to `human`'s side.
    pub fn observe(&mut self, human: Color, actor: Color, events: &[Event]) {
        for event in events {
            match event {
                Event::Captured { piece, .. } => {
                    if piece.color == human {
                        self.lost.push(piece.kind);
                    } else {
                        self.taken.push(piece.kind);
                    }
                }
                Event::Promoted { .. } if actor == human => self.promoted = true,
                Event::SkillUsed { color, .. } if *color == human => self.skills_used += 1,
                _ => {}
            }
        }
    }

    fn lost_any(&self, kinds: &[PieceKind]) -> bool {
        self.lost.iter().any(|k| kinds.contains(k))
    }
}

/// The stars a finished game earns. Only a win earns any: `mover` is the move
/// number the game ended on (the full-move counter, as chess scores it).
pub fn judge(level: &Level, human: Color, outcome: &Outcome, tally: &Tally, mover: u32) -> u8 {
    if outcome.winner() != Some(human) {
        return 0;
    }
    let objective = match level.objective {
        Objective::MateBefore { moves } => {
            matches!(outcome, Outcome::Checkmate { .. }) && mover < moves
        }
        Objective::Capture { piece } => tally.taken.contains(&piece),
        Objective::Take { count } => tally.taken.len() as u32 >= count,
        Objective::Promote => tally.promoted,
        Objective::LoseFewer { count } => {
            (tally.lost.iter().filter(|k| **k != PieceKind::Pawn).count() as u32) < count
        }
    };
    let challenge = match level.challenge {
        Challenge::KeepQueen => !tally.lost_any(&[PieceKind::Queen]),
        Challenge::NoMinorLoss => !tally.lost_any(&[PieceKind::Knight, PieceKind::Bishop]),
        Challenge::NoQueenTrade => {
            !(tally.lost_any(&[PieceKind::Queen]) && tally.taken.contains(&PieceKind::Queen))
        }
        Challenge::UseSkill => tally.skills_used > 0,
        Challenge::NoSkill => tally.skills_used == 0,
    };
    STAR_WIN
        | if objective { STAR_OBJECTIVE } else { 0 }
        | if challenge { STAR_CHALLENGE } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_five_levels_with_the_elo_of_the_design() {
        assert_eq!(all_ids().count(), 35);
        assert!(level(0).is_none() && level(18).is_none() && level(61).is_none());
        let elo = |id| level(id).unwrap().elo;
        assert_eq!((elo(11), elo(16), elo(17)), (400, 650, 750));
        assert_eq!((elo(21), elo(26), elo(27)), (800, 1050, 1150));
        assert_eq!((elo(51), elo(56), elo(57)), (2000, 2250, 2350));
    }

    #[test]
    fn chapters_one_and_two_impose_their_hand_the_others_let_you_choose() {
        let l = level(25).unwrap();
        assert_eq!(
            l.hand,
            vec![SkillId::Imune, SkillId::Forcefield, SkillId::Rollback]
        );
        assert!(!l.choose);
        assert_eq!(level(11).unwrap().hand, vec![SkillId::Terminator]);
        let l = level(35).unwrap();
        assert!(l.choose && l.hand.is_empty());
    }

    #[test]
    fn bosses_bring_two_signatures_and_a_rising_floor() {
        let floors: Vec<_> = (1..=CHAPTERS)
            .map(|c| level(boss_id(c)).unwrap().forge_min.unwrap())
            .collect();
        assert!(floors.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(floors[4], Rarity::Legendary);
        for c in 1..=CHAPTERS {
            let boss = level(boss_id(c)).unwrap();
            assert_eq!(boss.enemy.len(), 2);
            assert!(level(level_id(c, 1)).unwrap().forge_min.is_none());
        }
    }

    #[test]
    fn sage_never_brings_a_skill_the_player_is_handed() {
        for id in all_ids() {
            let l = level(id).unwrap();
            if !l.boss {
                assert!(l.enemy.iter().all(|s| !l.hand.contains(s)), "{id}");
            }
            assert!(l
                .enemy
                .iter()
                .all(|s| s.kind() == chessy_engine::SkillKind::Classic));
        }
    }

    fn progress(entries: &[(u8, u8)]) -> Progress {
        entries
            .iter()
            .map(|&(id, stars)| {
                (
                    id,
                    Record {
                        stars,
                        forge_pending: false,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn levels_open_one_after_another_and_the_boss_at_twelve_stars() {
        let mut p = progress(&[]);
        assert!(unlocked(&p, 11));
        assert!(!unlocked(&p, 12) && !unlocked(&p, 17) && !unlocked(&p, 21));
        p.extend(progress(&[(11, STAR_WIN)]));
        assert!(unlocked(&p, 12) && !unlocked(&p, 13));
        // Six wins is six stars: not enough for the boss.
        let mut p = progress(&(1..=6).map(|i| (10 + i, STAR_WIN)).collect::<Vec<_>>());
        assert!(!boss_open(&p, 1) && !unlocked(&p, 17));
        // Six more stars open it.
        for i in 1..=6 {
            p.insert(
                10 + i,
                Record {
                    stars: STAR_ALL,
                    forge_pending: false,
                },
            );
        }
        assert_eq!(chapter_stars(&p, 1), 18);
        assert!(unlocked(&p, 17));
        // The next chapter waits for the boss to be beaten.
        assert!(!unlocked(&p, 21));
        p.insert(
            17,
            Record {
                stars: STAR_WIN,
                forge_pending: false,
            },
        );
        assert!(unlocked(&p, 21) && !unlocked(&p, 22));
    }

    #[test]
    fn only_a_win_earns_stars() {
        let l = level(12).unwrap(); // capture a rook, do not lose the queens' trade
        let tally = Tally {
            taken: vec![PieceKind::Rook],
            ..Tally::default()
        };
        let mate = Outcome::Checkmate {
            winner: Color::White,
        };
        assert_eq!(judge(&l, Color::White, &mate, &tally, 20), STAR_ALL);
        let drawn = Outcome::Stalemate;
        assert_eq!(judge(&l, Color::White, &drawn, &tally, 20), 0);
        let lost = Outcome::Checkmate {
            winner: Color::Black,
        };
        assert_eq!(judge(&l, Color::White, &lost, &tally, 20), 0);
        // A win without the rook: the objective star is missing.
        assert_eq!(
            judge(&l, Color::White, &mate, &Tally::default(), 20),
            STAR_WIN | STAR_CHALLENGE
        );
    }

    #[test]
    fn mate_before_a_move_is_strict() {
        let l = level(11).unwrap(); // mate before move 40, keep the queen
        let mate = Outcome::Checkmate {
            winner: Color::White,
        };
        let t = Tally::default();
        assert_eq!(judge(&l, Color::White, &mate, &t, 39), STAR_ALL);
        assert_eq!(
            judge(&l, Color::White, &mate, &t, 40),
            STAR_WIN | STAR_CHALLENGE
        );
        // A win by resignation is a win, not a mate.
        let resigned = Outcome::Resignation {
            winner: Color::White,
        };
        assert_eq!(
            judge(&l, Color::White, &resigned, &t, 10),
            STAR_WIN | STAR_CHALLENGE
        );
    }

    #[test]
    fn challenges_look_at_what_the_player_lost_and_played() {
        let mate = Outcome::Checkmate {
            winner: Color::White,
        };
        let mut l = level(11).unwrap();
        l.challenge = Challenge::KeepQueen;
        let lost_queen = Tally {
            lost: vec![PieceKind::Queen],
            ..Tally::default()
        };
        assert_eq!(
            judge(&l, Color::White, &mate, &lost_queen, 10) & STAR_CHALLENGE,
            0
        );
        l.challenge = Challenge::NoQueenTrade;
        // Losing the queen for nothing is not a trade; taking theirs too is.
        assert_ne!(
            judge(&l, Color::White, &mate, &lost_queen, 10) & STAR_CHALLENGE,
            0
        );
        let traded = Tally {
            lost: vec![PieceKind::Queen],
            taken: vec![PieceKind::Queen],
            ..Tally::default()
        };
        assert_eq!(
            judge(&l, Color::White, &mate, &traded, 10) & STAR_CHALLENGE,
            0
        );
        l.challenge = Challenge::NoMinorLoss;
        let lost_knight = Tally {
            lost: vec![PieceKind::Knight],
            ..Tally::default()
        };
        assert_eq!(
            judge(&l, Color::White, &mate, &lost_knight, 10) & STAR_CHALLENGE,
            0
        );
        l.challenge = Challenge::NoSkill;
        let used = Tally {
            skills_used: 1,
            ..Tally::default()
        };
        assert_eq!(
            judge(&l, Color::White, &mate, &used, 10) & STAR_CHALLENGE,
            0
        );
        l.challenge = Challenge::UseSkill;
        assert_ne!(
            judge(&l, Color::White, &mate, &used, 10) & STAR_CHALLENGE,
            0
        );
    }

    #[test]
    fn forge_odds_follow_the_drop_weights_above_the_floor() {
        let pct = |min| {
            forge_odds(min)
                .iter()
                .map(|o| o.percent)
                .collect::<Vec<_>>()
        };
        assert_eq!(pct(Rarity::Legendary), vec![100]);
        assert_eq!(pct(Rarity::Epic), vec![86, 14]);
        let rare = pct(Rarity::Rare);
        assert_eq!(rare.iter().sum::<u32>(), 100);
        assert!(rare[0] > rare[1] && rare[1] > rare[2]);
        assert_eq!(pct(Rarity::Common).iter().sum::<u32>(), 100);
    }

    #[test]
    fn a_hand_is_checked_against_the_deck() {
        let l = level(35).unwrap();
        let deck = [SkillId::Freeze, SkillId::Imune, SkillId::Mind];
        assert!(valid_hand(&l, &[SkillId::Freeze], &deck));
        assert!(valid_hand(&l, &[SkillId::Freeze, SkillId::Imune], &deck));
        assert!(!valid_hand(&l, &[], &deck));
        assert!(!valid_hand(&l, &[SkillId::Tornado], &deck));
        assert!(!valid_hand(&l, &[SkillId::Freeze, SkillId::Freeze], &deck));
        assert!(
            !valid_hand(&l, &[SkillId::Mind], &deck),
            "unique skills stay out"
        );
        // A level that imposes its hand takes no choice.
        let imposed = level(21).unwrap();
        assert!(valid_hand(&imposed, &[], &deck));
        assert!(!valid_hand(&imposed, &[SkillId::Freeze], &deck));
    }
}
