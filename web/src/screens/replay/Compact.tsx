import type { Analysis, Color, MoveInfo, PlyAnalysis } from "../../protocol";
import { COLOR_FR } from "../../replay/frames";
import { explainSentence, formatAccuracy, LABEL_COLOR, LABEL_TEXT } from "../../replay/labels";
import type { NavAction, ReplayNav } from "../../replay/nav";
import type { AnalysisState } from "./AnalysisPanel";

/** Part des blancs (0..1) pour la barre d'évaluation : sigmoïde douce sur l'évaluation du moteur, 50 % sans analyse. */
export const evalShare = (cp: number | null): number => (cp === null ? 0.5 : 1 / (1 + Math.exp(-cp / 350)));

/** Barre d'évaluation horizontale au-dessus du plateau : les blancs à gauche. */
export function EvalStrip({ cp }: { cp: number | null }) {
  const share = Math.round(evalShare(cp) * 100);
  return (
    <div className="rc-eval" role="img" aria-label={cp === null ? "Évaluation : analyse non lancée" : `Évaluation : blancs ${share} %`}>
      <span style={{ width: `${share}%` }} />
    </div>
  );
}

interface CardProps {
  move: MoveInfo | null;
  analysis: PlyAnalysis | undefined;
  index: number;
  max: number;
  analysing: boolean;
  onAnalyse: () => void;
}

/** Carte du coup affiché : pastille de l'étiquette, notation, position dans la partie, puis une phrase qui explique. */
export function MoveCard({ move, analysis, index, max, analysing, onAnalyse }: CardProps) {
  const color = analysis ? LABEL_COLOR[analysis.label] : "var(--line-3)";
  const label = analysis ? LABEL_TEXT[analysis.label] : move ? `Aux ${COLOR_FR[move.color === "white" ? "black" : "white"]} de jouer` : "Position initiale";
  const why = move ? explainSentence(move, analysis) : "Avant le premier coup.";
  return (
    <section className="rc-card" style={{ borderLeftColor: color }} aria-label="Coup affiché">
      <div className="rc-card-h" aria-live="polite">
        <span className="dot" style={{ background: color }} aria-hidden="true" />
        <span className="rc-card-t">
          <strong>{label}</strong>
          {move && (
            <span className="muted">
              {" "}
              · {Math.ceil(move.ply / 2)}
              {move.color === "white" ? "." : "…"} {move.notation}
            </span>
          )}
        </span>
        <span className="meta">
          {index}/{max}
        </span>
      </div>
      {why ? (
        <p className="rc-why">{why}</p>
      ) : (
        <p className="rc-why muted">
          {analysing ? "Analyse en cours…" : "Lancez l'analyse pour voir la qualité de chaque coup."}{" "}
          {!analysing && (
            <button type="button" className="link rc-inline" onClick={onAnalyse}>
              Analyser
            </button>
          )}
        </p>
      )}
    </section>
  );
}

/** Précision de chaque camp ; avant l'analyse, un bouton pour la lancer. */
export function Accuracy({ data, state, onAnalyse, names }: { data: Analysis | null; state: AnalysisState; onAnalyse: () => void; names: Record<Color, string> }) {
  if (data) {
    return (
      <div className="rc-acc">
        {(["white", "black"] as const).map((c) => (
          <div key={c} className="card rc-acc-c">
            <span className="num">{formatAccuracy(data.accuracy[c])}</span> <span className="meta">précision {COLOR_FR[c]}</span>
            <span className="sr-only"> ({names[c]})</span>
          </div>
        ))}
      </div>
    );
  }
  if (state.status === "loading") return <p className="muted rc-note" role="status">Analyse en cours…</p>;
  return (
    <button type="button" className="btn block" onClick={onAnalyse}>
      Analyser la partie
    </button>
  );
}

const Ico = ({ d, size = 22 }: { d: string; size?: number }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
    <path d={d} />
  </svg>
);

/** Cinq boutons collés au pied de l'écran : début, précédent, lecture, suivant, fin. */
export function BottomControls({ nav, disabled, dispatch }: { nav: ReplayNav; disabled: boolean; dispatch: (a: NavAction) => void }) {
  const atStart = nav.index <= 0;
  const atEnd = nav.index >= nav.max;
  return (
    <nav className="rc-nav" aria-label="Contrôles du replay">
      <button type="button" className="rc-nav-b" aria-label="Début" disabled={disabled || atStart} onClick={() => dispatch({ type: "first" })}>
        <Ico d="M6 5v14M18 5l-9 7 9 7z" />
      </button>
      <button type="button" className="rc-nav-b" aria-label="Coup précédent" disabled={disabled || atStart} onClick={() => dispatch({ type: "prev" })}>
        <Ico d="M15 5l-7 7 7 7" />
      </button>
      <button type="button" className="rc-nav-b rc-play" aria-label={nav.playing ? "Pause" : "Lecture"} aria-pressed={nav.playing} disabled={disabled || nav.max === 0} onClick={() => dispatch({ type: "toggle" })}>
        {nav.playing ? <Ico d="M7 5v14M17 5v14" size={26} /> : <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor" aria-hidden="true"><path d="M8 5.5v13l10-6.5z" /></svg>}
      </button>
      <button type="button" className="rc-nav-b" aria-label="Coup suivant" disabled={disabled || atEnd} onClick={() => dispatch({ type: "next" })}>
        <Ico d="M9 5l7 7-7 7" />
      </button>
      <button type="button" className="rc-nav-b" aria-label="Fin" disabled={disabled || atEnd} onClick={() => dispatch({ type: "last" })}>
        <Ico d="M18 5v14M6 5l9 7-9 7z" />
      </button>
    </nav>
  );
}
