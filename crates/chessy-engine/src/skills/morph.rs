use super::{Skill, SkillId, SkillTarget};
use crate::position::Position;
use crate::types::*;

/// Turns a piece (not a king) into another type for a while: until the opponent
/// has played once if it is an enemy, a little longer if it is yours.
pub struct Morph;

const KINDS: [PieceKind; 5] = [
    PieceKind::Pawn,
    PieceKind::Knight,
    PieceKind::Bishop,
    PieceKind::Rook,
    PieceKind::Queen,
];

impl Skill for Morph {
    fn id(&self) -> SkillId {
        SkillId::Morph
    }

    fn targets(&self, pos: &Position, _color: Color) -> Vec<SkillTarget> {
        let mut out = Vec::new();
        for (i, slot) in pos.board.iter().enumerate() {
            let Some(piece) = slot else { continue };
            if piece.kind == PieceKind::King {
                continue;
            }
            let square = i as Square;
            for kind in KINDS {
                if kind != piece.kind && Position::can_stand(piece.color, kind, square) {
                    out.push(SkillTarget::Spawn { square, kind });
                }
            }
        }
        out
    }

    fn apply(&self, pos: &mut Position, color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::Spawn { square, kind } = target else {
            return;
        };
        let piece = pos.board[square as usize].as_mut().expect("morph target");
        let id = piece.id;
        let mut orig = piece.kind;
        let enemy = piece.color != color;
        piece.kind = kind;
        // Morphing a morphed piece keeps the type it will finally go back to.
        if let Some(old) = pos
            .effects
            .iter()
            .find(|e| e.piece == id && e.kind == EffectKind::Morphed)
        {
            orig = old.orig_kind.unwrap_or(orig);
        }
        pos.effects
            .retain(|e| !(e.piece == id && e.kind == EffectKind::Morphed));
        if kind != orig {
            let mut morphed =
                ActiveEffect::new(EffectKind::Morphed, id, pos.ply + if enemy { 2 } else { 4 });
            morphed.orig_kind = Some(orig);
            pos.effects.push(morphed);
        }
        ev.push(Event::Transformed { square, kind });
    }
}
