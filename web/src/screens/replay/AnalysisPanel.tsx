import { useT } from "../../i18n";
import type { Analysis, Color, GameRecord } from "../../protocol";
import { COLOR_FR, colorCap, seatName } from "../../replay/frames";
import { formatAccuracy, LABEL_MEANING, LABEL_ORDER, LABEL_SHORT, summarySentence } from "../../replay/labels";
import { LabelChip } from "./MoveList";

export type AnalysisState =
  | { status: "idle" }
  | { status: "loading"; depth: number }
  | { status: "ready"; depth: number; data: Analysis }
  | { status: "error"; depth: number; error: string };

export const DEPTHS = [2, 3, 4] as const;

interface Props {
  record: GameRecord;
  state: AnalysisState;
  depth: number;
  onDepth: (depth: number) => void;
  onAnalyse: () => void;
  onCancel: () => void;
}

function DepthPicker({ depth, onDepth, disabled }: { depth: number; onDepth: (d: number) => void; disabled?: boolean }) {
  const t = useT();
  return (
    <div className="rp-depth">
      <span className="field-label" id="rp-depth-l">
        {t("replay.depth")}
      </span>
      <div className="seg" role="group" aria-labelledby="rp-depth-l">
        {DEPTHS.map((d) => (
          <button key={d} type="button" className={depth === d ? "on" : undefined} aria-pressed={depth === d} onClick={() => onDepth(d)} disabled={disabled}>
            {d}
          </button>
        ))}
      </div>
    </div>
  );
}

/** Lexique : ce que veulent dire les étiquettes et comment la perte se mesure. */
function Legend() {
  const t = useT();
  return (
    <details className="rp-legend">
      <summary>{t("replay.legend_summary")}</summary>
      <p className="muted rp-help">
        {t("replay.legend_intro")}
      </p>
      <ul className="rp-legend-list">
        {LABEL_ORDER.map((label) => (
          <li key={label}>
            <LabelChip label={label} />
            <span>{LABEL_MEANING[label]}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

function AccuracyTile({ color, name, value }: { color: Color; name: string; value: number }) {
  const t = useT();
  return (
    <div className="rp-acc">
      <span className={`rp-dot ${color}`} aria-hidden="true" />
      <span className="rp-acc-name">{name}</span>
      <strong className="mono rp-acc-val">{formatAccuracy(value)}</strong>
      <span className="sr-only">{t("replay.accuracy_sr", { color: COLOR_FR[color] })}</span>
    </div>
  );
}

/** Lancement de l'analyse, état de chargement/erreur, précision des joueurs et résumé par étiquette. */
export function AnalysisPanel({ record, state, depth, onDepth, onAnalyse, onCancel }: Props) {
  const t = useT();
  const white = seatName(record.white);
  const black = seatName(record.black);
  const noMoves = record.plies === 0;

  return (
    <section className="gm-panel card rp-analysis" aria-labelledby="rp-analysis-h">
      <div className="gm-panel-head">
        <h2 id="rp-analysis-h" className="gm-h">
          {t("replay.analysis_heading")}
        </h2>
        {state.status === "ready" && <span className="mono muted">{t("replay.depth_value", { depth: state.data.depth })}</span>}
      </div>

      {state.status === "loading" && (
        <div className="rp-loading" role="status" aria-live="polite">
          <span className="rp-spinner" aria-hidden="true" />
          <div>
            <strong>{t("replay.analysing")}</strong>
            <p className="muted">{t("replay.analysing_sub")}</p>
          </div>
          <button type="button" className="btn sm ghost" onClick={onCancel}>
            {t("replay.cancel")}
          </button>
        </div>
      )}

      {state.status === "error" && (
        <div className="rp-inline-error" role="alert">
          <p>{state.error}</p>
        </div>
      )}

      {(state.status === "idle" || state.status === "error") && (
        <>
          {state.status === "idle" && <p className="muted rp-help">{t("replay.analysis_intro")}</p>}
          <Legend />
          <DepthPicker depth={depth} onDepth={onDepth} />
          <button type="button" className="btn pri rp-wide" onClick={onAnalyse} disabled={noMoves}>
            {t(state.status === "error" ? "replay.retry_analysis" : "replay.analyse_game")}
          </button>
        </>
      )}

      {state.status === "ready" && (
        <>
          <div className="rp-accs" role="group" aria-label={t("replay.accuracy_group")}>
            <AccuracyTile color="white" name={white} value={state.data.accuracy.white} />
            <AccuracyTile color="black" name={black} value={state.data.accuracy.black} />
          </div>
          <table className="rp-sum">
            <caption className="sr-only">{t("replay.summary_caption")}</caption>
            <thead>
              <tr>
                <th scope="col">
                  <span className="sr-only">{t("replay.label_col")}</span>
                </th>
                <th scope="col">{colorCap("white")}</th>
                <th scope="col">{colorCap("black")}</th>
              </tr>
            </thead>
            <tbody>
              {LABEL_ORDER.map((label) => (
                <tr key={label}>
                  <th scope="row">
                    <LabelChip label={label} compact />
                    <span className="rp-sum-name">{LABEL_SHORT[label]}</span>
                  </th>
                  <td className="mono">{state.data.summary.white[label]}</td>
                  <td className="mono">{state.data.summary.black[label]}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted rp-help">
            {t("replay.player_summary", { name: white, summary: summarySentence(state.data.summary.white) })}
            <br />
            {t("replay.player_summary", { name: black, summary: summarySentence(state.data.summary.black) })}
          </p>
          <Legend />
          <details className="rp-redo">
            <summary>{t("replay.redo_summary")}</summary>
            <DepthPicker depth={depth} onDepth={onDepth} />
            <button type="button" className="btn sm rp-wide" onClick={onAnalyse}>
              {t("replay.reanalyse")}
            </button>
          </details>
        </>
      )}
    </section>
  );
}
