use crate::common::*;

#[test]
fn a_piece_goes_back_to_the_square_it_came_from() {
    let mut g = start(&[SkillId::Temporal], &[]);
    assert!(!has_skill(&g, SkillId::Temporal), "nothing has moved yet");
    mv(&mut g, "e2", "e4");
    mv(&mut g, "a7", "a6");
    assert!(can_skill(&g, SkillId::Temporal, piece("e4")));
    let ev = use_skill(&mut g, SkillId::Temporal, piece("e4"));
    assert_eq!(kind_at(&g, "e2"), Some((Color::White, PieceKind::Pawn)));
    assert_eq!(kind_at(&g, "e4"), None);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Moved { from, to, .. } if *from == s("e4") && *to == s("e2")
    )));
    assert_eq!(g.side_to_move(), Color::Black, "it replaces the move");
}

#[test]
fn needs_a_last_move_and_a_free_or_capturable_square() {
    let g = game("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[SkillId::Temporal], &[]);
    assert!(!has_skill(&g, SkillId::Temporal));
    // An own piece sits on the old square.
    let mut g = game("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[SkillId::Temporal, SkillId::Temporal], &[]);
    mv(&mut g, "a1", "a3");
    mv(&mut g, "e8", "d8");
    mv(&mut g, "e1", "e2");
    mv(&mut g, "d8", "e8");
    assert!(can_skill(&g, SkillId::Temporal, piece("a3")));
}

#[test]
fn never_the_king() {
    let mut g = game("4k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Temporal], &[]);
    mv(&mut g, "e1", "e2");
    mv(&mut g, "e8", "e7");
    assert!(!has_skill(&g, SkillId::Temporal));
}

#[test]
fn a_slider_cannot_jump_over_a_piece_on_the_way_back() {
    let mut g = game(
        "4k2n/7p/8/8/8/8/8/R3K3 w - - 0 1",
        &[SkillId::Temporal],
        &[SkillId::Teleportation],
    );
    mv(&mut g, "a1", "a4");
    use_skill(&mut g, SkillId::Teleportation, piece_to("h8", "a2"));
    assert!(!can_skill(&g, SkillId::Temporal, piece("a4")), "a2 is in the way");
}

#[test]
fn counts_as_a_real_move() {
    let mut g = game(
        "4k3/8/8/8/8/8/8/R3K3 w - - 0 1",
        &[SkillId::Temporal],
        &[],
    );
    mv(&mut g, "a1", "a3");
    mv(&mut g, "e8", "d8");
    use_skill(&mut g, SkillId::Temporal, piece("a3"));
    assert_eq!(kind_at(&g, "a1"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(g.pos.ply, 3, "one turn, not two");
}
