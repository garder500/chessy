import { describe, expect, it } from "vitest";
import type { CampaignContext } from "../../protocol";
import { bannerModel, moveNumber } from "./objectiveBanner";

const level: CampaignContext = { chapter: 2, level: 3, move_limit: 30, objective: "Mater avant le coup 30", challenge: null };

describe("moveNumber", () => {
  it("numérote la paire blanc + noir pareil", () => {
    expect(moveNumber(0)).toBe(1);
    expect(moveNumber(1)).toBe(1);
    expect(moveNumber(2)).toBe(2);
    expect(moveNumber(3)).toBe(2);
  });
});

describe("bannerModel", () => {
  it("n'a pas de compteur sans limite", () => {
    expect(bannerModel({ ...level, move_limit: null }, 40).counter).toBeNull();
  });

  it("reste dans les temps jusqu'au coup 29", () => {
    expect(bannerModel(level, 56).counter).toEqual({ move: 29, limit: 30, exceeded: false });
  });

  it("passe en dépassé au coup 30", () => {
    expect(bannerModel(level, 58).counter).toEqual({ move: 30, limit: 30, exceeded: true });
    expect(bannerModel(level, 59).counter?.exceeded).toBe(true);
  });

  it("reprend les textes du niveau", () => {
    const model = bannerModel({ ...level, challenge: "Sans perdre de pièce" }, 0);
    expect(model.objective).toBe("Mater avant le coup 30");
    expect(model.challenge).toBe("Sans perdre de pièce");
  });
});
