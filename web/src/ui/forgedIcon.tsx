// L'icône d'une compétence forgée : UN signe à plat (comme les pictogrammes de signalétique ou game-icons.net),
// deux ou trois couleurs, posé sur un fond uni à la couleur de la famille. La rareté ne passe que par le contour
// (forme et couleur). Durée, usages, zone et règles sont dans l'infobulle, pas dans l'icône : une icône doit se
// comprendre en une seconde à 32 px. Tout est dessiné dans le viewBox 120x120 des 27 icônes écrites à la main.
import type { ReactNode } from "react";
import type { Family } from "../catalog";
import type { ForgedDef, Rarity } from "../forged";

/** Les trois encres d'un signe : crème (principal), or (accent), rouge (interdit/retrait), sombre (creux). */
export const INK_COLORS = { m: "#fff6e0", a: "#ffd34d", r: "#ff6a5a", k: "#1b1d26" } as const;
type Role = keyof typeof INK_COLORS;
/** Une forme : rôle de couleur, remplie ("f") ou tracée ("s"), épaisseur du trait, tracé SVG. */
type Part = [Role, "f" | "s", number, string];

/** Fond uni par famille (assez sombre pour que la crème et l'or ressortent). */
export const FAMILY_FLAT: Record<Family, string> = {
  attack: "#a63630",
  defense: "#1f6a41",
  mobility: "#146a78",
  control: "#3f63c8",
  create: "#7a4a12",
};

export const RARITY_EDGE: Record<Rarity, string> = {
  common: "#b4bccb",
  uncommon: "#5fd46f",
  rare: "#4aa8ff",
  epic: "#b66bff",
  legendary: "#ffb824",
};

/** Un signe par verbe de l'effet (les `GLYPHS` du serveur). */
export const SIGNS: Record<string, Part[]> = {
  snowflake: [["m","s",9,"M60.0 60.0L60.0 20.0M49.0 24.0L60.0 34.0L71.0 24.0M60.0 60.0L94.6 40.0M85.7 32.5L82.5 47.0L96.7 51.5M60.0 60.0L94.6 80.0M96.7 68.5L82.5 73.0L85.7 87.5M60.0 60.0L60.0 100.0M71.0 96.0L60.0 86.0L49.0 96.0M60.0 60.0L25.4 80.0M34.3 87.5L37.5 73.0L23.3 68.5M60.0 60.0L25.4 40.0M23.3 51.5L37.5 47.0L34.3 32.5"]],
  shield: [["m","f",0,"M60 16l34 12v26c0 24-16 38-34 48-18-10-34-24-34-48V28z"],["a","f",0,"M60 30l20 7v17c0 15-9 24-20 31z"]],
  veil: [["m","f",0,"M60 16c-24 0-36 20-36 42 0 16-6 26-8 40h88c-2-14-8-24-8-40 0-22-12-42-36-42z"],["k","f",0,"M60 36c-14 0-22 10-22 24s8 22 22 22 22-8 22-22-8-24-22-24z"],["a","f",0,"M46 58l10 4-10 4zM74 58l-10 4 10 4z"]],
  morph: [["m","s",10,"M24 56a36 36 0 0 1 62-18"],["a","s",10,"M96 64a36 36 0 0 1-62 18"],["m","f",0,"M92 20l2 26-26-2z"],["a","f",0,"M28 100l-2-26 26 2z"]],
  crown: [["a","f",0,"M22 88L16 36l26 20 18-30 18 30 26-20-6 52z"],["m","f",0,"M24 92h72v12H24z"]],
  erase: [["m","f",0,"M60 14a17 17 0 1 1 0 34 17 17 0 0 1 0-34zM42 54h36l-7 16 10 28H39l10-28z"],["r","s",12,"M24 100L96 22M24 22l72 78"]],
  banner: [["m","s",8,"M30 100V20"],["a","f",0,"M34 22h58l-16 20 16 20H34z"]],
  portal: [["m","s",9,"M60 18c-18 0-30 18-30 42s12 42 30 42 30-18 30-42-12-42-30-42z"],["a","f",0,"M60 38c-9 0-14 10-14 22s5 22 14 22 14-10 14-22-5-22-14-22z"]],
  echo: [["a","s",8,"M30 30h44v44H30z"],["m","f",0,"M50 50h44a0 0 0 0 1 0 0v44H50z"]],
  swap: [["m","s",10,"M22 44h62M70 28l18 16-18 16"],["a","s",10,"M98 78H36M50 62L32 78l18 16"]],
  summon: [["a","f",0,"M60 14c4 24 10 30 34 34-24 4-30 10-34 34-4-24-10-30-34-34 24-4 30-10 34-34z"],["m","f",0,"M92 70c2 12 5 15 14 17-9 2-12 5-14 17-2-12-5-15-14-17 9-2 12-5 14-17z"]],
  ankh: [["a","s",13,"M60 66v38M36 82h48"],["a","s",13,"M60 66c-24-6-26-46 0-46s24 40 0 46z"]],
  dove: [["m","f",0,"M16 62c16 4 26-4 30-16 4-12 16-20 34-16-6 6-6 12-4 16 8 2 16 0 22-6-2 18-14 32-32 32l-8 16-8-14c-14 2-28-2-34-12z"],["a","s",5,"M92 86c8 4 14 2 18-6"]],
  mirror: [["m","f",0,"M60 12c-20 0-32 12-32 30v38c0 18 12 28 32 28s32-10 32-28V42c0-18-12-30-32-30z"],["k","f",0,"M60 26c-12 0-20 7-20 18v34c0 11 8 16 20 16s20-5 20-16V44c0-11-8-18-20-18z"],["a","s",7,"M48 58l24-18M48 78l24-18"]],
  fog: [["m","f",0,"M34 70c-12 0-18-6-18-14 0-8 7-14 16-13 2-12 14-20 28-18 12 2 20 10 22 20 12-2 22 6 22 14s-8 12-18 12z"],["a","s",8,"M28 86h46M44 100h50"]],
  mute: [["m","f",0,"M20 46h18l26-22v72L38 74H20z"],["m","s",7,"M80 50c6 6 6 14 0 20M92 40c12 12 12 28 0 40"],["r","s",10,"M24 100L98 20"]],
  domain: [["m","s",10,"M24 46V24h22M74 24h22v22M96 74v22H74M46 96H24V74"],["a","f",0,"M60 36c3 15 9 21 24 24-15 3-21 9-24 24-3-15-9-21-24-24 15-3 21-9 24-24z"]],
};

export const GLYPH_NAMES = Object.keys(SIGNS);

const f1 = (n: number) => Math.round(n * 10) / 10;
const ngon = (n: number, r: number, start: number, inner?: number) =>
  Array.from({ length: inner ? n * 2 : n }, (_, i) => {
    const a = ((start + (i * 360) / (inner ? n * 2 : n)) * Math.PI) / 180;
    const rr = inner && i % 2 ? inner : r;
    return `${f1(60 + rr * Math.cos(a))},${f1(60 + rr * Math.sin(a))}`;
  }).join(" ");

/** Le contour dit la rareté : cercle, carré arrondi, hexagone, octogone, soleil. */
function frameShape(rarity: Rarity, p: Record<string, unknown>) {
  switch (rarity) {
    case "common":
      return <circle cx="60" cy="60" r="54" {...p} />;
    case "uncommon":
      return <rect x="8" y="8" width="104" height="104" rx="26" {...p} />;
    case "rare":
      return <polygon points={ngon(6, 56, -90)} {...p} />;
    case "epic":
      return <polygon points={ngon(8, 56, -67.5)} {...p} />;
    case "legendary":
      return <polygon points={ngon(12, 58, -90, 49)} {...p} />;
  }
}

/** Le signe d'un verbe inconnu : un point d'interrogation. */
const UNKNOWN: Part[] = [
  ["m", "s", 10, "M42 44c0-20 36-20 36 0 0 14-18 14-18 30"],
  ["m", "f", 0, "M60 86a7 7 0 1 1 0 14 7 7 0 0 1 0-14z"],
];

function Sign({ parts }: { parts: Part[] }): ReactNode {
  return (
    <g transform="translate(60 60) scale(.74) translate(-60 -60)">
      {parts.map(([role, kind, w, d], i) =>
        kind === "f" ? (
          <path key={i} d={d} fill={INK_COLORS[role]} stroke="none" />
        ) : (
          <path key={i} d={d} fill="none" stroke={INK_COLORS[role]} strokeWidth={w} strokeLinecap="round" strokeLinejoin="round" />
        ),
      )}
    </g>
  );
}

/** Les couches d'une icône forgée (à placer dans un `<svg viewBox="0 0 120 120">`). */
export function ForgedLayers({ def }: { def: ForgedDef | undefined }) {
  if (!def) return <circle cx="60" cy="60" r="50" stroke="currentColor" strokeWidth="3" opacity="0.35" />;
  return <ForgedBadge glyph={def.icon.glyph} family={def.family} rarity={def.rarity} />;
}

/** Le signe, son fond de famille et son contour de rareté (séparé de `ForgedDef` pour les planches de test). */
export function ForgedBadge({ glyph, family, rarity }: { glyph: string; family: Family; rarity: Rarity }) {
  return (
    <>
      {frameShape(rarity, { fill: FAMILY_FLAT[family], stroke: RARITY_EDGE[rarity], strokeWidth: 7, strokeLinejoin: "round" })}
      <Sign parts={SIGNS[glyph] ?? UNKNOWN} />
    </>
  );
}
