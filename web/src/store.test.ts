import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Me, ServerMsg, StateView } from "./protocol";
import { accountAfterGame, noticeText, Store } from "./store";

const me: Me = { player_id: "p", username: "jeremy", guest: false, elo: 1284, rank: 3, games: 4, wins: 2, draws: 1, losses: 1 };

const welcome: ServerMsg = { type: "welcome", player_id: "p", token: "t", deck: ["freeze"], pending_reward: null, account: me };

function stateMsg(over: Partial<StateView> = {}): ServerMsg {
  return {
    type: "state",
    game_id: "g",
    clock: { white_ms: 1, black_ms: 1, running: "white" },
    clock_enabled: true,
    rated: true,
    opponent: { username: "ada", elo: 1300, guest: false },
    draw_offer: "none",
    ply_count: 0,
    you: "white",
    ply: 0,
    to_move: "white",
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
    ...over,
  };
}

let store: Store;
beforeEach(() => {
  store = new Store();
});

describe("store messages", () => {
  it("keeps the account from welcome", () => {
    store.receive(welcome);
    expect(store.getState().account).toEqual(me);
    expect(store.getState().connection).toBe("open");
  });

  it("stores friends and search results", () => {
    store.receive({ type: "friends", friends: [{ username: "ada", elo: 1300, presence: "online", last_seen: null }], incoming: [{ username: "bob", elo: 1100 }], outgoing: [] });
    expect(store.getState().friends.incoming).toHaveLength(1);
    store.receive({ type: "user_results", query: "ad", users: [{ username: "ada", elo: 1300, relation: "friend" }] });
    expect(store.getState().userResults?.query).toBe("ad");
  });

  it("turns notices into French toasts", () => {
    store.receive({ type: "notice", code: "friend_request_received", username: "ada" });
    expect(store.getState().toasts[0].text).toBe("ada vous a envoyé une demande d'ami.");
    expect(noticeText("friend_busy", "bob")).toContain("bob");
    expect(noticeText("user_not_found")).toMatch(/introuvable/i);
  });

  it("tracks challenges", () => {
    store.receive({ type: "challenge_received", from: { username: "ada", elo: 1300 } });
    expect(store.getState().incomingChallenge).toEqual({ username: "ada", elo: 1300 });
    store.respondChallenge(false);
    expect(store.getState().incomingChallenge).toBeNull();
    store.receive({ type: "challenge_sent", username: "ada" });
    expect(store.getState().outgoingChallenge).toBe("ada");
    store.receive({ type: "notice", code: "challenge_declined", username: "ada" });
    expect(store.getState().outgoingChallenge).toBeNull();
  });

  it("resets chat and rematch when a new game starts", () => {
    store.receive({ type: "chat", text: "salut", mine: true });
    store.receive({ type: "rematch_offered" });
    expect(store.getState().chat).toEqual([{ mine: true, text: "salut" }]);
    expect(store.getState().rematch).toBe("received");
    store.receive({ type: "deck_select", game_id: "g2", opponent: { username: null, elo: null, guest: true }, rated: false, you: "black", deck: [], max_picks: 3, seconds: 30, submitted: false });
    expect(store.getState().chat).toEqual([]);
    expect(store.getState().rematch).toBe("none");
  });

  it("keeps the block list and the chat mute the server reports", () => {
    store.receive(welcome);
    store.receive({ type: "blocks", blocked: [{ username: "bob" }, { username: "carol" }] });
    expect(store.getState().blocked).toEqual(["bob", "carol"]);
    store.receive({ type: "chat_settings", chat_muted: true });
    expect(store.getState().account?.chat_muted).toBe(true);
    store.receive({ type: "blocks", blocked: [] });
    expect(store.getState().blocked).toEqual([]);
  });

  it("follows draw offers on the current game", () => {
    store.receive(stateMsg());
    store.receive({ type: "draw_offered" });
    expect(store.getState().game?.draw_offer).toBe("them");
    store.respondDraw(false);
    expect(store.getState().game?.draw_offer).toBe("none");
    store.offerDraw();
    expect(store.getState().game?.draw_offer).toBe("you");
    store.receive({ type: "draw_declined" });
    expect(store.getState().game?.draw_offer).toBe("none");
  });

  it("records the result and the new Elo", () => {
    store.receive(welcome);
    store.receive({ type: "game_over", outcome: { type: "timeout", winner: "white" }, reward: null, rated: true, elo: { you_before: 1284, you_after: 1298, opp_before: 1300, opp_after: 1290 }, reason: "timeout" });
    expect(store.getState().over?.reason).toBe("timeout");
    expect(store.getState().account?.elo).toBe(1298);
  });

  it("defaults clock_enabled to true and bot to false when the server omits them", () => {
    const { clock_enabled: _drop, ...legacy } = stateMsg() as StateView & { type: "state" };
    store.receive(legacy as unknown as ServerMsg);
    expect(store.getState().game?.clock_enabled).toBe(true);
    expect(store.getState().game?.opponent.bot).toBe(false);
    store.receive(stateMsg({ clock_enabled: false, opponent: { username: "Sage", elo: 1400, guest: true, bot: true } }));
    expect(store.getState().game?.clock_enabled).toBe(false);
    expect(store.getState().game?.opponent.bot).toBe(true);
  });

  describe("solo mode", () => {
    const botInfo = { username: "Sage", elo: 1400, guest: true, bot: true };
    const deckSelect: ServerMsg = { type: "deck_select", game_id: "g3", opponent: botInfo, rated: false, you: "white", deck: [], max_picks: 3, seconds: 30, submitted: false };
    const sent: unknown[] = [];
    let saved: Record<string, string>;

    beforeEach(() => {
      sent.length = 0;
      saved = {};
      vi.stubGlobal("localStorage", {
        getItem: (k: string) => saved[k] ?? null,
        setItem: (k: string, v: string) => {
          saved[k] = v;
        },
      });
      // Une socket factice suffit pour observer ce qui part vers le serveur.
      vi.stubGlobal("WebSocket", { OPEN: 1 });
      (store as unknown as { socket: unknown }).socket = { readyState: 1, send: (m: string) => sent.push(JSON.parse(m)) };
    });
    afterEach(() => vi.unstubAllGlobals());

    it("sends solo_start with a clamped level, remembers it and waits for the game", () => {
      store.startSolo(1425, "black");
      expect(sent).toEqual([{ type: "solo_start", elo: 1450, color: "black" }]);
      expect(store.getState().soloPending).toBe(true);
      expect(store.getState().solo).toEqual({ elo: 1450, color: "black" });
      expect(JSON.parse(saved["chessy.solo"])).toEqual({ elo: 1450, color: "black" });
      expect(new Store().getState().solo).toEqual({ elo: 1450, color: "black" });
    });

    it("stops waiting once the game is created or refused", () => {
      store.startSolo(1200, "random");
      store.receive(deckSelect);
      expect(store.getState().soloPending).toBe(false);
      expect(store.getState().deckSelect?.opponent.bot).toBe(true);

      store.startSolo(1200, "random");
      store.receive({ type: "error", code: "already_in_game", message: "" });
      expect(store.getState().soloPending).toBe(false);
    });

    it("clears the chat for every new game", () => {
      store.receive({ type: "chat", text: "salut", mine: true });
      store.receive(deckSelect);
      expect(store.getState().chat).toEqual([]);
    });

    it("rematch against the bot is a plain rematch_request", () => {
      store.requestRematch();
      expect(sent).toEqual([{ type: "rematch_request" }]);
    });
  });

  describe("session revoked", () => {
    let saved: Record<string, string>;
    let sockets: { closed: boolean }[];

    beforeEach(() => {
      saved = { "chessy.token": "old" };
      sockets = [];
      vi.stubGlobal("localStorage", {
        getItem: (k: string) => saved[k] ?? null,
        setItem: (k: string, v: string) => {
          saved[k] = v;
        },
        removeItem: (k: string) => {
          delete saved[k];
        },
      });
      vi.stubGlobal("location", { protocol: "http:", host: "chessy.test" });
      vi.stubGlobal(
        "WebSocket",
        class {
          static OPEN = 1;
          readyState = 0;
          closed = false;
          constructor() {
            sockets.push(this);
          }
          close() {
            this.closed = true;
          }
          send() {}
        },
      );
    });
    afterEach(() => vi.unstubAllGlobals());

    it("forgets the token and reconnects as a guest, once", () => {
      store.receive({ type: "error", code: "session_revoked", message: "" });
      expect(saved["chessy.token"]).toBeUndefined();
      expect(sockets).toHaveLength(1);
      expect(store.getState().connection).toBe("connecting");
      expect(store.getState().toasts[0].text).toBe("Votre session a pris fin : vous êtes repassé en invité.");
      // As a guest there is no session to lose: no reconnection loop.
      store.receive({ type: "error", code: "session_revoked", message: "" });
      expect(sockets).toHaveLength(1);
    });

    it("logging out detaches the old socket and opens exactly one new one", async () => {
      const old = new (WebSocket as unknown as new () => { closed: boolean })();
      sockets.length = 0;
      (store as unknown as { socket: unknown }).socket = old;
      vi.stubGlobal("fetch", async () => ({ ok: true }));
      await store.logout();
      expect(old.closed).toBe(true);
      expect(saved["chessy.token"]).toBeUndefined();
      expect(sockets).toHaveLength(1);
      // The server's session_revoked for the old socket arrives late and is ignored by the store's socket guard.
      expect((store as unknown as { socket: unknown }).socket).toBe(sockets[0]);
    });

    it("explains the new quota errors", () => {
      store.receive({ type: "error", code: "flooded", message: "x" });
      expect(store.getState().toasts[0].text).toBe("Connexion coupée : trop de messages envoyés.");
    });
  });

  it("tracks the rematch handshake", () => {
    store.requestRematch();
    expect(store.getState().rematch).toBe("offered");
    store.receive({ type: "rematch_declined" });
    expect(store.getState().rematch).toBe("none");
  });
});

describe("accountAfterGame", () => {
  const me = { player_id: "p", username: "a", guest: false, elo: 1200, rank: 1, games: 0, wins: 0, draws: 0, losses: 0 };
  const over = { type: "game_over" as const, outcome: { type: "draw" as const, reason: "agreement" as const }, reward: null, rated: false, elo: null, reason: "" };

  it("suit l'avancement de l'évaluation, puis adopte l'Elo estimé", () => {
    const mid = accountAfterGame(me, { ...over, placement: { done: 2, total: 5 } } as never);
    expect(mid.placement).toEqual({ placed: false, done: 2, total: 5 });
    expect(mid.elo).toBe(1200);
    const end = accountAfterGame(mid, { ...over, placement: { done: 5, total: 5, elo: 940, before: 1200 } } as never);
    expect(end.placement).toEqual({ placed: true, done: 5, total: 5 });
    expect(end.elo).toBe(940);
  });
});
