import { describe, expect, it } from "vitest";
import { bossProgress, chapterTitle, countStars, isLocked, toggleDeckPick, validPicks } from "./campaign";
import type { CampaignChapter, CampaignLevel, SkillId } from "./protocol";

const level = (boss: boolean): CampaignLevel => ({
  level: boss ? 6 : 0,
  name: "Test",
  elo: 400,
  boss,
  player_deck: [],
  bot_deck: [],
  deck_choice: false,
  start_fen: null,
  human_color: null,
  objective: null,
  challenge: null,
  best: [false, false, false],
  rewarded: false,
});

const chapter = (boss_unlocked: boolean): CampaignChapter => ({
  chapter: 0,
  family: "attack",
  name: "Attaque",
  title: "Maître d'armes",
  title_earned: false,
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

  it("ajoute et retire une compétence du deck choisi", () => {
    expect(toggleDeckPick([], "freeze")).toEqual(["freeze"]);
    expect(toggleDeckPick(["freeze", "clone"], "freeze")).toEqual(["clone"]);
  });

  it("écarte les choix sortis du deck", () => {
    expect(validPicks(["freeze", "clone"], ["clone", "trap"])).toEqual(["clone"]);
  });

  it("plafonne le deck choisi à trois compétences", () => {
    const full: SkillId[] = ["freeze", "clone", "trap"];
    expect(toggleDeckPick(full, "terminator")).toEqual(full);
    expect(toggleDeckPick(full, "trap")).toEqual(["freeze", "clone"]);
  });
});
