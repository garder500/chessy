import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import {
  actionKey,
  appendLog,
  capturedPieces,
  describeAction,
  evalShare,
  formatClock,
  launchOf,
  logFromHistory,
  materialBalance,
  remainingMs,
  sqName,
  turnsLeft,
} from "./logic";
import type { GameEvent, HistoryEntry, Piece, PieceKind, StateView } from "../protocol";

beforeAll(() => setLang("fr"));

let nextId = 0;
const p = (kind: PieceKind, color: "white" | "black"): Piece => ({ id: nextId++, kind, color });

function boardWith(...pieces: Piece[]): (Piece | null)[] {
  const b: (Piece | null)[] = Array(64).fill(null);
  pieces.forEach((piece, i) => (b[i] = piece));
  return b;
}

function fullSide(color: "white" | "black"): Piece[] {
  const kinds: PieceKind[] = [
    ...Array<PieceKind>(8).fill("pawn"),
    "rook", "rook", "knight", "knight", "bishop", "bishop", "queen", "king",
  ];
  return kinds.map((k) => p(k, color));
}

describe("clock", () => {
  const clock = { white_ms: 600_000, black_ms: 90_500, running: "black" as const };

  it("only the running side counts down", () => {
    expect(remainingMs(clock, "white", 5000)).toBe(600_000);
    expect(remainingMs(clock, "black", 5000)).toBe(85_500);
    expect(remainingMs(clock, "black", 999_999)).toBe(0);
    expect(remainingMs({ ...clock, running: null }, "black", 5000)).toBe(90_500);
  });

  it("formats minutes and shows tenths under ten seconds", () => {
    expect(formatClock(600_000)).toBe("10:00");
    expect(formatClock(59_999)).toBe("0:59");
    expect(formatClock(9_950)).toBe("0:09.9");
    expect(formatClock(0)).toBe("0:00.0");
    expect(formatClock(-5)).toBe("0:00.0");
  });
});

describe("material", () => {
  it("balances piece values", () => {
    const board = boardWith(p("queen", "white"), p("rook", "black"), p("pawn", "black"));
    expect(materialBalance(board, "white")).toBe(3);
    expect(materialBalance(board, "black")).toBe(-3);
  });

  it("squeezes the balance into a bar share", () => {
    expect(evalShare(0)).toBeCloseTo(0.5);
    expect(evalShare(9)).toBeGreaterThan(0.85);
    expect(evalShare(-9)).toBeLessThan(0.15);
  });

  it("lists captured pieces strongest first", () => {
    const pieces = fullSide("white").filter((x) => x.kind !== "queen");
    const board = boardWith(...pieces.slice(1)); // sans la dame ni un pion
    expect(capturedPieces(board, "white")).toEqual(["queen", "pawn"]);
  });

  it("does not count a promoted pawn as captured", () => {
    const pieces = fullSide("white");
    const board = boardWith(...pieces.slice(1), p("queen", "white")); // un pion devenu seconde dame
    expect(capturedPieces(board, "white")).toEqual([]);
  });
});

function view(partial: Partial<StateView>): StateView {
  return {
    game_id: "g",
    clock: { white_ms: 0, black_ms: 0, running: null },
    clock_enabled: true,
    rated: false,
    opponent: { username: null, elo: null, guest: true },
    draw_offer: "none",
    ply_count: 0,
    you: "white",
    ply: 1,
    to_move: "black",
    in_check: false,
    board: Array(64).fill(null),
    moves: [],
    skill_options: [],
    my_skills: [],
    opponent_skills: { total: 0, used: [] },
    effects: [],
    traps: [],
    benched: [],
    terrain: [],
    outcome: { type: "ongoing" },
    events: [],
    opponent_connected: true,
    ...partial,
  };
}

describe("journal", () => {
  it("names squares", () => {
    expect(sqName(0)).toBe("a1");
    expect(sqName(63)).toBe("h8");
  });

  it("describes a capturing move by the side that just played", () => {
    const board = boardWith();
    board[28] = { id: 1, kind: "knight", color: "white" };
    const line = describeAction(
      view({
        ply: 7,
        board,
        events: [
          { type: "moved", from: 11, to: 28, piece: 1 },
          { type: "captured", square: 28, piece: { id: 2, kind: "pawn", color: "black" } },
        ],
      }),
    );
    expect(line).toEqual({ ply: 7, actor: "white", kind: "move", text: "Cavalier d2–e4 · prend pion", skill: undefined });
  });

  it("recognises skill casts and who cast them", () => {
    const v = view({
      to_move: "white",
      events: [
        { type: "skill_used", color: "black", skill: "freeze", target: { kind: "piece", square: 3 } },
        { type: "effect_added", piece: 4, effect: "frozen", expires_at: 9 },
      ],
    });
    expect(launchOf(v)).toEqual({ color: "black", skill: "freeze" });
    expect(describeAction(v)).toMatchObject({ actor: "black", kind: "skill", text: "Freeze · pièce gelée" });
  });

  it("ignores empty histories and duplicate plies", () => {
    expect(describeAction(view({}))).toBeNull();
    const line = { ply: 3, actor: "white" as const, kind: "move" as const, text: "x" };
    expect(appendLog([line], line)).toHaveLength(1);
    expect(appendLog([line], null)).toHaveLength(1);
  });
});

describe("v3 journal and bookkeeping", () => {
  const cast = (skill: "wall" | "tornado" | "trap" | "mind", ...rest: GameEvent[]) =>
    view({ to_move: "black", events: [{ type: "skill_used", color: "white", skill, target: { kind: "none" } }, ...rest] });

  it("describes the new events in French", () => {
    expect(describeAction(cast("tornado", { type: "rotated", moves: [{ from: 1, to: 2 }, { from: 2, to: 1 }] }))?.text).toBe("Tornado · 2 pièces tournent");
    expect(describeAction(cast("trap", { type: "trap_set", square: 20 }))?.text).toBe("Trap Card · piège posé en e3");
    expect(describeAction(cast("mind", { type: "best_move", from: 12, to: 28 }))?.text).toBe("Mind Reading · meilleur coup e2–e4");
    expect(describeAction(view({ events: [{ type: "trap_sprung", square: 20, piece: 3 }] }))?.text).toBe("piège déclenché en e3");
    expect(describeAction(view({ events: [{ type: "saved", piece: 3, from: 20, to: 8 }] }))?.text).toBe("pièce sauvée, retour en a2");
  });

  it("groups the pawns of a Wall into one mention", () => {
    const pawn = (id: number, home: number) => ({ id, kind: "pawn" as const, color: "white" as const, home, wall: true });
    const line = describeAction(
      cast(
        "wall",
        { type: "spawned", square: 20, piece: pawn(1, 12) },
        { type: "spawned", square: 21, piece: pawn(2, 13) },
        { type: "effect_added", piece: 1, effect: "locked", expires_at: 5 },
        { type: "effect_added", piece: 2, effect: "locked", expires_at: 5 },
      ),
    );
    expect(line?.text).toBe("Wall · 2 pièces apparaissent · pion immobilisé");
  });

  it("keys actions so that Mind Reading and Mind Control, which keep the ply, are distinct", () => {
    const base = { ply: 4, my_skills: [{ skill: "mind" as const, used: false, uses: 0, max_uses: 3 }] };
    const first = actionKey({ ...base, events: [] });
    const afterMind = actionKey({ ...base, my_skills: [{ skill: "mind", used: false, uses: 1, max_uses: 3 }], events: [{ type: "best_move", from: 1, to: 2 }] });
    expect(afterMind).not.toBe(first);
    // Même position renvoyée (reconnexion) : même empreinte.
    expect(actionKey({ ...base, events: [] })).toBe(first);
    // Mind Reading deux fois de suite dans la même position : le compteur d'usages les distingue.
    const second = actionKey({ ...base, my_skills: [{ skill: "mind", used: false, uses: 2, max_uses: 3 }], events: [{ type: "best_move", from: 1, to: 2 }] });
    expect(second).not.toBe(afterMind);
  });

  it("logs several actions that share a ply", () => {
    const a = { ply: 4, actor: "white" as const, kind: "skill" as const, text: "a", key: "k1" };
    const b = { ...a, text: "b", key: "k2" };
    expect(appendLog(appendLog([], a), b)).toHaveLength(2);
    expect(appendLog([a], a)).toHaveLength(1);
  });

  it("counts remaining turns of an effect or terrain", () => {
    expect(turnsLeft(9, 7)).toBe(1);
    expect(turnsLeft(9, 4)).toBe(3);
    expect(turnsLeft(9, 12)).toBe(0);
  });

  it("keeps mirages and temporary copies out of the material count, and benched pieces in", () => {
    const b = boardWith(
      { id: 1, kind: "king", color: "white" },
      { id: 2, kind: "queen", color: "white", mirage: true },
      { id: 3, kind: "rook", color: "white", temp: true },
    );
    expect(materialBalance(b, "white")).toBe(0);
    const benched = [{ id: 4, kind: "rook" as const, color: "white" as const }];
    expect(materialBalance(b, "white", benched)).toBe(5);
    expect(capturedPieces(boardWith(...fullSide("white").slice(0, 15)), "white", benched)).not.toContain("rook");
  });
});

describe("journal restored after a page reload", () => {
  const history: HistoryEntry[] = [
    { ply: 1, to_move: "black", events: [{ type: "moved", from: 12, to: 28, piece: 1 }], landed: [{ square: 28, kind: "pawn" }] },
    {
      ply: 2,
      to_move: "white",
      events: [{ type: "skill_used", color: "black", skill: "mind", target: { kind: "none" } }, { type: "best_move", from: 12, to: 28 }],
      landed: [],
    },
    { ply: 2, to_move: "white", events: [], landed: [] },
    { ply: 3, to_move: "black", events: [{ type: "moved", from: 6, to: 21, piece: 2 }], landed: [{ square: 21, kind: "knight" }] },
  ];

  it("rebuilds one line per action from the server history", () => {
    const log = logFromHistory(history);
    expect(log.map((l) => [l.ply, l.actor, l.kind, l.text])).toEqual([
      [1, "white", "move", "Pion e2–e4"],
      [2, "black", "skill", "Mind Reading · meilleur coup e2–e4"],
      [3, "white", "move", "Cavalier g1–f3"],
    ]);
  });

  it("keeps lines that share a ply apart (the turn-keeping skills) and lets live lines follow", () => {
    const log = logFromHistory([history[1], { ...history[1], ply: 2 }]);
    expect(new Set(log.map((l) => l.key)).size).toBe(2);
    const live = { ply: 4, actor: "black" as const, kind: "move" as const, text: "Pion e7–e5", key: "live" };
    expect(appendLog(logFromHistory(history), live)).toHaveLength(4);
  });

  it("caps the journal like the live one", () => {
    const many = Array.from({ length: 5 }, (_, i) => ({ ...history[0], ply: i + 1 }));
    expect(logFromHistory(many, 3).map((l) => l.ply)).toEqual([3, 4, 5]);
  });
});
