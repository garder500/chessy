import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import type { ServerMsg } from "../protocol";
import { fixtureRecord, fixtureSpectatorView } from "./fixtures";
import {
  endedText,
  isLive,
  reduceSpectator,
  SPECTATE_ERRORS,
  spectateErrorText,
  spectatorKey,
  startSpectating,
  type SpectatingState,
} from "./spectator";

const record = fixtureRecord();
const state = (ply: number): ServerMsg => ({ type: "spectate_state", view: fixtureSpectatorView(ply, record) });

beforeAll(() => setLang("fr"));

describe("réducteur de spectateur", () => {
  it("passe de l'entrée à la retransmission à la première vue", () => {
    const joined = startSpectating(record.game_id);
    expect(joined.status).toBe("joining");
    expect(isLive(joined)).toBe(true);
    const s = reduceSpectator(joined, state(3))!;
    expect(s.status).toBe("watching");
    expect(s.view?.ply).toBe(3);
    expect(s.log).toHaveLength(1);
    expect(s.log[0]).toMatchObject({ ply: 3, actor: "white" });
  });

  it("ignore tout message quand on ne regarde rien (spectate_state après unspectate)", () => {
    expect(reduceSpectator(null, state(4))).toBeNull();
    expect(reduceSpectator(null, { type: "spectate_over", view: fixtureSpectatorView(16, record) })).toBeNull();
    expect(reduceSpectator(null, { type: "error", code: "no_such_game", message: "" })).toBeNull();
  });

  it("ignore une vue d'une autre partie", () => {
    const joined = startSpectating("autre-partie");
    expect(reduceSpectator(joined, state(4))).toBe(joined);
  });

  it("accumule le journal sans doublon quand la même vue est renvoyée", () => {
    let s: SpectatingState | null = startSpectating(record.game_id);
    for (const ply of [2, 3, 3, 4]) s = reduceSpectator(s, state(ply));
    expect(s?.log.map((l) => l.ply)).toEqual([2, 3, 4]);
  });

  it("termine sur spectate_over en gardant la vue pour afficher le résultat", () => {
    let s: SpectatingState | null = reduceSpectator(startSpectating(record.game_id), state(15));
    s = reduceSpectator(s, { type: "spectate_over", view: fixtureSpectatorView(16, record) });
    expect(s?.status).toBe("over");
    expect(s?.view?.outcome).toEqual({ type: "resignation", winner: "black" });
    expect(isLive(s)).toBe(false);
    // Une vue en retard après la fin ne ressuscite pas la partie.
    expect(reduceSpectator(s, state(14))).toBe(s);
  });

  it("note l'annulation de la partie", () => {
    const s = reduceSpectator(reduceSpectator(startSpectating(record.game_id), state(5)), { type: "spectate_ended", reason: "cancelled" })!;
    expect(s.status).toBe("ended");
    expect(s.endedReason).toBe("cancelled");
    expect(s.view?.ply).toBe(5);
  });

  it("retient les erreurs d'entrée seulement avant la première vue", () => {
    for (const code of SPECTATE_ERRORS) {
      const s = reduceSpectator(startSpectating("g"), { type: "error", code, message: "" })!;
      expect(s.status).toBe("error");
      expect(s.error).toBe(code);
    }
    const joined = startSpectating("g");
    expect(reduceSpectator(joined, { type: "error", code: "rate_limited", message: "" })).toBe(joined);
    const watching = reduceSpectator(startSpectating(record.game_id), state(2))!;
    expect(reduceSpectator(watching, { type: "error", code: "no_such_game", message: "" })).toBe(watching);
  });

  it("quitte le mode spectateur quand le joueur lance sa propre partie", () => {
    const s = reduceSpectator(startSpectating(record.game_id), state(2));
    expect(reduceSpectator(s, { type: "deck_select" } as ServerMsg)).toBeNull();
    expect(reduceSpectator(s, { type: "state" } as ServerMsg)).toBeNull();
  });

  it("ignore les messages sans rapport (même référence, aucun rendu)", () => {
    const s = startSpectating("g");
    expect(reduceSpectator(s, { type: "friends", friends: [], incoming: [], outgoing: [] })).toBe(s);
  });

  it("calcule une empreinte stable par action", () => {
    const a = fixtureSpectatorView(5, record);
    expect(spectatorKey(a)).toBe(spectatorKey(fixtureSpectatorView(5, record)));
    expect(spectatorKey(a)).not.toBe(spectatorKey(fixtureSpectatorView(6, record)));
  });

  it("explique les erreurs en français", () => {
    expect(spectateErrorText("spectate_full")).toMatch(/50 spectateurs/);
    expect(spectateErrorText("no_such_game")).toMatch(/n'existe pas/);
    expect(spectateErrorText("already_in_game")).toMatch(/déjà dans une partie/);
    expect(spectateErrorText(null)).toMatch(/Impossible/);
    expect(endedText("cancelled")).toBe("La partie a été annulée.");
  });
});
