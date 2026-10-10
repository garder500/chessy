// Logique pure du mode Campagne (docs/spec-v6.md) : chapitres, étoiles, niveau à reprendre, textes des
// objectifs et des défis. Le serveur décide de tout (règles, étoiles, niveaux ouverts) ; le client lit.

import { t, type Params } from "./i18n";
import type { CampaignChallenge, CampaignLevel, CampaignObjective, SkillId } from "./protocol";
import type { Family } from "./catalog";

/** Étoiles sur les six niveaux ordinaires d'un chapitre qui ouvrent le boss. */
export const BOSS_GATE = 12;
export const CHAPTER_COUNT = 5;
/** Étoiles possibles par chapitre (six niveaux et le boss) et en tout. */
export const CHAPTER_STARS = 21;
export const TOTAL_STARS = CHAPTER_STARS * CHAPTER_COUNT;

export const STAR_WIN = 1;
export const STAR_OBJECTIVE = 2;
export const STAR_CHALLENGE = 4;

/** Famille de compétences enseignée par chaque chapitre (la couleur du chapitre est celle de la famille). */
export const CHAPTER_FAMILY: readonly Family[] = ["attack", "defense", "mobility", "control", "create"];

export const familyOfChapter = (chapter: number): Family => CHAPTER_FAMILY[chapter - 1] ?? "attack";

export function starCount(mask: number): number {
  return (mask & STAR_WIN ? 1 : 0) + (mask & STAR_OBJECTIVE ? 1 : 0) + (mask & STAR_CHALLENGE ? 1 : 0);
}

export const chapterOf = (levels: readonly CampaignLevel[], chapter: number) => levels.filter((l) => l.chapter === chapter);

/** Étoiles du chapitre : tous ses niveaux, boss compris. */
export function chapterTotal(levels: readonly CampaignLevel[], chapter: number): number {
  return chapterOf(levels, chapter).reduce((n, l) => n + starCount(l.stars), 0);
}

/** Étoiles qui comptent pour la porte du boss : les niveaux ordinaires seulement. */
export function gateStars(levels: readonly CampaignLevel[], chapter: number): number {
  return chapterOf(levels, chapter)
    .filter((l) => !l.boss)
    .reduce((n, l) => n + starCount(l.stars), 0);
}

export function totalStars(levels: readonly CampaignLevel[]): number {
  return levels.reduce((n, l) => n + starCount(l.stars), 0);
}

export const bossOf = (levels: readonly CampaignLevel[], chapter: number) => levels.find((l) => l.chapter === chapter && l.boss);

/** Un chapitre est ouvert quand son premier niveau l'est. */
export function chapterOpen(levels: readonly CampaignLevel[], chapter: number): boolean {
  return !!levels.find((l) => l.chapter === chapter && l.index === 1)?.unlocked;
}

/** Le niveau où reprendre : le dernier niveau ouvert, dans l'ordre de jeu. */
export function resumeLevel(levels: readonly CampaignLevel[]): CampaignLevel | undefined {
  return [...levels].filter((l) => l.unlocked).sort((a, b) => b.id - a.id)[0];
}

/** Le niveau qui suit `id` dans la campagne, s'il est ouvert. */
export function nextOpenLevel(levels: readonly CampaignLevel[], id: number): CampaignLevel | undefined {
  const next = [...levels].sort((a, b) => a.id - b.id).find((l) => l.id > id);
  return next?.unlocked ? next : undefined;
}

export const levelById = (levels: readonly CampaignLevel[] | null, id: number) => levels?.find((l) => l.id === id);

/** Pourquoi un niveau est fermé : le texte à afficher à la place du bouton de lancement. */
export function lockedReason(levels: readonly CampaignLevel[], level: CampaignLevel): string {
  if (level.unlocked) return "";
  if ((level.index === 1 || level.boss) && level.chapter > 1 && !chapterOpen(levels, level.chapter)) {
    return t("campaign.locked_chapter", { n: level.chapter - 1, boss: bossName(level.chapter - 1) });
  }
  if (level.boss) return t("campaign.locked_gate", { need: BOSS_GATE, have: gateStars(levels, level.chapter) });
  return t("campaign.locked_prev", { chapter: level.chapter, index: level.index - 1 });
}

export const levelLabel = (l: Pick<CampaignLevel, "chapter" | "index" | "boss">) =>
  l.boss ? t("campaign.boss_level", { n: l.chapter }) : t("campaign.level_label", { chapter: l.chapter, index: l.index });

export const levelName = (l: Pick<CampaignLevel, "id" | "chapter" | "boss">) => (l.boss ? bossName(l.chapter) : t(`campaign.level_${l.id}`));
export const bossName = (chapter: number) => t(`campaign.boss_${chapter}`);
export const chapterName = (chapter: number) => t(`campaign.chapter_${chapter}`);

const PIECE_KEY: Record<string, string> = {
  pawn: "pawn",
  knight: "knight",
  bishop: "bishop",
  rook: "rook",
  queen: "queen",
  king: "king",
};

/** Clé et paramètres de la phrase d'un objectif (« Mat avant le coup 30 »). */
export function objectiveText(o: CampaignObjective): string {
  const [key, params] = ((): [string, Params] => {
    switch (o.kind) {
      case "mate_before":
        return ["campaign.obj_mate_before", { moves: o.moves }];
      case "capture":
        return ["campaign.obj_capture", { piece: t(`campaign.piece_${PIECE_KEY[o.piece] ?? "pawn"}`) }];
      case "take":
        return ["campaign.obj_take", { count: o.count }];
      case "promote":
        return ["campaign.obj_promote", {}];
      case "lose_fewer":
        return ["campaign.obj_lose_fewer", { count: o.count }];
    }
  })();
  return t(key, params);
}

export function challengeText(c: CampaignChallenge): string {
  return t(`campaign.chal_${c.kind}`);
}

/** Les trois étoiles d'un niveau, dans l'ordre d'affichage, avec leur texte et si elles sont gagnées. */
export function starLines(level: Pick<CampaignLevel, "objective" | "challenge" | "stars">, mask = level.stars) {
  return [
    { bit: STAR_WIN, text: t("campaign.star_win"), kind: "win" as const, got: !!(mask & STAR_WIN) },
    { bit: STAR_OBJECTIVE, text: objectiveText(level.objective), kind: "objective" as const, got: !!(mask & STAR_OBJECTIVE) },
    { bit: STAR_CHALLENGE, text: challengeText(level.challenge), kind: "challenge" as const, got: !!(mask & STAR_CHALLENGE) },
  ];
}

// ---- main du joueur (chapitres 3 à 5) ------------------------------------------------

const HAND_KEY = "chessy.campaignHand";
export const MAX_HAND = 3;

type KeyValueStore = Pick<Storage, "getItem" | "setItem">;

function defaultStorage(): KeyValueStore | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

/** Les compétences choisies la dernière fois, limitées à celles du deck (et à trois). */
export function readHand(deck: readonly SkillId[], storage: KeyValueStore | null = defaultStorage()): SkillId[] {
  try {
    const raw = storage?.getItem(HAND_KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    if (!Array.isArray(list)) return [];
    return list.filter((s): s is SkillId => typeof s === "string" && deck.includes(s as SkillId)).slice(0, MAX_HAND);
  } catch {
    return [];
  }
}

export function writeHand(hand: readonly SkillId[], storage: KeyValueStore | null = defaultStorage()) {
  try {
    storage?.setItem(HAND_KEY, JSON.stringify(hand));
  } catch {
    // Stockage indisponible : la main ne sera pas retenue.
  }
}

/** Ajoute ou retire `skill` de la main (trois au plus). */
export function toggleHand(hand: readonly SkillId[], skill: SkillId): SkillId[] {
  if (hand.includes(skill)) return hand.filter((s) => s !== skill);
  return hand.length >= MAX_HAND ? [...hand] : [...hand, skill];
}

// ---- pastille « Nouveau » de l'écran Jouer ----------------------------------------------

const SEEN_KEY = "chessy.campaignSeen";

/** Le joueur a déjà ouvert la campagne : la pastille « Nouveau » disparaît. */
export function campaignSeen(storage: KeyValueStore | null = defaultStorage()): boolean {
  try {
    return storage?.getItem(SEEN_KEY) === "1";
  } catch {
    return false;
  }
}

export function markCampaignSeen(storage: KeyValueStore | null = defaultStorage()) {
  try {
    storage?.setItem(SEEN_KEY, "1");
  } catch {
    // Stockage indisponible : la pastille reviendra.
  }
}
