import type { CSSProperties } from "react";
import { bossOf, type CampaignChapterView, type ChapterStatus, chapterStatus, wonLevels } from "../../campaign";
import { familyVar } from "../../catalog";
import { LockIcon } from "./LockIcon";

interface Props {
  chapters: CampaignChapterView[];
  selected: number;
  onSelect: (chapter: number) => void;
}

const STATUS_LABEL: Record<ChapterStatus, string> = { sealed: "scellé", current: "en cours", done: "terminé" };

function Detail({ chapters, chapter }: { chapters: CampaignChapterView[]; chapter: CampaignChapterView }) {
  if (chapter.unlocked) return <>{wonLevels(chapter)} / {chapter.levels.length} niveaux · {chapter.stars} ★</>;
  const previousBoss = bossOf(chapters[chapter.chapter - 1]);
  return <>Battez {previousBoss?.name ?? "le boss précédent"}</>;
}

/** Les chapitres en onglets : en cours, terminés ou scellés (consultables, mais fermés). */
export function ChapterRail({ chapters, selected, onSelect }: Props) {
  return (
    <nav className="cp-rail" aria-label="Chapitres">
      {chapters.map((chapter) => {
        const status = chapterStatus(chapter);
        return (
          <button
            key={chapter.chapter}
            type="button"
            className={`cp-tab ${status}`}
            style={{ "--fam": familyVar(chapter.family) } as CSSProperties}
            aria-current={chapter.chapter === selected ? "true" : undefined}
            aria-disabled={status === "sealed" || undefined}
            onClick={() => onSelect(chapter.chapter)}
          >
            <span className="cp-tab-status">
              {status === "sealed" && <LockIcon />}Chapitre {chapter.chapter + 1} · {STATUS_LABEL[status]}
            </span>
            <span className="cp-tab-name">{chapter.name}</span>
            <span className="cp-tab-detail">
              <Detail chapters={chapters} chapter={chapter} />
            </span>
          </button>
        );
      })}
    </nav>
  );
}
