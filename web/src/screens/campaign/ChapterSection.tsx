import type { CSSProperties } from "react";
import { bossProgress, chapterTitle, isLocked } from "../../campaign";
import type { CampaignChapterView, CampaignLevelView } from "../../campaign";
import { FAMILY_LABEL, familyVar } from "../../catalog";
import type { BossForgeInfo } from "../../protocol";
import { store } from "../../store";
import { Stars } from "../../ui/Stars";

interface Props {
  chapter: CampaignChapterView;
  onPick: (level: CampaignLevelView) => void;
  /** Ouvre la révélation de la compétence forgée ; branchée par l'écran qui la porte. */
  onReveal?: (chapter: number) => void;
}

function BossForgeAction({ chapter, forge, onReveal }: { chapter: number; forge: BossForgeInfo | null; onReveal: Props["onReveal"] }) {
  if (forge?.state === "forging") {
    return (
      <button type="button" className="btn pri cp-forge-btn" onClick={() => store.send({ type: "boss_forge_claim", chapter })}>
        Récupérer la forge
      </button>
    );
  }
  if (forge?.state === "pending") {
    return (
      <button type="button" className="btn pri cp-forge-btn" onClick={() => onReveal?.(chapter)}>
        Révéler
      </button>
    );
  }
  return null;
}

function LevelTile({ chapter, level, onPick }: { chapter: CampaignChapterView; level: CampaignLevelView; onPick: Props["onPick"] }) {
  const locked = isLocked(chapter, level);
  return (
    <li>
      <button type="button" className={`card cp-level${level.boss ? " boss" : ""}${locked ? " locked" : ""}`} onClick={() => onPick(level)}>
        <span className="cp-level-name">{level.boss ? "Boss · " : ""}{level.name}</span>
        <span className="mono muted cp-level-elo">Elo {level.elo}</span>
        {locked ? <span className="muted">Verrouillé · {bossProgress(chapter)} ★</span> : <Stars stars={level.best} />}
      </button>
    </li>
  );
}

/** Un chapitre de la campagne : ses niveaux et son boss, ou « à venir » s'il n'a pas encore de contenu. */
export function ChapterSection({ chapter, onPick, onReveal }: Props) {
  return (
    <section className="cp-chapter" style={{ "--fam": familyVar(chapter.family) } as CSSProperties} aria-labelledby={`cp-ch-${chapter.chapter}`}>
      <header className="cp-chapter-head">
        <div>
          <p className="eyebrow">{FAMILY_LABEL[chapter.family]}</p>
          <h2 id={`cp-ch-${chapter.chapter}`} className="cp-chapter-title">
            {chapterTitle(chapter)}
          </h2>
          <p className={`cp-chapter-honor${chapter.title_earned ? " earned" : ""}`}>
            {chapter.title_earned ? "Titre obtenu" : "Titre à gagner"} : {chapter.title}
          </p>
        </div>
        {chapter.available ? (
          <span className="mono muted">{bossProgress(chapter)} ★</span>
        ) : (
          <span className="muted">À venir</span>
        )}
      </header>
      {chapter.available && (
        <ul className="cp-levels">
          {chapter.levels.map((level) => (
            <LevelTile key={level.level} chapter={chapter} level={level} onPick={onPick} />
          ))}
        </ul>
      )}
      <BossForgeAction chapter={chapter.chapter} forge={chapter.boss_forge} onReveal={onReveal} />
    </section>
  );
}
