// Navigation d'un replay : position courante, lecture automatique et vitesse. Pur et testable.

import { intlLocale } from "../i18n";

export const SPEEDS = [0.5, 1, 2] as const;
export type Speed = (typeof SPEEDS)[number];

/** Durée d'un pas de lecture à la vitesse 1×. */
export const BASE_STEP_MS = 1400;

export interface ReplayNav {
  /** Indice de la position affichée (0 = position initiale). */
  index: number;
  /** Dernier indice possible (nombre d'actions). */
  max: number;
  playing: boolean;
  speed: Speed;
}

export type NavAction =
  | { type: "first" }
  | { type: "prev" }
  | { type: "next" }
  | { type: "last" }
  | { type: "goto"; index: number }
  | { type: "toggle" }
  | { type: "play" }
  | { type: "pause" }
  | { type: "speed"; speed: Speed }
  /** Un pas de la lecture automatique. */
  | { type: "tick" }
  /** Nouvelle partie (ou nouvelle longueur) : repart au début. */
  | { type: "reset"; max: number };

export function initialNav(max: number, index = 0): ReplayNav {
  const m = Math.max(0, Math.floor(max));
  return { index: clampIndex(index, m), max: m, playing: false, speed: 1 };
}

export function clampIndex(index: number, max: number): number {
  if (!Number.isFinite(index)) return 0;
  return Math.min(Math.max(0, Math.round(index)), Math.max(0, max));
}

/** Délai entre deux pas à la vitesse donnée. */
export function stepDelay(speed: Speed): number {
  return Math.round(BASE_STEP_MS / speed);
}

export function navReduce(state: ReplayNav, action: NavAction): ReplayNav {
  switch (action.type) {
    case "first":
      return { ...state, index: 0, playing: false };
    case "last":
      return { ...state, index: state.max, playing: false };
    case "prev":
      return { ...state, index: clampIndex(state.index - 1, state.max), playing: false };
    case "next": {
      const index = clampIndex(state.index + 1, state.max);
      return { ...state, index, playing: false };
    }
    case "goto":
      return { ...state, index: clampIndex(action.index, state.max), playing: false };
    case "play":
      // Relancer depuis la fin repart du début.
      return state.max === 0 ? state : { ...state, index: state.index >= state.max ? 0 : state.index, playing: true };
    case "pause":
      return { ...state, playing: false };
    case "toggle":
      return navReduce(state, { type: state.playing ? "pause" : "play" });
    case "speed":
      return { ...state, speed: action.speed };
    case "tick": {
      if (!state.playing) return state;
      const index = clampIndex(state.index + 1, state.max);
      return { ...state, index, playing: index < state.max };
    }
    case "reset":
      return initialNav(action.max);
  }
}

/** Libellé de vitesse : `0,5×`, `1×`, `2×`. */
export function speedLabel(speed: Speed): string {
  return `${speed.toLocaleString(intlLocale())}×`;
}

/** Touches de navigation d'un replay. `null` = touche ignorée. */
export function keyToNav(key: string): NavAction | null {
  switch (key) {
    case "ArrowLeft":
      return { type: "prev" };
    case "ArrowRight":
      return { type: "next" };
    case "Home":
      return { type: "first" };
    case "End":
      return { type: "last" };
    case " ":
    case "Spacebar":
      return { type: "toggle" };
    default:
      return null;
  }
}

interface KeyLike {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  target?: { tagName?: string; isContentEditable?: boolean } | null;
}

/**
 * Faut-il traiter cette touche comme un raccourci de replay ? Non avec un modificateur, dans un champ de
 * saisie ou une liste déroulante ; la barre d'espace est laissée aux boutons et liens focalisés (ils la
 * traitent eux-mêmes), sinon une même frappe agirait deux fois.
 */
export function shouldHandleKey(e: KeyLike): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey) return false;
  const tag = (e.target?.tagName ?? "").toUpperCase();
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || e.target?.isContentEditable) return false;
  if ((e.key === " " || e.key === "Spacebar") && (tag === "BUTTON" || tag === "A" || tag === "SUMMARY")) return false;
  return true;
}
