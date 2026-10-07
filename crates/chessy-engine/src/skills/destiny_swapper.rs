use super::{Skill, SkillId, SkillTarget};
use crate::position::Position;
use crate::types::{Color, Event};

/// Swaps the positions of two of your pieces.
pub struct DestinySwapper;

impl Skill for DestinySwapper {
    fn id(&self) -> SkillId {
        SkillId::DestinySwapper
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        let own: Vec<_> = pos
            .pieces(color)
            .filter(|(_, p)| !pos.is_frozen(p.id))
            .collect();
        let mut out = Vec::new();
        for (i, &(a, pa)) in own.iter().enumerate() {
            for &(b, pb) in &own[i + 1..] {
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
