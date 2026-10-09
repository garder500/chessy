import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import type { Action, ExploreResponse } from "../protocol";
import {
  beginExplore,
  canExtend,
  currentResponse,
  exploreErrorText,
  exploreRequest,
  extendExplore,
  lineEntries,
  MAX_LINE,
  undoExplore,
} from "./explore";
import { fixtureExplore, fixtureRecord, sq } from "./fixtures";

const record = fixtureRecord();
const e4: Action = { type: "move", from: sq("e2"), to: sq("e4") };
const e5: Action = { type: "move", from: sq("e7"), to: sq("e5") };

/** Réponse à `line` : `n` actions appliquées sur la position `ply`. */
function respond(ply: number, notation: string[], over: Partial<ExploreResponse> = {}): ExploreResponse {
  const base = fixtureExplore(record, ply + notation.length, notation);
  return { ...base, ...over };
}

beforeAll(() => setLang("fr"));

describe("exploration : départ", () => {
  it("démarre avec la position de départ", () => {
    const state = beginExplore(4, respond(4, []))!;
    expect(state.ply).toBe(4);
    expect(state.line).toEqual([]);
    expect(currentResponse(state).frame?.ply).toBe(4);
  });

  it("refuse une réponse invalide ou sans position", () => {
    expect(beginExplore(4, respond(4, [], { valid: false, error: "bad_ply" }))).toBeNull();
    expect(beginExplore(4, respond(4, [], { frame: null }))).toBeNull();
  });

  it("construit la requête avec la ligne complète", () => {
    expect(exploreRequest(4, [], undefined, 2)).toEqual({ ply: 4, line: [], depth: 2 });
    expect(exploreRequest(4, [e4], e5)).toEqual({ ply: 4, line: [e4, e5], depth: 3 });
  });
});

describe("exploration : ajout et annulation de coups", () => {
  it("ajoute un coup avec sa notation et la réponse du serveur", () => {
    const start = beginExplore(4, respond(4, []))!;
    const { state, error } = extendExplore(start, e4, respond(4, ["e4"]));
    expect(error).toBeNull();
    expect(state.line).toEqual([e4]);
    expect(state.notation).toEqual(["e4"]);
    expect(state.responses).toHaveLength(2);
    expect(currentResponse(state).frame?.ply).toBe(5);
    expect(start.line).toEqual([]);
  });

  it("enchaîne plusieurs coups", () => {
    let state = beginExplore(4, respond(4, []))!;
    state = extendExplore(state, e4, respond(4, ["e4"])).state;
    state = extendExplore(state, e5, respond(4, ["e4", "e5"])).state;
    expect(state.line).toEqual([e4, e5]);
    expect(state.notation).toEqual(["e4", "e5"]);
    expect(exploreRequest(state.ply, state.line)).toMatchObject({ line: [e4, e5] });
  });

  it("annule le dernier coup sans réseau, jusqu'au départ", () => {
    let state = beginExplore(4, respond(4, []))!;
    state = extendExplore(state, e4, respond(4, ["e4"])).state;
    state = extendExplore(state, e5, respond(4, ["e4", "e5"])).state;
    state = undoExplore(state);
    expect(state.line).toEqual([e4]);
    expect(state.notation).toEqual(["e4"]);
    expect(currentResponse(state).frame?.ply).toBe(5);
    state = undoExplore(state);
    expect(state.line).toEqual([]);
    expect(undoExplore(state)).toBe(state);
  });

  it("garde l'état et signale l'erreur quand le coup est refusé", () => {
    const start = beginExplore(4, respond(4, []))!;
    const bad = respond(4, [], { valid: false, error: "illegal_action", at: 0, frame: null });
    const { state, error } = extendExplore(start, e4, bad);
    expect(state).toBe(start);
    expect(error).toBe("illegal_action");
  });

  it("refuse un coup si le serveur n'a appliqué qu'une partie de la ligne", () => {
    let state = beginExplore(4, respond(4, []))!;
    state = extendExplore(state, e4, respond(4, ["e4"])).state;
    const partial = respond(4, ["e4"], { valid: false, error: "illegal_action", at: 1 });
    const { state: after, error } = extendExplore(state, e5, partial);
    expect(after).toBe(state);
    expect(error).toBe("illegal_action");
  });

  it("signale une réponse valide sans position", () => {
    const start = beginExplore(4, respond(4, []))!;
    expect(extendExplore(start, e4, respond(4, ["e4"], { frame: null })).error).toBe("bad_response");
  });

  it("plafonne la variation à 200 actions", () => {
    const start = beginExplore(4, respond(4, []))!;
    const long = { ...start, line: Array<Action>(MAX_LINE).fill(e4) };
    expect(canExtend(start)).toBe(true);
    expect(canExtend(long)).toBe(false);
    expect(extendExplore(long, e5, respond(4, ["x"])).error).toBe("too_long");
  });
});

describe("exploration : affichage de la variation", () => {
  it("numérote les coups dans le prolongement de la partie et donne leur camp", () => {
    let state = beginExplore(4, respond(4, []))!;
    state = extendExplore(state, e4, respond(4, ["e4"])).state;
    state = extendExplore(state, e5, respond(4, ["e4", "e5"])).state;
    expect(lineEntries(state)).toEqual([
      { ply: 5, color: "white", notation: "e4" },
      { ply: 6, color: "black", notation: "e5" },
    ]);
  });

  it("explique les erreurs en français", () => {
    expect(exploreErrorText("illegal_action")).toMatch(/pas possible/);
    expect(exploreErrorText("bad_ply")).toMatch(/introuvable/);
    expect(exploreErrorText("too_long")).toMatch(/200/);
    expect(exploreErrorText("autre")).toMatch(/inattendue/);
  });
});
