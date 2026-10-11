import { describe, expect, it } from "vitest";
import type { BossForgeInfo } from "../../protocol";
import { bossForgeStep, claimMsg, legendaryNote, markRevealed, placeMsg, wasRevealed } from "./bossForge";

const info = (patch: Partial<BossForgeInfo>): BossForgeInfo => ({
  chapter: 2,
  state: "pending",
  skill: "freeze",
  deck_full: false,
  legendary_unavailable: false,
  ...patch,
});

describe("bossForgeStep", () => {
  it("n'affiche rien sans forge ou une fois placée", () => {
    expect(bossForgeStep(null, false)).toBe("none");
    expect(bossForgeStep(info({ state: "placed" }), true)).toBe("none");
  });

  it("attend la forge tant que la compétence n'est pas enregistrée", () => {
    expect(bossForgeStep(info({ state: "forging", skill: null }), false)).toBe("wait");
  });

  it("révèle d'abord, même si le deck est plein", () => {
    expect(bossForgeStep(info({}), false)).toBe("reveal");
    expect(bossForgeStep(info({ deck_full: true }), false)).toBe("reveal");
  });

  it("place d'office quand le deck a de la place, fait choisir quand il est plein", () => {
    expect(bossForgeStep(info({}), true)).toBe("place");
    expect(bossForgeStep(info({ deck_full: true }), true)).toBe("choose");
  });
});

describe("messages", () => {
  it("réclame la forge d'un chapitre", () => {
    expect(claimMsg(3)).toEqual({ type: "boss_forge_claim", chapter: 3 });
  });

  it("ne joint `replace` que pour remplacer une compétence", () => {
    expect(placeMsg(1, null)).toEqual({ type: "boss_forge_place", chapter: 1 });
    expect(placeMsg(1, "freeze")).toEqual({ type: "boss_forge_place", chapter: 1, replace: "freeze" });
  });
});

describe("notes et révélation vue", () => {
  it("signale l'Épique garantie quand plus aucune Légendaire n'est disponible", () => {
    expect(legendaryNote(info({ legendary_unavailable: true }))).toContain("Épique garantie");
    expect(legendaryNote(info({}))).toBeNull();
  });

  it("retient la révélation vue pour cette compétence seulement", () => {
    markRevealed(info({ chapter: 4, skill: "godhelp" }));
    expect(wasRevealed(info({ chapter: 4, skill: "godhelp" }))).toBe(true);
    expect(wasRevealed(info({ chapter: 4, skill: "freeze" }))).toBe(false);
    expect(wasRevealed(null)).toBe(false);
  });
});
