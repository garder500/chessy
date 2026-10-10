import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { CATALOG } from "../catalog";
import type { SkillHistoryEntry } from "../protocol";
import { dayLabel, describeEntry, filterHistory, groupByDay, historyStats, RULES } from "./collectionData";

beforeAll(() => setLang("fr"));

const entry = (id: number, over: Partial<SkillHistoryEntry> = {}): SkillHistoryEntry => ({
  id,
  skill: "freeze",
  change: "gained",
  source: "starter",
  at: "2026-10-07T10:00:00Z",
  ...over,
});

describe("collection", () => {
  it("fournit des règles pour chaque compétence du catalogue", () => {
    for (const c of CATALOG) expect(RULES[c.id], c.id).toBeTruthy();
    expect(Object.keys(RULES)).toHaveLength(CATALOG.length);
  });
});

describe("historique des compétences", () => {
  const all = [
    entry(5, { skill: "forged_9", source: "forged" }),
    entry(4, { skill: "clone", source: "stolen", other: "bob" }),
    entry(3, { skill: "imune", change: "lost", source: "taken", other: "alice" }),
    entry(2, { skill: "tornado", change: "lost", source: "replaced" }),
    entry(1, { skill: "freeze", source: "starter" }),
  ];

  it("filtre : forgées, obtenues (tout sauf forgées), perdues", () => {
    expect(filterHistory(all, "all")).toHaveLength(5);
    expect(filterHistory(all, "forged").map((e) => e.id)).toEqual([5]);
    expect(filterHistory(all, "obtained").map((e) => e.id)).toEqual([4, 1]);
    expect(filterHistory(all, "lost").map((e) => e.id)).toEqual([3, 2]);
    expect(historyStats(all)).toEqual({ forged: 1, obtained: 2, lost: 2 });
  });

  it("raconte chaque ligne", () => {
    expect(describeEntry(all[0])).toContain("Forgée");
    expect(describeEntry(all[1])).toBe("Volée à bob");
    expect(describeEntry(all[2])).toBe("Prise par alice après une défaite");
    expect(describeEntry(entry(9, { change: "lost", source: "taken" }))).toContain("adversaire");
    expect(describeEntry(entry(9, { source: "stolen" }))).toContain("sans compte");
    expect(describeEntry(all[3])).toContain("Remplacée");
    expect(describeEntry(all[4])).toBe("Deck de départ");
    expect(describeEntry(entry(9, { source: "earlier" }))).toContain("Déjà");
    expect(describeEntry(entry(9, { source: "refill" }))).toContain("Offerte");
    expect(describeEntry(entry(9, { source: "won" }))).toContain("Gagnée");
  });

  it("regroupe par jour sans changer l'ordre", () => {
    const now = new Date("2026-10-07T18:00:00Z");
    const mixed = [
      entry(4, { at: "2026-10-07T12:00:00Z" }),
      entry(3, { at: "2026-10-07T09:00:00Z" }),
      entry(2, { at: "2026-10-06T20:00:00Z" }),
      entry(1, { at: "2026-09-01T08:00:00Z" }),
    ];
    const days = groupByDay(mixed, now);
    expect(days.map((d) => [d.label, d.entries.map((e) => e.id)])).toEqual([
      ["Aujourd'hui", [4, 3]],
      ["Hier", [2]],
      [expect.stringContaining("septembre 2026"), [1]],
    ]);
    expect(dayLabel("pas une date", now)).toBe("Date inconnue");
  });
});
