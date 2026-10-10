import type { CampaignChapter, CampaignLevel, CampaignStars } from "./protocol";

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

export function chapterTitle(chapter: CampaignChapter): string {
  return `Chapitre ${chapter.chapter + 1} · ${chapter.name}`;
}
