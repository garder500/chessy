use chessy_engine::{Color, PieceKind, SkillId::*};

use super::{imposed, starting, Level};
use crate::campaign::Objective::{KeepPiece, NoPieceLostBefore, UseAllSkills, UseSkill, WinWithin};

const WALL_START: &str = "rnbqkbnr/pppppppp/2p2p2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub(super) const LEVELS: &[Level] = &[
    imposed(
        "Premier rempart",
        &[Imune],
        &[Celestial],
        UseSkill(Imune),
        KeepPiece(PieceKind::Queen),
        "Imune protège une pièce de toute prise : choisissez celle que Sage convoite le plus.",
    ),
    imposed(
        "Voile d'ombre",
        &[Imune, Invisibility],
        &[Imune],
        UseSkill(Invisibility),
        WinWithin(35),
        "Une pièce invisible reste hors de vue de Sage : avancez-la pour préparer une attaque surprise.",
    ),
    imposed(
        "Bouclier d'énergie",
        &[Forcefield, Imune],
        &[Celestial, Invisibility],
        UseSkill(Forcefield),
        KeepPiece(PieceKind::Rook),
        "Force Field repousse celui qui prend la pièce protégée : posez-le sur une pièce que Sage voudra attaquer.",
    ),
    imposed(
        "Grâce céleste",
        &[Invisibility, Forcefield],
        &[Imune, Forcefield],
        UseAllSkills,
        NoPieceLostBefore(20),
        "Cachez une pièce, protégez-en une autre, et ne laissez à Sage aucune prise facile avant le coup 20.",
    ),
    imposed(
        "Mur de brume",
        &[Invisibility, Forcefield, Imune],
        &[Forcefield, Celestial, Invisibility],
        UseSkill(Imune),
        WinWithin(32),
        "Imune sauve une pièce clouée : gardez-la pour le moment où Sage prépare une prise.",
    ),
    imposed(
        "Citadelle",
        &[Invisibility, Forcefield, Imune],
        &[Imune, Celestial, Forcefield],
        UseAllSkills,
        KeepPiece(PieceKind::Queen),
        "Posez vos trois protections tôt, puis avancez à l'abri : la dame doit rester en vie.",
    ),
    starting(
        imposed(
            "La Muraille",
            &[Forcefield, Imune, Invisibility],
            &[Forcefield, Imune, Celestial],
            UseAllSkills,
            NoPieceLostBefore(20),
            "Sa Force Field protège une seule pièce et repousse celui qui la prend : prenez-la avec un pion, ou attaquez une autre cible.",
        ),
        WALL_START,
        Color::White,
    ),
];
