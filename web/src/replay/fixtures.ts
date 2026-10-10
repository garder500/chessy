// Données de démonstration réalistes pour les tests et le mock de développement (`?mock=1`).
// Elles respectent le contrat de docs/spec-v4.md §1-§3 ; elles ne sont importées que par les tests
// et par `src/dev/mock.ts` (jamais par le code de production, donc absentes du bundle).

import type {
  Action,
  ActiveEffect,
  Analysis,
  AnalysisLabel,
  Color,
  ExploreResponse,
  Frame,
  GameEvent,
  GameRecord,
  GameSummary,
  LiveGame,
  MoveInfo,
  Piece,
  PieceKind,
  Seat,
  SkillId,
  SpectatorView,
  Square,
} from "../protocol";

export const sq = (name: string): Square => "abcdefgh".indexOf(name[0]) + (Number(name[1]) - 1) * 8;

const BACK: PieceKind[] = ["rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook"];

export function initialBoard(): (Piece | null)[] {
  const board: (Piece | null)[] = Array(64).fill(null);
  for (let file = 0; file < 8; file++) {
    board[file] = { id: 1 + file, kind: BACK[file], color: "white", home: file };
    board[8 + file] = { id: 9 + file, kind: "pawn", color: "white", home: 8 + file };
    board[48 + file] = { id: 25 + file, kind: "pawn", color: "black", home: 48 + file };
    board[56 + file] = { id: 17 + file, kind: BACK[file], color: "black", home: 56 + file };
  }
  return board;
}

const ONGOING = { type: "ongoing" } as const;

/** Construit une partie pas à pas : chaque appel ajoute une action et la position qui en résulte. */
class Builder {
  board = initialBoard();
  effects: ActiveEffect[] = [];
  used: { white: SkillId[]; black: SkillId[] } = { white: [], black: [] };
  moves: MoveInfo[] = [];
  frames: Frame[] = [];
  private toMove: Color = "white";

  constructor() {
    this.frames.push(this.frame([], false));
  }

  private frame(events: GameEvent[], inCheck: boolean): Frame {
    return {
      ply: this.frames.length,
      to_move: this.toMove,
      in_check: inCheck,
      board: this.board.map((p) => (p ? { ...p } : null)),
      effects: this.effects.map((e) => ({ ...e })),
      traps: [],
      terrain: [],
      benched: [],
      events,
      used: { white: [...this.used.white], black: [...this.used.black] },
      outcome: ONGOING,
    };
  }

  private push(color: Color, action: Action, notation: string, events: GameEvent[], inCheck: boolean) {
    this.moves.push({ ply: this.moves.length + 1, color, action, notation });
    this.toMove = color === "white" ? "black" : "white";
    this.frames.push(this.frame(events, inCheck));
  }

  move(from: string, to: string, notation: string, inCheck = false): this {
    const a = sq(from);
    const b = sq(to);
    const piece = this.board[a]!;
    const events: GameEvent[] = [];
    const taken = this.board[b];
    if (taken) events.push({ type: "captured", square: b, piece: { ...taken } });
    events.push({ type: "moved", from: a, to: b, piece: piece.id });
    this.board[b] = piece;
    this.board[a] = null;
    this.push(piece.color, { type: "move", from: a, to: b }, notation, events, inCheck);
    return this;
  }

  freeze(color: Color, target: string, notation: string): this {
    const square = sq(target);
    const piece = this.board[square]!;
    this.used[color].push("freeze");
    this.effects.push({ kind: "frozen", piece: piece.id, expires_at: this.frames.length + 4 });
    const skillTarget = { kind: "piece", square } as const;
    this.push(
      color,
      { type: "skill", skill: "freeze", target: skillTarget },
      notation,
      [
        { type: "skill_used", color, skill: "freeze", target: skillTarget },
        { type: "effect_added", piece: piece.id, effect: "frozen", expires_at: this.frames.length + 4 },
      ],
      false,
    );
    return this;
  }

  teleport(color: Color, from: string, to: string, notation: string): this {
    const a = sq(from);
    const b = sq(to);
    const piece = this.board[a]!;
    this.board[b] = piece;
    this.board[a] = null;
    this.used[color].push("teleportation");
    const target = { kind: "piece_to", from: a, to: b } as const;
    this.push(
      color,
      { type: "skill", skill: "teleportation", target },
      notation,
      [
        { type: "skill_used", color, skill: "teleportation", target },
        { type: "teleported", from: a, to: b },
      ],
      false,
    );
    return this;
  }
}

export const WHITE_SEAT: Seat = { username: "jeremy", elo: 1284, bot: false };
export const BLACK_SEAT: Seat = { username: "ada", elo: 1311, bot: false };

/** Partie de 16 actions : deux compétences (Freeze, Téléportation) et une fin par abandon des blancs. */
export function fixtureRecord(overrides: Partial<GameRecord> = {}): GameRecord {
  const b = new Builder()
    .move("e2", "e4", "e4")
    .move("e7", "e5", "e5")
    .move("g1", "f3", "Cf3")
    .move("b8", "c6", "Cc6")
    .move("f1", "c4", "Fc4")
    .freeze("black", "c4", "Freeze sur c4")
    .move("d2", "d3", "d3")
    .move("g8", "f6", "Cf6")
    .move("c1", "g5", "Fg5")
    .move("c6", "d4", "Cd4")
    .move("f3", "d4", "Cxd4")
    .move("e5", "d4", "exd4")
    .move("g5", "f6", "Fxf6")
    .move("d8", "f6", "Dxf6")
    .teleport("white", "d1", "h5", "Teleportation d1→h5")
    .move("f6", "f2", "Dxf2+", true);
  const last = b.frames[b.frames.length - 1];
  last.outcome = { type: "resignation", winner: "black" };
  return {
    game_id: "g-demo-1",
    kind: "duel",
    rated: true,
    white: WHITE_SEAT,
    black: BLACK_SEAT,
    result: { outcome: last.outcome, reason: "resignation" },
    plies: b.moves.length,
    time_control: "short",
    at: "2026-10-06T20:12:07Z",
    loadouts: { white: ["teleportation", "imune", "tornado"], black: ["freeze", "wall", "clone"] },
    moves: b.moves,
    frames: b.frames,
    ...overrides,
  };
}

// ---- analyse -----------------------------------------------------------------

interface Row {
  eval: number;
  loss: number;
  label: AnalysisLabel;
  best?: { from: string; to: string; notation: string; eval: number };
}

const ROWS: Row[] = [
  { eval: 30, loss: 0, label: "best" },
  { eval: 25, loss: 5, label: "best" },
  { eval: 30, loss: 8, label: "best" },
  { eval: 25, loss: 4, label: "best" },
  { eval: 20, loss: 35, label: "good", best: { from: "d2", to: "d4", notation: "d4", eval: 55 } },
  { eval: 15, loss: 10, label: "best" },
  { eval: 5, loss: 45, label: "good", best: { from: "c2", to: "c3", notation: "c3", eval: 50 } },
  { eval: 10, loss: 5, label: "best" },
  { eval: -130, loss: 140, label: "inaccuracy", best: { from: "b1", to: "c3", notation: "Cc3", eval: 10 } },
  { eval: -150, loss: 10, label: "best" },
  { eval: -120, loss: 30, label: "good", best: { from: "g5", to: "f6", notation: "Fxf6", eval: -90 } },
  { eval: -140, loss: 5, label: "best" },
  { eval: -110, loss: 12, label: "best" },
  { eval: -130, loss: 8, label: "best" },
  { eval: -640, loss: 510, label: "blunder", best: { from: "c4", to: "b3", notation: "Fb3", eval: -140 } },
  { eval: -900, loss: 0, label: "best" },
];

const move = (from: string, to: string): Action => ({ type: "move", from: sq(from), to: sq(to) });

export function fixtureAnalysis(depth = 3, record: GameRecord = fixtureRecord()): Analysis {
  const plies = ROWS.map((row, i) => ({
    ply: i + 1,
    eval_cp: row.eval,
    best: row.best
      ? { action: move(row.best.from, row.best.to), notation: row.best.notation, eval_cp: row.best.eval }
      : { action: record.moves[i].action, notation: record.moves[i].notation, eval_cp: row.eval },
    loss_cp: row.loss,
    label: row.label,
  }));
  const counts = () => ({ best: 0, good: 0, inaccuracy: 0, mistake: 0, blunder: 0 });
  const summary = { white: counts(), black: counts() };
  const losses: Record<Color, number[]> = { white: [], black: [] };
  plies.forEach((p, i) => {
    const color = record.moves[i].color;
    summary[color][p.label]++;
    losses[color].push(p.loss_cp);
  });
  const accuracy = (color: Color) => {
    const l = losses[color];
    return Math.round(100 * Math.exp(-(l.reduce((a, c) => a + c, 0) / Math.max(1, l.length)) / 250));
  };
  return { depth, plies, accuracy: { white: accuracy("white"), black: accuracy("black") }, summary };
}

// ---- exploration (réponse figée, pour les tests) -------------------------------

/** Réponse d'exploration minimale : la position de la partie après `ply` actions, rejouée sans variation. */
export function fixtureExplore(record: GameRecord, ply: number, notation: string[] = []): ExploreResponse {
  const frame = record.frames[ply];
  return {
    valid: true,
    at: notation.length,
    frame,
    moves: [
      { from: sq("e2"), to: sq("e4") },
      { from: sq("g1"), to: sq("f3") },
    ],
    skill_options: [{ skill: "tornado", targets: [{ kind: "none" }] }],
    eval_cp: 20,
    best: { action: move("e2", "e4"), notation: "e4", eval_cp: 25 },
    notation,
  };
}

// ---- listes -------------------------------------------------------------------

const seat = (username: string | null, elo: number | null, bot = false): Seat => ({ username, elo, bot });

/** Parties de `jeremy` : classées, amicales, solo ; la plus récente d'abord. */
export function fixtureGames(): GameSummary[] {
  return [
    { game_id: "g-demo-1", kind: "duel", rated: true, white: WHITE_SEAT, black: BLACK_SEAT, color: "white", result: "loss", reason: "resignation", plies: 16, time_control: "short", elo_delta: -9, at: "2026-10-06T20:12:07Z" },
    { game_id: "g-demo-2", kind: "solo", rated: false, white: seat("jeremy", 1284), black: seat(null, 1400, true), color: "white", result: "win", reason: "checkmate", plies: 41, time_control: null, elo_delta: null, at: "2026-10-06T18:40:00Z" },
    { game_id: "g-demo-3", kind: "challenge", rated: false, white: seat("lea", 1190), black: seat("jeremy", 1290), color: "black", result: "draw", reason: "agreed_draw", plies: 33, time_control: "medium", elo_delta: null, at: "2026-10-05T21:02:11Z" },
    { game_id: "g-demo-4", kind: "duel", rated: true, white: seat("jeremy", 1275), black: seat("marc", 1260), color: "white", result: "win", reason: "checkmate", plies: 58, time_control: "short", elo_delta: 14, at: "2026-10-05T19:30:45Z" },
    { game_id: "g-demo-5", kind: "room", rated: false, white: seat("sam", 1500), black: seat("jeremy", 1280), color: "black", result: "loss", reason: "timeout", plies: 27, time_control: "long", elo_delta: null, at: "2026-10-03T10:00:00Z" },
    { game_id: "g-demo-6", kind: "duel", rated: true, white: seat("jeremy", 1262), black: seat(null, null), color: "white", result: "win", reason: "resignation", plies: 22, time_control: "short", elo_delta: 12, at: "2026-09-28T09:15:00Z" },
  ];
}

export function fixtureLive(): LiveGame[] {
  return [
    { game_id: "g-live-1", kind: "duel", rated: true, white: seat("ada", 1311), black: seat("jeremy", 1284), ply: 18, started_at: "2026-10-07T09:10:00Z", spectators: 3 },
    { game_id: "g-live-2", kind: "duel", rated: true, white: seat("nora", 1720), black: seat("viktor", 1688), ply: 41, started_at: "2026-10-07T08:50:00Z", spectators: 12 },
    { game_id: "g-live-3", kind: "challenge", rated: false, white: seat("lea", 1190), black: seat("marc", 1260), ply: 0, started_at: "2026-10-07T09:20:00Z", spectators: 0 },
    { game_id: "g-live-4", kind: "solo", rated: false, white: seat("sam", 1500), black: seat(null, 1800, true), ply: 7, started_at: "2026-10-07T09:18:00Z", spectators: 1 },
  ];
}

// ---- spectateur ----------------------------------------------------------------

/** Vue d'un spectateur sur la position `ply` de la partie de démonstration (retransmission de 30 s). */
export function fixtureSpectatorView(ply: number, record: GameRecord = fixtureRecord(), delayMs = 30_000): SpectatorView {
  const frame = record.frames[Math.min(ply, record.frames.length - 1)];
  const over = frame.outcome.type !== "ongoing";
  return {
    game_id: record.game_id,
    kind: record.kind,
    rated: record.rated,
    white: record.white,
    black: record.black,
    ply: frame.ply,
    to_move: frame.to_move,
    in_check: frame.in_check,
    board: frame.board,
    effects: frame.effects.filter((e) => e.kind !== "invisible"),
    terrain: frame.terrain,
    clock: { white_ms: 600_000 - frame.ply * 21_000, black_ms: 600_000 - frame.ply * 18_000, running: over ? null : frame.to_move },
    clock_enabled: true,
    events: frame.events,
    used: frame.used,
    outcome: frame.outcome,
    spectators: 3,
    delay_ms: delayMs,
  };
}
