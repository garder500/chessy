import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { formatDelta, parseServerDate, presenceLabel, reasonText, relativeTime, sortFriends, winRate } from "./social";

beforeAll(() => setLang("fr"));

const now = new Date("2026-10-06T12:00:00Z");

describe("relativeTime", () => {
  it("formate les durées récentes", () => {
    expect(relativeTime("2026-10-06T11:59:40Z", now)).toBe("à l'instant");
    expect(relativeTime("2026-10-06T11:55:00Z", now)).toBe("il y a 5 min");
    expect(relativeTime("2026-10-06T09:00:00Z", now)).toBe("il y a 3 h");
    expect(relativeTime("2026-10-05T09:00:00Z", now)).toBe("hier");
    expect(relativeTime("2026-10-02T12:00:00Z", now)).toBe("il y a 4 j");
  });
  it("retombe sur une date complète au-delà d'une semaine", () => {
    expect(relativeTime("2026-09-01T12:00:00Z", now)).toMatch(/1er septembre 2026|1 septembre 2026/);
  });
  it("accepte le format SQLite en UTC", () => {
    expect(parseServerDate("2026-10-06 11:00:00")?.toISOString()).toBe("2026-10-06T11:00:00.000Z");
    expect(relativeTime("2026-10-06 11:00:00", now)).toBe("il y a 1 h");
  });
  it("tolère une valeur invalide", () => {
    expect(relativeTime("n'importe quoi", now)).toBe("");
  });
});

describe("reasonText", () => {
  it("dépend du résultat", () => {
    expect(reasonText("resignation", "win")).toBe("Abandon de l'adversaire");
    expect(reasonText("resignation", "loss")).toBe("Vous avez abandonné");
    expect(reasonText("agreed_draw", "draw")).toBe("Nulle par accord");
    expect(reasonText("inconnu", "win")).toBe("inconnu");
  });
});

describe("divers", () => {
  it("winRate", () => {
    expect(winRate(0, 0)).toBeNull();
    expect(winRate(1, 3)).toBe(33);
  });
  it("formatDelta", () => {
    expect(formatDelta(12)).toBe("+12");
    expect(formatDelta(-8)).toBe("−8");
    expect(formatDelta(0)).toBe("±0");
    expect(formatDelta(null)).toBe("—");
  });
  it("presenceLabel", () => {
    expect(presenceLabel("online", null, now)).toBe("En ligne");
    expect(presenceLabel("in_game", null, now)).toBe("En partie");
    expect(presenceLabel("offline", "2026-10-06T10:00:00Z", now)).toBe("Hors ligne · il y a 2 h");
    expect(presenceLabel("offline", null, now)).toBe("Hors ligne");
  });
  it("sortFriends : en ligne, en partie, hors ligne, puis alphabétique", () => {
    const sorted = sortFriends([
      { username: "zed", presence: "online" as const },
      { username: "bob", presence: "offline" as const },
      { username: "amy", presence: "in_game" as const },
      { username: "abe", presence: "online" as const },
    ]);
    expect(sorted.map((f) => f.username)).toEqual(["abe", "zed", "amy", "bob"]);
  });
});
