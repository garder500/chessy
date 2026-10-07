use crate::common::*;

#[test]
fn an_enemy_piece_changes_type_for_one_enemy_turn() {
    let mut g = game("1n2k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    let ev = use_skill(&mut g, SkillId::Morph, spawn("b8", PieceKind::Rook));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Transformed { kind, .. } if *kind == PieceKind::Rook
    )));
    assert_eq!(kind_at(&g, "b8"), Some((Color::Black, PieceKind::Rook)));
    // Black's turn: it moves like a rook, not like a knight.
    let moves = moves_from(&g, "b8");
    assert!(moves.contains(&"b1".to_string()));
    assert!(!moves.contains(&"c6".to_string()));
    // ... and goes back to a knight once black has played, wherever it is.
    mv(&mut g, "b8", "b2");
    assert_eq!(kind_at(&g, "b2"), Some((Color::Black, PieceKind::Knight)));
    assert!(g.pos.effects.is_empty());
}

#[test]
fn one_of_your_own_pieces_keeps_the_new_type_a_little_longer() {
    let mut g = game("4k3/8/8/8/8/8/P7/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("a2", PieceKind::Queen));
    mv(&mut g, "e8", "d8");
    assert_eq!(kind_at(&g, "a2"), Some((Color::White, PieceKind::Queen)));
    assert!(
        can_move(&g, "a2", "a8"),
        "still a queen on white's next turn"
    );
    mv(&mut g, "e1", "d1");
    mv(&mut g, "d8", "e8");
    assert_eq!(kind_at(&g, "a2"), Some((Color::White, PieceKind::Pawn)));
}

#[test]
fn only_other_types_and_never_a_king() {
    let g = game("r3k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    let targets = targets_of(&g, SkillId::Morph);
    assert!(
        !targets.contains(&spawn("a8", PieceKind::Rook)),
        "same type"
    );
    assert!(
        targets.contains(&spawn("a8", PieceKind::Pawn)),
        "a black pawn may rest on its own back rank"
    );
    assert!(targets.contains(&spawn("a8", PieceKind::Queen)));
    assert!(targets
        .iter()
        .all(|t| !matches!(t, SkillTarget::Spawn { square, .. } if *square == s("e1") || *square == s("e8"))));
    assert!(!targets.iter().any(|t| matches!(
        t,
        SkillTarget::Spawn {
            kind: PieceKind::King,
            ..
        }
    )));
}

#[test]
fn a_pawn_that_would_return_on_a_back_rank_is_promoted() {
    let mut g = game("8/P7/8/7k/8/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("a7", PieceKind::Rook));
    mv(&mut g, "h5", "h4");
    mv(&mut g, "a7", "a8");
    assert_eq!(kind_at(&g, "a8"), Some((Color::White, PieceKind::Rook)));
    mv(&mut g, "h4", "h3");
    assert_eq!(kind_at(&g, "a8"), Some((Color::White, PieceKind::Queen)));
}

#[test]
fn morphing_twice_still_goes_back_to_the_original() {
    let mut g = game(
        "4k3/8/8/8/8/8/P7/4K3 w - - 0 1",
        &[SkillId::Morph],
        &[SkillId::Morph, SkillId::Freeze],
    );
    use_skill(&mut g, SkillId::Morph, spawn("a2", PieceKind::Knight));
    use_skill(&mut g, SkillId::Morph, spawn("a2", PieceKind::Bishop));
    assert_eq!(kind_at(&g, "a2"), Some((Color::White, PieceKind::Bishop)));
    mv(&mut g, "e1", "d1");
    assert_eq!(kind_at(&g, "a2"), Some((Color::White, PieceKind::Pawn)));
}

#[test]
fn the_graveyard_remembers_what_the_piece_really_was() {
    // A pawn morphed into a knight and taken is a lost pawn.
    let mut g = game("4k3/8/8/3b4/8/8/P7/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("a2", PieceKind::Knight));
    mv(&mut g, "d5", "a2");
    assert_eq!(g.pos.captured_pawns, [1, 0]);

    // A knight morphed into a pawn and taken is not.
    let mut g = game("4k3/8/8/3b4/8/8/N7/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("a2", PieceKind::Pawn));
    mv(&mut g, "d5", "a2");
    assert_eq!(g.pos.captured_pawns, [0, 0]);
}

#[test]
fn can_defuse_a_check() {
    let g = game("6k1/8/8/4r3/8/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    assert!(can_skill(&g, SkillId::Morph, spawn("e5", PieceKind::Pawn)));
    assert!(can_skill(
        &g,
        SkillId::Morph,
        spawn("e5", PieceKind::Knight)
    ));
    assert!(
        !can_skill(&g, SkillId::Morph, spawn("e5", PieceKind::Queen)),
        "still check"
    );
}
