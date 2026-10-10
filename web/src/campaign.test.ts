import { describe, expect, it } from "vitest";
import { bossProgress, chapterTitle, countStars, isLocked } from "./campaign";
import type { CampaignChapter, CampaignLevel } from "./protocol";

const level = (boss: boolean): CampaignLevel => ({
  level: boss ? 6 : 0,
  name: "Test",
  elo: 400,
  boss,
  player_deck: [],
  bot_deck: [],
  objective: null,
  challenge: null,
  best: [false, false, false],
  rewarded: false,
});

const chapter = (boss_unlocked: boolean): CampaignChapter => ({
  chapter: 0,
  family: "attack",
  name: "Attaque",
  available: true,
  stars: 7,
  boss_stars_required: 12,
  boss_unlocked,
  levels: [],
});

describe("campagne", () => {
  it("compte les étoiles obtenues", () => {
    expect(countStars([true, false, true])).toBe(2);
    expect(countStars([false, false, false])).toBe(0);
  });

  it("ne verrouille que le boss d'un chapitre fermé", () => {
    expect(isLocked(chapter(false), level(true))).toBe(true);
    expect(isLocked(chapter(true), level(true))).toBe(false);
    expect(isLocked(chapter(false), level(false))).toBe(false);
  });

  it("formate la progression vers le boss et le titre", () => {
    expect(bossProgress(chapter(false))).toBe("7/12");
    expect(chapterTitle(chapter(false))).toBe("Chapitre 1 · Attaque");
  });
});
