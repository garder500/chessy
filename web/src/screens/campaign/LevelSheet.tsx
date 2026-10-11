import { type CSSProperties, useState } from "react";
import { colorLabel, forgeNote, formatElo, isLocked, requiredPicks, splitDeck, STAR_LABELS, toggleDeckPick, validPicks } from "../../campaign";
import type { CampaignChapterView, CampaignLevelView } from "../../campaign";
import { familyVar } from "../../catalog";
import { RARITY_LABEL } from "../../forged";
import type { SkillId } from "../../protocol";
import { skillInfo } from "../../skills";
import { store } from "../../store";
import { SkillArt } from "../../ui/SkillArt";
import { Sheet } from "../../ui/Sheet";
import { Stars } from "../../ui/Stars";
import { DeckPicker } from "./DeckPicker";

interface Props {
  chapter: CampaignChapterView;
  level: CampaignLevelView | null;
  ownDeck: SkillId[];
  connected: boolean;
  pending: boolean;
  onClose: () => void;
}

function Deck({ label, note, skills }: { label: string; note?: string; skills: SkillId[] }) {
  return (
    <div className="cp-deck">
      <p className="lab">{label}</p>
      {note && <p className="muted">{note}</p>}
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

/** Ce que le boss peut offrir : rareté et taux, affichés avant le combat. */
function ForgeTable({ chapter }: { chapter: CampaignChapterView }) {
  if (chapter.forge_table.length === 0) return null;
  const note = forgeNote(chapter);
  return (
    <div className="cp-deck">
      <p className="lab">Forge du boss</p>
      <ul className="cp-forge-table">
        {chapter.forge_table.map(({ rarity, percent }) => (
          <li key={rarity}>
            <span>{RARITY_LABEL[rarity]}</span>
            <span className="mono">{percent} %</span>
          </li>
        ))}
      </ul>
      {note && <p className="muted">{note}</p>}
    </div>
  );
}

/** Détail d'un niveau : adversaire, objectif et défi, mains, lancement. */
export function LevelSheet({ chapter, level, ownDeck, connected, pending, onClose }: Props) {
  const [chosen, setChosen] = useState<SkillId[]>([]);
  if (!level) return null;
  const pickable = splitDeck(ownDeck, level.lent).pickable;
  const picked = validPicks(chosen, pickable);
  const locked = isLocked(chapter, level);
  const needsPick = level.deck_choice && picked.length < requiredPicks(pickable);
  const play = () => {
    store.startCampaign(chapter.chapter, level.level, level.deck_choice ? picked : undefined);
    onClose();
  };

  return (
    <Sheet open title={level.name} onClose={onClose}>
      <p className="sheet-sub">
        {level.boss ? "Boss · " : ""}Sage · {formatElo(level.elo)}
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
      {level.hint && (
        <p className="cp-hint">
          <span className="lab">Conseil</span> {level.hint}
        </p>
      )}
      {level.start_fen && (
        <p className="muted">
          Position de départ spéciale{level.human_color && <> · vous jouez {colorLabel(level.human_color)}</>}
        </p>
      )}
      {level.deck_choice ? (
        <DeckPicker deck={ownDeck} lent={level.lent} picked={picked} onToggle={(skill) => setChosen((cur) => toggleDeckPick(validPicks(cur, pickable), skill))} />
      ) : (
        <Deck label="Compétences imposées · une fois par partie" skills={level.player_deck} />
      )}
      <Deck
        label="Main de Sage"
        note={level.boss ? "Rien n'est caché. Ses 3 compétences sont dans sa main dès le premier coup." : "Rien n'est caché."}
        skills={level.bot_deck}
      />
      {level.boss && <ForgeTable chapter={chapter} />}
      <button type="button" className="btn pri block" disabled={locked || needsPick || !connected || pending} onClick={play}>
        {locked ? "Boss verrouillé" : "Jouer"}
      </button>
    </Sheet>
  );
}
