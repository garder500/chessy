//! Campaign content and rules (see `docs/spec-campagne.md`): the level table,
//! the objectives behind the stars and the forge table of each boss.
//!
//! A campaign game is an ordinary Solo game with imposed decks; this module is
//! pure data and rules, the game flow lives in `hub/campaign.rs`.

use chessy_engine::{Color, Game, PieceKind, SkillId};
use serde::Deserialize;

use crate::campaign_store::CampaignRow;

mod forge_table;
mod levels;
mod titles;
pub use forge_table::{forge_table, ForgeTable};
pub use levels::CHAPTERS;
pub use titles::{best_title, title_earned, titles_earned};

pub const LEVELS_PER_CHAPTER: u8 = 6;
/// Wire level of the boss of a chapter.
pub const BOSS_LEVEL: u8 = LEVELS_PER_CHAPTER;
/// Stars (out of `3 * LEVELS_PER_CHAPTER`) that open the boss.
pub const BOSS_STARS: u8 = 12;
/// Stars of the whole campaign: three per level, the bosses included.
pub const MAX_STARS: u16 = 105;
/// Consecutive defeats on a level after which its written hint is shown.
pub const HINT_AFTER_DEFEATS: u32 = 3;

const ELO_PER_CHAPTER: i32 = 400;
const ELO_PER_LEVEL: i32 = 50;

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
        let chapter = i32::from(self.chapter) + 1;
        if self.is_boss() {
            return ELO_PER_CHAPTER * (chapter + 1);
        }
        ELO_PER_CHAPTER * chapter + ELO_PER_LEVEL * i32::from(self.level)
    }
}

/// A condition checked on the finished game, once it is won.
#[derive(Clone, Copy, Debug)]
pub enum Objective {
    /// Mate before this move number of the player's own notation (a skill that
    /// ends the turn counts as one).
    WinWithin(u32),
    KeepPiece(PieceKind),
    UseSkill(SkillId),
    UseAnySkill,
    NoSkillUsed,
    UseAllSkills,
    /// No piece lost before this move number of the player's own notation.
    NoPieceLostBefore(u16),
}

/// What a finished game knows beyond its final position.
#[derive(Clone, Copy, Default)]
pub struct Context<'a> {
    /// Ply at which the human lost their first piece.
    pub first_loss_ply: Option<u32>,
    /// The only skills `UseAllSkills` counts: on a level where the player
    /// brings their own deck, that deck must not have to be used up.
    pub counted: Option<&'a [SkillId]>,
}

impl Objective {
    pub fn met(self, game: &Game, human: Color, context: &Context) -> bool {
        let slots = &game.loadout(human).slots;
        match self {
            Objective::WinWithin(max) => own_turns(game.pos.ply, human) < max,
            Objective::KeepPiece(kind) => game.pos.pieces(human).any(|(_, p)| p.kind == kind),
            Objective::UseSkill(skill) => slots.iter().any(|s| s.skill == skill && s.uses > 0),
            Objective::UseAnySkill => slots.iter().any(|s| s.uses > 0),
            Objective::NoSkillUsed => slots.iter().all(|s| s.uses == 0),
            Objective::UseAllSkills => slots
                .iter()
                .filter(|s| context.counted.is_none_or(|only| only.contains(&s.skill)))
                .all(|s| s.uses > 0),
            Objective::NoPieceLostBefore(min) => context
                .first_loss_ply
                .is_none_or(|ply| own_turns(ply, human) >= u32::from(min)),
        }
    }

    pub fn text(self) -> String {
        match self {
            Objective::WinWithin(turns) => format!("Mater avant le coup {turns}"),
            Objective::KeepPiece(kind) => format!("Terminer avec {}", piece_label(kind)),
            Objective::UseSkill(skill) => format!("Utiliser {}", skill_label(skill)),
            Objective::UseAnySkill => "Utiliser une compétence".to_string(),
            Objective::NoSkillUsed => "Gagner sans utiliser de compétence".to_string(),
            Objective::UseAllSkills => "Utiliser toutes ses compétences".to_string(),
            Objective::NoPieceLostBefore(turns) => {
                format!("Ne perdre aucune pièce avant le coup {turns}")
            }
        }
    }
}

/// Turns the player has finished: `pos.ply` advances on every turn handed over,
/// and White moves on even plies. Every start has White to move and
/// `Position::from_fen` yields ply 0 whatever the fullmove number, so the
/// count is the same for a custom start.
fn own_turns(ply: u32, human: Color) -> u32 {
    let white_first = u32::from(human == Color::White);
    (ply + white_first) / 2
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
    /// Written advice shown after `HINT_AFTER_DEFEATS` defeats.
    pub hint: &'static str,
    /// Skills lent to the player for this level.
    pub lent: &'static [SkillId],
}

impl Level {
    /// The move number before which the mate must come, if a goal sets one.
    pub fn move_limit(&self) -> Option<u32> {
        [self.objective, self.challenge]
            .into_iter()
            .flatten()
            .find_map(|goal| match goal {
                Objective::WinWithin(turns) => Some(turns),
                _ => None,
            })
    }
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
pub fn stars_earned(
    at: LevelRef,
    game: &Game,
    human: Color,
    first_loss_ply: Option<u32>,
    won: bool,
) -> u8 {
    let Some(level) = level(at).filter(|_| won) else {
        return 0;
    };
    let context = Context {
        first_loss_ply,
        counted: level.deck_choice.then_some(level.lent),
    };
    level_stars(level, |goal| goal.met(game, human, &context))
}

/// Every star the level offers.
pub fn all_stars(at: LevelRef) -> u8 {
    level(at).map_or(0, |level| level_stars(level, |_| true))
}

fn level_stars(level: &Level, reached: impl Fn(Objective) -> bool) -> u8 {
    let star = |goal: Option<Objective>, bit: u8| if goal.is_some_and(&reached) { bit } else { 0 };
    STAR_WIN | star(level.objective, STAR_OBJECTIVE) | star(level.challenge, STAR_CHALLENGE)
}

/// Stars of a mask as `[win, objective, challenge]`.
pub fn star_flags(mask: u8) -> [bool; 3] {
    [STAR_WIN, STAR_OBJECTIVE, STAR_CHALLENGE].map(|bit| mask & bit != 0)
}

/// Stars gathered on the six levels of a chapter (the boss does not count
/// towards opening itself).
pub fn chapter_stars(rows: &[CampaignRow], chapter: u8) -> u8 {
    rows.iter()
        .filter(|r| r.at.chapter == chapter && r.at.level < BOSS_LEVEL)
        .map(|r| r.stars.count_ones() as u8)
        .sum()
}

pub fn boss_unlocked(rows: &[CampaignRow], chapter: u8) -> bool {
    chapter_stars(rows, chapter) >= BOSS_STARS
}

/// Stars gathered on the whole campaign, the bosses included.
pub fn total_stars(rows: &[CampaignRow]) -> u16 {
    rows.iter().map(|r| r.stars.count_ones() as u16).sum()
}

#[cfg(test)]
mod tests {
    use chessy_engine::Position;

    use super::*;

    const NONE: Context = Context {
        first_loss_ply: None,
        counted: None,
    };

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
        let within = Objective::WinWithin(4);
        // White has played turns 1-3 after five plies, Black two.
        assert!(within.met(&game_at(5), Color::White, &NONE));
        assert!(!within.met(&game_at(7), Color::White, &NONE));
        assert!(within.met(&game_at(7), Color::Black, &NONE));
        assert!(!within.met(&game_at(8), Color::Black, &NONE));
    }

    #[test]
    fn win_within_n_means_mate_before_move_n() {
        let within = Objective::WinWithin(3);
        // White's mating move is its turn n - 1 (ply 3) or its turn n (ply 5).
        assert!(within.met(&game_at(3), Color::White, &NONE));
        assert!(!within.met(&game_at(5), Color::White, &NONE));
        assert!(within.met(&game_at(4), Color::Black, &NONE));
        assert!(!within.met(&game_at(6), Color::Black, &NONE));
    }

    #[test]
    fn win_within_counts_from_a_custom_start() {
        for level in all_levels().filter(|l| l.start.is_some()) {
            let start = level.start.as_ref().unwrap();
            let pos = Position::from_fen(start.fen).unwrap();
            let mut game = Game::from_position(pos, &[], &[]);
            assert_eq!(game.pos.ply, 0);
            game.pos.ply = 5;
            assert!(Objective::WinWithin(4).met(&game, Color::White, &NONE));
            assert!(!Objective::WinWithin(3).met(&game, Color::White, &NONE));
            assert!(Objective::WinWithin(3).met(&game, Color::Black, &NONE));
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
            matches!(
                goal,
                Some(Objective::UseSkill(_) | Objective::UseAnySkill | Objective::UseAllSkills)
            )
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
    fn use_all_skills_needs_every_slot_used() {
        let mut game = Game::new(&[SkillId::Trap, SkillId::Wall], &[]);
        assert!(!Objective::UseAllSkills.met(&game, Color::White, &NONE));
        game.loadouts[0].slots[0].uses = 1;
        assert!(!Objective::UseAllSkills.met(&game, Color::White, &NONE));
        game.loadouts[0].slots[1].uses = 1;
        assert!(Objective::UseAllSkills.met(&game, Color::White, &NONE));
        assert_eq!(
            Objective::UseAllSkills.text(),
            "Utiliser toutes ses compétences"
        );
    }

    #[test]
    fn use_all_skills_on_a_chosen_deck_counts_only_the_lent_skills() {
        let mut game = Game::new(&[SkillId::Trap, SkillId::Wall], &[]);
        let lent = [SkillId::Wall];
        let context = Context {
            first_loss_ply: None,
            counted: Some(&lent),
        };
        assert!(!Objective::UseAllSkills.met(&game, Color::White, &context));
        game.loadouts[0].slots[1].uses = 1;
        assert!(Objective::UseAllSkills.met(&game, Color::White, &context));
    }

    #[test]
    fn no_piece_lost_is_judged_on_the_move_of_the_first_loss() {
        let goal = Objective::NoPieceLostBefore(20);
        let game = Game::new(&[], &[]);
        let lost_at = |ply| Context {
            first_loss_ply: Some(ply),
            counted: None,
        };
        assert!(goal.met(&game, Color::White, &NONE));
        assert!(goal.met(&game, Color::White, &lost_at(40)));
        assert!(!goal.met(&game, Color::White, &lost_at(30)));
        assert!(!goal.met(&game, Color::Black, &lost_at(30)));
        assert_eq!(goal.text(), "Ne perdre aucune pièce avant le coup 20");
    }

    #[test]
    fn win_within_text_and_move_limit() {
        assert_eq!(Objective::WinWithin(31).text(), "Mater avant le coup 31");
        let limit = |level: &Level| level.move_limit();
        let levels: Vec<_> = all_levels().collect();
        assert!(levels.iter().any(|l| limit(l).is_some()));
        assert!(levels.iter().any(|l| limit(l).is_none()));
    }

    #[test]
    fn elo_climbs_by_chapter_and_level_and_the_boss_tops_the_chapter() {
        let at = |chapter, level| LevelRef { chapter, level }.elo();
        assert_eq!((at(0, 0), at(0, 1), at(0, 5)), (400, 450, 650));
        assert_eq!(
            (at(0, BOSS_LEVEL), at(4, 0), at(4, BOSS_LEVEL)),
            (800, 2000, 2400)
        );
    }

    #[test]
    fn every_boss_offers_three_stars() {
        for chapter in 0..CHAPTERS.len() as u8 {
            let boss = LevelRef {
                chapter,
                level: BOSS_LEVEL,
            };
            assert_eq!(all_stars(boss), STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE);
        }
        let all: Vec<_> = (0..CHAPTERS.len() as u8)
            .flat_map(|chapter| (0..=BOSS_LEVEL).map(move |level| row(chapter, level, 7)))
            .collect();
        assert_eq!(total_stars(&all), MAX_STARS);
    }

    #[test]
    fn boss_goals_respect_the_hands() {
        for (chapter, c) in CHAPTERS.iter().enumerate() {
            let boss = &c.levels[usize::from(BOSS_LEVEL)];
            let goals = [boss.objective, boss.challenge];
            let queen_sac = boss.player_deck.contains(&SkillId::Queensac);
            for goal in goals.into_iter().flatten() {
                assert!(!(queen_sac && matches!(goal, Objective::KeepPiece(PieceKind::Queen))));
                assert!(chapter > 2 || !matches!(goal, Objective::NoSkillUsed));
            }
        }
    }

    #[test]
    fn a_draw_or_stalemate_earns_no_star() {
        let at = LevelRef {
            chapter: 0,
            level: 0,
        };
        assert_eq!(
            stars_earned(at, &Game::new(&[], &[]), Color::White, None, false),
            0
        );
    }
}
