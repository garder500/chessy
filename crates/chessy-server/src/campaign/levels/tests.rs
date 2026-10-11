use chessy_engine::{PieceKind, SkillId};

use super::CHAPTERS;
use crate::campaign::{
    all_stars, Level, LevelRef, Objective, BOSS_LEVEL, LEVELS_PER_CHAPTER, MAX_STARS,
};

const SAGE_HAND_SIZES: [usize; 6] = [1, 1, 2, 2, 3, 3];
const BOSS_HAND_SIZE: usize = 3;
const LENDING_FROM_CHAPTER: usize = 2;

fn levels() -> impl Iterator<Item = (usize, usize, &'static Level)> {
    CHAPTERS.iter().enumerate().flat_map(|(chapter, c)| {
        c.levels
            .iter()
            .enumerate()
            .map(move |(level, l)| (chapter, level, l))
    })
}

fn goals(level: &Level) -> impl Iterator<Item = Objective> {
    [level.objective, level.challenge].into_iter().flatten()
}

fn player_hand(level: &Level) -> impl Iterator<Item = &'static SkillId> {
    level.player_deck.iter().chain(level.lent)
}

#[test]
fn there_are_thirty_five_levels_worth_105_stars() {
    assert_eq!(levels().count(), 35);
    let stars: u32 = levels()
        .map(|(chapter, level, _)| {
            let at = LevelRef {
                chapter: chapter as u8,
                level: level as u8,
            };
            all_stars(at).count_ones()
        })
        .sum();
    assert_eq!(stars, u32::from(MAX_STARS));
}

#[test]
fn use_skill_goals_name_a_skill_of_the_players_hand() {
    for (chapter, level, l) in levels() {
        for goal in goals(l) {
            if let Objective::UseSkill(skill) = goal {
                assert!(player_hand(l).any(|s| *s == skill), "{chapter}-{level}");
            }
        }
    }
}

#[test]
fn the_first_two_chapters_never_ask_for_no_skill() {
    for (chapter, level, l) in levels().filter(|(chapter, ..)| *chapter < 2) {
        assert!(
            !goals(l).any(|g| matches!(g, Objective::NoSkillUsed)),
            "{chapter}-{level}"
        );
    }
}

#[test]
fn keeping_the_queen_is_never_asked_with_queen_sacrifice() {
    for (chapter, level, l) in
        levels().filter(|(_, _, l)| player_hand(l).any(|s| *s == SkillId::Queensac))
    {
        assert!(
            !goals(l).any(|g| matches!(g, Objective::KeepPiece(PieceKind::Queen))),
            "{chapter}-{level}"
        );
    }
}

#[test]
fn sage_hands_grow_with_the_level_and_bosses_bring_three_skills() {
    for chapter in &CHAPTERS {
        let sizes: Vec<_> = chapter.levels.iter().map(|l| l.bot_deck.len()).collect();
        assert_eq!(sizes[..6], SAGE_HAND_SIZES);
        assert_eq!(sizes[usize::from(BOSS_LEVEL)], BOSS_HAND_SIZE);
    }
}

#[test]
fn elo_follows_the_chapter_and_the_level() {
    for (chapter, level, _) in levels() {
        let at = LevelRef {
            chapter: chapter as u8,
            level: level as u8,
        };
        let c = chapter as i32 + 1;
        let expected = if at.is_boss() {
            400 * (c + 1)
        } else {
            400 * c + 50 * level as i32
        };
        assert_eq!(at.elo(), expected, "{chapter}-{level}");
    }
}

#[test]
fn the_objective_and_the_challenge_differ() {
    for (chapter, level, l) in levels() {
        let (objective, challenge) = (l.objective.unwrap(), l.challenge.unwrap());
        assert_ne!(
            format!("{objective:?}"),
            format!("{challenge:?}"),
            "{chapter}-{level}"
        );
    }
}

#[test]
fn every_level_carries_its_own_hint() {
    let mut hints: Vec<_> = levels().map(|(.., l)| l.hint).collect();
    assert!(hints.iter().all(|h| !h.is_empty()));
    hints.sort_unstable();
    hints.dedup();
    assert_eq!(
        hints.len(),
        usize::from(LEVELS_PER_CHAPTER + 1) * CHAPTERS.len()
    );
}

#[test]
fn imposed_chapters_lend_nothing_and_the_others_lend_their_three_skills() {
    for (chapter, level, l) in levels() {
        let lends = chapter >= LENDING_FROM_CHAPTER;
        assert_eq!(l.deck_choice, lends, "{chapter}-{level}");
        assert_eq!(l.lent.len(), if lends { 3 } else { 0 }, "{chapter}-{level}");
        assert_eq!(l.player_deck.is_empty(), lends, "{chapter}-{level}");
    }
}
