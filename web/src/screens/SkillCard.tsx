import type { CSSProperties } from "react";
import { useT } from "../i18n";
import { FAMILY_LABEL, skillEntry } from "../catalog";
import { skillInfo } from "../skills";
import { SkillArt } from "../ui/SkillArt";
import { RarityTag } from "../ui/RarityTag";
import { UniqueBadge } from "../ui/UniqueBadge";
import { tileRarity } from "../ui/tileRarity";

interface Props {
  skill: string;
  selected?: boolean;
  /** Rang de sélection (1-3) affiché dans une étiquette blanche. */
  order?: number;
  disabled?: boolean;
  /** Compétence verrouillée : toujours incluse, hors quota. */
  locked?: boolean;
  badge?: string;
  /** Rend la carte comme un bouton radio (récompenses). */
  radio?: boolean;
  onClick?: () => void;
}

/** Carte de compétence : illustration, nom, famille, description, liseré de famille. */
export function SkillCard({ skill, selected, order, disabled, locked, badge, radio, onClick }: Props) {
  const t = useT();
  const info = skillInfo(skill);
  const rarity = skillEntry(skill).rarity;
  const style = { "--fam": `var(--fam-${info.family})` } as CSSProperties;
  const classes = ["skc", selected ? "on" : "", locked ? "locked" : "", radio ? "radio" : "", info.unique ? "foil" : ""].filter(Boolean).join(" ");
  const content = (
    <>
      <span className="skc-art" data-rar={tileRarity(skill)}>
        <SkillArt id={skill} size={46} />
        {info.unique && <UniqueBadge />}
      </span>
      <span className="skc-body">
        <span className="skc-name">{info.name}</span>
        <span className="skc-meta">
          <span className="eyebrow">{FAMILY_LABEL[info.family]}</span>
          {rarity ? <RarityTag rarity={rarity} /> : info.unique && <span className="tag foil-tag">{t("skills.unique_tag")}</span>}
          {locked && (
            <span className="tag skc-lock">
              <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" focusable="false">
                <rect x="2" y="5.5" width="8" height="5" rx="1.2" fill="none" stroke="currentColor" strokeWidth="1.3" />
                <path d="M4 5.5V4a2 2 0 014 0v1.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
              </svg>
              {t("skills.locked_tag")}
            </span>
          )}
          {badge && <span className="tag">{badge}</span>}
        </span>
        <span className="skc-desc">{info.description}</span>
      </span>
      {order !== undefined && (
        <span className="skc-num" aria-hidden="true">
          {order}
        </span>
      )}
      {radio && <span className="skc-ring" aria-hidden="true" />}
    </>
  );
  if (!onClick) {
    return (
      <div className={classes} style={style}>
        {content}
      </div>
    );
  }
  return (
    <button
      type="button"
      className={classes}
      style={style}
      disabled={disabled}
      onClick={onClick}
      {...(radio ? { role: "radio", "aria-checked": !!selected } : { "aria-pressed": !!selected })}
    >
      {content}
    </button>
  );
}
