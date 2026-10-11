import { FAMILY_LABEL, skillEntry } from "../../catalog";
import { isForgedId, RARITY_LABEL } from "../../forged";
import { useT } from "../../i18n";
import type { SkillId } from "../../protocol";
import { SkillArt } from "../../ui/SkillArt";
import { FAMILY_FLAT, RARITY_EDGE } from "../../ui/forgedIcon";
import { SkillPreview } from "../../ui/skillPreview";
import { tileRarity } from "../../ui/tileRarity";
import { classicNote, clockTime, RULES, uniqueNote } from "../collectionData";

interface Props {
  skill: SkillId;
  /** Comment la compétence a été obtenue ou perdue. */
  note: string;
  open: boolean;
  onToggle: () => void;
  lost?: boolean;
  /** Pastille « dans votre deck » (journal). */
  current?: boolean;
  /** Date ISO de la ligne (journal) : affiche l'heure. */
  at?: string;
}

/** Une compétence repliable : médaillon, nom, famille · rareté, origine, puis les règles au clic. */
export function SkillRow({ skill, note, open, onToggle, lost = false, current = false, at }: Props) {
  const t = useT();
  const info = skillEntry(skill);
  const rar = tileRarity(skill);
  const forged = isForgedId(skill);
  const kind = [FAMILY_LABEL[info.family], info.rarity ? RARITY_LABEL[info.rarity] : null].filter(Boolean).join(" · ");
  return (
    <li className={`hl-row${lost ? " lost" : ""}`}>
      <button type="button" className="hl-head" aria-expanded={open} onClick={onToggle}>
        <span className={`hl-badge${forged ? "" : " flat"}`} style={{ ["--fam" as string]: FAMILY_FLAT[info.family], ["--edge" as string]: RARITY_EDGE[rar] }}>
          <SkillArt id={skill} size={forged ? 48 : 30} />
        </span>
        <span className="hl-body">
          <span className="hl-name">
            {info.name}
            {current && <span className="hl-deck" role="img" aria-label={t("collection.stat_in_deck")} title={t("collection.stat_in_deck")} />}
          </span>
          <span className="hl-kind">{info.unique ? t("skills.unique_tag") : kind}</span>
          <span className={`hl-note${lost ? " out" : ""}`}>{note}</span>
        </span>
        {at && (
          <time className="hl-when" dateTime={at}>
            {clockTime(at)}
          </time>
        )}
      </button>
      {open && (
        <div className="hl-detail">
          <p className="hl-rules">{(RULES as Record<string, string>)[skill] ?? info.description}</p>
          <SkillPreview id={skill} caption />
          <p className="muted hl-foot">{info.unique ? uniqueNote() : classicNote()}</p>
        </div>
      )}
    </li>
  );
}
