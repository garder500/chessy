use super::{Skill, SkillId, SkillTarget};
use crate::position::Position;
use crate::types::*;

/// Two pieces, of any side, swap squares. Refused if either king ends up in check.
pub struct Transposition;

impl Skill for Transposition {
    fn id(&self) -> SkillId {
        SkillId::Transposition
    }

    fn forbids_check(&self) -> bool {
        true
    }

    fn targets(&self, pos: &Position, _color: Color) -> Vec<SkillTarget> {
        let all: Vec<(Square, Piece)> = pos
            .board
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.map(|p| (i as Square, p)))
            .collect();
        let mut out = Vec::new();
        for (i, &(a, pa)) in all.iter().enumerate() {
            for &(b, pb) in &all[i + 1..] {
                if Position::can_stand(pa.color, pa.kind, b) && Position::can_stand(pb.color, pb.kind, a) {
                    out.push(SkillTarget::Pair { a, b });
                }
            }
        }
        out
    }

    fn apply(&self, pos: &mut Position, _color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::Pair { a, b } = target else {
            return;
        };
        pos.board.swap(a as usize, b as usize);
        for s in [a, b] {
            if let Some(p) = pos.board[s as usize].as_mut() {
                p.prev = None;
            }
        }
        ev.push(Event::Swapped { a, b });
    }
}
