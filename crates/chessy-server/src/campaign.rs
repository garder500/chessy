//! Campaign content and rules (see `docs/spec-campagne.md`): the level table,
//! the objectives behind the stars and the boss reward range.
//!
//! A campaign game is an ordinary Solo game with imposed decks; this module is
//! pure data and rules, the game flow lives in `hub/campaign.rs`.

use std::ops::RangeInclusive;

use chessy_engine::forge::Rarity;
use chessy_engine::{Color, Game, PieceKind, SkillId};
use serde::Deserialize;

use crate::campaign_store::CampaignRow;

mod levels;
pub use levels::CHAPTERS;

pub const LEVELS_PER_CHAPTER: u8 = 6;
/// Wire level of the boss of a chapter.
pub const BOSS_LEVEL: u8 = LEVELS_PER_CHAPTER;
/// Stars (out of `3 * LEVELS_PER_CHAPTER`) that open the boss.
pub const BOSS_STARS: u8 = 12;

const BASE_ELO: i32 = 400;
const ELO_PER_CHAPTER: i32 = 400;
const ELO_PER_LEVEL: i32 = 50;
const BOSS_ELO_BONUS: i32 = 100;

/// Bits of the stars mask stored per level.
pub const STAR_WIN: u8 = 1;
pub const STAR_OBJECTIVE: u8 = 2;
pub const STAR_CHALLENGE: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct LevelRef {
    pub chapter: u8,
    pub level: u8,
}

impl LevelRef {
    pub fn is_boss(self) -> bool {
        self.level == BOSS_LEVEL
    }

    pub fn elo(self) -> i32 {
        let level = i32::from(self.level.min(LEVELS_PER_CHAPTER - 1));
        let boss = if self.is_boss() { BOSS_ELO_BONUS } else { 0 };
        BASE_ELO + ELO_PER_CHAPTER * i32::from(self.chapter) + ELO_PER_LEVEL * level + boss
    }
}

/// A condition checked on the finished game, once it is won.
#[derive(Clone, Copy, Debug)]
pub enum Objective {
    /// Won within this many of the player's own turns (a skill that ends the
    /// turn counts as one).
    WinWithin(u32),
    KeepPiece(PieceKind),
    UseSkill(SkillId),
    UseAnySkill,
    NoSkillUsed,
}

impl Objective {
    pub fn met(self, game: &Game, human: Color) -> bool {
        let slots = &game.loadout(human).slots;
        match self {
            Objective::WinWithin(max) => own_turns(game, human) <= max,
            Objective::KeepPiece(kind) => game.pos.pieces(human).any(|(_, p)| p.kind == kind),
            Objective::UseSkill(skill) => slots.iter().any(|s| s.skill == skill && s.uses > 0),
            Objective::UseAnySkill => slots.iter().any(|s| s.uses > 0),
            Objective::NoSkillUsed => slots.iter().all(|s| s.uses == 0),
        }
    }

    pub fn text(self) -> String {
        match self {
            Objective::WinWithin(turns) => format!("Gagner en {turns} coups ou moins"),
            Objective::KeepPiece(kind) => format!("Terminer avec {}", piece_label(kind)),
            Objective::UseSkill(skill) => format!("Utiliser {}", skill_label(skill)),
            Objective::UseAnySkill => "Utiliser une compétence".to_string(),
            Objective::NoSkillUsed => "Gagner sans utiliser de compétence".to_string(),
        }
    }
}

/// Turns the player has finished: `pos.ply` advances on every turn handed over,
/// and White moves on even plies. Every start has White to move and
/// `Position::from_fen` yields ply 0 whatever the fullmove number, so the
/// count is the same for a custom start.
fn own_turns(game: &Game, human: Color) -> u32 {
    let white_first = u32::from(human == Color::White);
    (game.pos.ply + white_first) / 2
}

fn piece_label(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "un pion",
        PieceKind::Knight => "un cavalier",
        PieceKind::Bishop => "un fou",
        PieceKind::Rook => "une tour",
        PieceKind::Queen => "sa dame",
        PieceKind::King => "son roi",
    }
}

fn skill_label(skill: SkillId) -> &'static str {
    match skill {
        SkillId::Teleportation => "Teleportation",
        SkillId::Imune => "Imune",
        SkillId::Freeze => "Freeze",
        SkillId::Rollback => "Rollback",
        SkillId::Clone => "Clone",
        SkillId::DestinySwapper => "Destiny Swapper",
        SkillId::Remover => "Remover",
        SkillId::Wall => "Wall",
        SkillId::Mirage => "Mirage",
        SkillId::Evolve => "Evolve",
        SkillId::Switch => "Switch Sides",
        SkillId::Mind => "Mind Reading",
        SkillId::Control => "Mind Control",
        SkillId::Morph => "Morph",
        SkillId::Canceller => "Canceller",
        SkillId::Tornado => "Tornado",
        SkillId::Invisibility => "Invisibility",
        SkillId::Terminator => "Terminator",
        SkillId::Trap => "Trap Card",
        SkillId::Bench => "The Bench",
        SkillId::Forcefield => "Force Field",
        SkillId::Transposition => "Transposition",
        SkillId::Queensac => "Queen Sacrifice",
        SkillId::Temporal => "Temporal Distortion",
        SkillId::Geomancy => "Geomancy",
        SkillId::Celestial => "Celestial Intervention",
        SkillId::Godhelp => "God Help",
        SkillId::Forged(_) => "une compétence forgée",
    }
}

/// A custom starting position (White to move), designed for one side.
pub struct Start {
    pub fen: &'static str,
    pub human: Color,
}

pub struct Level {
    pub name: &'static str,
    pub player_deck: &'static [SkillId],
    pub bot_deck: &'static [SkillId],
    /// The player picks 1..=3 skills of their own deck; `player_deck` is then empty.
    pub deck_choice: bool,
    pub start: Option<Start>,
    pub objective: Option<Objective>,
    pub challenge: Option<Objective>,
}

pub struct Chapter {
    pub family: &'static str,
    pub name: &'static str,
    /// Title earned by beating the boss.
    pub title: &'static str,
    /// The six levels then the boss.
    pub levels: &'static [Level],
}

impl Chapter {
    pub fn available(&self) -> bool {
        !self.levels.is_empty()
    }
}

/// The level behind a reference, if the chapter has content for it.
pub fn level(at: LevelRef) -> Option<&'static Level> {
    CHAPTERS
        .get(usize::from(at.chapter))?
        .levels
        .get(usize::from(at.level))
}

/// Stars mask of a finished game; objective and challenge only count on a win.
pub fn stars_earned(at: LevelRef, game: &Game, human: Color, won: bool) -> u8 {
    let Some(level) = level(at).filter(|_| won) else {
        return 0;
    };
    level_stars(level, |goal| goal.met(game, human))
}

/// Every star the level offers (a boss has no objective nor challenge).
pub fn all_stars(at: LevelRef) -> u8 {
    level(at).map_or(0, |level| level_stars(level, |_| true))
}

fn level_stars(level: &Level, reached: impl Fn(Objective) -> bool) -> u8 {
    let star = |goal: Option<Objective>, bit: u8| if goal.is_some_and(&reached) { bit } else { 0 };
    STAR_WIN | star(level.objective, STAR_OBJECTIVE) | star(level.challenge, STAR_CHALLENGE)
}

/// Rarities the boss of a chapter may forge.
pub fn rarity_range(chapter: u8) -> RangeInclusive<Rarity> {
    match chapter {
        0 => Rarity::Uncommon..=Rarity::Epic,
        1 | 2 => Rarity::Rare..=Rarity::Legendary,
        _ => Rarity::Epic..=Rarity::Legendary,
    }
}

/// Stars of a mask as `[win, objective, challenge]`.
pub fn star_flags(mask: u8) -> [bool; 3] {
    [STAR_WIN, STAR_OBJECTIVE, STAR_CHALLENGE].map(|bit| mask & bit != 0)
}

/// Stars gathered on the six levels of a chapter (the boss does not count).
pub fn chapter_stars(rows: &[CampaignRow], chapter: u8) -> u8 {
    rows.iter()
        .filter(|r| r.at.chapter == chapter && r.at.level < BOSS_LEVEL)
        .map(|r| r.stars.count_ones() as u8)
        .sum()
}

pub fn boss_unlocked(rows: &[CampaignRow], chapter: u8) -> bool {
    chapter_stars(rows, chapter) >= BOSS_STARS
}

/// Whether the boss of a chapter has been beaten.
pub fn title_earned(rows: &[CampaignRow], chapter: u8) -> bool {
    rows.iter()
        .any(|r| r.at.chapter == chapter && r.at.is_boss() && r.stars & STAR_WIN != 0)
}

/// Title of the highest chapter whose boss has been beaten.
pub fn best_title(rows: &[CampaignRow]) -> Option<&'static str> {
    CHAPTERS
        .iter()
        .enumerate()
        .rev()
        .find(|&(chapter, _)| title_earned(rows, chapter as u8))
        .map(|(_, c)| c.title)
}

#[cfg(test)]
mod tests {
    use chessy_engine::Position;

    use super::*;

    fn game_at(ply: u32) -> Game {
        let mut game = Game::new(&[], &[]);
        game.pos.ply = ply;
        game
    }

    fn row(chapter: u8, level: u8, stars: u8) -> CampaignRow {
        CampaignRow {
            at: LevelRef { chapter, level },
            stars,
            rewarded: false,
        }
    }

    fn all_levels() -> impl Iterator<Item = &'static Level> {
        CHAPTERS.iter().flat_map(|c| c.levels)
    }

    #[test]
    fn win_within_counts_the_players_own_turns() {
        let within = Objective::WinWithin(3);
        // White has played turns 1-3 after five plies, Black two.
        assert!(within.met(&game_at(5), Color::White));
        assert!(!within.met(&game_at(7), Color::White));
        assert!(within.met(&game_at(7), Color::Black));
        assert!(!within.met(&game_at(8), Color::Black));
    }

    #[test]
    fn win_within_counts_from_a_custom_start() {
        for level in all_levels().filter(|l| l.start.is_some()) {
            let start = level.start.as_ref().unwrap();
            let pos = Position::from_fen(start.fen).unwrap();
            let mut game = Game::from_position(pos, &[], &[]);
            assert_eq!(game.pos.ply, 0);
            game.pos.ply = 5;
            assert!(Objective::WinWithin(3).met(&game, Color::White));
            assert!(!Objective::WinWithin(2).met(&game, Color::White));
            assert!(Objective::WinWithin(2).met(&game, Color::Black));
        }
    }

    #[test]
    fn every_chapter_has_six_levels_and_a_boss() {
        for chapter in &CHAPTERS {
            assert_eq!(chapter.levels.len(), usize::from(LEVELS_PER_CHAPTER) + 1);
        }
    }

    #[test]
    fn no_level_pairs_a_skill_use_with_no_skill_used() {
        let uses_skill = |goal: Option<Objective>| {
            matches!(goal, Some(Objective::UseSkill(_) | Objective::UseAnySkill))
        };
        let no_skill = |goal: Option<Objective>| matches!(goal, Some(Objective::NoSkillUsed));
        for level in all_levels() {
            let (a, b) = (level.objective, level.challenge);
            assert!(!(uses_skill(a) && no_skill(b)) && !(no_skill(a) && uses_skill(b)));
        }
    }

    #[test]
    fn decks_are_small_and_the_bot_never_gets_mind_or_control() {
        for level in all_levels() {
            assert!(level.player_deck.len() <= 3 && level.bot_deck.len() <= 3);
            assert!(!level
                .bot_deck
                .iter()
                .any(|s| matches!(s, SkillId::Mind | SkillId::Control)));
        }
    }

    #[test]
    fn starts_are_playable_positions() {
        for start in all_levels().filter_map(|l| l.start.as_ref()) {
            let pos = Position::from_fen(start.fen).unwrap();
            assert_eq!(pos.side, Color::White);
            assert!(!pos.legal_moves().is_empty(), "{}", start.fen);
        }
    }

    #[test]
    fn chosen_deck_levels_have_no_imposed_deck_or_use_skill_goal() {
        let is_use_skill = |goal: Option<Objective>| matches!(goal, Some(Objective::UseSkill(_)));
        for level in all_levels().filter(|l| l.deck_choice) {
            assert!(level.player_deck.is_empty());
            assert!(!is_use_skill(level.objective) && !is_use_skill(level.challenge));
        }
    }

    #[test]
    fn titles_follow_the_beaten_bosses() {
        assert_eq!(best_title(&[]), None);
        let rows = [row(0, BOSS_LEVEL, STAR_WIN), row(2, BOSS_LEVEL, STAR_WIN)];
        assert!(title_earned(&rows, 0) && !title_earned(&rows, 1));
        assert_eq!(best_title(&rows), Some("Marcheur du Vide"));
        assert_eq!(best_title(&[row(1, 2, STAR_WIN)]), None);
    }
}
