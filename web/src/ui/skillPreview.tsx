import { memo, useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { useT } from "../i18n";
import { familyVar, skillEntry } from "../catalog";
import type { CatalogId } from "../catalog";
import { animClass, BOARD_SIZE, resolveItem, SCENES } from "./skillScenes";
import type { Cell, Glyph, PieceSprite, Scene, SceneItem, Snapshot } from "./skillScenes";
import "./skillPreview.css";

const CELL = 20;

/** Vrai quand l'utilisateur demande moins de mouvement : l'aperçu devient un avant / après statique. */
export function usePrefersReducedMotion(): boolean {
  const query = "(prefers-reduced-motion: reduce)";
  const [reduced, setReduced] = useState(() => typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia(query).matches);
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const mq = window.matchMedia(query);
    const on = () => setReduced(mq.matches);
    on();
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return reduced;
}

// Pictogrammes dans une case de 20x20 (centre 10,10), couleur par défaut incluse.
const GLYPH: Record<Glyph, { color: string; body: ReactNode }> = {
  ice: {
    color: "#9fd8ff",
    body: (
      <>
        <rect x="2" y="2" width="16" height="16" rx="3" fill="#9fd8ff" fillOpacity=".2" />
        <path d="M10 4v12M4.8 7l10.4 6M4.8 13l10.4-6" />
      </>
    ),
  },
  shield: { color: "#5fd0a0", body: <path d="M10 2l7 2.6v5c0 4.4-3 7.4-7 9-4-1.6-7-4.6-7-9v-5z" fill="#5fd0a0" fillOpacity=".2" /> },
  wall: {
    color: "#eec06a",
    body: (
      <>
        <rect x="2" y="4" width="16" height="13" rx="1.2" fill="#eec06a" fillOpacity=".22" />
        <path d="M2 8.3h16M2 12.6h16M8 4v4.3M14 4v4.3M5 8.3v4.3M11 8.3v4.3M17 8.3v4.3M8 12.6V17M14 12.6V17" />
      </>
    ),
  },
  rock: { color: "#aab2c0", body: <path d="M2 17L8 6l4 6 2.5-3.5L19 17z" fill="#aab2c0" fillOpacity=".22" /> },
  trap: {
    color: "#ee8272",
    body: (
      <>
        <path d="M2 16l2.4-9 2.4 6 3.2-8.5 3.2 8.5 2.4-6 2.4 9z" fill="#ee8272" fillOpacity=".22" />
        <path d="M2 16h16" />
      </>
    ),
  },
  eye: {
    color: "#b79cff",
    body: (
      <>
        <path d="M1.5 10C4 5.5 7 4 10 4s6 1.5 8.5 6c-2.5 4.5-5.5 6-8.5 6s-6-1.5-8.5-6z" />
        <circle cx="10" cy="10" r="2.4" fill="currentColor" />
      </>
    ),
  },
  eyeoff: {
    color: "#b79cff",
    body: (
      <>
        <path d="M1.5 10C4 5.5 7 4 10 4s6 1.5 8.5 6c-2.5 4.5-5.5 6-8.5 6s-6-1.5-8.5-6z" />
        <circle cx="10" cy="10" r="2.4" fill="currentColor" />
        <path d="M3 17L17 3" />
      </>
    ),
  },
  spark: { color: "#ffffff", body: <path d="M10 1.5l2 6.5 6.5 2-6.5 2-2 6.5-2-6.5-6.5-2L8 8z" fill="currentColor" fillOpacity=".85" /> },
  ring: { color: "#ffffff", body: <circle cx="10" cy="10" r="8" strokeDasharray="2 2.4" /> },
  cross: { color: "#ee8272", body: <path d="M4 4l12 12M16 4L4 16" strokeWidth="2.4" /> },
  target: {
    color: "#ee8272",
    body: (
      <>
        <circle cx="10" cy="10" r="6.5" />
        <path d="M10 1.5v5M10 13.5v5M1.5 10h5M13.5 10h5" />
      </>
    ),
  },
  swirl: {
    color: "#b79cff",
    body: (
      <>
        <path d="M10 2.5a7.5 7.5 0 0 1 7.5 7.5M10 17.5A7.5 7.5 0 0 1 2.5 10" />
        <path d="M10 6a4 4 0 0 1 4 4M10 14a4 4 0 0 1-4-4" />
        <path d="M15.4 8l2.1 2 2-2.2M4.6 12l-2.1-2-2 2.2" />
      </>
    ),
  },
  hl: { color: "#8fb4ff", body: <rect x="1.2" y="1.2" width="17.6" height="17.6" rx="2" fill="#8fb4ff" fillOpacity=".18" strokeDasharray="2.4 2" /> },
  hourglass: { color: "#7aa2ff", body: <path d="M5 3h10M5 17h10M6 3c0 4 8 4.5 8 7s-8 3-8 7M14 3c0 4-8 4.5-8 7s8 3 8 7" /> },
  halo: {
    color: "#eec06a",
    body: (
      <>
        <ellipse cx="10" cy="2.6" rx="5.5" ry="1.7" />
        <path d="M2.5 4.5l2 1.6M17.5 4.5l-2 1.6" />
      </>
    ),
  },
  strings: { color: "#b79cff", body: <path d="M10 5V-40M6 7L-2 -40M14 7l8-47" strokeWidth="1" strokeDasharray="3 2" /> },
  cancel: {
    color: "#ee8272",
    body: (
      <>
        <circle cx="10" cy="10" r="7.5" fill="#ee8272" fillOpacity=".15" />
        <path d="M4.7 4.7l10.6 10.6" />
      </>
    ),
  },
  forcering: { color: "#5fd0a0", body: <circle cx="10" cy="10" r="9" fill="#5fd0a0" fillOpacity=".1" strokeDasharray="3 2" /> },
  dice: {
    color: "#eec06a",
    body: (
      <>
        <rect x="3" y="3" width="14" height="14" rx="3" />
        <circle cx="7.2" cy="7.2" r="1.1" fill="currentColor" />
        <circle cx="12.8" cy="7.2" r="1.1" fill="currentColor" />
        <circle cx="10" cy="10" r="1.1" fill="currentColor" />
        <circle cx="7.2" cy="12.8" r="1.1" fill="currentColor" />
        <circle cx="12.8" cy="12.8" r="1.1" fill="currentColor" />
      </>
    ),
  },
  bench: { color: "#eec06a", body: <path d="M2 9h16M2 13h16M4 9v8M16 9v8" /> },
  undo: { color: "#7aa2ff", body: <path d="M14 6a6 6 0 1 0 1 6M14 2v4h-4" /> },
  swap: { color: "#9ec0ff", body: <path d="M3 7h13l-3-3M17 13H4l3 3" /> },
};

function cellPos([col, row]: Cell): string {
  return `translate(${col * CELL} ${row * CELL})`;
}

function Sprite({ item }: { item: SceneItem }) {
  const s = item.s;
  if (s.includes(":")) {
    const [side, kind] = (s as PieceSprite).split(":");
    return <use href={`#p-${kind}`} className={`sv-pc sv-${side}`} transform="translate(.1 .3) scale(.33)" />;
  }
  const g = GLYPH[s as Glyph];
  const k = item.k ?? 1;
  return (
    <g className="sv-gl" style={{ color: item.c ?? g.color }} stroke="currentColor" transform={`translate(10 10) scale(${k}) translate(-10 -10)`}>
      {g.body}
    </g>
  );
}

function Squares() {
  const cells: ReactNode[] = [];
  for (let r = 0; r < BOARD_SIZE; r++) {
    for (let c = 0; c < BOARD_SIZE; c++) {
      cells.push(<rect key={`${c}-${r}`} x={c * CELL} y={r * CELL} width={CELL} height={CELL} className={(c + r) % 2 ? "sv-sq1" : "sv-sq0"} />);
    }
  }
  return <>{cells}</>;
}

function Board({ scene, snap, label, famVar }: { scene: Scene; snap?: Snapshot; label: string; famVar: string }) {
  const style = { ["--fam" as string]: famVar } as CSSProperties;
  return (
    <svg className="sv" viewBox={`0 0 ${BOARD_SIZE * CELL} ${BOARD_SIZE * CELL}`} role="img" aria-label={label} style={style}>
      <Squares />
      <g className={snap ? undefined : "sv-it sv-loop"}>
        {scene.map((item, idx) => {
          if (snap) {
            const r = resolveItem(item, snap);
            if (!r) return null;
            return (
              <g key={idx} transform={cellPos(r.at)} opacity={r.opacity}>
                <Sprite item={item} />
              </g>
            );
          }
          const cls = animClass(item);
          const move: CSSProperties = item.to
            ? ({ "--tx": `${(item.to[0] - item.at[0]) * CELL}px`, "--ty": `${(item.to[1] - item.at[1]) * CELL}px` } as CSSProperties)
            : {};
          return (
            <g key={idx} transform={cellPos(item.at)} opacity={item.dim}>
              <g className={cls ? `sv-it ${cls}` : undefined} style={move}>
                <Sprite item={item} />
              </g>
            </g>
          );
        })}
      </g>
    </svg>
  );
}

interface Props {
  id: string;
  /** Affiche la légende (description courte du catalogue). */
  caption?: boolean;
  /** Version réduite (vignette) : en mouvement réduit, seul l'état final est montré. */
  compact?: boolean;
  className?: string;
}

/**
 * Démonstration animée d'une compétence sur un mini-échiquier 5x5, en boucle, sans réseau ni Phaser.
 * Avec `prefers-reduced-motion`, affiche un « avant → après » statique.
 */
export const SkillPreview = memo(function SkillPreview({ id, caption, compact, className }: Props) {
  const t = useT();
  const reduced = usePrefersReducedMotion();
  const entry = skillEntry(id);
  const scene = SCENES[id as CatalogId];
  if (!scene) return null;
  const famVar = familyVar(entry.family);
  const label = t("skills.preview_label", { name: entry.name });
  return (
    <figure className={["sv-fig", className].filter(Boolean).join(" ")}>
      {!reduced ? (
        // La clé relance la boucle à zéro quand la compétence change.
        <Board key={id} scene={scene} label={label} famVar={famVar} />
      ) : compact ? (
        <Board scene={scene} snap="after" label={t("skills.preview_final", { label })} famVar={famVar} />
      ) : (
        <div className="sv-pair">
          <div>
            <Board scene={scene} snap="before" label={t("skills.before")} famVar={famVar} />
            <span className="sv-pair-cap">{t("skills.before")}</span>
          </div>
          <span className="sv-pair-arrow" aria-hidden="true">
            →
          </span>
          <div>
            <Board scene={scene} snap="after" label={t("skills.after")} famVar={famVar} />
            <span className="sv-pair-cap">{t("skills.after")}</span>
          </div>
        </div>
      )}
      {caption && entry.description && <figcaption>{entry.description}</figcaption>}
    </figure>
  );
});
