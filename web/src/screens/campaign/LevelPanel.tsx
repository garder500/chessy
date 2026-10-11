import { type CSSProperties, useState } from "react";
import { colorLabel, forgeNote, formatElo, requiredPicks, splitDeck, STAR_LABELS, toggleDeckPick, validPicks } from "../../campaign";
import type { CampaignChapterView, CampaignLevelView } from "../../campaign";
import { familyVar } from "../../catalog";
import { RARITY_LABEL } from "../../forged";
import type { SkillId } from "../../protocol";
import { skillInfo } from "../../skills";
import { store } from "../../store";
import { SkillArt } from "../../ui/SkillArt";
import { Stars } from "../../ui/Stars";
import { DeckPicker } from "./DeckPicker";
import { LockIcon } from "./LockIcon";

interface Props {
  chapter: CampaignChapterView;
  level: CampaignLevelView;
  /** Pourquoi le niveau est fermé ; `null` s'il se joue. */
  lockReason: string | null;
  ownDeck: SkillId[];
  connected: boolean;
  pending: boolean;
}

const PANEL_ID = "cp-panel";

/** En colonne, le panneau est sous la carte : on le ramène à l'écran quand on choisit un niveau. */
export function showPanel() {
  requestAnimationFrame(() => document.getElementById(PANEL_ID)?.scrollIntoView({ behavior: "smooth", block: "nearest" }));
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

/** Détail du niveau choisi sur la carte : adversaire, objectif et défi, mains, lancement ou raison du verrou. */
export function LevelPanel({ chapter, level, lockReason, ownDeck, connected, pending }: Props) {
  const [chosen, setChosen] = useState<SkillId[]>([]);
  const pickable = splitDeck(ownDeck, level.lent).pickable;
  const picked = validPicks(chosen, pickable);
  const needsPick = level.deck_choice && picked.length < requiredPicks(pickable);
  const play = () => store.startCampaign(chapter.chapter, level.level, level.deck_choice ? picked : undefined);

  return (
    <aside id={PANEL_ID} className="cp-panel" aria-label="Niveau sélectionné" style={{ "--fam": familyVar(chapter.family) } as CSSProperties}>
      <p className="eyebrow">
        {chapter.name} · {level.boss ? "boss" : `niveau ${level.level + 1} sur ${chapter.levels.length}`}
      </p>
      <h3 className="cp-panel-title">{level.name}</h3>
      <p className="muted">Sage · {formatElo(level.elo)}</p>
      {lockReason && (
        <p className="cp-lock-reason" role="status">
          <LockIcon size={16} /> {lockReason}
        </p>
      )}
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
      {!lockReason && (
        <button type="button" className="btn pri block cp-play" disabled={needsPick || !connected || pending} onClick={play}>
          Jouer
        </button>
      )}
    </aside>
  );
}
