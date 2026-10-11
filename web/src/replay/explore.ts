// Exploration d'une variation (docs/spec-v4.md §2) : le serveur est sans état, le client garde la ligne jouée
// et renvoie `line` complète à chaque coup. On conserve aussi la réponse de chaque position, pour qu'annuler
// un coup soit instantané (aucun appel réseau).

import { t } from "../i18n";
import type { Action, Color, ExploreRequest, ExploreResponse } from "../protocol";

/** Limite du serveur : `line` ≤ 200 actions. */
export const MAX_LINE = 200;
export const EXPLORE_DEPTH = 3;

export interface ExploreState {
  /** Nombre d'actions de la partie jouées avant le départ de la variation. */
  ply: number;
  line: Action[];
  /** Notation de chaque action de `line`. */
  notation: string[];
  /** `responses[0]` = position de départ, `responses[i]` = après `i` actions de la variation. */
  responses: ExploreResponse[];
}

export type ExploreError = "illegal_action" | "bad_ply" | "bad_response" | "too_long";

export interface ExtendResult {
  state: ExploreState;
  error: ExploreError | null;
}

/** Réponse utilisable : valide et avec une position. */
export function usable(res: ExploreResponse | null | undefined): res is ExploreResponse & { frame: NonNullable<ExploreResponse["frame"]> } {
  return !!res && res.valid && !!res.frame;
}

/** Corps de la requête pour la ligne courante, éventuellement prolongée de `next`. */
export function exploreRequest(ply: number, line: Action[], next?: Action, depth = EXPLORE_DEPTH): ExploreRequest {
  return { ply, line: next ? [...line, next] : line, depth };
}

/** Démarre l'exploration à partir de la réponse du serveur pour `line: []`. */
export function beginExplore(ply: number, first: ExploreResponse): ExploreState | null {
  return usable(first) ? { ply, line: [], notation: [], responses: [first] } : null;
}

export function currentResponse(state: ExploreState): ExploreResponse {
  return state.responses[state.responses.length - 1];
}

export function canExtend(state: ExploreState): boolean {
  return state.line.length < MAX_LINE;
}

/**
 * Intègre la réponse du serveur à `line + action`. Si le serveur n'a pas pu appliquer la nouvelle action
 * (`at` < longueur envoyée) ou ne renvoie pas de position, l'état ne change pas et l'erreur est rapportée.
 */
export function extendExplore(state: ExploreState, action: Action, res: ExploreResponse): ExtendResult {
  if (state.line.length >= MAX_LINE) return { state, error: "too_long" };
  const sent = state.line.length + 1;
  if (!res.valid || res.at < sent) return { state, error: res.error ?? "illegal_action" };
  if (!res.frame) return { state, error: "bad_response" };
  return {
    state: {
      ply: state.ply,
      line: [...state.line, action],
      notation: res.notation.length === sent ? res.notation : [...state.notation, res.notation[sent - 1] ?? "?"],
      responses: [...state.responses, res],
    },
    error: null,
  };
}

/** Annule le dernier coup de la variation ; sans coup à annuler, l'état est inchangé. */
export function undoExplore(state: ExploreState): ExploreState {
  if (state.line.length === 0) return state;
  return {
    ply: state.ply,
    line: state.line.slice(0, -1),
    notation: state.notation.slice(0, -1),
    responses: state.responses.slice(0, -1),
  };
}

export interface LineEntry {
  /** Numéro d'action dans la partie (la variation prolonge la partie principale). */
  ply: number;
  color: Color;
  notation: string;
}

/** Variation affichée sous la partie principale : un coup par entrée, avec son camp. */
export function lineEntries(state: ExploreState): LineEntry[] {
  return state.line.map((_, i) => ({
    ply: state.ply + i + 1,
    color: state.responses[i].frame?.to_move ?? "white",
    notation: state.notation[i] ?? "?",
  }));
}

export function exploreErrorText(code: ExploreError | string): string {
  switch (code) {
    case "illegal_action":
      return t("replay.explore_illegal");
    case "bad_ply":
      return t("replay.explore_bad_ply");
    case "too_long":
      return t("replay.explore_too_long");
    default:
      return t("replay.explore_unexpected");
  }
}
