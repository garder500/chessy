use chessy_engine::*;

fn s(name: &str) -> Square {
    parse_square(name).unwrap_or_else(|| panic!("bad square {name}"))
}

fn mv(g: &mut Game, from: &str, to: &str) {
    g.apply(Action::Move {
        from: s(from),
        to: s(to),
        promo: None,
    })
    .unwrap_or_else(|e| panic!("{from}{to}: {e}"));
}

fn game_from(fen: &str, white: &[SkillId], black: &[SkillId]) -> Game {
    Game::from_position(Position::from_fen(fen).unwrap(), white, black)
}

fn can_move(g: &Game, from: &str, to: &str) -> bool {
    g.legal_actions()
        .iter()
        .any(|a| matches!(a, Action::Move { from: f, to: t, .. } if *f == s(from) && *t == s(to)))
}

fn can_skill(g: &Game, id: SkillId, target: SkillTarget) -> bool {
    g.legal_actions()
        .iter()
        .any(|a| matches!(a, Action::Skill { skill, target: t } if *skill == id && *t == target))
}

fn piece(sqr: &str) -> SkillTarget {
    SkillTarget::Piece { square: s(sqr) }
}

fn piece_to(from: &str, to: &str) -> SkillTarget {
    SkillTarget::PieceTo {
        from: s(from),
        to: s(to),
    }
}

fn use_skill(g: &mut Game, id: SkillId, target: SkillTarget) {
    g.apply(Action::Skill { skill: id, target })
        .unwrap_or_else(|e| panic!("{id:?}: {e}"));
}

fn has_piece_moves(g: &Game, from: &str) -> bool {
    g.legal_actions()
        .iter()
        .any(|a| matches!(a, Action::Move { from: f, .. } if *f == s(from)))
}

#[test]
fn fools_mate_ends_the_game() {
    let mut g = Game::new(&[], &[]);
    mv(&mut g, "f2", "f3");
    mv(&mut g, "e7", "e5");
    mv(&mut g, "g2", "g4");
    assert_eq!(g.outcome(), Outcome::Ongoing);
    mv(&mut g, "d8", "h4");
    assert_eq!(
        g.outcome(),
        Outcome::Checkmate {
            winner: Color::Black
        }
    );
    assert_eq!(
        g.apply(Action::Move {
            from: s("a2"),
            to: s("a3"),
            promo: None
        }),
        Err(RuleError::GameOver)
    );
}

#[test]
fn illegal_moves_are_rejected() {
    let mut g = Game::new(&[], &[]);
    assert_eq!(
        g.apply(Action::Move {
            from: s("e2"),
            to: s("e5"),
            promo: None
        }),
        Err(RuleError::IllegalAction)
    );
    assert_eq!(g.side_to_move(), Color::White);
}

#[test]
fn stalemate_and_insufficient_material() {
    let g = game_from("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1", &[], &[]);
    assert_eq!(g.outcome(), Outcome::Stalemate);
    let g = game_from("4k3/8/8/8/8/8/8/4K3 w - - 0 1", &[], &[]);
    assert_eq!(g.outcome(), Outcome::InsufficientMaterial);
}

#[test]
fn threefold_repetition() {
    let mut g = Game::new(&[], &[]);
    for _ in 0..2 {
        mv(&mut g, "g1", "f3");
        mv(&mut g, "g8", "f6");
        mv(&mut g, "f3", "g1");
        mv(&mut g, "f6", "g8");
    }
    assert_eq!(g.outcome(), Outcome::Repetition);
}

#[test]
fn resignation() {
    let mut g = Game::new(&[], &[]);
    g.resign(Color::White);
    assert_eq!(
        g.outcome(),
        Outcome::Resignation {
            winner: Color::Black
        }
    );
}

#[test]
fn timeout_and_agreed_draw() {
    let mut g = Game::new(&[], &[]);
    g.flag(Color::Black);
    assert_eq!(
        g.outcome(),
        Outcome::Timeout {
            winner: Color::White
        }
    );
    assert_eq!(g.outcome().winner(), Some(Color::White));
    // A finished game stays finished.
    g.agree_draw();
    g.resign(Color::White);
    assert!(matches!(g.outcome(), Outcome::Timeout { .. }));
    assert_eq!(
        g.apply(Action::Move {
            from: s("e2"),
            to: s("e4"),
            promo: None
        }),
        Err(RuleError::GameOver)
    );

    let mut d = Game::new(&[], &[]);
    d.agree_draw();
    assert_eq!(d.outcome(), Outcome::DrawAgreed);
    assert!(d.outcome().is_over());
    assert_eq!(d.outcome().winner(), None);
    assert_eq!(
        serde_json::to_value(d.outcome()).unwrap(),
        serde_json::json!({"type": "draw_agreed"})
    );
    assert_eq!(
        serde_json::to_value(g.outcome()).unwrap(),
        serde_json::json!({"type": "timeout", "winner": "white"})
    );
}

#[test]
fn promotion_requires_a_piece_choice() {
    let mut g = game_from("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", &[], &[]);
    assert!(g
        .apply(Action::Move {
            from: s("a7"),
            to: s("a8"),
            promo: None
        })
        .is_err());
    g.apply(Action::Move {
        from: s("a7"),
        to: s("a8"),
        promo: Some(PieceKind::Knight),
    })
    .unwrap();
    assert_eq!(g.pos.piece_at(s("a8")).unwrap().kind, PieceKind::Knight);
}

#[test]
fn skill_kinds() {
    const UNIQUE: [SkillId; 7] = [
        SkillId::Remover,
        SkillId::Wall,
        SkillId::Mirage,
        SkillId::Evolve,
        SkillId::Switch,
        SkillId::Mind,
        SkillId::Control,
    ];
    assert_eq!(SkillId::ALL.len(), 27);
    for id in SkillId::ALL {
        let expected = if UNIQUE.contains(&id) {
            SkillKind::Unique
        } else {
            SkillKind::Classic
        };
        assert_eq!(id.kind(), expected, "{id:?}");
    }
}

#[test]
fn skills_are_once_per_game_and_cost_the_turn() {
    let mut g = Game::new(&[SkillId::Teleportation], &[]);
    use_skill(&mut g, SkillId::Teleportation, piece_to("a1", "e4"));
    assert_eq!(g.side_to_move(), Color::Black);
    mv(&mut g, "a7", "a6");
    assert!(!g
        .legal_actions()
        .iter()
        .any(|a| matches!(a, Action::Skill { .. })));
    assert_eq!(
        g.apply(Action::Skill {
            skill: SkillId::Teleportation,
            target: piece_to("b1", "e5")
        }),
        Err(RuleError::IllegalAction)
    );
}

#[test]
fn unowned_skills_are_rejected() {
    let mut g = Game::new(&[SkillId::Freeze], &[]);
    assert_eq!(
        g.apply(Action::Skill {
            skill: SkillId::Teleportation,
            target: piece_to("a1", "e4")
        }),
        Err(RuleError::IllegalAction)
    );
}

#[test]
fn teleportation_moves_a_piece_anywhere_empty() {
    let mut g = Game::new(&[SkillId::Teleportation], &[]);
    assert!(can_skill(&g, SkillId::Teleportation, piece_to("a1", "e4")));
    assert!(
        !can_skill(&g, SkillId::Teleportation, piece_to("a1", "e2")),
        "occupied target"
    );
    assert!(
        !can_skill(&g, SkillId::Teleportation, piece_to("a2", "a8")),
        "pawn on a back rank"
    );
    use_skill(&mut g, SkillId::Teleportation, piece_to("a1", "e4"));
    assert_eq!(g.pos.piece_at(s("e4")).unwrap().kind, PieceKind::Rook);
    assert!(g.pos.piece_at(s("a1")).is_none());
    assert_eq!(
        g.pos.castling & (WHITE_QUEEN_SIDE_BIT),
        0,
        "queen-side castling lost with the rook"
    );
}

const WHITE_QUEEN_SIDE_BIT: u8 = chessy_engine::position::WHITE_QUEEN_SIDE;

#[test]
fn skills_cannot_leave_your_king_in_check() {
    let g = game_from(
        "4k3/8/8/8/4r3/8/P7/4K3 w - - 0 1",
        &[SkillId::Teleportation],
        &[],
    );
    assert!(
        can_skill(&g, SkillId::Teleportation, piece_to("a2", "e2")),
        "block the check"
    );
    assert!(
        !can_skill(&g, SkillId::Teleportation, piece_to("a2", "h5")),
        "ignores the check"
    );
}

#[test]
fn imune_blocks_one_capture_turn() {
    let mut g = game_from("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", &[SkillId::Imune], &[]);
    assert!(can_skill(&g, SkillId::Imune, piece("e4")));
    assert!(!can_skill(&g, SkillId::Imune, piece("e1")), "not the king");
    use_skill(&mut g, SkillId::Imune, piece("e4"));
    assert!(!can_move(&g, "d5", "e4"), "immune pawn cannot be captured");
    assert!(can_move(&g, "d5", "d4"));
    mv(&mut g, "e8", "e7");
    mv(&mut g, "e1", "d1");
    assert!(
        can_move(&g, "d5", "e4"),
        "immunity lasted only one opponent turn"
    );
}

#[test]
fn freeze_lasts_two_opponent_turns() {
    let mut g = game_from("r3k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Freeze], &[]);
    use_skill(&mut g, SkillId::Freeze, piece("a8"));
    assert!(
        !has_piece_moves(&g, "a8"),
        "frozen on the first opponent turn"
    );
    mv(&mut g, "e8", "e7");
    mv(&mut g, "e1", "d1");
    assert!(
        !has_piece_moves(&g, "a8"),
        "still frozen on the second opponent turn"
    );
    mv(&mut g, "e7", "e6");
    mv(&mut g, "d1", "e1");
    assert!(has_piece_moves(&g, "a8"), "free again");
}

#[test]
fn freezing_the_checker_resolves_check() {
    let g = game_from("4k3/8/8/8/8/8/8/r3K3 w - - 0 1", &[SkillId::Freeze], &[]);
    assert!(g.pos.in_check(Color::White));
    assert!(can_skill(&g, SkillId::Freeze, piece("a1")));
}

#[test]
fn a_skill_can_save_you_from_mate() {
    let fen = "6k1/8/8/8/8/8/5PPP/r5K1 w - - 0 1";
    assert_eq!(
        game_from(fen, &[], &[]).outcome(),
        Outcome::Checkmate {
            winner: Color::Black
        }
    );
    let g = game_from(fen, &[SkillId::Freeze], &[]);
    assert_eq!(g.outcome(), Outcome::Ongoing);
    assert!(can_skill(&g, SkillId::Freeze, piece("a1")));
}

#[test]
fn rollback_sends_a_piece_back() {
    let mut g = Game::new(&[SkillId::Rollback], &[]);
    assert!(
        !g.legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Skill { .. })),
        "nothing has moved yet"
    );
    mv(&mut g, "e2", "e4");
    mv(&mut g, "e7", "e5");
    assert!(can_skill(&g, SkillId::Rollback, piece("e4")));
    use_skill(&mut g, SkillId::Rollback, piece("e4"));
    assert_eq!(g.pos.piece_at(s("e2")).unwrap().kind, PieceKind::Pawn);
    assert!(g.pos.piece_at(s("e4")).is_none());
}

#[test]
fn rollback_is_not_allowed_on_the_king() {
    let mut g = game_from("4k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Rollback], &[]);
    mv(&mut g, "e1", "e2");
    mv(&mut g, "e8", "e7");
    assert!(!can_skill(&g, SkillId::Rollback, piece("e2")));
}

#[test]
fn clone_copies_onto_an_adjacent_empty_square() {
    let mut g = Game::new(&[SkillId::Clone], &[]);
    assert!(can_skill(&g, SkillId::Clone, piece_to("e2", "e3")));
    assert!(
        !can_skill(&g, SkillId::Clone, piece_to("e2", "e4")),
        "not adjacent"
    );
    assert!(
        !can_skill(&g, SkillId::Clone, piece_to("e1", "e2")),
        "king and occupied"
    );
    use_skill(&mut g, SkillId::Clone, piece_to("e2", "e3"));
    let (a, b) = (
        g.pos.piece_at(s("e2")).unwrap(),
        g.pos.piece_at(s("e3")).unwrap(),
    );
    assert_eq!((a.kind, b.kind), (PieceKind::Pawn, PieceKind::Pawn));
    assert_ne!(a.id, b.id);
}

#[test]
fn destiny_swapper_swaps_allies() {
    let mut g = Game::new(&[SkillId::DestinySwapper], &[]);
    assert!(can_skill(
        &g,
        SkillId::DestinySwapper,
        SkillTarget::Pair {
            a: s("b1"),
            b: s("c1")
        }
    ));
    assert!(
        can_skill(
            &g,
            SkillId::DestinySwapper,
            SkillTarget::Pair {
                a: s("a1"),
                b: s("a2")
            }
        ),
        "a pawn may rest on its own back rank"
    );
    use_skill(
        &mut g,
        SkillId::DestinySwapper,
        SkillTarget::Pair {
            a: s("b1"),
            b: s("c1"),
        },
    );
    assert_eq!(g.pos.piece_at(s("b1")).unwrap().kind, PieceKind::Bishop);
    assert_eq!(g.pos.piece_at(s("c1")).unwrap().kind, PieceKind::Knight);
}

#[test]
fn swapping_the_king_forfeits_castling() {
    let mut g = game_from(
        "3rk1r1/8/8/8/8/8/8/R3K2R w KQ - 0 1",
        &[SkillId::DestinySwapper],
        &[],
    );
    use_skill(
        &mut g,
        SkillId::DestinySwapper,
        SkillTarget::Pair {
            a: s("e1"),
            b: s("a1"),
        },
    );
    assert!(g.pos.to_fen().contains(" b - - "), "{}", g.pos.to_fen());
}

#[test]
fn remover_takes_enemy_pawns_but_never_by_checkmate() {
    let g = game_from("7k/p7/8/7p/8/8/8/K5RR w - - 0 1", &[SkillId::Remover], &[]);
    assert!(can_skill(&g, SkillId::Remover, piece("a7")));
    assert!(
        !can_skill(&g, SkillId::Remover, piece("h5")),
        "removing the shielding pawn would be checkmate"
    );
    assert!(!can_skill(&g, SkillId::Remover, piece("g1")), "only pawns");
}

#[test]
fn actions_serialize_for_the_wire() {
    let m = Action::Move {
        from: 12,
        to: 28,
        promo: None,
    };
    assert_eq!(
        serde_json::to_string(&m).unwrap(),
        r#"{"type":"move","from":12,"to":28}"#
    );
    let k = Action::Skill {
        skill: SkillId::Teleportation,
        target: SkillTarget::PieceTo { from: 0, to: 28 },
    };
    let json = serde_json::to_string(&k).unwrap();
    assert_eq!(
        json,
        r#"{"type":"skill","skill":"teleportation","target":{"kind":"piece_to","from":0,"to":28}}"#
    );
    assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), k);
}
