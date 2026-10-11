use chessy_engine::{Color, PieceKind, SkillId, SkillId::*};

use super::{lending, starting, Level};
use crate::campaign::Objective::{KeepPiece, NoSkillUsed, UseAllSkills, UseAnySkill, WinWithin};

const LENT: &[SkillId] = &[Teleportation, Transposition, Bench];
const PASSEUR_START: &str = "rnbqkbnr/pppppppp/4b3/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub(super) const LEVELS: &[Level] = &[
    lending(
        "Premier pas",
        LENT,
        &[Rollback],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
        "Teleportation envoie une pièce sur une case libre : sortez une pièce du danger ou approchez-la du roi adverse.",
    ),
    lending(
        "Détour",
        LENT,
        &[Teleportation],
        UseAnySkill,
        WinWithin(36),
        "Contournez les défenses de Sage : une pièce qui apparaît derrière ses pions gagne plusieurs coups.",
    ),
    lending(
        "Faille",
        LENT,
        &[Temporal, DestinySwapper],
        UseAnySkill,
        KeepPiece(PieceKind::Rook),
        "Transposition échange deux de vos pièces : utilisez-la pour mettre une pièce forte là où elle frappe.",
    ),
    lending(
        "Écho",
        LENT,
        &[Rollback, Bench],
        UseAllSkills,
        WinWithin(34),
        "Il faut utiliser toutes les compétences que vous emmenez : n'en choisissez que deux si vous voulez la jouer simple.",
    ),
    lending(
        "Sables du temps",
        LENT,
        &[Teleportation, Rollback, Temporal],
        UseAllSkills,
        KeepPiece(PieceKind::Queen),
        "The Bench met une pièce à l'abri sur le banc : sortez-la quand l'attaque de Sage est passée.",
    ),
    lending(
        "Au-delà du voile",
        LENT,
        &[DestinySwapper, Transposition, Temporal],
        WinWithin(30),
        NoSkillUsed,
        "Pour le défi, jouez sans compétence ; pour mater vite, appuyez-vous sur les échanges de pièces.",
    ),
    starting(
        lending(
            "Le Passeur",
            LENT,
            &[Teleportation, DestinySwapper, Rollback],
            UseAllSkills,
            WinWithin(36),
            "Le Passeur déplace et inverse vos pièces : gardez votre roi à l'abri et laissez-le s'user avant de contre-attaquer.",
        ),
        PASSEUR_START,
        Color::White,
    ),
];
