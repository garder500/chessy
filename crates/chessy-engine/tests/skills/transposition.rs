use chessy_engine::position::WHITE_QUEEN_SIDE;

use crate::common::*;

#[test]
fn two_pieces_swap_squares() {
    let mut g = start(&[SkillId::Transposition], &[]);
    let (n, b) = (at(&g, "b1").unwrap().id, at(&g, "c1").unwrap().id);
    let ev = use_skill(&mut g, SkillId::Transposition, pair("b1", "c1"));
    assert_eq!(at(&g, "c1").unwrap().id, n);
    assert_eq!(at(&g, "b1").unwrap().id, b);
    assert_eq!(kind_at(&g, "c1"), Some((Color::White, PieceKind::Knight)));
    assert!(ev.iter().any(|e| matches!(e, Event::Swapped { .. })));
    assert_eq!(g.side_to_move(), Color::Black);
}

#[test]
fn works_across_sides_and_pairs_are_unordered() {
    let mut g = start(&[SkillId::Transposition], &[]);
    // A white rook and a black rook: the white one lands behind the black lines.
    assert!(can_skill(&g, SkillId::Transposition, pair("a8", "a1")));
    use_skill(&mut g, SkillId::Transposition, pair("a1", "a8"));
    assert_eq!(kind_at(&g, "a8"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "a1"), Some((Color::Black, PieceKind::Rook)));
}

#[test]
fn pawns_never_end_up_on_their_promotion_rank() {
    let g = start(&[SkillId::Transposition], &[]);
    assert!(can_skill(&g, SkillId::Transposition, pair("e2", "e1")));
    assert!(!can_skill(&g, SkillId::Transposition, pair("e2", "a8")));
    assert!(can_skill(&g, SkillId::Transposition, pair("a2", "a7")));
}

#[test]
fn refused_when_it_puts_either_king_in_check() {
    // The queen would land on b5 and check the king on e8.
    let g = game(
        "4k3/8/8/1n6/8/8/8/Q3K3 w - - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    assert!(!can_skill(&g, SkillId::Transposition, pair("a1", "b5")));
    // With the knight on b4 the queen only gets closer.
    let g = game(
        "4k3/8/8/8/1n6/8/8/Q3K3 w - - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    assert!(can_skill(&g, SkillId::Transposition, pair("a1", "b4")));
}

#[test]
fn refused_when_it_leaves_the_casters_own_king_in_check() {
    let g = game(
        "4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    assert!(
        !can_skill(&g, SkillId::Transposition, pair("e1", "e2")),
        "the king would step onto the open file"
    );
    // Putting the black king next to ours is check as well.
    assert!(!can_skill(&g, SkillId::Transposition, pair("e2", "g8")));
}

#[test]
fn a_king_can_change_places_and_castling_rights_follow() {
    let mut g = game(
        "7k/8/8/8/8/8/8/R3K3 w Q - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    assert_ne!(g.pos.castling & WHITE_QUEEN_SIDE, 0);
    use_skill(&mut g, SkillId::Transposition, pair("a1", "e1"));
    assert_eq!(kind_at(&g, "a1"), Some((Color::White, PieceKind::King)));
    assert_eq!(g.pos.castling, 0);
}

#[test]
fn both_pieces_forget_their_last_move() {
    let mut g = start(&[SkillId::Transposition], &[]);
    mv(&mut g, "b1", "c3");
    mv(&mut g, "b8", "c6");
    use_skill(&mut g, SkillId::Transposition, pair("c3", "g1"));
    assert_eq!(at(&g, "g1").unwrap().prev, None);
    assert_eq!(at(&g, "c3").unwrap().prev, None);
}
