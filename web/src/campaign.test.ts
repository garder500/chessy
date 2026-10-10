import { describe, expect, it } from "vitest";
import {
  BOSS_GATE,
  chapterTotal,
  gateStars,
  nextOpenLevel,
  readHand,
  resumeLevel,
  starCount,
  toggleHand,
  totalStars,
  writeHand,
} from "./campaign";
import type { CampaignLevel, SkillId } from "./protocol";

const level = (id: number, over: Partial<CampaignLevel> = {}): CampaignLevel => ({
  id,
  chapter: Math.floor(id / 10),
  index: id % 10,
  boss: id % 10 === 7,
  elo: 400,
  hand: [],
  choose: false,
  enemy: [],
  objective: { kind: "promote" },
  challenge: { kind: "keep_queen" },
  stars: 0,
  unlocked: false,
  forge_pending: false,
  ...over,
});

const memory = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};

describe("étoiles", () => {
  it("compte les bits", () => {
    expect(starCount(0)).toBe(0);
    expect(starCount(1)).toBe(1);
    expect(starCount(5)).toBe(2);
    expect(starCount(7)).toBe(3);
  });

  it("la porte du boss ignore le boss lui-même", () => {
    const levels = [level(11, { stars: 7 }), level(12, { stars: 3 }), level(17, { boss: true, stars: 7 })];
    expect(gateStars(levels, 1)).toBe(5);
    expect(chapterTotal(levels, 1)).toBe(8);
    expect(totalStars(levels)).toBe(8);
    expect(BOSS_GATE).toBe(12);
  });
});

describe("progression", () => {
  const levels = [level(11, { unlocked: true }), level(12, { unlocked: true }), level(13)];

  it("reprend au dernier niveau ouvert", () => {
    expect(resumeLevel(levels)?.id).toBe(12);
  });

  it("propose le suivant seulement s'il est ouvert", () => {
    expect(nextOpenLevel(levels, 11)?.id).toBe(12);
    expect(nextOpenLevel(levels, 12)).toBeUndefined();
  });
});

describe("main du joueur", () => {
  const deck = ["a", "b", "c", "d"] as unknown as SkillId[];

  it("limite à trois compétences", () => {
    let hand: SkillId[] = [];
    for (const s of deck) hand = toggleHand(hand, s);
    expect(hand).toEqual(deck.slice(0, 3));
    expect(toggleHand(hand, deck[0])).toEqual(deck.slice(1, 3));
  });

  it("relit la main en la filtrant sur le deck", () => {
    const store = memory();
    writeHand(["a", "zz", "b"] as unknown as SkillId[], store);
    expect(readHand(deck, store)).toEqual(["a", "b"]);
    store.setItem("chessy.campaignHand", "pas du json");
    expect(readHand(deck, store)).toEqual([]);
  });
});
