//! The level tables of the five chapters (see `docs/spec-campagne.md`).

use chessy_engine::{Color, PieceKind, SkillId};

use super::{Chapter, Level, Objective, Start};
use Objective::{
    KeepPiece, NoPieceLostBefore, NoSkillUsed, UseAllSkills, UseAnySkill, UseSkill, WinWithin,
};
use SkillId::*;

const fn imposed(
    name: &'static str,
    player_deck: &'static [SkillId],
    bot_deck: &'static [SkillId],
    objective: Objective,
    challenge: Objective,
) -> Level {
    Level {
        name,
        player_deck,
        bot_deck,
        deck_choice: false,
        start: None,
        objective: Some(objective),
        challenge: Some(challenge),
        hint: "",
        lent: &[],
    }
}

const fn chosen(
    name: &'static str,
    bot_deck: &'static [SkillId],
    objective: Objective,
    challenge: Objective,
) -> Level {
    Level {
        name,
        player_deck: &[],
        bot_deck,
        deck_choice: true,
        start: None,
        objective: Some(objective),
        challenge: Some(challenge),
        hint: "",
        lent: &[],
    }
}

const fn boss(
    name: &'static str,
    player_deck: &'static [SkillId],
    bot_deck: &'static [SkillId],
    start: Option<Start>,
    objective: Objective,
    challenge: Objective,
) -> Level {
    Level {
        name,
        player_deck,
        bot_deck,
        deck_choice: player_deck.is_empty(),
        start,
        objective: Some(objective),
        challenge: Some(challenge),
        hint: "",
        lent: &[],
    }
}

const fn white_start(fen: &'static str) -> Option<Start> {
    Some(Start {
        fen,
        human: Color::White,
    })
}

const ATTACK: &[Level] = &[
    imposed(
        "Première piste",
        &[Trap],
        &[],
        UseSkill(Trap),
        KeepPiece(PieceKind::Queen),
    ),
    imposed(
        "Appât",
        &[Trap, Terminator],
        &[Trap],
        UseSkill(Terminator),
        WinWithin(41),
    ),
    imposed(
        "Sacrifice",
        &[Queensac, Trap],
        &[Terminator],
        KeepPiece(PieceKind::Rook),
        NoSkillUsed,
    ),
    imposed(
        "Retrait",
        &[Remover, Trap],
        &[Trap, Queensac],
        UseSkill(Remover),
        WinWithin(36),
    ),
    imposed(
        "Renversement",
        &[Switch, Terminator],
        &[Remover, Trap],
        UseSkill(Switch),
        KeepPiece(PieceKind::Queen),
    ),
    imposed(
        "Tempête d'acier",
        &[Remover, Switch, Terminator],
        &[Terminator, Trap, Queensac],
        UseSkill(Terminator),
        WinWithin(31),
    ),
    boss(
        "Le Stratège",
        &[Remover, Switch, Trap],
        &[Terminator, Trap, Queensac],
        None,
        UseAllSkills,
        NoPieceLostBefore(20),
    ),
];

const DEFENSE: &[Level] = &[
    imposed(
        "Premier rempart",
        &[Imune],
        &[Trap],
        UseSkill(Imune),
        KeepPiece(PieceKind::Queen),
    ),
    imposed(
        "Voile d'ombre",
        &[Invisibility, Imune],
        &[Terminator],
        UseSkill(Invisibility),
        WinWithin(46),
    ),
    imposed(
        "Bouclier d'énergie",
        &[Forcefield, Imune],
        &[Trap, Remover],
        KeepPiece(PieceKind::Rook),
        NoSkillUsed,
    ),
    imposed(
        "Grâce céleste",
        &[Celestial, Forcefield],
        &[Queensac, Trap],
        UseSkill(Celestial),
        WinWithin(41),
    ),
    imposed(
        "Mur de brume",
        &[Invisibility, Forcefield, Imune],
        &[Remover, Trap, Switch],
        UseSkill(Forcefield),
        KeepPiece(PieceKind::Queen),
    ),
    imposed(
        "Citadelle",
        &[Celestial, Imune, Invisibility],
        &[Terminator, Trap, Queensac],
        UseSkill(Celestial),
        WinWithin(31),
    ),
    boss(
        "Le Gardien",
        &[Celestial, Forcefield, Invisibility],
        &[Terminator, Remover, Switch],
        white_start("rnbqkbnr/pppppppp/2p2p2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        KeepPiece(PieceKind::Queen),
        WinWithin(41),
    ),
];

const MOBILITY: &[Level] = &[
    chosen(
        "Premier pas",
        &[Teleportation],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Détour",
        &[Rollback],
        KeepPiece(PieceKind::Rook),
        WinWithin(46),
    ),
    chosen(
        "Faille",
        &[Transposition, Bench],
        UseAnySkill,
        WinWithin(43),
    ),
    chosen(
        "Écho",
        &[DestinySwapper, Rollback],
        KeepPiece(PieceKind::Rook),
        WinWithin(41),
    ),
    chosen(
        "Sables du temps",
        &[Temporal, Teleportation, Bench],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Au-delà du voile",
        &[Teleportation, Rollback, Temporal],
        NoSkillUsed,
        WinWithin(31),
    ),
    boss(
        "Le Passeur",
        &[],
        &[Teleportation, DestinySwapper, Transposition],
        white_start("rnbqkbnr/pppppppp/4b3/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        UseAnySkill,
        WinWithin(36),
    ),
];

const CONTROL: &[Level] = &[
    chosen(
        "Premier signe",
        &[Canceller],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Vent contraire",
        &[Tornado],
        KeepPiece(PieceKind::Rook),
        WinWithin(46),
    ),
    chosen("Givre", &[Freeze, Canceller], UseAnySkill, WinWithin(43)),
    chosen(
        "Fracture",
        &[Tornado, Geomancy],
        KeepPiece(PieceKind::Rook),
        WinWithin(41),
    ),
    chosen(
        "Œil du cyclone",
        &[Freeze, Tornado, Canceller],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Maîtrise du terrain",
        &[Geomancy, Freeze, Tornado],
        NoSkillUsed,
        WinWithin(31),
    ),
    boss(
        "Le Métronome",
        &[],
        &[Canceller, Tornado, Freeze],
        white_start("rnbqkbnr/pppppppp/5n2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        KeepPiece(PieceKind::Rook),
        WinWithin(36),
    ),
];

const CREATE: &[Level] = &[
    chosen(
        "Première pierre",
        &[Wall],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Double",
        &[Clone],
        KeepPiece(PieceKind::Rook),
        WinWithin(46),
    ),
    chosen("Mirage", &[Mirage, Wall], UseAnySkill, WinWithin(43)),
    chosen(
        "Métamorphose",
        &[Morph, Evolve],
        KeepPiece(PieceKind::Rook),
        WinWithin(41),
    ),
    chosen(
        "Main divine",
        &[Clone, Godhelp, Wall],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
    ),
    chosen(
        "Chef-d'œuvre",
        &[Evolve, Mirage, Morph],
        NoSkillUsed,
        WinWithin(31),
    ),
    boss(
        "L'Architecte",
        &[],
        &[Clone, Godhelp, Morph],
        Some(Start {
            fen: "rnbqkbnr/pppppppp/8/8/8/2P2P2/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            human: Color::Black,
        }),
        UseAnySkill,
        KeepPiece(PieceKind::Rook),
    ),
];

pub const CHAPTERS: [Chapter; 5] = [
    Chapter {
        family: "attack",
        name: "Attaque",
        title: "Tombeur du Bélier",
        levels: ATTACK,
    },
    Chapter {
        family: "defense",
        name: "Défense",
        title: "Briseur de Muraille",
        levels: DEFENSE,
    },
    Chapter {
        family: "mobility",
        name: "Mobilité",
        title: "Maître des Routes",
        levels: MOBILITY,
    },
    Chapter {
        family: "control",
        name: "Contrôle",
        title: "Démasqueur",
        levels: CONTROL,
    },
    Chapter {
        family: "create",
        name: "Création",
        title: "Maître de forge",
        levels: CREATE,
    },
];
