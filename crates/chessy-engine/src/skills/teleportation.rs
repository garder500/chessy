use super::{Skill, SkillId, SkillTarget};
use crate::position::Position;
use crate::types::{Color, Event, PieceKind};

/// Moves one of your pieces (not the king) to any empty square, ignoring obstacles.
pub struct Teleportation;

impl Skill for Teleportation {
    fn id(&self) -> SkillId {
        SkillId::Teleportation
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        let mut out = Vec::new();
        for (from, piece) in pos.pieces(color).filter(|(_, p)| p.kind != PieceKind::King) {
            if pos.is_frozen(piece.id) {
                continue;
            }
            for to in 0..64u8 {
                if pos.can_place(color, piece.kind, to) {
                    out.push(SkillTarget::PieceTo { from, to });
                }
            }
        }
        out
    }

    fn apply(&self, pos: &mut Position, _color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::PieceTo { from, to } = target else {
            return;
        };
        let mut piece = pos.board[from as usize].take().expect("teleport source");
        piece.prev = None;
        pos.board[to as usize] = Some(piece);
        ev.push(Event::Teleported { from, to });
    }
}
