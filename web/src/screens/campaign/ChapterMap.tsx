import type { CSSProperties } from "react";
import { bossOf, type CampaignChapterView } from "../../campaign";
import { FAMILY_LABEL, familyVar } from "../../catalog";
import { MAP_LAYOUTS, type MapLayout } from "../../campaignMap";
import { BossForgeAction } from "./BossForgeAction";
import { LockIcon } from "./LockIcon";
import { MapNode } from "./MapNode";

interface Props {
  chapters: CampaignChapterView[];
  chapter: CampaignChapterView;
  selectedLevel: number;
  compact: boolean;
  onPick: (level: number) => void;
  onReveal: (chapter: number) => void;
}

/** Le sentier entre deux niveaux est tracé en couleur de famille une fois le second ouvert. */
function Trail({ chapter, layout }: { chapter: CampaignChapterView; layout: MapLayout }) {
  return (
    <svg className="cp-trail" aria-hidden="true" viewBox={`0 0 ${layout.width} ${layout.height}`}>
      {layout.nodes.slice(1).map(([x, y], i) => {
        const [x0, y0] = layout.nodes[i];
        const open = chapter.levels[i + 1]?.unlocked;
        return <line key={i} className={open ? "open" : undefined} x1={x0} y1={y0} x2={x} y2={y} />;
      })}
    </svg>
  );
}

/** Bandeau du chapitre suivant tant qu'il est scellé. */
function NextSealed({ chapters, chapter }: { chapters: CampaignChapterView[]; chapter: CampaignChapterView }) {
  const next = chapters[chapter.chapter + 1];
  if (!next || next.unlocked) return null;
  return (
    <div className="cp-sealed">
      <LockIcon size={22} />
      <div>
        <p className="cp-sealed-title">Chapitre {next.chapter + 1} · {next.name} — scellé</p>
        <p>
          Battez {bossOf(chapter)?.name ?? "le boss"} pour briser le sceau{next.levels[0] && <> et ouvrir {next.levels[0].name}</>}.
        </p>
      </div>
    </div>
  );
}

/** La carte d'un chapitre : sept niveaux sur un sentier, le boss en bout de route. */
export function ChapterMap({ chapters, chapter, selectedLevel, compact, onPick, onReveal }: Props) {
  const layout = compact ? MAP_LAYOUTS.phone : MAP_LAYOUTS.desktop;
  const gate = chapter.unlocked && !chapter.boss_unlocked ? { stars: chapter.stars, required: chapter.boss_stars_required } : null;
  return (
    <section className="cp-chapter" style={{ "--fam": familyVar(chapter.family) } as CSSProperties} aria-labelledby="cp-map-title">
      <header className="cp-chapter-head">
        <div>
          <p className="eyebrow">{FAMILY_LABEL[chapter.family]}</p>
          <h2 id="cp-map-title" className="cp-chapter-title">Chapitre {chapter.chapter + 1} · {chapter.name}</h2>
        </div>
        <p className={`cp-chapter-honor${chapter.title_earned ? " earned" : ""}`}>
          {chapter.title_earned ? "Titre obtenu" : "Titre à gagner"} : {chapter.title}
        </p>
      </header>
      <BossForgeAction chapter={chapter.chapter} forge={chapter.boss_forge} onReveal={onReveal} />
      {chapter.available ? (
        <div className="cp-map" style={{ aspectRatio: `${layout.width} / ${layout.height}` }}>
          <Trail chapter={chapter} layout={layout} />
          {chapter.levels.map((level, i) => (
            <MapNode
              key={level.level}
              level={level}
              point={layout.nodes[i]}
              layout={layout}
              selected={level.level === selectedLevel}
              gate={gate}
              onPick={() => onPick(level.level)}
            />
          ))}
        </div>
      ) : (
        <p className="muted">À venir</p>
      )}
      <NextSealed chapters={chapters} chapter={chapter} />
    </section>
  );
}
