// Paliers de classement : < 1200 Novice, 1200–1399 Initié, 1400–1599 Adepte,
// 1600–1799 Expert, ≥ 1800 Maître.

import { t } from "../i18n";

export interface Tier {
  name: string;
  /** Elo minimal du palier. */
  min: number;
}

/** Palier dont le nom est traduit à chaque lecture (la langue peut changer). */
const tier = (id: string, min: number): Tier => ({
  min,
  get name() {
    return t(`tier.${id}`);
  },
});

export const TIERS: readonly Tier[] = [
  tier("novice", Number.NEGATIVE_INFINITY),
  tier("initiate", 1200),
  tier("adept", 1400),
  tier("expert", 1600),
  tier("master", 1800),
];

export function tierIndex(elo: number): number {
  let idx = 0;
  TIERS.forEach((t, i) => {
    if (elo >= t.min) idx = i;
  });
  return idx;
}

export function tierOf(elo: number): Tier {
  return TIERS[tierIndex(elo)];
}

export function nextTier(elo: number): Tier | null {
  return TIERS[tierIndex(elo) + 1] ?? null;
}

/** Points d'Elo restants avant le palier suivant (null au palier maximal). */
export function pointsToNextTier(elo: number): { points: number; tier: Tier } | null {
  const next = nextTier(elo);
  return next ? { points: next.min - elo, tier: next } : null;
}

/** Progression 0..1 dans le palier courant (1 au palier maximal ; Novice démarre à 1000). */
export function tierProgress(elo: number): number {
  const idx = tierIndex(elo);
  const next = TIERS[idx + 1];
  if (!next) return 1;
  const floor = idx === 0 ? 1000 : TIERS[idx].min;
  return Math.min(1, Math.max(0, (elo - floor) / (next.min - floor)));
}
