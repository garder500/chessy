// État du mode spectateur et son réducteur (docs/spec-v4.md §3). Pur : le store ne fait que l'appliquer.

import { t } from "../i18n";
import { appendLog, describeAction, type LogLine } from "../game/logic";
import type { ServerMsg, SpectatorView } from "../protocol";
import { normalizeSpectatorView, spectatorToView } from "./frames";

export type SpectateStatus = "joining" | "watching" | "over" | "ended" | "error";

export interface SpectatingState {
  gameId: string;
  status: SpectateStatus;
  /** Dernière vue reçue (conservée après la fin pour afficher le résultat). */
  view: SpectatorView | null;
  /** Une ligne par action vue depuis l'entrée dans la partie. */
  log: LogLine[];
  /** Code d'erreur serveur quand `status` vaut `error`. */
  error: string | null;
  /** Motif d'annulation quand `status` vaut `ended`. */
  endedReason: string | null;
}

/** Erreurs serveur qui concernent l'entrée en mode spectateur. */
export const SPECTATE_ERRORS = ["already_in_game", "spectate_full", "no_such_game"];

export function startSpectating(gameId: string): SpectatingState {
  return { gameId, status: "joining", view: null, log: [], error: null, endedReason: null };
}

/** `true` tant que le serveur peut nous envoyer des vues (et qu'il faut lui dire `unspectate`). */
export function isLive(s: SpectatingState | null): boolean {
  return !!s && (s.status === "joining" || s.status === "watching");
}

/** Empreinte d'une vue : change à chaque action, pas quand le serveur renvoie la même position. */
export function spectatorKey(view: Pick<SpectatorView, "ply" | "events">): string {
  return `${view.ply}|${JSON.stringify(view.events)}`;
}

function withView(s: SpectatingState, raw: SpectatorView, status: SpectateStatus): SpectatingState {
  const view = normalizeSpectatorView(raw);
  const line = describeAction(spectatorToView(view, "white"));
  return {
    ...s,
    status,
    view,
    log: appendLog(s.log, line && { ...line, key: spectatorKey(view) }),
    error: null,
  };
}

/**
 * Applique un message serveur. `null` (pas de spectateur en cours) ignore tout : un `spectate_state` en vol
 * après `unspectate` ne doit pas ressusciter la partie. Un état inchangé est renvoyé tel quel (même référence).
 */
export function reduceSpectator(s: SpectatingState | null, msg: ServerMsg): SpectatingState | null {
  if (!s) return s;
  switch (msg.type) {
    case "spectate_state":
      if (msg.view.game_id !== s.gameId || !isLive(s)) return s;
      return withView(s, msg.view, "watching");
    case "spectate_over":
      if (msg.view.game_id !== s.gameId || !isLive(s)) return s;
      return withView(s, msg.view, "over");
    case "spectate_ended":
      return isLive(s) ? { ...s, status: "ended", endedReason: msg.reason } : s;
    case "error":
      // Seules les erreurs d'entrée comptent, et seulement avant d'avoir reçu une vue.
      return s.status === "joining" && SPECTATE_ERRORS.includes(msg.code) ? { ...s, status: "error", error: msg.code } : s;
    case "state":
    case "deck_select":
      // Le client lance une partie : il quitte automatiquement le mode spectateur.
      return null;
    default:
      return s;
  }
}

export function spectateErrorText(code: string | null): string {
  switch (code) {
    case "spectate_full":
      return t("replay.err_full");
    case "no_such_game":
      return t("replay.err_missing");
    case "already_in_game":
      return t("replay.err_in_game");
    default:
      return t("replay.err_default");
  }
}

export function endedText(reason: string | null): string {
  return reason === "cancelled" || !reason ? t("replay.ended_cancelled") : t("replay.ended_reason", { reason });
}
