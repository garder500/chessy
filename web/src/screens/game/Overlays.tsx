import { useEffect, useRef, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../../catalog";
import { pieceName } from "../../game/logic";
import { useT } from "../../i18n";
import type { PieceKind, SpawnKind } from "../../protocol";
import { skillInfo } from "../../skills";
import { PieceIcon, SkillArt } from "../../ui/SkillArt";

export function PromotionPicker({ options, onPick, onCancel }: { options: PieceKind[]; onPick: (k: PieceKind) => void; onCancel: () => void }) {
  const t = useT();
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  return (
    <div className="gm-veil" role="dialog" aria-modal="true" aria-label={t("game.promo_title")}>
      <div className="gm-promo card">
        <p className="eyebrow">{t("game.promo_title")}</p>
        <div className="gm-promo-row">
          {options.map((kind, i) => (
            <button key={kind} type="button" className="btn gm-promo-btn" ref={i === 0 ? first : undefined} onClick={() => onPick(kind)}>
              <PieceIcon kind={kind} size={34} />
              <span>{pieceName(kind)}</span>
            </button>
          ))}
        </div>
        <button type="button" className="btn sm ghost" onClick={onCancel}>
          {t("game.cancel")}
        </button>
      </div>
    </div>
  );
}

/** Mirage / Morph : choix du type de pièce une fois la case choisie (comme la promotion). */
export function SpawnPicker({
  skill,
  options,
  onPick,
  onCancel,
}: {
  skill: string;
  options: SpawnKind[];
  onPick: (k: SpawnKind) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  const info = skillInfo(skill);
  return (
    <div className="gm-veil" role="dialog" aria-modal="true" aria-label={t("game.spawn_aria", { skill: info.name })}>
      <div className="gm-promo card" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
        <p className="eyebrow">{info.name}</p>
        <p className="gm-promo-ask">{t("game.spawn_ask")}</p>
        <div className="gm-promo-row">
          {options.map((kind, i) => (
            <button key={kind} type="button" className="btn gm-promo-btn" ref={i === 0 ? first : undefined} onClick={() => onPick(kind)}>
              <PieceIcon kind={kind} size={34} />
              <span>{pieceName(kind)}</span>
            </button>
          ))}
        </div>
        <button type="button" className="btn sm ghost" onClick={onCancel}>
          {t("game.cancel")}
        </button>
      </div>
    </div>
  );
}

/** Grande carte centrale affichée quand une compétence est lancée (~1,9 s). */
export function LaunchCard({ skill, mine }: { skill: string; mine: boolean }) {
  const t = useT();
  const info = skillInfo(skill);
  return (
    <div className="gm-launch" role="status" aria-label={`${t(mine ? "game.launch_you" : "game.launch_opp")} ${info.name}`}>
      <div className="gm-launch-card card" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
        <p className="eyebrow">{t(mine ? "game.launch_you" : "game.launch_opp")}</p>
        <div className="gm-launch-art">
          <SkillArt id={skill} size={132} />
        </div>
        <h3 className="gm-launch-name">{info.name}</h3>
        <p className="gm-launch-fam eyebrow">{FAMILY_LABEL[info.family]}</p>
        <p className="gm-launch-desc">{info.description}</p>
        {info.unique && <span className="tag gm-launch-unique">{t("game.unique_skill")}</span>}
      </div>
    </div>
  );
}
