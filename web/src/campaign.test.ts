import { describe, expect, it } from "vitest";
import {
  bossProgress,
  type CampaignChapterView,
  type CampaignLevelView,
  chapterTitle,
  countStars,
  currentLevel,
  currentTitle,
  defaultSelection,
  forgeNote,
  formatElo,
  isLocked,
  LEGENDARY_CHAPTER,
  lockReason,
  requiredPicks,
  splitDeck,
  toggleDeckPick,
  totalLabel,
  validPicks,
  withLiveForge,
} from "./campaign";
import { MAP_LAYOUTS, percent } from "./campaignMap";
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
  unlocked: true,
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
  unlocked: true,
  levels: [],
});

describe("campagne", () => {
  it("exige trois choix, ou tout le deck s'il est plus court", () => {
    const skills = ["a", "b", "c", "d"] as unknown as SkillId[];
    expect(requiredPicks(skills)).toBe(3);
    expect(requiredPicks(skills.slice(0, 2))).toBe(2);
    expect(requiredPicks([])).toBe(0);
  });

  it("compte les étoiles obtenues", () => {
    expect(countStars([true, false, true])).toBe(2);
    expect(countStars([false, false, false])).toBe(0);
  });

  it("ne lit que le drapeau du serveur pour verrouiller un niveau", () => {
    expect(isLocked({ unlocked: false })).toBe(true);
    expect(isLocked({ unlocked: true })).toBe(false);
  });

  describe("progression", () => {
    const lv = (n: number, over: Partial<CampaignLevelView> = {}) => ({ ...level(n === 6), level: n, name: `N${n}`, ...over }) as CampaignLevelView;
    const ch = (n: number, wonCount: number, unlocked = true): CampaignChapterView => ({
      ...chapter(false),
      chapter: n,
      unlocked,
      levels: Array.from({ length: 7 }, (_, i) => lv(i, { unlocked: unlocked && i <= wonCount, best: [i < wonCount, false, false] })),
    }) as CampaignChapterView;

    it("sélectionne le chapitre ouvert le plus avancé et son premier niveau non gagné", () => {
      expect(defaultSelection([ch(0, 7), ch(1, 3), ch(2, 0, false)])).toEqual({ chapter: 1, level: 3 });
      expect(defaultSelection([ch(0, 0), ch(1, 0, false)])).toEqual({ chapter: 0, level: 0 });
    });

    it("retombe sur le dernier niveau d'un chapitre entièrement gagné", () => {
      expect(currentLevel(ch(0, 7))).toBe(6);
    });

    it("explique pourquoi un niveau est fermé", () => {
      const chapters = [ch(0, 4), ch(1, 0, false)];
      expect(lockReason(chapters, chapters[0], chapters[0].levels[4])).toBeNull();
      expect(lockReason(chapters, chapters[0], chapters[0].levels[6])).toBe("Il manque 5 ★ dans ce chapitre pour ouvrir la porte du boss.");
      expect(lockReason(chapters, chapters[0], { ...chapters[0].levels[5], unlocked: false })).toBe("Gagnez d'abord N4.");
      expect(lockReason(chapters, chapters[1], chapters[1].levels[0])).toBe("Battez N6 pour ouvrir ce chapitre.");
    });

    it("garde le dernier titre obtenu", () => {
      expect(currentTitle([{ title: "A", title_earned: true }, { title: "B", title_earned: true }, { title: "C", title_earned: false }])).toBe("B");
      expect(currentTitle([{ title: "A", title_earned: false }])).toBeNull();
    });

    it("place un point par niveau sur chaque carte", () => {
      expect(MAP_LAYOUTS.desktop.nodes).toHaveLength(7);
      expect(MAP_LAYOUTS.phone.nodes).toHaveLength(7);
      expect(percent(80, 800)).toBe("10%");
    });
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
