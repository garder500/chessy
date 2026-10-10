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
    /// Won within this many plies (both sides' actions).
    WinWithin(usize),
    KeepPiece(PieceKind),
    UseSkill(SkillId),
    NoSkillUsed,
}

impl Objective {
    pub fn met(self, game: &Game, human: Color, plies: usize) -> bool {
        let slots = &game.loadout(human).slots;
        match self {
            Objective::WinWithin(max) => plies <= max,
            Objective::KeepPiece(kind) => game.pos.pieces(human).any(|(_, p)| p.kind == kind),
            Objective::UseSkill(skill) => slots.iter().any(|s| s.skill == skill && s.uses > 0),
            Objective::NoSkillUsed => slots.iter().all(|s| s.uses == 0),
        }
    }

    pub fn text(self) -> String {
        match self {
            Objective::WinWithin(plies) => format!("Gagner en {} coups ou moins", plies / 2),
            Objective::KeepPiece(kind) => format!("Terminer avec {}", piece_label(kind)),
            Objective::UseSkill(skill) => format!("Utiliser {}", skill_label(skill)),
            Objective::NoSkillUsed => "Gagner sans utiliser de compétence".to_string(),
        }
    }
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

fn skill_label(skill: SkillId) -> String {
    match skill {
        SkillId::Terminator => "Terminator".to_string(),
        SkillId::Trap => "Trap Card".to_string(),
        SkillId::Queensac => "Queen Sacrifice".to_string(),
        SkillId::Remover => "Remover".to_string(),
        SkillId::Switch => "Switch".to_string(),
        other => other.to_string(),
    }
}

pub struct Level {
    pub name: &'static str,
    pub player_deck: &'static [SkillId],
    pub bot_deck: &'static [SkillId],
    pub objective: Option<Objective>,
    pub challenge: Option<Objective>,
}

pub struct Chapter {
    pub family: &'static str,
    pub name: &'static str,
    /// The six levels then the boss; empty while the chapter has no content.
    pub levels: &'static [Level],
}

impl Chapter {
    pub fn available(&self) -> bool {
        !self.levels.is_empty()
    }
}

use Objective::{KeepPiece, NoSkillUsed, UseSkill, WinWithin};
use SkillId::{Queensac, Remover, Switch, Terminator, Trap};

const ATTACK: &[Level] = &[
    Level {
        name: "Première piste",
        player_deck: &[Trap],
        bot_deck: &[],
        objective: Some(UseSkill(Trap)),
        challenge: Some(KeepPiece(PieceKind::Queen)),
    },
    Level {
        name: "Appât",
        player_deck: &[Trap, Terminator],
        bot_deck: &[Trap],
        objective: Some(UseSkill(Terminator)),
        challenge: Some(WinWithin(80)),
    },
    Level {
        name: "Sacrifice",
        player_deck: &[Queensac, Trap],
        bot_deck: &[Terminator],
        objective: Some(KeepPiece(PieceKind::Rook)),
        challenge: Some(NoSkillUsed),
    },
    Level {
        name: "Retrait",
        player_deck: &[Remover, Trap],
        bot_deck: &[Trap, Queensac],
        objective: Some(UseSkill(Remover)),
        challenge: Some(WinWithin(70)),
    },
    Level {
        name: "Renversement",
        player_deck: &[Switch, Terminator],
        bot_deck: &[Remover, Trap],
        objective: Some(UseSkill(Switch)),
        challenge: Some(KeepPiece(PieceKind::Queen)),
    },
    Level {
        name: "Tempête d'acier",
        player_deck: &[Remover, Switch, Terminator],
        bot_deck: &[Terminator, Trap, Queensac],
        objective: Some(UseSkill(Terminator)),
        challenge: Some(WinWithin(60)),
    },
    Level {
        name: "Le Stratège",
        player_deck: &[Remover, Switch, Trap],
        bot_deck: &[Terminator, Trap, Queensac],
        objective: None,
        challenge: None,
    },
];

pub const CHAPTERS: [Chapter; 5] = [
    Chapter {
        family: "attack",
        name: "Attaque",
        levels: ATTACK,
    },
    Chapter {
        family: "defense",
        name: "Défense",
        levels: &[],
    },
    Chapter {
        family: "mobility",
        name: "Mobilité",
        levels: &[],
    },
    Chapter {
        family: "control",
        name: "Contrôle",
        levels: &[],
    },
    Chapter {
        family: "create",
        name: "Création",
        levels: &[],
    },
];

/// The level behind a reference, if the chapter has content for it.
pub fn level(at: LevelRef) -> Option<&'static Level> {
    CHAPTERS
        .get(usize::from(at.chapter))?
        .levels
        .get(usize::from(at.level))
}

/// Stars mask of a finished game; objective and challenge only count on a win.
pub fn stars_earned(at: LevelRef, game: &Game, human: Color, won: bool, plies: usize) -> u8 {
    let Some(level) = level(at).filter(|_| won) else {
        return 0;
    };
    let met = |goal: Option<Objective>| goal.is_some_and(|g| g.met(game, human, plies));
    STAR_WIN
        | if met(level.objective) { STAR_OBJECTIVE } else { 0 }
        | if met(level.challenge) { STAR_CHALLENGE } else { 0 }
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
