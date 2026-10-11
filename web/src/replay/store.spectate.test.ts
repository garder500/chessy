import { beforeAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../i18n";
import type { ClientMsg, Me, ServerMsg } from "../protocol";
import { Store } from "../store";
import { fixtureRecord, fixtureSpectatorView } from "./fixtures";

const record = fixtureRecord();
const me: Me = { player_id: "p", username: "jeremy", guest: false, elo: 1284, rank: 3, games: 4, wins: 2, draws: 1, losses: 1 };
const welcome: ServerMsg = { type: "welcome", player_id: "p", token: "t", deck: [], pending_reward: null, account: me };
const view = (ply: number): ServerMsg => ({ type: "spectate_state", view: fixtureSpectatorView(ply, record) });

let store: Store;
let sent: ClientMsg[];

beforeEach(() => {
  store = new Store();
  sent = [];
  vi.stubGlobal("WebSocket", { OPEN: 1 });
  (store as unknown as { socket: unknown }).socket = { readyState: 1, send: (m: string) => sent.push(JSON.parse(m) as ClientMsg) };
});
afterEach(() => vi.unstubAllGlobals());

beforeAll(() => setLang("fr"));

describe("store : mode spectateur", () => {
  it("envoie spectate puis suit les vues", () => {
    store.spectate(record.game_id);
    expect(sent).toEqual([{ type: "spectate", game_id: record.game_id }]);
    expect(store.getState().spectating?.status).toBe("joining");
    store.receive(view(3));
    expect(store.getState().spectating?.status).toBe("watching");
    expect(store.getState().spectating?.view?.ply).toBe(3);
    store.receive(view(4));
    expect(store.getState().spectating?.log.map((l) => l.ply)).toEqual([3, 4]);
  });

  it("envoie unspectate et ignore les spectate_state qui arrivent ensuite", () => {
    store.spectate(record.game_id);
    store.receive(view(3));
    store.unspectate();
    expect(sent.at(-1)).toEqual({ type: "unspectate" });
    expect(store.getState().spectating).toBeNull();
    store.receive(view(5));
    expect(store.getState().spectating).toBeNull();
  });

  it("n'envoie pas unspectate si la retransmission est déjà finie", () => {
    store.spectate(record.game_id);
    store.receive({ type: "spectate_over", view: fixtureSpectatorView(16, record) });
    expect(store.getState().spectating?.status).toBe("over");
    sent.length = 0;
    store.unspectate();
    expect(sent).toEqual([]);
    expect(store.getState().spectating).toBeNull();
  });

  it("montre une erreur d'entrée sans toast", () => {
    store.spectate("fantome");
    store.receive({ type: "error", code: "no_such_game", message: "x" });
    expect(store.getState().spectating).toMatchObject({ status: "error", error: "no_such_game" });
    expect(store.getState().toasts).toEqual([]);
  });

  it("garde le toast pour les autres erreurs", () => {
    store.spectate(record.game_id);
    store.receive({ type: "error", code: "rate_limited", message: "Doucement." });
    expect(store.getState().spectating?.status).toBe("joining");
    expect(store.getState().toasts[0].text).toBe("Doucement.");
  });

  it("quitte le mode spectateur quand le joueur lance une partie", () => {
    store.spectate(record.game_id);
    store.receive(view(2));
    store.receive({
      type: "deck_select",
      game_id: "mine",
      opponent: { username: "bob", elo: 1200, guest: false },
      rated: false,
      you: "white",
      deck: [],
      max_picks: 3,
      seconds: 30,
      submitted: false,
    });
    expect(store.getState().spectating).toBeNull();
  });

  it("note une partie annulée", () => {
    store.spectate(record.game_id);
    store.receive(view(2));
    store.receive({ type: "spectate_ended", reason: "cancelled" });
    expect(store.getState().spectating).toMatchObject({ status: "ended", endedReason: "cancelled" });
  });

  it("se réinscrit après une reconnexion (nouveau welcome)", () => {
    store.spectate(record.game_id);
    store.receive(view(2));
    sent.length = 0;
    store.receive(welcome);
    expect(sent).toEqual([{ type: "spectate", game_id: record.game_id }]);
    store.unspectate();
    sent.length = 0;
    store.receive(welcome);
    expect(sent).toEqual([]);
  });

  it("n'avertit pas les abonnés quand un message est ignoré", () => {
    const listener = vi.fn();
    store.subscribe(listener);
    store.receive(view(4));
    expect(listener).not.toHaveBeenCalled();
  });
});
