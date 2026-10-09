// Libellés, couleurs et phrases de l'analyse (docs/spec-v4.md §2 et §5). Pur et testable.

import type { Analysis, AnalysisLabel, Color, LabelCounts, MoveInfo, PlyAnalysis } from "../protocol";
import { intlLocale, t } from "../i18n";
import { COLOR_FR } from "./frames";

/** Du meilleur au pire. */
export const LABEL_ORDER: AnalysisLabel[] = ["best", "good", "inaccuracy", "mistake", "blunder"];

/** Enregistrement dont chaque valeur est traduite à la lecture (jamais au chargement du module). */
function lazyLabels(prefix: string): Record<AnalysisLabel, string> {
  const out = {} as Record<AnalysisLabel, string>;
  for (const label of LABEL_ORDER) Object.defineProperty(out, label, { enumerable: true, get: () => t(`${prefix}${label}`) });
  return out;
}

export const LABEL_TEXT: Record<AnalysisLabel, string> = lazyLabels("replay.label_");

/** Forme courte (tableau récapitulatif). */
export const LABEL_SHORT: Record<AnalysisLabel, string> = lazyLabels("replay.short_");

/** Symbole accolé à la pastille : la couleur ne porte jamais seule l'information. */
export const LABEL_GLYPH: Record<AnalysisLabel, string> = {
  best: "★",
  good: "✓",
  inaccuracy: "?!",
  mistake: "?",
  blunder: "??",
};

/**
 * Couleurs distinctes, sans vert vif : « meilleur » en bleu, « bon » en gris-bleu, imprécision ambre,
 * erreur orange, gaffe rouge. Définies en CSS par `--lb-<label>` (voir replay.css), reprises ici pour le SVG.
 */
export const LABEL_COLOR: Record<AnalysisLabel, string> = {
  best: "#7aa2ff",
  good: "#9aa3b2",
  inaccuracy: "#eec06a",
  mistake: "#f0955a",
  blunder: "#e5484d",
};

export function isLabel(value: unknown): value is AnalysisLabel {
  return typeof value === "string" && (LABEL_ORDER as string[]).includes(value);
}

/** Seuils du serveur (perte en centipions). */
export function labelFromLoss(loss: number): AnalysisLabel {
  if (loss <= 10) return "best";
  if (loss <= 50) return "good";
  if (loss <= 120) return "inaccuracy";
  if (loss <= 300) return "mistake";
  return "blunder";
}

/** Analyse indexée par numéro d'action. */
export function analysisByPly(analysis: Analysis | null | undefined): Map<number, PlyAnalysis> {
  const map = new Map<number, PlyAnalysis>();
  if (analysis) for (const p of analysis.plies) map.set(p.ply, p);
  return map;
}

const fixed1 = (n: number): string => n.toLocaleString(intlLocale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 });

/** Évaluation en pions, du point de vue des blancs : `+0,8`, `−1,3`, `0,0`, `+mat`. */
export function formatEval(cp: number): string {
  if (Math.abs(cp) >= 2000) return t(cp > 0 ? "replay.eval_mate_pos" : "replay.eval_mate_neg");
  const pawns = Math.round(Math.abs(cp) / 10) / 10;
  if (pawns === 0) return fixed1(0);
  const text = fixed1(pawns);
  return cp > 0 ? `+${text}` : `−${text}`;
}

/** Précision `87 %` (arrondie, bornée à 0..100). */
export function formatAccuracy(value: number): string {
  return t("replay.accuracy_value", { value: Math.round(Math.min(100, Math.max(0, value))) });
}

export type Subject = "you" | Color;

/**
 * Phrase du panneau d'analyse : « Vous avez joué Fg5 ; le meilleur coup était Cf3 (+0,8). »
 * `null` sans analyse de ce coup.
 */
export function bestMoveSentence(move: MoveInfo, analysis: PlyAnalysis | undefined, subject: Subject): string | null {
  if (!analysis) return null;
  const kind = subject === "you" ? "you" : "side";
  const color = subject === "you" ? "" : COLOR_FR[subject];
  const best = analysis.best;
  if (analysis.label === "best" || (best && best.notation === move.notation)) {
    return t(`replay.sentence_best_${kind}`, { color, move: move.notation });
  }
  if (!best) return t(`replay.sentence_noalt_${kind}`, { color, move: move.notation, eval: formatEval(analysis.eval_cp) });
  return t(`replay.sentence_alt_${kind}`, { color, move: move.notation, best: best.notation, eval: formatEval(best.eval_cp) });
}

/** Ce que chaque étiquette veut dire, avec son seuil (perte par rapport au meilleur coup du moteur, en pions). */
export const LABEL_MEANING: Record<AnalysisLabel, string> = lazyLabels("replay.meaning_");

/** Perte en pions, accordée : `0,3 pion`, `1,5 pion`, `2,4 pions`. Au-delà de 10 pions, on parle de la partie. */
export function formatLoss(cp: number): string {
  if (cp >= 1000) return t("replay.loss_huge");
  const pawns = Math.round(Math.max(0, cp) / 10) / 10;
  return t("replay.loss", { count: pawns >= 2 ? 2 : 1, value: fixed1(pawns) });
}

/**
 * Petite phrase qui explique pourquoi le coup porte son étiquette : ce qu'il coûte (en pions, un pion valant 1)
 * et, quand le moteur voit un mat, qui le gagne. `null` sans analyse de ce coup.
 */
export function explainSentence(move: MoveInfo, analysis: PlyAnalysis | undefined): string | null {
  if (!analysis) return null;
  // Évaluation du point de vue de celui qui vient de jouer.
  const own = move.color === "white" ? analysis.eval_cp : -analysis.eval_cp;
  const loss = formatLoss(analysis.loss_cp);
  switch (analysis.label) {
    case "best":
      return t(own >= 2000 ? "replay.explain_best_mate" : "replay.explain_best");
    case "good":
      return t("replay.explain_good", { loss });
    case "inaccuracy":
      return t("replay.explain_inaccuracy", { loss });
    case "mistake":
      return t(own <= -2000 ? "replay.explain_mistake_mate" : "replay.explain_mistake", { loss });
    case "blunder":
      return t(own <= -2000 ? "replay.explain_blunder_mate" : "replay.explain_blunder", { loss });
  }
}

/** Total de coups étiquetés « mauvais » (imprécision, erreur, gaffe) d'un camp. */
export function mistakeTotal(counts: LabelCounts): number {
  return counts.inaccuracy + counts.mistake + counts.blunder;
}

/** « 2 gaffes, 1 erreur » : uniquement les catégories non vides, de la pire à la moins grave. */
export function summarySentence(counts: LabelCounts): string {
  const parts: string[] = [];
  for (const label of ["blunder", "mistake", "inaccuracy"] as const) {
    const n = counts[label];
    if (n > 0) parts.push(t(`replay.summary_${label}`, { count: n }));
  }
  return parts.length ? parts.join(", ") : t("replay.summary_none");
}
