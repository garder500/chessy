import { describe, expect, it } from "vitest";
import {
  bossProgress,
  type CampaignChapterView,
  chapterTitle,
  countStars,
  forgeNote,
  formatElo,
  isLocked,
  LEGENDARY_CHAPTER,
  splitDeck,
  toggleDeckPick,
  totalLabel,
  validPicks,
  withLiveForge,
} from "./campaign";
import type { BossForgeInfo, CampaignChapter, CampaignLevel, SkillId } from "./protocol";

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

  it("affiche la porte du boss sur 18 étoiles et le total sur 105", () => {
    expect(bossProgress({ stars: 12, boss_stars_required: 18 })).toBe("12/18");
    expect(totalLabel(42, 105)).toBe("42 / 105 ★");
  });

  it("formate l'Elo avec séparateur de milliers", () => {
    expect(formatElo(1000)).toBe("1 000");
    expect(formatElo(800)).toBe("800");
  });

  it("note la Légendaire possible ou épuisée au chapitre 5 seulement", () => {
    const forgeOf = (legendary_unavailable: boolean) => ({ chapter: LEGENDARY_CHAPTER, boss_forge: { chapter: LEGENDARY_CHAPTER, state: "forging", skill: null, deck_full: false, legendary_unavailable } }) as CampaignChapterView;
    expect(forgeNote(forgeOf(false))).toBe("Épique ou mieux, Légendaire possible");
    expect(forgeNote(forgeOf(true))).toBe("Plus aucune Légendaire disponible : Épique garantie");
    expect(forgeNote({ chapter: 1, boss_forge: null } as CampaignChapterView)).toBeNull();
  });

  it("remplace la forge d'un chapitre par le message en direct", () => {
    const chapters = [{ chapter: 0, boss_forge: null }, { chapter: 1, boss_forge: null }] as CampaignChapterView[];
    const live: BossForgeInfo = { chapter: 1, state: "pending", skill: "freeze", deck_full: false, legendary_unavailable: false };
    expect(withLiveForge(chapters, live)[1].boss_forge).toBe(live);
    expect(withLiveForge(chapters, live)[0].boss_forge).toBeNull();
  });

  it("sépare les uniques du deck des classiques choisissables avec les prêtées", () => {
    expect(splitDeck(["freeze", "remover"], ["clone"])).toEqual({ pickable: ["freeze", "clone"], extras: ["remover"] });
  });

  it("plafonne le deck choisi à trois compétences", () => {
    const full: SkillId[] = ["freeze", "clone", "trap"];
    expect(toggleDeckPick(full, "terminator")).toEqual(full);
    expect(toggleDeckPick(full, "trap")).toEqual(["freeze", "clone"]);
  });
});
