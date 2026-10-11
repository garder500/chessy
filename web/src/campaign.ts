import type { Rarity } from "./forged";
import type { BossForgeInfo, CampaignChapter, CampaignLevel, CampaignStars, SkillId } from "./protocol";
import { skillInfo } from "./skills";

export interface ForgeTableRow {
  rarity: Rarity;
  percent: number;
}

/** Niveau tel que `GET /api/campaign` le rend : champs ajoutés au type de base du protocole. */
export interface CampaignLevelView extends CampaignLevel {
  lent: SkillId[];
  move_limit: number | null;
  /** Présent seulement après plusieurs défaites d'affilée. */
  hint?: string;
}

export interface CampaignChapterView extends Omit<CampaignChapter, "levels"> {
  forge_table: ForgeTableRow[];
  boss_forge: BossForgeInfo | null;
  levels: CampaignLevelView[];
}

export interface CampaignView {
  chapters: CampaignChapterView[];
  total_stars: number;
  max_stars: number;
}

/** Dernier chapitre : sa forge est la seule où la Légendaire est possible. */
export const LEGENDARY_CHAPTER = 4;

export function totalLabel(total: number, max: number): string {
  return `${total} / ${max} ★`;
}

/** Séparateur de milliers fin insécable, comme la typographie française. */
export function formatElo(elo: number): string {
  return String(elo).replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

export function forgeNote(chapter: CampaignChapterView): string | null {
  if (chapter.chapter !== LEGENDARY_CHAPTER) return null;
  return chapter.boss_forge?.legendary_unavailable
    ? "Plus aucune Légendaire disponible : Épique garantie"
    : "Épique ou mieux, Légendaire possible";
}

/** Le message `boss_forge` du serveur est plus récent que la liste REST du chapitre. */
export function withLiveForge(chapters: CampaignChapterView[], live: BossForgeInfo | null): CampaignChapterView[] {
  if (!live) return chapters;
  return chapters.map((c) => (c.chapter === live.chapter ? { ...c, boss_forge: live } : c));
}

/** Les uniques du deck s'ajoutent d'office aux trois choisies ; le reste se choisit avec les prêtées. */
export function splitDeck(deck: SkillId[], lent: SkillId[]): { pickable: SkillId[]; extras: SkillId[] } {
  const extras = deck.filter((s) => skillInfo(s).unique);
  return { pickable: [...deck.filter((s) => !extras.includes(s)), ...lent], extras };
}

/** Intitulés des trois étoiles d'un niveau, dans l'ordre du fil. */
export const STAR_LABELS = ["Victoire", "Objectif", "Défi"] as const;

/** Numéro de niveau du boss d'un chapitre. */
export const BOSS_LEVEL = 6;

export function countStars(stars: CampaignStars): number {
  return stars.filter(Boolean).length;
}

/** Le boss reste fermé tant que le chapitre n'a pas assez d'étoiles. */
export function isLocked(chapter: Pick<CampaignChapter, "boss_unlocked">, level: Pick<CampaignLevel, "boss">): boolean {
  return level.boss && !chapter.boss_unlocked;
}

export function bossProgress(chapter: Pick<CampaignChapter, "stars" | "boss_stars_required">): string {
  return `${chapter.stars}/${chapter.boss_stars_required}`;
}

/** Nombre maximal de compétences emmenées dans un niveau à deck au choix. */
export const MAX_DECK_PICKS = 3;

export function toggleDeckPick(picked: SkillId[], skill: SkillId): SkillId[] {
  if (picked.includes(skill)) return picked.filter((s) => s !== skill);
  return picked.length < MAX_DECK_PICKS ? [...picked, skill] : picked;
}

/** Écarte les choix qui ne sont plus dans le deck (après un `deck_update`). */
export function validPicks(picked: SkillId[], deck: SkillId[]): SkillId[] {
  return picked.filter((s) => deck.includes(s));
}

export function colorLabel(color: "white" | "black"): string {
  return color === "white" ? "les blancs" : "les noirs";
}

export function chapterTitle(chapter: Pick<CampaignChapter, "chapter" | "name">): string {
  return `Chapitre ${chapter.chapter + 1} · ${chapter.name}`;
}
