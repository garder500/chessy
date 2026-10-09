import { useEffect, useRef } from "react";
import { useT } from "../../i18n";
import type { Analysis, MoveInfo } from "../../protocol";
import { analysisByPly, LABEL_GLYPH, LABEL_MEANING, LABEL_SHORT, LABEL_TEXT } from "../../replay/labels";
import { COLOR_FR } from "../../replay/frames";
import { SkillArt } from "../../ui/SkillArt";
import { keepVisible } from "./scroll";

interface Props {
  moves: MoveInfo[];
  /** Position affichée : 0 = initiale, `n` = après l'action `n`. */
  index: number;
  analysis: Analysis | null;
  onSelect: (index: number) => void;
}

/** Pastille d'étiquette : couleur + symbole + texte accessible. */
export function LabelChip({ label, compact = false }: { label: keyof typeof LABEL_TEXT; compact?: boolean }) {
  const t = useT();
  return (
    <span className={`lb lb-${label}`} title={t("replay.chip_title", { label: LABEL_TEXT[label], meaning: LABEL_MEANING[label] })}>
      <span aria-hidden="true">{compact ? LABEL_GLYPH[label] : `${LABEL_GLYPH[label]} ${LABEL_SHORT[label]}`}</span>
      <span className="sr-only">{LABEL_TEXT[label]}</span>
    </span>
  );
}

/** Liste de coups cliquable : numéro, camp, notation (compétences en icône), étiquette si analysée. */
export function MoveList({ moves, index, analysis, onSelect }: Props) {
  const t = useT();
  const byPly = analysisByPly(analysis);
  const current = useRef<HTMLLIElement>(null);
  const list = useRef<HTMLOListElement>(null);

  useEffect(() => {
    keepVisible(list.current, current.current);
  }, [index]);

  return (
    <section className="gm-panel card rp-moves" aria-labelledby="rp-moves-h">
      <div className="gm-panel-head">
        <h2 id="rp-moves-h" className="gm-h">
          {t("replay.moves_heading")}
        </h2>
        <span className="mono muted">{moves.length}</span>
      </div>
      <ol className="rp-movelist" ref={list}>
        <li ref={index === 0 ? current : undefined}>
          <button type="button" className="rp-move" aria-current={index === 0 ? "step" : undefined} onClick={() => onSelect(0)}>
            <span className="mono rp-move-n">0</span>
            <span className="rp-move-txt muted">{t("replay.initial_position")}</span>
          </button>
        </li>
        {moves.map((m) => {
          const a = byPly.get(m.ply);
          const here = index === m.ply;
          return (
            <li key={m.ply} ref={here ? current : undefined}>
              <button
                type="button"
                className={`rp-move ${m.color}`}
                aria-current={here ? "step" : undefined}
                aria-label={t(a ? "replay.move_aria_label" : "replay.move_aria", { ply: m.ply, color: COLOR_FR[m.color], notation: m.notation, label: a ? LABEL_TEXT[a.label] : "" })}
                onClick={() => onSelect(m.ply)}
              >
                <span className="mono rp-move-n" aria-hidden="true">
                  {m.ply}
                </span>
                <span className={`rp-dot ${m.color}`} aria-hidden="true" />
                <span className="rp-move-txt" aria-hidden="true">
                  {m.action.type === "skill" && (
                    <span className="gm-log-ico">
                      <SkillArt id={m.action.skill} size={16} />
                    </span>
                  )}
                  <span className="mono">{m.notation}</span>
                </span>
                {a && <LabelChip label={a.label} compact />}
              </button>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
