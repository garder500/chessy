import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { fixtureGames, fixtureLive } from "./fixtures";
import {
  averageElo,
  countByFilter,
  delayText,
  emptyGamesText,
  filterGames,
  hasMore,
  liveTag,
  matchesFilter,
  mergePage,
  neutralReason,
  outcomeWinner,
  resultLine,
  opponentLabel,
  plyText,
  showsDelta,
  sortLive,
  spectatorsText,
  winnerText,
} from "./lists";

const games = fixtureGames();

beforeAll(() => setLang("fr"));

describe("Mes parties : filtre", () => {
  it("sépare classées, amicales et solo", () => {
    expect(filterGames(games, "all")).toHaveLength(6);
    expect(filterGames(games, "ranked").map((g) => g.game_id)).toEqual(["g-demo-1", "g-demo-4", "g-demo-6"]);
    expect(filterGames(games, "friendly").map((g) => g.game_id)).toEqual(["g-demo-3", "g-demo-5"]);
    expect(filterGames(games, "solo").map((g) => g.game_id)).toEqual(["g-demo-2"]);
  });

  it("ne classe jamais une partie solo parmi les classées ou les amicales", () => {
    const solo = { kind: "solo" as const, rated: true };
    expect(matchesFilter(solo, "ranked")).toBe(false);
    expect(matchesFilter(solo, "friendly")).toBe(false);
    expect(matchesFilter(solo, "solo")).toBe(true);
  });

  it("compte les parties de chaque filtre", () => {
    expect(countByFilter(games)).toEqual({ all: 6, ranked: 3, friendly: 2, solo: 1 });
  });

  it("explique une liste vide", () => {
    expect(emptyGamesText("all", 0)).toMatch(/pas encore/);
    expect(emptyGamesText("solo", 4)).toMatch(/solo/);
  });
});

describe("Mes parties : pagination", () => {
  it("fusionne les pages sans doublon", () => {
    const first = games.slice(0, 3);
    const second = games.slice(2, 6);
    expect(mergePage(first, second, 3).map((g) => g.game_id)).toEqual(games.map((g) => g.game_id));
    expect(mergePage(first, second, 0)).toBe(second);
  });

  it("sait s'il reste des parties", () => {
    expect(hasMore(20, 45, 20)).toBe(true);
    expect(hasMore(45, 45, 5)).toBe(false);
    expect(hasMore(20, 45, 0)).toBe(false);
  });
});

describe("Mes parties : lignes", () => {
  it("nomme l'adversaire du demandeur", () => {
    expect(opponentLabel(games[0])).toBe("ada");
    expect(opponentLabel(games[1])).toBe("IA (niveau 1400)");
    expect(opponentLabel(games[2])).toBe("lea");
    expect(opponentLabel(games[5])).toBe("Invité");
  });

  it("n'affiche la variation d'Elo que pour une partie classée comptée", () => {
    expect(showsDelta(games[0])).toBe(true);
    expect(showsDelta(games[1])).toBe(false);
    expect(showsDelta(games[2])).toBe(false);
    expect(showsDelta({ kind: "duel", rated: true, elo_delta: null })).toBe(false);
  });
});

describe("En direct", () => {
  it("trie par Elo moyen décroissant puis ancienneté", () => {
    const sorted = sortLive(fixtureLive());
    expect(sorted.map((g) => g.game_id)).toEqual(["g-live-2", "g-live-4", "g-live-1", "g-live-3"]);
  });

  it("à Elo égal, la partie la plus ancienne passe d'abord", () => {
    const a = { ...fixtureLive()[0], game_id: "a", started_at: "2026-10-07T09:00:00Z" };
    const b = { ...fixtureLive()[0], game_id: "b", started_at: "2026-10-07T08:00:00Z" };
    expect(sortLive([a, b]).map((g) => g.game_id)).toEqual(["b", "a"]);
  });

  it("ne modifie pas la liste d'origine", () => {
    const live = fixtureLive();
    const copy = [...live];
    sortLive(live);
    expect(live).toEqual(copy);
  });

  it("calcule l'Elo moyen avec des Elo manquants", () => {
    const g = fixtureLive();
    expect(averageElo(g[0])).toBe((1311 + 1284) / 2);
    expect(averageElo({ white: { username: null, elo: null, bot: false }, black: { username: null, elo: null, bot: false } })).toBe(0);
    expect(averageElo(g[3])).toBe((1500 + 1800) / 2);
  });

  it("étiquette les parties : Solo, Classée, Amicale", () => {
    const g = fixtureLive();
    expect(liveTag(g[0])).toBe("Classée");
    expect(liveTag(g[2])).toBe("Amicale");
    expect(liveTag(g[3])).toBe("Solo");
  });

  it("accorde les compteurs", () => {
    expect(plyText(0)).toBe("Pas encore de coup");
    expect(plyText(1)).toBe("1 coup");
    expect(plyText(18)).toBe("18 coups");
    expect(spectatorsText(0)).toBe("Aucun spectateur");
    expect(spectatorsText(1)).toBe("1 spectateur");
    expect(spectatorsText(12)).toBe("12 spectateurs");
  });

  it("annonce le délai de retransmission", () => {
    expect(delayText(30_000)).toBe("Retransmission différée de 30 s");
    expect(delayText(0)).toBeNull();
    expect(delayText(180_000)).toBe("Retransmission différée de 3 min");
    expect(winnerText("white")).toBe("Victoire des blancs");
    expect(winnerText(null)).toBe("Partie nulle");
  });
});

describe("résultat neutre", () => {
  it("annonce le vainqueur et le motif", () => {
    expect(resultLine({ type: "resignation", winner: "black" }, "resignation")).toBe("Victoire des noirs · Abandon");
    expect(resultLine({ type: "stalemate" }, "stalemate")).toBe("Partie nulle · Pat");
    expect(resultLine({ type: "checkmate", winner: "white" }, "")).toBe("Victoire des blancs · Échec et mat");
    expect(resultLine({ type: "ongoing" }, "")).toBe("");
  });

  it("trouve le vainqueur", () => {
    expect(outcomeWinner({ type: "timeout", winner: "white" })).toBe("white");
    expect(outcomeWinner({ type: "repetition" })).toBeNull();
    expect(outcomeWinner({ type: "ongoing" })).toBeUndefined();
    expect(neutralReason("agreed_draw")).toBe("Nulle par accord");
    expect(neutralReason("inconnu")).toBe("inconnu");
  });
});
