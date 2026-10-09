import { t } from "./i18n";
import type { Color, Outcome } from "./protocol";

/** One-line description of how a game ended, from `you`'s point of view. */
export function describeOutcome(outcome: Outcome, you: Color): string {
  switch (outcome.type) {
    case "ongoing":
      return "";
    case "checkmate":
      return t(outcome.winner === you ? "outcome.checkmate_win" : "outcome.checkmate_loss");
    case "resignation":
      return t(outcome.winner === you ? "outcome.resignation_win" : "outcome.resignation_loss");
    case "timeout":
      return t(outcome.winner === you ? "outcome.timeout_win" : "outcome.timeout_loss");
    case "draw_agreed":
      return t("outcome.draw_agreed");
    case "stalemate":
      return t("outcome.stalemate");
    case "fifty_moves":
      return t("outcome.fifty_moves");
    case "repetition":
      return t("outcome.repetition");
    case "insufficient_material":
      return t("outcome.insufficient_material");
  }
}

export type Result = "win" | "loss" | "draw";

/** Résultat du point de vue de `you`. */
export function resultFor(outcome: Outcome, you: Color): Result | null {
  if (outcome.type === "ongoing") return null;
  if ("winner" in outcome) return outcome.winner === you ? "win" : "loss";
  return "draw";
}

/** Titre court + motif pour le panneau de fin de partie. */
export function resultHeadline(outcome: Outcome, you: Color): { title: string; reason: string } {
  const result = resultFor(outcome, you);
  const title = t(result === "win" ? "outcome.title_win" : result === "loss" ? "outcome.title_loss" : "outcome.title_draw");
  // [point de vue du vainqueur ou nulle, point de vue du perdant]
  const reasons: Record<string, [string, string]> = {
    checkmate: ["outcome.reason_checkmate", "outcome.reason_checkmate"],
    resignation: ["outcome.reason_resignation_win", "outcome.reason_resignation_loss"],
    timeout: ["outcome.reason_timeout_win", "outcome.reason_timeout_loss"],
    draw_agreed: ["outcome.reason_draw_agreed", "outcome.reason_draw_agreed"],
    stalemate: ["outcome.reason_stalemate", "outcome.reason_stalemate"],
    fifty_moves: ["outcome.reason_fifty_moves", "outcome.reason_fifty_moves"],
    repetition: ["outcome.reason_repetition", "outcome.reason_repetition"],
    insufficient_material: ["outcome.reason_insufficient_material", "outcome.reason_insufficient_material"],
  };
  const pair = reasons[outcome.type];
  return { title, reason: pair ? t(result === "loss" ? pair[1] : pair[0]) : "" };
}

/** Variation d'Elo formatée avec un vrai signe moins : `+14`, `−9`, `±0`. */
export function formatDelta(delta: number): string {
  if (delta > 0) return `+${delta}`;
  if (delta < 0) return `−${Math.abs(delta)}`;
  return "±0";
}
