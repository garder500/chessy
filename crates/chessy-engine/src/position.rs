use crate::skills::SkillId;
use crate::types::*;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub const WHITE_KING_SIDE: u8 = 1;
pub const WHITE_QUEEN_SIDE: u8 = 2;
pub const BLACK_KING_SIDE: u8 = 4;
pub const BLACK_QUEEN_SIDE: u8 = 8;

const KNIGHT_DELTAS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING_DELTAS: [(i8, i8); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const DIAGONALS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const ORTHOGONALS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

pub fn offset(s: Square, df: i8, dr: i8) -> Option<Square> {
    let f = file_of(s) as i8 + df;
    let r = rank_of(s) as i8 + dr;
    if (0..8).contains(&f) && (0..8).contains(&r) {
        Some(sq(f as u8, r as u8))
    } else {
        None
    }
}

/// How a capture changes what happens after the capturer lands (see
/// [`Position::begin_capture`]).
pub(crate) enum Reaction {
    None,
    /// A Force Field pushes the capturer back.
    Push,
    /// Celestial Intervention: the victim goes home instead of dying.
    Saved(Piece, Square),
}

/// The position just before a skill was played, so Canceller can undo it.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub position: Position,
    pub skill: SkillId,
}

/// What a piece may do while its moves are generated.
#[derive(Clone, Copy)]
struct Gen {
    color: Color,
    /// Squares it may not stop on or cross (enemy terrain).
    blocked: u64,
    /// False for mirages.
    captures: bool,
}

/// Chebyshev distance between two squares.
pub fn king_distance(a: Square, b: Square) -> u8 {
    file_of(a)
        .abs_diff(file_of(b))
        .max(rank_of(a).abs_diff(rank_of(b)))
}

fn signum(x: i8) -> i8 {
    x.signum()
}

/// The type a morphed piece goes back to; a pawn that would land on a back rank
/// is promoted instead.
fn revert_kind(orig: PieceKind, at: Square) -> PieceKind {
    if orig == PieceKind::Pawn && !Position::can_stand(orig, at) {
        PieceKind::Queen
    } else {
        orig
    }
}

/// Everything needed to play from a given moment: cheap to clone, so legality
/// checks and perft can simulate moves on copies.
#[derive(Clone, Debug)]
pub struct Position {
    pub board: [Option<Piece>; 64],
    pub side: Color,
    pub castling: u8,
    pub en_passant: Option<Square>,
    pub halfmove: u32,
    pub fullmove: u32,
    /// Number of actions (moves or skills) played so far; drives effect expiry.
    pub ply: u32,
    pub effects: Vec<ActiveEffect>,
    pub traps: Vec<Trap>,
    pub benched: Vec<BenchedPiece>,
    /// Pawns each side has lost (indexed by `Color::index`); Wall brings them back.
    pub captured_pawns: [u8; 2],
    /// Every real piece lost so far, with the type it really had (a morphed
    /// piece is recorded under its original type). Forged skills that revive
    /// pieces draw from it; mirages, wall pawns and temporary pieces never enter.
    pub graveyard: Vec<Piece>,
    /// Set by a skill that passed the turn, cleared by any other action.
    pub last_skill_snapshot: Option<Box<Snapshot>>,
    pub(crate) next_id: PieceId,
}

impl Position {
    pub fn empty() -> Self {
        Position {
            board: [None; 64],
            side: Color::White,
            castling: 0,
            en_passant: None,
            halfmove: 0,
            fullmove: 1,
            ply: 0,
            effects: Vec::new(),
            traps: Vec::new(),
            benched: Vec::new(),
            captured_pawns: [0; 2],
            graveyard: Vec::new(),
            last_skill_snapshot: None,
            next_id: 0,
        }
    }

    pub fn startpos() -> Self {
        Self::from_fen(START_FEN).expect("start FEN is valid")
    }

    pub fn from_fen(fen: &str) -> Result<Self, RuleError> {
        let bad = |msg: &str| RuleError::InvalidFen(msg.to_string());
        let mut parts = fen.split_whitespace();
        let placement = parts.next().ok_or_else(|| bad("missing placement"))?;
        let side = parts.next().ok_or_else(|| bad("missing side to move"))?;
        let castling = parts.next().unwrap_or("-");
        let ep = parts.next().unwrap_or("-");
        let halfmove = parts.next().unwrap_or("0");
        let fullmove = parts.next().unwrap_or("1");

        let mut pos = Position::empty();
        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(bad("placement must have 8 ranks"));
        }
        for (i, row) in ranks.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file = 0u8;
            for c in row.chars() {
                if let Some(n) = c.to_digit(10) {
                    file += n as u8;
                } else {
                    let (color, kind) =
                        PieceKind::from_fen_char(c).ok_or_else(|| bad("unknown piece"))?;
                    if file > 7 {
                        return Err(bad("rank overflows"));
                    }
                    let id = pos.alloc_id();
                    let at = sq(file, rank);
                    pos.board[at as usize] = Some(Piece::new(id, kind, color, at));
                    file += 1;
                }
            }
            if file != 8 {
                return Err(bad("rank does not have 8 files"));
            }
        }
        pos.side = match side {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err(bad("side to move must be w or b")),
        };
        for c in castling.chars() {
            pos.castling |= match c {
                'K' => WHITE_KING_SIDE,
                'Q' => WHITE_QUEEN_SIDE,
                'k' => BLACK_KING_SIDE,
                'q' => BLACK_QUEEN_SIDE,
                '-' => 0,
                _ => return Err(bad("bad castling field")),
            };
        }
        pos.en_passant = if ep == "-" {
            None
        } else {
            Some(parse_square(ep).ok_or_else(|| bad("bad en passant square"))?)
        };
        pos.halfmove = halfmove.parse().map_err(|_| bad("bad halfmove clock"))?;
        pos.fullmove = fullmove.parse().map_err(|_| bad("bad fullmove number"))?;
        Ok(pos)
    }

    pub fn to_fen(&self) -> String {
        let mut out = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.board[sq(file, rank) as usize] {
                    None => empty += 1,
                    Some(p) => {
                        if empty > 0 {
                            out.push_str(&empty.to_string());
                            empty = 0;
                        }
                        out.push(p.kind.fen_char(p.color));
                    }
                }
            }
            if empty > 0 {
                out.push_str(&empty.to_string());
            }
            if rank > 0 {
                out.push('/');
            }
        }
        out.push(' ');
        out.push(if self.side == Color::White { 'w' } else { 'b' });
        out.push(' ');
        if self.castling == 0 {
            out.push('-');
        } else {
            for (bit, c) in [
                (WHITE_KING_SIDE, 'K'),
                (WHITE_QUEEN_SIDE, 'Q'),
                (BLACK_KING_SIDE, 'k'),
                (BLACK_QUEEN_SIDE, 'q'),
            ] {
                if self.castling & bit != 0 {
                    out.push(c);
                }
            }
        }
        out.push(' ');
        match self.en_passant {
            Some(s) => out.push_str(&square_name(s)),
            None => out.push('-'),
        }
        out.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        out
    }

    pub fn alloc_id(&mut self) -> PieceId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn piece_at(&self, s: Square) -> Option<Piece> {
        self.board[s as usize]
    }

    pub fn pieces(&self, color: Color) -> impl Iterator<Item = (Square, Piece)> + '_ {
        self.board
            .iter()
            .enumerate()
            .filter_map(move |(i, p)| p.filter(|p| p.color == color).map(|p| (i as Square, p)))
    }

    pub fn has_effect(&self, id: PieceId, kind: EffectKind) -> bool {
        self.effects.iter().any(|e| e.piece == id && e.kind == kind)
    }

    /// Whether the piece cannot move: frozen, or a Wall pawn that is still locked.
    /// Such a piece attacks nothing either.
    pub fn is_frozen(&self, id: PieceId) -> bool {
        self.effects
            .iter()
            .any(|e| e.piece == id && matches!(e.kind, EffectKind::Frozen | EffectKind::Locked))
    }

    pub fn is_immune(&self, id: PieceId) -> bool {
        self.has_effect(id, EffectKind::Immune)
    }

    /// Whether an Armistice is on: nothing attacks, so nothing is captured and
    /// nobody is in check.
    pub fn truce(&self) -> bool {
        self.effects.iter().any(|e| e.kind == EffectKind::Truce)
    }

    pub fn fog(&self) -> bool {
        self.effects.iter().any(|e| e.kind == EffectKind::Fog)
    }

    /// Whether `color` is forbidden to use skills (Silence).
    pub fn is_silenced(&self, color: Color) -> bool {
        self.effects
            .iter()
            .any(|e| e.kind == EffectKind::Silenced && e.owner == Some(color))
    }

    /// Starts a game-wide effect of `duration_plies`, aimed at `owner` if any.
    pub fn add_global_effect(
        &mut self,
        kind: EffectKind,
        owner: Option<Color>,
        duration_plies: u32,
    ) -> Event {
        let mut effect = ActiveEffect::new(kind, NO_PIECE, self.ply + duration_plies);
        effect.owner = owner;
        self.effects.push(effect);
        Event::GlobalEffect {
            effect: kind,
            expires_at: effect.expires_at,
            owner,
        }
    }

    pub fn add_effect(&mut self, kind: EffectKind, piece: PieceId, duration_plies: u32) -> Event {
        self.push_effect(ActiveEffect::new(kind, piece, self.ply + duration_plies))
    }

    /// Adds a fully specified effect and reports it.
    pub fn push_effect(&mut self, effect: ActiveEffect) -> Event {
        self.effects.push(effect);
        Event::EffectAdded {
            piece: effect.piece,
            effect: effect.kind,
            expires_at: effect.expires_at,
        }
    }

    /// Squares the pieces of `color` may neither stop on nor cross: the
    /// Geomancy terrain of the other side.
    pub fn blocked_mask(&self, color: Color) -> u64 {
        let mut mask = 0u64;
        for e in &self.effects {
            if e.kind == EffectKind::Terrain && e.owner != Some(color) {
                if let Some(s) = e.square {
                    mask |= 1u64 << s;
                }
            }
        }
        mask
    }

    pub fn trap_at(&self, s: Square) -> bool {
        self.traps.iter().any(|t| t.square == s)
    }

    /// Where a skill may put a piece of `color` and `kind`: an empty square it
    /// can stand on that is neither trapped nor enemy terrain.
    pub fn can_place(&self, color: Color, kind: PieceKind, s: Square) -> bool {
        self.board[s as usize].is_none()
            && Position::can_stand(kind, s)
            && !self.trap_at(s)
            && self.blocked_mask(color) & (1u64 << s) == 0
    }

    /// The free square closest to `origin` (king distance, then square order)
    /// where a piece of `color` and `kind` may be put.
    pub fn nearest_free(&self, origin: Square, color: Color, kind: PieceKind) -> Option<Square> {
        (0..64u8)
            .filter(|&s| self.can_place(color, kind, s))
            .min_by_key(|&s| (king_distance(origin, s), s))
    }

    /// The safety net for forged skills: after one played, each side still has
    /// as many kings as in `before`, no piece id is on the board twice and no
    /// pawn stands on a back rank. A skill that breaks this is refused rather
    /// than allowed to corrupt the game.
    pub(crate) fn board_is_sane(&self, before: &Position) -> bool {
        let kings = |p: &Position, c: Color| {
            p.pieces(c)
                .filter(|(_, x)| x.kind == PieceKind::King)
                .count()
        };
        if Color::BOTH
            .iter()
            .any(|&c| kings(self, c) != kings(before, c))
        {
            return false;
        }
        let mut seen = std::collections::HashSet::new();
        self.board.iter().flatten().all(|p| seen.insert(p.id))
            && self
                .board
                .iter()
                .enumerate()
                .all(|(i, p)| p.is_none_or(|p| Position::can_stand(p.kind, i as Square)))
    }

    pub fn find_piece(&self, id: PieceId) -> Option<Square> {
        self.board
            .iter()
            .position(|p| p.is_some_and(|p| p.id == id))
            .map(|i| i as Square)
    }

    /// Every square the piece on `from` attacks (including the first piece
    /// each ray runs into). Frozen pieces and mirages attack nothing.
    pub fn attacked_squares(&self, from: Square) -> Vec<Square> {
        let Some(piece) = self.board[from as usize] else {
            return Vec::new();
        };
        if piece.mirage || self.is_frozen(piece.id) || self.truce() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let steps = |deltas: &[(i8, i8)], out: &mut Vec<Square>| {
            out.extend(deltas.iter().filter_map(|&(df, dr)| offset(from, df, dr)));
        };
        match piece.kind {
            PieceKind::Pawn => {
                let fwd = piece.color.forward();
                steps(&[(-1, fwd), (1, fwd)], &mut out);
            }
            PieceKind::Knight => steps(&KNIGHT_DELTAS, &mut out),
            PieceKind::King => steps(&KING_DELTAS, &mut out),
            PieceKind::Bishop | PieceKind::Rook | PieceKind::Queen => {
                let mut dirs: Vec<(i8, i8)> = Vec::new();
                if piece.kind != PieceKind::Rook {
                    dirs.extend(DIAGONALS);
                }
                if piece.kind != PieceKind::Bishop {
                    dirs.extend(ORTHOGONALS);
                }
                for (df, dr) in dirs {
                    let mut cur = from;
                    while let Some(next) = offset(cur, df, dr) {
                        cur = next;
                        out.push(cur);
                        if self.board[cur as usize].is_some() {
                            break;
                        }
                    }
                }
            }
        }
        out
    }

    pub fn king_square(&self, color: Color) -> Option<Square> {
        self.pieces(color)
            .find(|(_, p)| p.kind == PieceKind::King)
            .map(|(s, _)| s)
    }

    /// Whether `target` is attacked by a piece of color `by`. Frozen pieces
    /// cannot move, and mirages never capture, so they attack nothing; enemy
    /// terrain cannot be stopped on or crossed.
    pub fn is_attacked(&self, target: Square, by: Color) -> bool {
        if !self.effects.is_empty() && self.truce() {
            return false;
        }
        let blocked = self.blocked_mask(by);
        if blocked & (1u64 << target) != 0 {
            return false;
        }
        let hits = |s: Square, kinds: &[PieceKind]| -> bool {
            matches!(self.board[s as usize], Some(p)
                if p.color == by && kinds.contains(&p.kind) && !p.mirage && !self.is_frozen(p.id))
        };

        // Pawns attack diagonally forward, so they sit one rank "behind" the target.
        for df in [-1, 1] {
            if let Some(s) = offset(target, df, -by.forward()) {
                if hits(s, &[PieceKind::Pawn]) {
                    return true;
                }
            }
        }
        for (df, dr) in KNIGHT_DELTAS {
            if let Some(s) = offset(target, df, dr) {
                if hits(s, &[PieceKind::Knight]) {
                    return true;
                }
            }
        }
        for (df, dr) in KING_DELTAS {
            if let Some(s) = offset(target, df, dr) {
                if hits(s, &[PieceKind::King]) {
                    return true;
                }
            }
        }
        let rays: [(&[(i8, i8); 4], PieceKind); 2] = [
            (&DIAGONALS, PieceKind::Bishop),
            (&ORTHOGONALS, PieceKind::Rook),
        ];
        for (dirs, slider) in rays {
            for &(df, dr) in dirs {
                let mut cur = target;
                while let Some(next) = offset(cur, df, dr) {
                    cur = next;
                    if blocked & (1u64 << cur) != 0 {
                        break;
                    }
                    if self.board[cur as usize].is_some() {
                        if hits(cur, &[slider, PieceKind::Queen]) {
                            return true;
                        }
                        break;
                    }
                }
            }
        }
        false
    }

    pub fn in_check(&self, color: Color) -> bool {
        self.king_square(color)
            .is_some_and(|k| self.is_attacked(k, color.opposite()))
    }

    /// Whether a piece of `color` may stand on `target` (pawns never rest on a
    /// back rank).
    pub fn can_stand(kind: PieceKind, target: Square) -> bool {
        kind != PieceKind::Pawn || !matches!(rank_of(target), 0 | 7)
    }

    pub fn pseudo_moves(&self, out: &mut Vec<Move>) {
        let color = self.side;
        let blocked = self.blocked_mask(color);
        let truce = !self.effects.is_empty() && self.truce();
        for (from, piece) in self.pieces(color) {
            if self.is_frozen(piece.id) {
                continue;
            }
            // A mirage can walk but never takes anything, and neither can
            // anyone during an Armistice.
            let ctx = Gen {
                color,
                blocked,
                captures: !piece.mirage && !truce,
            };
            match piece.kind {
                PieceKind::Pawn => self.pawn_moves(from, ctx, out),
                PieceKind::Knight => self.step_moves(from, ctx, &KNIGHT_DELTAS, out),
                PieceKind::King => {
                    self.step_moves(from, ctx, &KING_DELTAS, out);
                    self.castling_moves(from, ctx, out);
                }
                PieceKind::Bishop => self.slide_moves(from, ctx, &DIAGONALS, out),
                PieceKind::Rook => self.slide_moves(from, ctx, &ORTHOGONALS, out),
                PieceKind::Queen => {
                    self.slide_moves(from, ctx, &DIAGONALS, out);
                    self.slide_moves(from, ctx, &ORTHOGONALS, out);
                }
            }
        }
    }

    /// Whether the piece described by `ctx` may end a move on `target`.
    fn can_land(&self, ctx: Gen, target: Square) -> bool {
        if ctx.blocked & (1u64 << target) != 0 {
            return false;
        }
        match self.board[target as usize] {
            None => true,
            Some(p) => ctx.captures && p.color != ctx.color && !self.is_immune(p.id),
        }
    }

    fn push_pawn_move(&self, from: Square, to: Square, color: Color, out: &mut Vec<Move>) {
        if rank_of(to) == color.promotion_rank() {
            for promo in PieceKind::PROMOTIONS {
                out.push(Move {
                    from,
                    to,
                    promo: Some(promo),
                });
            }
        } else {
            out.push(Move {
                from,
                to,
                promo: None,
            });
        }
    }

    fn pawn_moves(&self, from: Square, ctx: Gen, out: &mut Vec<Move>) {
        let color = ctx.color;
        let fwd = color.forward();
        if let Some(one) = offset(from, 0, fwd) {
            if self.board[one as usize].is_none() && ctx.blocked & (1u64 << one) == 0 {
                self.push_pawn_move(from, one, color, out);
                if rank_of(from) == color.pawn_start_rank() {
                    if let Some(two) = offset(from, 0, 2 * fwd) {
                        if self.board[two as usize].is_none() && ctx.blocked & (1u64 << two) == 0 {
                            out.push(Move {
                                from,
                                to: two,
                                promo: None,
                            });
                        }
                    }
                }
            }
        }
        if !ctx.captures {
            return;
        }
        for df in [-1, 1] {
            let Some(to) = offset(from, df, fwd) else {
                continue;
            };
            if ctx.blocked & (1u64 << to) != 0 {
                continue;
            }
            match self.board[to as usize] {
                Some(p) if p.color != color && !self.is_immune(p.id) => {
                    self.push_pawn_move(from, to, color, out)
                }
                None if self.en_passant == Some(to) => {
                    let victim = sq(file_of(to), rank_of(from));
                    if matches!(self.board[victim as usize], Some(p)
                        if p.color != color && p.kind == PieceKind::Pawn && !self.is_immune(p.id))
                    {
                        out.push(Move {
                            from,
                            to,
                            promo: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn step_moves(&self, from: Square, ctx: Gen, deltas: &[(i8, i8); 8], out: &mut Vec<Move>) {
        for &(df, dr) in deltas {
            if let Some(to) = offset(from, df, dr) {
                if self.can_land(ctx, to) {
                    out.push(Move {
                        from,
                        to,
                        promo: None,
                    });
                }
            }
        }
    }

    fn slide_moves(&self, from: Square, ctx: Gen, dirs: &[(i8, i8); 4], out: &mut Vec<Move>) {
        for &(df, dr) in dirs {
            let mut cur = from;
            while let Some(to) = offset(cur, df, dr) {
                cur = to;
                if ctx.blocked & (1u64 << to) != 0 {
                    break;
                }
                if self.board[to as usize].is_none() {
                    out.push(Move {
                        from,
                        to,
                        promo: None,
                    });
                } else {
                    if self.can_land(ctx, to) {
                        out.push(Move {
                            from,
                            to,
                            promo: None,
                        });
                    }
                    break;
                }
            }
        }
    }

    fn castling_moves(&self, from: Square, ctx: Gen, out: &mut Vec<Move>) {
        let color = ctx.color;
        let rank = color.home_rank();
        if from != sq(4, rank) {
            return;
        }
        let (king_bit, queen_bit) = match color {
            Color::White => (WHITE_KING_SIDE, WHITE_QUEEN_SIDE),
            Color::Black => (BLACK_KING_SIDE, BLACK_QUEEN_SIDE),
        };
        let enemy = color.opposite();
        let rook_ready = |file: u8| {
            matches!(self.board[sq(file, rank) as usize], Some(p)
                if p.kind == PieceKind::Rook && p.color == color && !p.mirage
                    && !self.is_frozen(p.id))
        };
        let empty = |files: &[u8]| {
            files
                .iter()
                .all(|&f| self.board[sq(f, rank) as usize].is_none())
        };
        let safe = |files: &[u8]| files.iter().all(|&f| !self.is_attacked(sq(f, rank), enemy));
        let open = |files: &[u8]| {
            files
                .iter()
                .all(|&f| ctx.blocked & (1u64 << sq(f, rank)) == 0)
        };

        if self.castling & king_bit != 0
            && rook_ready(7)
            && empty(&[5, 6])
            && open(&[5, 6])
            && safe(&[4, 5, 6])
        {
            out.push(Move {
                from,
                to: sq(6, rank),
                promo: None,
            });
        }
        if self.castling & queen_bit != 0
            && rook_ready(0)
            && empty(&[1, 2, 3])
            && open(&[2, 3])
            && safe(&[4, 3, 2])
        {
            out.push(Move {
                from,
                to: sq(2, rank),
                promo: None,
            });
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut pseudo = Vec::with_capacity(48);
        self.pseudo_moves(&mut pseudo);
        let color = self.side;
        let mut sink = Vec::new();
        pseudo
            .into_iter()
            .filter(|&mv| {
                let mut next = self.clone();
                sink.clear();
                next.make_move(mv, &mut sink);
                !next.in_check(color)
            })
            .collect()
    }

    /// The first enemy trap `piece` runs into on its way `from` -> `to`. Sliders
    /// and double pawn steps cross every square in between; everything else
    /// only touches the square it lands on.
    fn trap_on_path(&self, piece: &Piece, from: Square, to: Square) -> Option<Square> {
        let sprung = |s: Square| {
            self.traps
                .iter()
                .any(|t| t.square == s && t.owner != piece.color)
        };
        let crosses = match piece.kind {
            PieceKind::Bishop | PieceKind::Rook | PieceKind::Queen => true,
            PieceKind::Pawn => rank_of(from).abs_diff(rank_of(to)) == 2,
            _ => false,
        };
        if !crosses {
            return sprung(to).then_some(to);
        }
        let df = signum(file_of(to) as i8 - file_of(from) as i8);
        let dr = signum(rank_of(to) as i8 - rank_of(from) as i8);
        let mut cur = from;
        while cur != to {
            cur = offset(cur, df, dr)?;
            if sprung(cur) {
                return Some(cur);
            }
        }
        None
    }

    /// Records the capture of `victim` on `square`: reports it, keeps the
    /// graveyard of pawns and applies Celestial Intervention / Force Field.
    /// The caller finishes the job with [`Position::finish_capture`] once the
    /// capturer stands on its new square.
    pub(crate) fn begin_capture(
        &mut self,
        square: Square,
        victim: Piece,
        ev: &mut Vec<Event>,
    ) -> Reaction {
        let mut reaction = Reaction::None;
        let mut real_kind = victim.kind;
        if !self.effects.is_empty() {
            if self.has_effect(victim.id, EffectKind::Celestial) {
                self.effects
                    .retain(|e| !(e.piece == victim.id && e.kind == EffectKind::Celestial));
                return Reaction::Saved(victim, square);
            }
            if self.has_effect(victim.id, EffectKind::Forcefield) {
                reaction = Reaction::Push;
            }
            if let Some(orig) = self
                .effects
                .iter()
                .find(|e| e.piece == victim.id && e.kind == EffectKind::Morphed)
                .and_then(|e| e.orig_kind)
            {
                real_kind = orig;
            }
            self.effects.retain(|e| e.piece != victim.id);
        }
        ev.push(Event::Captured {
            square,
            piece: victim,
        });
        if !victim.wall && !victim.mirage && !victim.temp {
            if real_kind == PieceKind::Pawn {
                let n = &mut self.captured_pawns[victim.color.index()];
                *n = n.saturating_add(1);
            }
            if real_kind != PieceKind::King {
                self.graveyard.push(Piece {
                    kind: real_kind,
                    prev: None,
                    ..victim
                });
            }
        }
        reaction
    }

    /// Second half of a capture: the capturer now stands on `to`, having left `from`.
    pub(crate) fn finish_capture(
        &mut self,
        reaction: Reaction,
        from: Square,
        to: Square,
        ev: &mut Vec<Event>,
    ) {
        match reaction {
            Reaction::None => {}
            Reaction::Saved(piece, at) => {
                match self.nearest_free(piece.home, piece.color, piece.kind) {
                    Some(dest) => {
                        let mut saved = piece;
                        saved.prev = None;
                        self.board[dest as usize] = Some(saved);
                        ev.push(Event::Saved {
                            piece: piece.id,
                            from: at,
                            to: dest,
                        });
                    }
                    None => {
                        // Nowhere to go: the piece is lost after all.
                        self.effects.retain(|e| e.piece != piece.id);
                        ev.push(Event::Captured { square: at, piece });
                    }
                }
            }
            Reaction::Push => {
                let Some(mut piece) = self.board[to as usize] else {
                    return;
                };
                let df = signum(file_of(from) as i8 - file_of(to) as i8);
                let dr = signum(rank_of(from) as i8 - rank_of(to) as i8);
                let blocked = self.blocked_mask(piece.color);
                let mut cur = to;
                for _ in 0..2 {
                    let Some(next) = offset(cur, df, dr) else {
                        break;
                    };
                    if self.board[next as usize].is_some()
                        || blocked & (1u64 << next) != 0
                        || !Position::can_stand(piece.kind, next)
                    {
                        break;
                    }
                    cur = next;
                }
                if cur != to {
                    self.board[to as usize] = None;
                    piece.prev = None;
                    self.board[cur as usize] = Some(piece);
                    ev.push(Event::Pushed {
                        piece: piece.id,
                        from: to,
                        to: cur,
                    });
                }
            }
        }
    }

    /// Plays `mv` without checking legality; callers go through `legal_moves`.
    /// A slider that runs into an enemy trap stops on it, so the move that
    /// actually happens may end earlier than `mv.to`.
    pub fn make_move(&mut self, mv: Move, ev: &mut Vec<Event>) {
        self.play_move(mv, ev);
        self.end_turn_events(ev);
    }

    /// [`Position::make_move`] without handing the turn over (Temporal
    /// Distortion replays a move inside a skill, whose caller ends the turn).
    pub(crate) fn play_move(&mut self, mv: Move, ev: &mut Vec<Event>) {
        let color = self.side;
        let mut mv = mv;
        let mut piece = self.board[mv.from as usize]
            .take()
            .expect("move from an occupied square");

        let mut sprung = None;
        if !self.traps.is_empty() {
            if let Some(t) = self.trap_on_path(&piece, mv.from, mv.to) {
                sprung = Some(t);
                if t != mv.to {
                    mv.to = t;
                    mv.promo = None;
                }
            }
        }

        let captured = self.board[mv.to as usize].take();
        let mut reaction = Reaction::None;
        let mut took = captured.is_some();
        let mut new_ep = None;

        if piece.kind == PieceKind::Pawn {
            if Some(mv.to) == self.en_passant
                && file_of(mv.from) != file_of(mv.to)
                && captured.is_none()
            {
                let victim = sq(file_of(mv.to), rank_of(mv.from));
                if let Some(p) = self.board[victim as usize].take() {
                    reaction = self.begin_capture(victim, p, ev);
                    took = true;
                }
            }
            if rank_of(mv.from).abs_diff(rank_of(mv.to)) == 2 {
                new_ep = Some((mv.from + mv.to) / 2);
            }
        }

        if let Some(p) = captured {
            reaction = self.begin_capture(mv.to, p, ev);
        }

        if piece.kind == PieceKind::King && file_of(mv.from).abs_diff(file_of(mv.to)) == 2 {
            let rank = rank_of(mv.from);
            let (rook_from, rook_to) = if file_of(mv.to) == 6 {
                (sq(7, rank), sq(5, rank))
            } else {
                (sq(0, rank), sq(3, rank))
            };
            let mut rook = self.board[rook_from as usize]
                .take()
                .expect("castling rook present");
            rook.prev = None;
            self.board[rook_to as usize] = Some(rook);
            ev.push(Event::Castled { rook_from, rook_to });
        }

        // Castling rights: lost when a king or rook leaves, or a rook is captured.
        if piece.kind == PieceKind::King {
            self.castling &= match color {
                Color::White => !(WHITE_KING_SIDE | WHITE_QUEEN_SIDE),
                Color::Black => !(BLACK_KING_SIDE | BLACK_QUEEN_SIDE),
            };
        }
        for s in [mv.from, mv.to] {
            self.castling &= match s {
                0 => !WHITE_QUEEN_SIDE,
                7 => !WHITE_KING_SIDE,
                56 => !BLACK_QUEEN_SIDE,
                63 => !BLACK_KING_SIDE,
                _ => 0xF,
            };
        }

        if piece.kind == PieceKind::Pawn || took {
            self.halfmove = 0;
        } else {
            self.halfmove += 1;
        }

        ev.push(Event::Moved {
            from: mv.from,
            to: mv.to,
            piece: piece.id,
        });

        if piece.kind == PieceKind::Pawn && rank_of(mv.to) == color.promotion_rank() {
            let kind = mv.promo.unwrap_or(PieceKind::Queen);
            piece.kind = kind;
            piece.prev = None;
            ev.push(Event::Promoted {
                square: mv.to,
                to: kind,
            });
        } else {
            piece.prev = Some(mv.from);
        }
        self.board[mv.to as usize] = Some(piece);
        self.en_passant = new_ep;
        self.last_skill_snapshot = None;

        self.finish_capture(reaction, mv.from, mv.to, ev);

        if let Some(trap) = sprung {
            self.traps
                .retain(|t| !(t.square == trap && t.owner != color));
            ev.push(Event::TrapSprung {
                square: trap,
                piece: piece.id,
            });
            // Two of the owner's turns: ply + 1 is the end of this move.
            let frozen = self.add_effect(EffectKind::Frozen, piece.id, 5);
            ev.push(frozen);
        }
    }

    /// Miroir: every piece goes to the square opposite it (same file, rank
    /// `7 - r`) and changes color, so each player inherits the other's army and
    /// its formation, on their own side of the board. Piece ids, effects and
    /// the side to move are kept; everything that has a color or a square
    /// follows. Returns the moves for the clients to animate.
    pub fn mirror_armies(&mut self) -> Vec<Shift> {
        let flip = |s: Square| sq(file_of(s), 7 - rank_of(s));
        let mut board = [None; 64];
        let mut shifts = Vec::new();
        for (i, slot) in self.board.iter().enumerate() {
            let Some(mut piece) = *slot else { continue };
            let from = i as Square;
            let to = flip(from);
            piece.color = piece.color.opposite();
            piece.home = flip(piece.home);
            piece.prev = None;
            board[to as usize] = Some(piece);
            shifts.push(Shift { from, to });
        }
        self.board = board;
        for e in &mut self.effects {
            e.square = e.square.map(flip);
            e.owner = e.owner.map(Color::opposite);
            e.orig_color = e.orig_color.map(Color::opposite);
        }
        for t in &mut self.traps {
            t.square = flip(t.square);
            t.owner = t.owner.opposite();
        }
        for b in &mut self.benched {
            b.square = flip(b.square);
            b.piece.color = b.piece.color.opposite();
            b.piece.home = flip(b.piece.home);
        }
        for g in &mut self.graveyard {
            g.color = g.color.opposite();
            g.home = flip(g.home);
        }
        self.captured_pawns.swap(0, 1);
        // The kings changed places and color: so did the castling rights.
        let white = self.castling & (WHITE_KING_SIDE | WHITE_QUEEN_SIDE);
        let black = self.castling & (BLACK_KING_SIDE | BLACK_QUEEN_SIDE);
        self.castling = (white << 2) | (black >> 2);
        self.en_passant = None;
        shifts
    }

    /// Drops castling rights whose king or rook is no longer on its home square
    /// (skills can relocate pieces without going through `make_move`).
    pub fn sanitize_castling(&mut self) {
        let checks = [
            (WHITE_KING_SIDE, 4, 7, Color::White),
            (WHITE_QUEEN_SIDE, 4, 0, Color::White),
            (BLACK_KING_SIDE, 60, 63, Color::Black),
            (BLACK_QUEEN_SIDE, 60, 56, Color::Black),
        ];
        for (bit, king_sq, rook_sq, color) in checks {
            let king_ok = matches!(self.board[king_sq], Some(p)
                if p.kind == PieceKind::King && p.color == color);
            let rook_ok = matches!(self.board[rook_sq], Some(p)
                if p.kind == PieceKind::Rook && p.color == color);
            if !king_ok || !rook_ok {
                self.castling &= !bit;
            }
        }
    }

    /// Hands the turn over: advances the move counters and expires effects.
    pub fn end_turn(&mut self) {
        let mut sink = Vec::new();
        self.end_turn_events(&mut sink);
    }

    /// [`Position::end_turn`], reporting what the expiries did (temporary
    /// pieces vanishing, borrowed pieces going home, benched pieces returning...).
    pub fn end_turn_events(&mut self, ev: &mut Vec<Event>) {
        if self.side == Color::Black {
            self.fullmove += 1;
        }
        self.side = self.side.opposite();
        self.ply += 1;
        if !self.effects.is_empty() {
            self.expire_effects(ev);
        }
        if !self.benched.is_empty() {
            self.return_benched(ev);
        }
    }

    fn expire_effects(&mut self, ev: &mut Vec<Event>) {
        let ply = self.ply;
        if self.effects.iter().all(|e| e.expires_at > ply) {
            return;
        }
        let expired: Vec<ActiveEffect> = self
            .effects
            .iter()
            .filter(|e| e.expires_at <= ply)
            .copied()
            .collect();
        self.effects.retain(|e| e.expires_at > ply);
        for e in expired {
            match e.kind {
                EffectKind::ColorLoan => {
                    let Some(back) = e.orig_color else { continue };
                    if let Some(s) = self.find_piece(e.piece) {
                        let piece = self.board[s as usize].as_mut().expect("found piece");
                        piece.color = back;
                        piece.prev = None;
                        ev.push(Event::LoanEnded {
                            square: s,
                            piece: *piece,
                        });
                    }
                }
                EffectKind::Morphed => {
                    let Some(orig) = e.orig_kind else { continue };
                    if let Some(s) = self.find_piece(e.piece) {
                        let piece = self.board[s as usize].as_mut().expect("found piece");
                        piece.kind = revert_kind(orig, s);
                        ev.push(Event::Transformed {
                            square: s,
                            kind: piece.kind,
                        });
                    } else if let Some(b) = self.benched.iter_mut().find(|b| b.piece.id == e.piece)
                    {
                        b.piece.kind = revert_kind(orig, b.square);
                    }
                }
                EffectKind::Vanish => {
                    if let Some(s) = self.find_piece(e.piece) {
                        let piece = self.board[s as usize].take().expect("found piece");
                        ev.push(Event::Vanished { square: s, piece });
                    } else {
                        self.benched.retain(|b| b.piece.id != e.piece);
                    }
                    self.effects.retain(|o| o.piece != e.piece);
                }
                _ => {}
            }
        }
    }

    fn return_benched(&mut self, ev: &mut Vec<Event>) {
        let ply = self.ply;
        if self.benched.iter().all(|b| b.back_at > ply) {
            return;
        }
        let (back, stay): (Vec<_>, Vec<_>) = std::mem::take(&mut self.benched)
            .into_iter()
            .partition(|b| b.back_at <= ply);
        self.benched = stay;
        for b in back {
            let mut piece = b.piece;
            match self.nearest_free(b.square, piece.color, piece.kind) {
                Some(dest) => {
                    piece.prev = None;
                    self.board[dest as usize] = Some(piece);
                    ev.push(Event::Unbenched {
                        square: dest,
                        piece,
                    });
                }
                None => self.effects.retain(|e| e.piece != piece.id),
            }
        }
    }

    /// Number of leaf nodes at `depth` plies of plain chess (no skills).
    pub fn perft(&self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = self.legal_moves();
        if depth == 1 {
            return moves.len() as u64;
        }
        let mut sink = Vec::new();
        moves
            .into_iter()
            .map(|mv| {
                let mut next = self.clone();
                sink.clear();
                next.make_move(mv, &mut sink);
                next.perft(depth - 1)
            })
            .sum()
    }
}
