import { useEffect, useRef, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../../catalog";
import type { PieceKind, SpawnKind } from "../../protocol";
import { skillInfo } from "../../skills";
import { PieceIcon, SkillArt } from "../../ui/SkillArt";

const PROMO_LABEL: Record<string, string> = { queen: "Dame", rook: "Tour", bishop: "Fou", knight: "Cavalier" };

export function PromotionPicker({ options, onPick, onCancel }: { options: PieceKind[]; onPick: (k: PieceKind) => void; onCancel: () => void }) {
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  return (
    <div className="gm-veil" role="dialog" aria-modal="true" aria-label="Promotion">
      <div className="gm-promo card">
        <p className="eyebrow">Promotion</p>
        <div className="gm-promo-row">
          {options.map((kind, i) => (
            <button key={kind} type="button" className="btn gm-promo-btn" ref={i === 0 ? first : undefined} onClick={() => onPick(kind)}>
              <PieceIcon kind={kind} size={34} />
              <span>{PROMO_LABEL[kind] ?? kind}</span>
            </button>
          ))}
        </div>
        <button type="button" className="btn sm ghost" onClick={onCancel}>
          Annuler
        </button>
      </div>
    </div>
  );
}

const SPAWN_LABEL: Record<string, string> = { pawn: "Pion", knight: "Cavalier", bishop: "Fou", rook: "Tour", queen: "Dame" };

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
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  const info = skillInfo(skill);
  return (
    <div className="gm-veil" role="dialog" aria-modal="true" aria-label={`${info.name} : type de pièce`}>
      <div className="gm-promo card" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
        <p className="eyebrow">{info.name}</p>
        <p className="gm-promo-ask">Choisissez le type de pièce</p>
        <div className="gm-promo-row">
          {options.map((kind, i) => (
            <button key={kind} type="button" className="btn gm-promo-btn" ref={i === 0 ? first : undefined} onClick={() => onPick(kind)}>
              <PieceIcon kind={kind} size={34} />
              <span>{SPAWN_LABEL[kind] ?? kind}</span>
            </button>
          ))}
        </div>
        <button type="button" className="btn sm ghost" onClick={onCancel}>
          Annuler
        </button>
      </div>
    </div>
  );
}

/** Grande carte centrale affichée quand une compétence est lancée (~1,9 s). */
export function LaunchCard({ skill, mine }: { skill: string; mine: boolean }) {
  const info = skillInfo(skill);
  return (
    <div className="gm-launch" role="status" aria-label={`${mine ? "Vous lancez" : "L'adversaire lance"} ${info.name}`}>
      <div className="gm-launch-card card" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
        <p className="eyebrow">{mine ? "Vous lancez" : "L'adversaire lance"}</p>
        <div className="gm-launch-art">
          <SkillArt id={skill} size={132} />
        </div>
        <h3 className="gm-launch-name">{info.name}</h3>
        <p className="gm-launch-fam eyebrow">{FAMILY_LABEL[info.family]}</p>
        <p className="gm-launch-desc">{info.description}</p>
        {info.unique && <span className="tag gm-launch-unique">compétence unique</span>}
      </div>
    </div>
  );
}
