import type { CSSProperties } from "react";
import { bossProgress, chapterTitle, isLocked } from "../../campaign";
import { FAMILY_LABEL, familyVar } from "../../catalog";
import type { CampaignChapter, CampaignLevel } from "../../protocol";
import { Stars } from "../../ui/Stars";

interface Props {
  chapter: CampaignChapter;
  onPick: (level: CampaignLevel) => void;
}

function LevelTile({ chapter, level, onPick }: { chapter: CampaignChapter; level: CampaignLevel; onPick: Props["onPick"] }) {
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
export function ChapterSection({ chapter, onPick }: Props) {
  return (
    <section className="cp-chapter" style={{ "--fam": familyVar(chapter.family) } as CSSProperties} aria-labelledby={`cp-ch-${chapter.chapter}`}>
      <header className="cp-chapter-head">
        <div>
          <p className="eyebrow">{FAMILY_LABEL[chapter.family]}</p>
          <h2 id={`cp-ch-${chapter.chapter}`} className="cp-chapter-title">
            {chapterTitle(chapter)}
          </h2>
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
    </section>
  );
}
