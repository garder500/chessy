// Thèmes (docs/spec-v4.md §4) : plateau, jeu de pièces, accent, mode de déplacement, animations, premoves.
// Persisté dans `chessy.theme`, appliqué à la racine du document via des variables CSS et lu par Phaser.
import { useSyncExternalStore } from "react";

export type BoardThemeId = "glacier" | "graphite" | "emerald" | "walnut" | "ocean" | "amethyst" | "coral";
export type PieceSetId = "cburnett" | "classic" | "neon" | "gold" | "ember";
export type AccentId = "jade" | "cyan" | "gold" | "blue" | "violet" | "coral" | "amber" | "mint" | "rose";
export type MoveMode = "drag" | "click";
/** Apparence de l'interface : suit le système, ou forcée en clair / sombre. */
export type ColorMode = "system" | "light" | "dark";

export const BOARD_THEMES: { id: BoardThemeId; label: string; light: string; dark: string; flat?: boolean }[] = [
  { id: "glacier", label: "Glacier", light: "#e4eaf6", dark: "#7f96c2", flat: true },
  { id: "graphite", label: "Graphite", light: "#cdd1d9", dark: "#69727f" },
  { id: "emerald", label: "Émeraude", light: "#eeeed2", dark: "#769656" },
  { id: "walnut", label: "Noyer", light: "#f0d9b5", dark: "#b58863" },
  { id: "ocean", label: "Océan", light: "#dce6f2", dark: "#5b7fa6" },
  { id: "amethyst", label: "Améthyste", light: "#e3d8f1", dark: "#8467b3" },
  { id: "coral", label: "Corail", light: "#fbe3d4", dark: "#d9805f" },
];

/** `white`/`black` : couleurs de base des deux camps ; `null` = ivoire/ébène d'origine. */
export const PIECE_SETS: { id: PieceSetId; label: string; white: string | null; black: string | null }[] = [
  { id: "cburnett", label: "Staunton", white: null, black: null },
  { id: "classic", label: "Classique", white: null, black: null },
  { id: "neon", label: "Néon", white: "#5ce1e6", black: "#ff5fc8" },
  { id: "gold", label: "Or et argent", white: "#f2c94c", black: "#aab4c3" },
  { id: "ember", label: "Braise et azur", white: "#ff6b5a", black: "#5aa9ff" },
];

export const ACCENTS: { id: AccentId; label: string; color: string }[] = [
  { id: "jade", label: "Jade", color: "#1fb89a" },
  { id: "cyan", label: "Cyan", color: "#3de0ff" },
  { id: "gold", label: "Or", color: "#e3c98a" },
  { id: "blue", label: "Bleu", color: "#8fb4ff" },
  { id: "violet", label: "Violet", color: "#b79cff" },
  { id: "coral", label: "Corail", color: "#ee8272" },
  { id: "amber", label: "Ambre", color: "#eec06a" },
  { id: "mint", label: "Menthe", color: "#5fd0a0" },
  { id: "rose", label: "Rose", color: "#f08fc0" },
];

export interface ThemeSettings {
  board: BoardThemeId;
  pieces: PieceSetId;
  accent: AccentId;
  mode: ColorMode;
  move: MoveMode;
  reduceMotion: boolean;
  /** Premoves : un coup posé pendant le tour de l'adversaire. */
  premove: boolean;
}

export const THEME_KEY = "chessy.theme";
/** Réglages enregistrés avant l'interface « Jade » : le décor par défaut change une fois, puis le choix du joueur reprend. */
const THEME_SKIN_KEY = "chessy.theme.skin";
const THEME_SKIN = "jade";

export const THEME_DEFAULTS: ThemeSettings = {
  board: "glacier",
  pieces: "cburnett",
  accent: "jade",
  mode: "system",
  move: "drag",
  reduceMotion: false,
  premove: true,
};

const oneOf = <T extends string>(list: readonly { id: T }[], v: unknown, fallback: T): T =>
  list.some((x) => x.id === v) ? (v as T) : fallback;

export function sanitizeTheme(raw: unknown, defaults: ThemeSettings = THEME_DEFAULTS): ThemeSettings {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  return {
    board: oneOf(BOARD_THEMES, o.board, defaults.board),
    pieces: oneOf(PIECE_SETS, o.pieces, defaults.pieces),
    accent: oneOf(ACCENTS, o.accent, defaults.accent),
    mode: o.mode === "light" || o.mode === "dark" || o.mode === "system" ? o.mode : defaults.mode,
    move: o.move === "click" || o.move === "drag" ? o.move : defaults.move,
    reduceMotion: typeof o.reduceMotion === "boolean" ? o.reduceMotion : defaults.reduceMotion,
    premove: typeof o.premove === "boolean" ? o.premove : defaults.premove,
  };
}

export const boardTheme = (id: BoardThemeId) => BOARD_THEMES.find((b) => b.id === id) ?? BOARD_THEMES[0];
export const pieceSet = (id: PieceSetId) => PIECE_SETS.find((p) => p.id === id) ?? PIECE_SETS[0];
export const accentColor = (id: AccentId) => (ACCENTS.find((a) => a.id === id) ?? ACCENTS[0]).color;

// ---- couleurs ---------------------------------------------------------------------------------

export const hexToNum = (hex: string) => parseInt(hex.replace("#", ""), 16);

/** Mélange `a` vers `b` (t = 0 : a, t = 1 : b). */
export function mixHex(a: string, b: string, t: number): string {
  const x = hexToNum(a);
  const y = hexToNum(b);
  const ch = (shift: number) => Math.round(((x >> shift) & 255) * (1 - t) + ((y >> shift) & 255) * t);
  return `#${[16, 8, 0].map((s) => ch(s).toString(16).padStart(2, "0")).join("")}`;
}

/** Couleur des premoves : l'accent mêlé de rouge à 35 %. */
export const premoveColor = (accent: AccentId) => mixHex(accentColor(accent), "#ff4d3d", 0.35);

/** Luminance relative approchée (0..1), pour choisir un texte lisible sur une case. */
export function luminance(hex: string): number {
  const n = hexToNum(hex);
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * lin(n >> 16) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}

// ---- état global --------------------------------------------------------------------------------

function systemDefaults(): ThemeSettings {
  let reduce = false;
  try {
    reduce = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    reduce = false;
  }
  return { ...THEME_DEFAULTS, reduceMotion: reduce };
}

function read(): ThemeSettings {
  const defaults = systemDefaults();
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(THEME_KEY) : null;
    if (raw && typeof localStorage !== "undefined" && localStorage.getItem(THEME_SKIN_KEY) !== THEME_SKIN) {
      localStorage.setItem(THEME_SKIN_KEY, THEME_SKIN);
      const old = sanitizeTheme(JSON.parse(raw), defaults);
      return { ...old, board: defaults.board, pieces: defaults.pieces, accent: defaults.accent };
    }
    return raw ? sanitizeTheme(JSON.parse(raw), defaults) : defaults;
  } catch {
    return defaults;
  }
}

let current: ThemeSettings = read();
const listeners = new Set<() => void>();

export const getTheme = () => current;

/** Écrit les variables CSS du thème sur `<html>` (no-op hors navigateur). */
export function applyTheme(theme: ThemeSettings = current, root: HTMLElement | null = typeof document !== "undefined" ? document.documentElement : null) {
  if (!root) return;
  const board = boardTheme(theme.board);
  const set = pieceSet(theme.pieces);
  const accent = accentColor(theme.accent);
  root.style.setProperty("--accent", accent);
  root.style.setProperty("--premove", premoveColor(theme.accent));
  root.style.setProperty("--board-light", board.light);
  root.style.setProperty("--board-dark", board.dark);
  root.style.setProperty("--piece-white", set.white ?? "#ece8de");
  root.style.setProperty("--piece-black", set.black ?? "#2a2c33");
  root.dataset.pieces = theme.pieces;
  root.dataset.board = theme.board;
  root.dataset.motion = theme.reduceMotion ? "reduce" : "full";
  root.dataset.move = theme.move;
  root.dataset.mode = resolveMode(theme.mode);
}

function systemPrefersLight(): boolean {
  try {
    return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: light)").matches;
  } catch {
    return false;
  }
}

/** Mode effectif : `system` suit `prefers-color-scheme`. */
export function resolveMode(mode: ColorMode, prefersLight: boolean = systemPrefersLight()): "light" | "dark" {
  return mode === "system" ? (prefersLight ? "light" : "dark") : mode;
}

export function setTheme(patch: Partial<ThemeSettings>) {
  current = sanitizeTheme({ ...current, ...patch });
  try {
    localStorage.setItem(THEME_KEY, JSON.stringify(current));
    localStorage.setItem(THEME_SKIN_KEY, THEME_SKIN);
  } catch {
    // Mode privé : le thème ne survit pas à la session.
  }
  applyTheme(current);
  listeners.forEach((fn) => fn());
}

export function subscribeTheme(fn: () => void) {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

export function useTheme(): ThemeSettings {
  return useSyncExternalStore(subscribeTheme, getTheme);
}

/** À appeler une fois au démarrage, avant le premier rendu. */
export function initTheme() {
  applyTheme(current);
  // En mode « système », l'interface suit le changement clair/sombre du système sans recharger.
  try {
    matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => {
      if (current.mode === "system") applyTheme(current);
    });
  } catch {
    // Pas de matchMedia (tests) : le mode reste celui du démarrage.
  }
}
