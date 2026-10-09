import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { actionKey } from "../game/logic";
import {
  colorOf,
  defaultOrientation,
  endSound,
  exploreViews,
  frameToView,
  isReplayable,
  kindLabel,
  lastIndex,
  normalizeClock,
  normalizeSpectatorView,
  seatName,
  spectatorToView,
} from "./frames";
import { fixtureExplore, fixtureRecord, fixtureSpectatorView, sq } from "./fixtures";

const record = fixtureRecord();

beforeAll(() => setLang("fr"));

describe("fixtures", () => {
  it("respectent le contrat : frames.length == plies + 1", () => {
    expect(record.frames).toHaveLength(record.plies + 1);
    expect(record.moves).toHaveLength(record.plies);
    expect(record.frames.map((f) => f.ply)).toEqual(record.frames.map((_, i) => i));
    expect(record.moves.map((m) => m.ply)).toEqual(record.moves.map((_, i) => i + 1));
  });
});

describe("frameToView", () => {
  it("convertit une position en vue plateau en lecture seule", () => {
    const view = frameToView(record, record.frames[5], "white");
    expect(view.you).toBe("white");
    expect(view.ply).toBe(5);
    expect(view.to_move).toBe("black");
    expect(view.moves).toEqual([]);
    expect(view.skill_options).toEqual([]);
    expect(view.clock_enabled).toBe(false);
    expect(view.board).toBe(record.frames[5].board);
    expect(view.opponent.username).toBe("ada");
    expect(view.outcome.type).toBe("ongoing");
  });

  it("oriente le plateau vers le camp choisi et inverse l'adversaire", () => {
    const view = frameToView(record, record.frames[2], "black");
    expect(view.you).toBe("black");
    expect(view.opponent.username).toBe("jeremy");
    expect(view.opponent.elo).toBe(1284);
  });

  it("suit l'usage des compétences et garde les effets, événements et pièges", () => {
    const early = frameToView(record, record.frames[5], "black");
    expect(early.my_skills.map((s) => [s.skill, s.used])).toEqual([["freeze", false], ["wall", false], ["clone", false]]);
    const after = frameToView(record, record.frames[6], "black");
    expect(after.my_skills.find((s) => s.skill === "freeze")?.used).toBe(true);
    expect(after.opponent_skills).toEqual({ total: 3, used: [] });
    expect(after.effects[0].kind).toBe("frozen");
    expect(after.events.map((e) => e.type)).toEqual(["skill_used", "effect_added"]);
  });

  it("change d'empreinte d'action à chaque position (le plateau rejoue les effets)", () => {
    const keys = record.frames.map((f) => actionKey(frameToView(record, f, "white")));
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("expose la fin de partie sur la dernière position", () => {
    const view = frameToView(record, record.frames[record.plies], "white");
    expect(view.outcome).toEqual({ type: "resignation", winner: "black" });
    expect(view.in_check).toBe(true);
  });

  it("rend visibles les pièges et le banc des deux camps", () => {
    const frame = {
      ...record.frames[3],
      traps: [{ square: sq("e4"), owner: "white" as const }, { square: sq("d5"), owner: "black" as const }],
      benched: [{ piece: { id: 3, kind: "bishop" as const, color: "white" as const }, square: sq("c1"), owner: "white" as const, back_at: 9 }],
    };
    const view = frameToView(record, frame, "white");
    expect(view.traps).toEqual([sq("e4"), sq("d5")]);
    expect(view.benched).toHaveLength(1);
  });
});

describe("exploreViews", () => {
  it("sépare l'affichage (orientation choisie) et le jeu (camp au trait)", () => {
    const res = fixtureExplore(record, 4);
    const { display, play } = exploreViews(record, res.frame!, "black", res.moves, res.skill_options);
    expect(display.you).toBe("black");
    expect(display.moves).toEqual([]);
    expect(play.you).toBe("white");
    expect(play.to_move).toBe("white");
    expect(play.moves).toHaveLength(2);
    expect(play.skill_options[0].skill).toBe("tornado");
  });
});

describe("spectatorToView", () => {
  it("convertit une retransmission, sans pièges ni banc", () => {
    const sv = fixtureSpectatorView(7);
    const view = spectatorToView(sv, "white");
    expect(view.ply).toBe(7);
    expect(view.traps).toEqual([]);
    expect(view.benched).toEqual([]);
    expect(view.moves).toEqual([]);
    expect(view.clock_enabled).toBe(true);
    expect(view.clock.running).toBe("black");
    expect(view.spectators).toBe(3);
  });

  it("normalise l'horloge : booléen, camp, et arrêt en fin de partie", () => {
    expect(normalizeClock({ white_ms: 1, black_ms: 2, running: true }, "black", false).running).toBe("black");
    expect(normalizeClock({ white_ms: 1, black_ms: 2, running: false }, "black", false).running).toBeNull();
    expect(normalizeClock({ white_ms: 1, black_ms: 2, running: "white" }, "black", false).running).toBe("white");
    expect(normalizeClock({ white_ms: 1, black_ms: 2, running: "white" }, "black", true).running).toBeNull();
  });

  it("complète les champs optionnels manquants", () => {
    const raw = { ...fixtureSpectatorView(3), effects: undefined, events: undefined, spectators: undefined, delay_ms: undefined } as never;
    const sv = normalizeSpectatorView(raw);
    expect(sv.events).toEqual([]);
    expect(sv.spectators).toBe(0);
    expect(sv.delay_ms).toBe(0);
  });
});

describe("son de fin de partie", () => {
  it("choisit le son selon le résultat du point de vue de l'observateur", () => {
    expect(endSound({ type: "checkmate", winner: "white" }, "white")).toBe("game_win");
    expect(endSound({ type: "checkmate", winner: "white" }, "black")).toBe("game_lose");
    expect(endSound({ type: "stalemate" }, "white")).toBe("game_draw");
    expect(endSound({ type: "ongoing" }, "white")).toBeNull();
  });
});

describe("orientation et libellés", () => {
  it("trouve le camp du joueur sans tenir compte de la casse", () => {
    expect(colorOf(record, "JEREMY")).toBe("white");
    expect(colorOf(record, "Ada")).toBe("black");
    expect(colorOf(record, "zoe")).toBeNull();
    expect(colorOf(record, null)).toBeNull();
  });

  it("choisit l'orientation par défaut", () => {
    expect(defaultOrientation(record, "ada")).toBe("black");
    expect(defaultOrientation(record, "zoe")).toBe("white");
    const solo = { white: { username: null, elo: 1400, bot: true }, black: { username: null, elo: null, bot: false } };
    expect(defaultOrientation(solo, null)).toBe("black");
  });

  it("nomme les joueurs", () => {
    expect(seatName({ username: "ada", elo: 1, bot: false })).toBe("ada");
    expect(seatName({ username: null, elo: 1400, bot: true })).toBe("IA");
    expect(seatName({ username: null, elo: null, bot: false })).toBe("Invité");
  });

  it("étiquette le type de partie", () => {
    expect(kindLabel("solo", false)).toBe("Solo");
    expect(kindLabel("duel", true)).toBe("Classée");
    expect(kindLabel("challenge", false)).toBe("Amicale");
    expect(kindLabel("room", false)).toBe("Amicale");
  });

  it("détecte un enregistrement inutilisable", () => {
    expect(isReplayable(record)).toBe(true);
    expect(isReplayable({ frames: [] })).toBe(false);
    expect(isReplayable(null)).toBe(false);
    expect(lastIndex(record)).toBe(16);
    expect(lastIndex({ frames: [] })).toBe(0);
  });
});
