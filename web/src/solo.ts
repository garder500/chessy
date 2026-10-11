// Logique pure du mode Solo (jouer contre l'IA) : niveaux d'Elo, paliers, réglage mémorisé.

import { t } from "./i18n";
import type { SoloColor } from "./protocol";

export const SOLO_MIN = 400;
export const SOLO_MAX = 2800;
export const SOLO_STEP = 50;
export const SOLO_KEY = "chessy.solo";

export interface SoloSetting {
  elo: number;
  color: SoloColor;
}

export const SOLO_DEFAULT: SoloSetting = { elo: 1200, color: "random" };

export interface SoloTier {
  name: string;
  /** Elo minimal du palier. */
  min: number;
  /** Comportement de l'IA à ce niveau. */
  blurb: string;
}

/** Palier dont le nom et la description sont traduits à chaque lecture (la langue peut changer). */
const tier = (id: string, min: number): SoloTier => ({
  min,
  get name() {
    return t(`solo.tier_${id}_name`);
  },
  get blurb() {
    return t(`solo.tier_${id}_blurb`);
  },
});

export const SOLO_TIERS: readonly SoloTier[] = [
  tier("beginner", SOLO_MIN),
  tier("amateur", 800),
  tier("club", 1200),
  tier("expert", 1600),
  tier("master", 2000),
  tier("grandmaster", 2400),
];

/** Palier nommé d'un niveau (< 800 Débutant, < 1200 Amateur, < 1600 Club, < 2000 Expert, < 2400 Maître, sinon Grand Maître). */
export function soloTier(elo: number): SoloTier {
  let found = SOLO_TIERS[0];
  for (const tr of SOLO_TIERS) if (elo >= tr.min) found = tr;
  return found;
}

/** Ramène une valeur dans [400, 2800] sur la grille de 50. Non numérique : niveau par défaut. */
export function clampElo(value: number): number {
  if (!Number.isFinite(value)) return SOLO_DEFAULT.elo;
  const snapped = Math.round(value / SOLO_STEP) * SOLO_STEP;
  return Math.min(SOLO_MAX, Math.max(SOLO_MIN, snapped));
}

export function isSoloColor(value: unknown): value is SoloColor {
  return value === "white" || value === "black" || value === "random";
}

type KeyValueStore = Pick<Storage, "getItem" | "setItem">;

function defaultStorage(): KeyValueStore | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

/** Dernier réglage mémorisé (valeurs corrigées), sinon le réglage par défaut. */
export function readSolo(storage: KeyValueStore | null = defaultStorage()): SoloSetting {
  try {
    const raw = storage?.getItem(SOLO_KEY);
    if (!raw) return SOLO_DEFAULT;
    const data = JSON.parse(raw) as Partial<SoloSetting> | null;
    return {
      elo: typeof data?.elo === "number" ? clampElo(data.elo) : SOLO_DEFAULT.elo,
      color: isSoloColor(data?.color) ? data.color : SOLO_DEFAULT.color,
    };
  } catch {
    return SOLO_DEFAULT;
  }
}

export function writeSolo(setting: SoloSetting, storage: KeyValueStore | null = defaultStorage()) {
  try {
    storage?.setItem(SOLO_KEY, JSON.stringify(setting));
  } catch {
    // Stockage indisponible (navigation privée) : le réglage ne sera pas retenu.
  }
}
