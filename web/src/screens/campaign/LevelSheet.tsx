import type { CSSProperties } from "react";
import { isLocked, STAR_LABELS } from "../../campaign";
import { familyVar } from "../../catalog";
import type { CampaignChapter, CampaignLevel, SkillId } from "../../protocol";
import { skillInfo } from "../../skills";
import { store } from "../../store";
import { SkillArt } from "../../ui/SkillArt";
import { Sheet } from "../../ui/Sheet";
import { Stars } from "../../ui/Stars";

interface Props {
  chapter: CampaignChapter;
  level: CampaignLevel | null;
  connected: boolean;
  pending: boolean;
  guest: boolean;
  onClose: () => void;
}

function Deck({ label, skills }: { label: string; skills: SkillId[] }) {
  return (
    <div className="cp-deck">
      <p className="lab">{label}</p>
      {skills.length === 0 ? (
        <p className="muted">Aucune compétence</p>
      ) : (
        <ul className="cp-chips">
          {skills.map((id) => {
            const info = skillInfo(id);
            return (
              <li key={id} className="cp-chip" style={{ "--fam": familyVar(info.family) } as CSSProperties}>
                <SkillArt id={id} size={20} />
                {info.name}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

/** Détail d'un niveau : adversaire, objectif et défi, decks imposés, lancement. */
export function LevelSheet({ chapter, level, connected, pending, guest, onClose }: Props) {
  if (!level) return null;
  const locked = isLocked(chapter, level);
  const play = () => {
    store.startCampaign(chapter.chapter, level.level);
    onClose();
  };

  return (
    <Sheet open title={level.name} onClose={onClose}>
      <p className="sheet-sub">
        {level.boss ? "Boss · " : ""}Sage · Elo {level.elo}
      </p>
      <Stars stars={level.best} size={22} />
      <dl className="cp-goals">
        <dt>{STAR_LABELS[0]}</dt>
        <dd>Gagner la partie</dd>
        {level.objective && (
          <>
            <dt>{STAR_LABELS[1]}</dt>
            <dd>{level.objective}</dd>
          </>
        )}
        {level.challenge && (
          <>
            <dt>{STAR_LABELS[2]}</dt>
            <dd>{level.challenge}</dd>
          </>
        )}
      </dl>
      <Deck label="Votre deck" skills={level.player_deck} />
      <Deck label="Deck de Sage" skills={level.bot_deck} />
      {level.boss && guest && <p className="muted">La récompense de forge du boss demande un compte : créez-en un pour la recevoir.</p>}
      <button type="button" className="btn pri block" disabled={locked || !connected || pending} onClick={play}>
        {locked ? "Boss verrouillé" : "Jouer"}
      </button>
    </Sheet>
  );
}
