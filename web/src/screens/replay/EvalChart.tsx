import { useId, useMemo, useRef, useState, type PointerEvent } from "react";
import { useT } from "../../i18n";
import type { Analysis } from "../../protocol";
import { buildCurve, curveMarks, evalSeries, indexFromX, xForIndex } from "../../replay/evalCurve";
import { colorCap } from "../../replay/frames";
import { formatEval, LABEL_COLOR, LABEL_TEXT } from "../../replay/labels";

const W = 480;
const H = 130;

interface Props {
  analysis: Analysis;
  plies: number;
  index: number;
  onSeek: (index: number) => void;
}

/** Courbe d'évaluation cliquable (blancs en haut), avec repères pour les erreurs et les gaffes. */
export function EvalChart({ analysis, plies, index, onSeek }: Props) {
  const t = useT();
  const clipId = useId().replace(/:/g, "");
  const box = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const series = useMemo(() => evalSeries(analysis, plies), [analysis, plies]);
  const curve = useMemo(() => buildCurve(series, W, H), [series]);
  const marks = useMemo(() => curveMarks(analysis, curve), [analysis, curve]);

  const indexAt = (e: PointerEvent) => {
    const rect = box.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return null;
    return indexFromX(((e.clientX - rect.left) / rect.width) * W, W, plies);
  };

  const cursor = xForIndex(index, plies, W);
  const shown = hover ?? index;

  return (
    <section className="gm-panel card rp-chart" aria-labelledby="rp-chart-h">
      <div className="gm-panel-head">
        <h2 id="rp-chart-h" className="gm-h">
          {t("replay.eval_heading")}
        </h2>
        <span className="mono muted" aria-live="polite">
          {t("replay.chart_shown", { n: shown, eval: formatEval(series[shown] ?? 0) })}
        </span>
      </div>
      <div
        ref={box}
        className="rp-chart-box"
        role="slider"
        tabIndex={0}
        aria-label={t("replay.chart_aria")}
        aria-valuemin={0}
        aria-valuemax={plies}
        aria-valuenow={index}
        aria-valuetext={t("replay.chart_valuetext", { index, eval: formatEval(series[index] ?? 0) })}
        onPointerDown={(e) => {
          const i = indexAt(e);
          if (i !== null) onSeek(i);
        }}
        onPointerMove={(e) => setHover(indexAt(e))}
        onPointerLeave={() => setHover(null)}
      >
        <svg viewBox={`0 0 ${W} ${H}`} aria-hidden="true" focusable="false">
          <defs>
            <clipPath id={`${clipId}-up`}>
              <rect x="0" y="0" width={W} height={curve.mid} />
            </clipPath>
            <clipPath id={`${clipId}-down`}>
              <rect x="0" y={curve.mid} width={W} height={H - curve.mid} />
            </clipPath>
          </defs>
          <path d={curve.area} className="rp-area-white" clipPath={`url(#${clipId}-up)`} />
          <path d={curve.area} className="rp-area-black" clipPath={`url(#${clipId}-down)`} />
          <line x1="0" y1={curve.mid} x2={W} y2={curve.mid} className="rp-mid" />
          <path d={curve.line} className="rp-line" vectorEffect="non-scaling-stroke" />
          {marks.map((m) => (
            <circle key={m.index} cx={m.x} cy={m.y} r="4.5" fill={LABEL_COLOR[m.label]} className="rp-mark" vectorEffect="non-scaling-stroke">
              <title>{t("replay.mark_title", { n: m.index, label: LABEL_TEXT[m.label] })}</title>
            </circle>
          ))}
          {hover !== null && <line x1={xForIndex(hover, plies, W)} y1="0" x2={xForIndex(hover, plies, W)} y2={H} className="rp-hover" vectorEffect="non-scaling-stroke" />}
          <line x1={cursor} y1="0" x2={cursor} y2={H} className="rp-cursor" vectorEffect="non-scaling-stroke" />
        </svg>
        <span className="rp-side-label top" aria-hidden="true">
          {colorCap("white")}
        </span>
        <span className="rp-side-label bottom" aria-hidden="true">
          {colorCap("black")}
        </span>
      </div>
    </section>
  );
}
