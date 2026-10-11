import { beforeAll, afterEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../i18n";
import { api, ApiError, gameErrorText } from "../api";
import { fixtureAnalysis, fixtureExplore, fixtureGames, fixtureLive, fixtureRecord, sq } from "./fixtures";

beforeAll(() => setLang("fr"));

function mockFetch(res: { status: number; body?: unknown } | Error) {
  const fn = vi.fn(async () => {
    if (res instanceof Error) throw res;
    return new Response(res.body === undefined ? null : JSON.stringify(res.body), { status: res.status });
  });
  vi.stubGlobal("fetch", fn);
  return fn;
}

const call = (fn: ReturnType<typeof mockFetch>) => fn.mock.calls[0] as unknown as [string, RequestInit];

afterEach(() => vi.unstubAllGlobals());

describe("API replays, analyse, direct", () => {
  it("liste mes parties avec pagination et Bearer", async () => {
    const fn = mockFetch({ status: 200, body: { total: 6, games: fixtureGames() } });
    const res = await api.myGames("tok", 20, 40);
    expect(res.games).toHaveLength(6);
    const [url, init] = call(fn);
    expect(url).toBe("/api/me/games?limit=20&offset=40");
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer tok");
  });

  it("lit une partie, avec ou sans jeton", async () => {
    const fn = mockFetch({ status: 200, body: fixtureRecord() });
    await api.game("g/1");
    const [url, init] = call(fn);
    expect(url).toBe("/api/games/g%2F1");
    expect((init.headers as Record<string, string>).Authorization).toBeUndefined();
    const withToken = mockFetch({ status: 200, body: fixtureRecord() });
    await api.game("g1", "tok");
    expect((call(withToken)[1].headers as Record<string, string>).Authorization).toBe("Bearer tok");
  });

  it("demande l'analyse à la profondeur choisie", async () => {
    const fn = mockFetch({ status: 200, body: fixtureAnalysis() });
    await api.analysis("g1", 4);
    expect(call(fn)[0]).toBe("/api/games/g1/analysis?depth=4");
    const fn2 = mockFetch({ status: 200, body: fixtureAnalysis() });
    await api.analysis("g1");
    expect(call(fn2)[0]).toBe("/api/games/g1/analysis?depth=3");
  });

  it("envoie la ligne complète à l'exploration", async () => {
    const record = fixtureRecord();
    const fn = mockFetch({ status: 200, body: fixtureExplore(record, 4) });
    const line = [{ type: "move" as const, from: sq("e2"), to: sq("e4") }];
    await api.explore("g1", { ply: 4, line, depth: 3 }, "tok");
    const [url, init] = call(fn);
    expect(url).toBe("/api/games/g1/explore");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({ ply: 4, line, depth: 3 });
  });

  it("liste les parties en direct", async () => {
    const fn = mockFetch({ status: 200, body: { games: fixtureLive() } });
    const res = await api.live(10);
    expect(res.games).toHaveLength(4);
    expect(call(fn)[0]).toBe("/api/live?limit=10");
  });
});

describe("erreurs de replay", () => {
  it("distingue partie introuvable, replay indisponible et accès refusé", async () => {
    mockFetch({ status: 404, body: { error: "not_found" } });
    const notFound = await api.game("x").catch((e) => e);
    expect(notFound).toBeInstanceOf(ApiError);
    expect(gameErrorText(notFound)).toBe("Partie introuvable.");

    mockFetch({ status: 409, body: { error: "no_replay" } });
    expect(gameErrorText(await api.game("x").catch((e) => e))).toBe("Replay indisponible pour cette partie.");

    mockFetch({ status: 404 });
    expect(gameErrorText(await api.game("x").catch((e) => e))).toBe("Partie introuvable.");

    mockFetch({ status: 403, body: { error: "forbidden" } });
    expect(gameErrorText(await api.game("x").catch((e) => e))).toMatch(/pas accessible/);
  });

  it("traite aussi le code no_replay sur un 404", () => {
    expect(gameErrorText(new ApiError("no_replay", 404))).toBe("Replay indisponible pour cette partie.");
  });

  it("explique les erreurs d'exploration, d'analyse, réseau et serveur", () => {
    expect(gameErrorText(new ApiError("illegal_action", 400))).toMatch(/impossible/i);
    expect(gameErrorText(new ApiError("bad_ply", 400))).toMatch(/introuvable/);
    expect(gameErrorText(new ApiError("analysis_timeout", 504))).toMatch(/analyse a échoué/i);
    expect(gameErrorText(new ApiError("network", 0))).toMatch(/joindre le serveur/);
    expect(gameErrorText(new ApiError("http_500", 500))).toMatch(/serveur a rencontré/);
    expect(gameErrorText(new Error("?"))).toMatch(/erreur est survenue/);
  });

  it("propage une erreur réseau", async () => {
    mockFetch(new TypeError("offline"));
    await expect(api.live()).rejects.toMatchObject({ code: "network" });
  });
});
