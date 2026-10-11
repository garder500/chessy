use crate::common::*;

#[test]
fn teleportation_never_moves_the_king() {
    let g = game(
        "4k3/8/8/8/8/8/8/R3K3 w - - 0 1",
        &[SkillId::Teleportation],
        &[],
    );
    assert!(!targets_of(&g, SkillId::Teleportation)
        .iter()
        .any(|t| matches!(t, SkillTarget::PieceTo { from, .. } if *from == s("e1"))));
    assert!(can_skill(&g, SkillId::Teleportation, piece_to("a1", "h8")));
    assert!(!can_skill(&g, SkillId::Teleportation, piece_to("e1", "d1")));
}
