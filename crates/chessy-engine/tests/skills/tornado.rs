use crate::common::*;

fn rotated(ev: &[Event]) -> Vec<(String, String)> {
    ev.iter()
        .find_map(|e| match e {
            Event::Rotated { moves } => Some(
                moves
                    .iter()
                    .map(|m| (name(m.from), name(m.to)))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .expect("a rotation")
}

#[test]
fn pieces_hop_to_the_next_piece_counterclockwise() {
    // e5, d5, d4, e4 in counterclockwise order from the east.
    let mut g = game(
        "k7/8/8/3bR3/3Nr3/8/8/7K w - - 0 1",
        &[SkillId::Tornado],
        &[],
    );
    let ids: Vec<PieceId> = ["e5", "d5", "d4", "e4"]
        .iter()
        .map(|q| at(&g, q).unwrap().id)
        .collect();
    let ev = use_skill(&mut g, SkillId::Tornado, none());
    assert_eq!(kind_at(&g, "d5"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "d4"), Some((Color::Black, PieceKind::Bishop)));
    assert_eq!(kind_at(&g, "e4"), Some((Color::White, PieceKind::Knight)));
    assert_eq!(kind_at(&g, "e5"), Some((Color::Black, PieceKind::Rook)));
    assert_eq!(
        at(&g, "d5").unwrap().id,
        ids[0],
        "pieces keep their identity"
    );
    assert_eq!(at(&g, "e5").unwrap().id, ids[3]);
    assert_eq!(
        rotated(&ev),
        [("e5", "d5"), ("d5", "d4"), ("d4", "e4"), ("e4", "e5")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn kings_stay_put() {
    let mut g = game(
        "k7/8/8/3bR3/3Nr3/8/8/7K w - - 0 1",
        &[SkillId::Tornado],
        &[],
    );
    use_skill(&mut g, SkillId::Tornado, none());
    assert_eq!(kind_at(&g, "a8"), Some((Color::Black, PieceKind::King)));
    assert_eq!(kind_at(&g, "h1"), Some((Color::White, PieceKind::King)));
}

#[test]
fn needs_at_least_two_pieces_to_move() {
    let g = game(KINGS, &[SkillId::Tornado], &[]);
    assert!(!has_skill(&g, SkillId::Tornado));
    let g = game("4k3/8/8/8/8/8/4N3/4K3 w - - 0 1", &[SkillId::Tornado], &[]);
    assert!(
        !has_skill(&g, SkillId::Tornado),
        "one piece is nothing to rotate"
    );
}

#[test]
fn a_pawn_on_its_own_back_rank_is_not_promoted() {
    // d8 -> c7 -> a1: the white pawn lands on a1, its own back rank.
    let mut g = game("3r4/2P4k/8/7R/8/8/8/4K3 w - - 0 1", &[SkillId::Tornado], &[]);
    let ev = use_skill(&mut g, SkillId::Tornado, none());
    assert!(!ev.iter().any(|e| matches!(e, Event::Promoted { .. })));
    assert_eq!(kind_at(&g, "d8"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "c7"), Some((Color::Black, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "h5"), Some((Color::White, PieceKind::Pawn)));
}

#[test]
fn a_pawn_never_lands_on_its_promotion_rank() {
    // The white pawn would land on d8, white's promotion rank: it trades its
    // destination with a piece that can stand there.
    let mut g = game("3r4/4P3/7k/8/8/1R6/8/K7 w - - 0 1", &[SkillId::Tornado], &[]);
    let ev = use_skill(&mut g, SkillId::Tornado, none());
    assert!(!ev.iter().any(|e| matches!(e, Event::Promoted { .. })));
    assert_ne!(kind_at(&g, "d8"), Some((Color::White, PieceKind::Pawn)));
    let pawns = (0..64u8)
        .filter(|&q| g.pos.board[q as usize].is_some_and(|p| p.kind == PieceKind::Pawn))
        .count();
    assert_eq!(pawns, 1, "still a pawn");
}

#[test]
fn collinear_pieces_are_ordered_by_square() {
    let mut g = game(
        "k7/6N1/5N2/4N3/8/8/8/K7 w - - 0 1",
        &[SkillId::Tornado],
        &[],
    );
    let ev = use_skill(&mut g, SkillId::Tornado, none());
    assert_eq!(
        rotated(&ev),
        [("e5", "f6"), ("f6", "g7"), ("g7", "e5")].map(|(a, b)| (a.to_string(), b.to_string()))
    );
}

#[test]
fn is_refused_when_it_would_leave_the_casters_king_in_check() {
    let g = game("4r2k/8/8/8/8/4R3/8/4K3 w - - 0 1", &[SkillId::Tornado], &[]);
    assert!(
        !has_skill(&g, SkillId::Tornado),
        "the rooks would swap, exposing e1"
    );
}

#[test]
fn does_not_leave_castling_rights_behind() {
    let mut g = game("4k3/8/8/8/3N4/8/8/R3K3 w Q - 0 1", &[SkillId::Tornado], &[]);
    assert_ne!(g.pos.castling, 0);
    use_skill(&mut g, SkillId::Tornado, none());
    assert_eq!(kind_at(&g, "d4"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(g.pos.castling, 0, "the rook left its corner");
}
