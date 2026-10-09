import { useT } from "../../i18n";
import type { BestMove, MoveInfo, PlyAnalysis } from "../../protocol";
import { currentResponse, exploreErrorText, lineEntries, MAX_LINE, type ExploreState } from "../../replay/explore";
import { bestMoveSentence, explainSentence, formatEval, type Subject } from "../../replay/labels";
import { COLOR_FR } from "../../replay/frames";
import { LabelChip } from "./MoveList";

interface MovePanelProps {
  /** Dernière action jouée (celle qui a mené à la position affichée), `null` à la position initiale. */
  move: MoveInfo | null;
  analysis: PlyAnalysis | undefined;
  subject: Subject;
  /** Évaluation de la position affichée (point de vue des blancs), si l'analyse est faite. */
  evalCp: number | null;
  /** Meilleur coup depuis la position affichée (action suivante). */
  bestHere: BestMove | null;
  showBest: boolean;
  onShowBest: (on: boolean) => void;
  /** Revenir à la position avant le coup joué pour voir le meilleur coup en flèche. */
  onSeeAlternative: () => void;
}

/** Ce qui s'est passé au dernier coup : phrase « Vous avez joué X ; le meilleur coup était Y (+0,8) ». */
export function MovePanel({ move, analysis, subject, evalCp, bestHere, showBest, onShowBest, onSeeAlternative }: MovePanelProps) {
  const t = useT();
  const sentence = move ? bestMoveSentence(move, analysis, subject) : null;
  const why = move ? explainSentence(move, analysis) : null;
  const alternative = !!move && !!analysis && analysis.label !== "best" && !!analysis.best && analysis.best.notation !== move.notation;
  return (
    <section className="gm-panel card rp-now" aria-labelledby="rp-now-h">
      <div className="gm-panel-head">
        <h2 id="rp-now-h" className="gm-h">
          {move ? t("replay.move_heading", { ply: move.ply }) : t("replay.initial_position")}
        </h2>
        {analysis && <LabelChip label={analysis.label} />}
      </div>
      <div aria-live="polite">
        {move ? (
          <p className="rp-now-move">
            <span className={`rp-dot ${move.color}`} aria-hidden="true" /> <span className="mono">{move.notation}</span>
            <span className="muted"> · {COLOR_FR[move.color]}</span>
          </p>
        ) : (
          <p className="muted">{t("replay.before_first")}</p>
        )}
        {sentence && <p className="rp-sentence">{sentence}</p>}
        {why && analysis && <p className={`rp-why rp-why-${analysis.label}`}>{why}</p>}
        {!analysis && move && <p className="muted rp-help">{t("replay.run_analysis_move")}</p>}
      </div>
      {alternative && (
        <button type="button" className="btn sm rp-wide" onClick={onSeeAlternative}>
          {t("replay.see_best")}
        </button>
      )}
      {evalCp !== null && (
        <p className="rp-eval">
          {t("replay.eval_label")} <strong className="mono">{formatEval(evalCp)}</strong> <span className="muted">{t("replay.eval_white_side")}</span>
        </p>
      )}
      {bestHere && (
        <label className="rp-check">
          <input type="checkbox" checked={showBest} onChange={(e) => onShowBest(e.target.checked)} />
          <span>
            {t("replay.arrow_here")} <strong className="mono">{bestHere.notation}</strong> <span className="muted">({formatEval(bestHere.eval_cp)})</span>
          </span>
        </label>
      )}
    </section>
  );
}

interface ExplorePanelProps {
  state: ExploreState;
  pending: boolean;
  error: string | null;
  onUndo: () => void;
  onExit: () => void;
}

/** Variation en cours d'exploration : coups joués, annulation, retour, évaluation et meilleur coup du moteur. */
export function ExplorePanel({ state, pending, error, onUndo, onExit }: ExplorePanelProps) {
  const t = useT();
  const res = currentResponse(state);
  const entries = lineEntries(state);
  const toMove = res.frame?.to_move ?? "white";
  return (
    <section className="gm-panel card rp-explore" aria-labelledby="rp-explore-h">
      <div className="gm-panel-head">
        <h2 id="rp-explore-h" className="gm-h">
          {t("replay.explore_heading")}
        </h2>
        <span className="tag">{t("replay.variation_tag")}</span>
      </div>
      <p className="muted rp-help">
        {t("replay.explore_help", { ply: state.ply })}
      </p>

      <div className="rp-variation" aria-label={t("replay.variation_aria")}>
        {entries.length === 0 ? (
          <span className="muted">{t("replay.variation_empty", { color: COLOR_FR[toMove] })}</span>
        ) : (
          <ol className="rp-line-list">
            {entries.map((e) => (
              <li key={e.ply} className={e.color}>
                <span className="mono muted">{e.ply}</span>
                <span className={`rp-dot ${e.color}`} aria-hidden="true" />
                <span className="mono">{e.notation}</span>
              </li>
            ))}
          </ol>
        )}
      </div>

      <div className="rp-eval-now" aria-live="polite">
        <p>
          {t("replay.eval_label")} <strong className="mono">{formatEval(res.eval_cp)}</strong> <span className="muted">{t("replay.eval_white_side")}</span>
        </p>
        {res.best ? (
          <p>
            {t("replay.explore_best", { color: COLOR_FR[toMove] })} <strong className="mono">{res.best.notation}</strong> <span className="muted">({formatEval(res.best.eval_cp)})</span>
          </p>
        ) : (
          <p className="muted">{t("replay.no_suggestion")}</p>
        )}
      </div>

      {pending && (
        <p className="muted" role="status">
          {t("replay.thinking")}
        </p>
      )}
      {error && (
        <p className="rp-inline-error" role="alert">
          {error}
        </p>
      )}
      {state.line.length >= MAX_LINE && <p className="muted">{exploreErrorText("too_long")}</p>}

      <div className="gm-row">
        <button type="button" className="btn sm" onClick={onUndo} disabled={pending || state.line.length === 0}>
          {t("replay.undo_last")}
        </button>
        <button type="button" className="btn sm pri" onClick={onExit}>
          {t("replay.back_to_game")}
        </button>
      </div>
    </section>
  );
}
