// L'icône d'une compétence forgée est assemblée à partir de sa `IconSpec` : un glyphe central (un par effet),
// la silhouette de la pièce concernée, un badge de durée et un cadre à la couleur de la rareté.
// Tout est dessiné dans le même viewBox 120x120 que les 27 icônes écrites à la main.
import type { ForgedDef, Rarity } from "../forged";

/** Un tracé par glyphe nommé par le serveur (`GLYPHS` dans `forge/identity.rs`). */
export const GLYPH_PATHS: Record<string, string> = {
  snowflake: "M60 20v80M25 40l70 40M25 80l70-40M50 26l10 8 10-8M50 94l10-8 10 8",
  shield: "M60 18l32 12v26c0 22-14 36-32 44-18-8-32-22-32-44V30z",
  veil: "M24 30h72M30 30c0 30 8 54 30 64 22-10 30-34 30-64M46 44c2 18 6 30 14 38",
  morph: "M32 58a28 28 0 0148-18M80 26v16H64M88 62a28 28 0 01-48 18M40 94V78h16",
  crown: "M22 88l-6-44 24 20 20-30 20 30 24-20-6 44zM24 98h72",
  erase: "M60 20a40 40 0 100 80 40 40 0 000-80zM32 32l56 56",
  banner: "M36 22v78M36 26h50l-12 17 12 17H36",
  portal: "M60 20c-14 0-24 18-24 40s10 40 24 40 24-18 24-40-10-40-24-40zM60 36c-6 0-10 10-10 24s4 24 10 24 10-10 10-24-4-24-10-24zM14 60h20M86 60h20",
  echo: "M60 52a8 8 0 100 16 8 8 0 000-16zM42 60a18 18 0 0118-18M78 60a18 18 0 01-18 18M30 60a30 30 0 0130-30M90 60a30 30 0 01-30 30",
  swap: "M26 44h62l-12-12M94 76H32l12 12",
  summon: "M60 22v18M60 80v18M22 60h18M80 60h18M33 33l13 13M74 74l13 13M87 33L74 46M46 74L33 87M60 50a10 10 0 100 20 10 10 0 000-20z",
  ankh: "M60 70v32M42 84h36M60 70c-18-6-20-40 0-40s18 34 0 40z",
  dove: "M24 90C48 74 74 52 98 28M52 68c-10-14-2-26 10-28 2 13-2 24-10 28zM72 50c-5-13 4-22 15-22 0 11-5 20-15 22z",
  mirror: "M46 20h28a12 12 0 0112 12v56a12 12 0 01-12 12H46a12 12 0 01-12-12V32a12 12 0 0112-12zM46 42l26 34M46 62l14 18",
  fog: "M22 44q12-10 24 0t24 0t24 0M22 62q12-10 24 0t24 0t24 0M22 80q12-10 24 0t24 0t24 0",
  mute: "M26 50h16l22-18v56L42 70H26zM78 46l22 28M100 46L78 74",
  domain: "M20 20h80v80H20zM36 36h48v48H36zM60 40l12 20-12 20-12-20zM8 8l16 16M112 8L96 24M8 112l16-16M112 112L96 96",
};

const FALLBACK = "M60 24a36 36 0 100 72 36 36 0 000-72zM52 52c0-12 16-12 16 0 0 8-8 8-8 18M60 82v2";

const RARITY_COLOR: Record<Rarity, string> = {
  common: "var(--rar-common)",
  uncommon: "var(--rar-uncommon)",
  rare: "var(--rar-rare)",
  epic: "var(--rar-epic)",
  legendary: "var(--rar-legendary)",
};

/** Badge « pour de bon » : l'infini. Une durée en coups se lit sur la jauge, pas sur une horloge. */
function Forever() {
  return (
    <g transform="translate(78 6)" strokeWidth="2.6">
      <path d="M6 18c0-6 6-8 10-4s8 8 12 4c4-4-2-10-8-8-4 1-6 4-8 6-4 4-10 2-10-2z" transform="translate(2 -6)" />
    </g>
  );
}

/** Nombre de crans de la jauge : la durée maximale d'un effet, en coups. */
export const GAUGE_SLOTS = 8;
const GAUGE_STEP = 12;

/** Les crans de la jauge de durée, en bas du cadre : `[x1, y1, x2, y2, allumé]`. Un cran allumé par coup. */
export function gaugeTicks(plies: number): [number, number, number, number, boolean][] {
  const out: [number, number, number, number, boolean][] = [];
  // De gauche à droite : le premier cran est en bas à gauche.
  const start = 90 + ((GAUGE_SLOTS - 1) * GAUGE_STEP) / 2;
  for (let i = 0; i < GAUGE_SLOTS; i++) {
    const a = ((start - i * GAUGE_STEP) * Math.PI) / 180;
    const at = (r: number) => [60 + r * Math.cos(a), 60 + r * Math.sin(a)];
    const [x1, y1] = at(45);
    const [x2, y2] = at(50);
    out.push([+x1.toFixed(1), +y1.toFixed(1), +x2.toFixed(1), +y2.toFixed(1), i < plies]);
  }
  return out;
}

/** Les règles autour de la compétence, un petit signe chacune, centré sur l'origine (10 px). */
export const MARK_PATHS: Record<string, string> = {
  in_check: "M0-5L5 4H-5zM0-2v3M0 3v.01",
  no_mate: "M-5 0a5 5 0 1010 0a5 5 0 10-10 0M-3.5-3.5l7 7",
  no_check: "M0-5v10M-3-2h6M-5 5L5-5",
  free: "M2-6L-3 1H1L-2 6L4-1H0z",
};

/** À qui s'adresse la compétence, dans le coin haut gauche. */
function Target({ kind }: { kind: string }) {
  return (
    <g transform="translate(24 24)" strokeWidth="2.2">
      {kind === "own" && <circle r="5" fill="currentColor" />}
      {kind === "enemy" && <path d="M-5 0a5 5 0 1010 0a5 5 0 10-10 0M0-9v5M0 4v5M-9 0h5M4 0h5" />}
      {kind === "any" && <path d="M-7 0a5 5 0 1010 0a5 5 0 10-10 0M-3 0a5 5 0 1010 0a5 5 0 10-10 0" />}
    </g>
  );
}

/** Les couches d'une icône forgée (à placer dans un `<svg viewBox="0 0 120 120">`). */
export function ForgedLayers({ def }: { def: ForgedDef | undefined }) {
  const glyph = def ? (GLYPH_PATHS[def.icon.glyph] ?? FALLBACK) : FALLBACK;
  const rarity: Rarity = def?.rarity ?? "common";
  const legendary = rarity === "legendary";
  return (
    <>
      <circle cx="60" cy="60" r="55" stroke={RARITY_COLOR[rarity]} strokeWidth={legendary ? 3.5 : 2.2} opacity={def ? 0.9 : 0.35} />
      {legendary && <circle cx="60" cy="60" r="50" stroke={RARITY_COLOR[rarity]} strokeWidth="1.2" opacity="0.7" />}
      {(rarity === "epic" || legendary) && <path d="M60 2v7M60 111v7M2 60h7M111 60h7" stroke={RARITY_COLOR[rarity]} strokeWidth="2.4" />}
      <g transform="translate(60 60) scale(.74) translate(-60 -60)">
        <path d={glyph} />
      </g>
      {def?.icon.piece && (
        <g transform="translate(66 62) scale(.72)">
          <use href={`#p-${def.icon.piece}`} />
        </g>
      )}
      {(def?.icon.kinds?.length ?? 0) > 1 &&
        def!.icon.kinds!.slice(1).map((k, i) => <circle key={k} cx={72 + i * 8} cy="56" r="2.2" fill="currentColor" stroke="none" />)}
      {def?.icon.target && <Target kind={def.icon.target} />}
      {def?.icon.plies !== undefined && (
        <g strokeWidth="3" data-gauge={def.icon.plies}>
          {gaugeTicks(def.icon.plies).map(([x1, y1, x2, y2, on], i) => (
            <path key={i} d={`M${x1} ${y1}L${x2} ${y2}`} opacity={on ? 1 : 0.2} />
          ))}
        </g>
      )}
      {(def?.icon.uses ?? 1) > 1 &&
        Array.from({ length: def!.icon.uses! }, (_, i) => (
          <circle key={i} cx={60 + (i - (def!.icon.uses! - 1) / 2) * 9} cy="15" r="2.6" fill="currentColor" stroke="none" />
        ))}
      {def?.icon.marks?.map((m, i, all) => (
        <g key={m} transform={`translate(16 ${60 + (i - (all.length - 1) / 2) * 14})`} strokeWidth="2">
          <path d={MARK_PATHS[m] ?? ""} fill={m === "free" ? "currentColor" : "none"} />
        </g>
      ))}
      {def?.icon.badge === "forever" && <Forever />}
    </>
  );
}
