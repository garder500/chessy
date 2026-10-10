import type { CampaignChapter, CampaignLevel, CampaignStars, SkillId } from "./protocol";

/** Intitulés des trois étoiles d'un niveau, dans l'ordre du fil. */
export const STAR_LABELS = ["Victoire", "Objectif", "Défi"] as const;

export function countStars(stars: CampaignStars): number {
  return stars.filter(Boolean).length;
}

/** Le boss reste fermé tant que le chapitre n'a pas assez d'étoiles. */
export function isLocked(chapter: CampaignChapter, level: CampaignLevel): boolean {
  return level.boss && !chapter.boss_unlocked;
}

export function bossProgress(chapter: CampaignChapter): string {
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

export function chapterTitle(chapter: CampaignChapter): string {
  return `Chapitre ${chapter.chapter + 1} · ${chapter.name}`;
}
