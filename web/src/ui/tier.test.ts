import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { nextTier, pointsToNextTier, tierOf, tierProgress } from "./tier";

beforeAll(() => setLang("fr"));

describe("tierOf", () => {
  it.each([
    [0, "Novice"],
    [1199, "Novice"],
    [1200, "Initié"],
    [1399, "Initié"],
    [1400, "Adepte"],
    [1599, "Adepte"],
    [1600, "Expert"],
    [1799, "Expert"],
    [1800, "Maître"],
    [2500, "Maître"],
  ])("%i -> %s", (elo, name) => {
    expect(tierOf(elo).name).toBe(name);
  });
});

describe("pointsToNextTier", () => {
  it("compte les points restants", () => {
    expect(pointsToNextTier(1200)).toEqual({ points: 200, tier: { name: "Adepte", min: 1400 } });
    expect(pointsToNextTier(1399)?.points).toBe(1);
    expect(pointsToNextTier(1100)?.points).toBe(100);
  });
  it("est nul au palier maximal", () => {
    expect(pointsToNextTier(1800)).toBeNull();
    expect(nextTier(2000)).toBeNull();
  });
});

describe("tierProgress", () => {
  it("reste entre 0 et 1", () => {
    expect(tierProgress(1200)).toBe(0);
    expect(tierProgress(1300)).toBe(0.5);
    expect(tierProgress(1800)).toBe(1);
    expect(tierProgress(100)).toBe(0);
    expect(tierProgress(1100)).toBe(0.5);
  });
});
