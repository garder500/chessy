// L'icône d'une compétence forgée : un médaillon dont le héros est une pièce d'échecs sur laquelle l'effet agit
// (glace, dissolution, nuage, dôme…), un anneau qui se remplit avec la durée, un fond pour la zone, une pastille
// pour la règle qui compte et un sigil décoratif tiré d'une graine. Le cadre dit la rareté.
// Tout est dessiné dans le viewBox 120x120 des 27 icônes écrites à la main ; le serveur décrit (`IconSpec`),
// le client dessine. Les mesures à 32 px qui ont guidé ce dessin sont dans `docs/spec-forge.md`.
import type { ReactNode } from "react";
import type { Family } from "../catalog";
import type { ForgedDef, IconSpec, Rarity } from "../forged";

const INK = "#0d1020";

/** Fond du médaillon par famille : [centre, bord]. */
export const FAMILY_BG: Record<Family, [string, string]> = {
  attack: ["#e8644e", "#5a1018"],
  defense: ["#3fae62", "#0c3a22"],
  mobility: ["#2fb5c4", "#08354a"],
  control: ["#5f7fe6", "#14246a"],
  create: ["#e9a63a", "#5a3206"],
};

const hex2 = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, "0");

/** Décale la teinte d'une couleur #rrggbb de `deg` degrés (pour distinguer les effets d'une même famille). */
export function hueShift(hex: string, deg: number): string {
  const n = parseInt(hex.slice(1), 16);
  const r = (n >> 16) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2, d = mx - mn;
  let h = 0, sat = 0;
  if (d > 0) {
    sat = d / (1 - Math.abs(2 * l - 1));
    h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h *= 60;
  }
  h = (((h + deg) % 360) + 360) % 360;
  const c = (1 - Math.abs(2 * l - 1)) * sat, x = c * (1 - Math.abs(((h / 60) % 2) - 1)), m = l - c / 2;
  const [rr, gg, bb] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return `#${hex2((rr + m) * 255)}${hex2((gg + m) * 255)}${hex2((bb + m) * 255)}`;
}

/** Décalage de teinte de chaque effet dans sa famille : sept effets de contrôle ne doivent pas tous être bleus. */
export const GLYPH_HUE: Record<string, number> = {
  snowflake: 0, morph: 45, dove: -22, mirror: 24, fog: -42, mute: 70, domain: -62,
  shield: 0, veil: 38,
  erase: 0, banner: 26,
  swap: 0, portal: -24,
  summon: 0, crown: -18, echo: 18, ankh: -34,
};

/** Couleur du sigil par famille. */
const FAMILY_ACCENT: Record<Family, string> = {
  attack: "#ff9a86",
  defense: "#86e89a",
  mobility: "#74e6ea",
  control: "#9db4ff",
  create: "#ffdc82",
};

export const RARITY_EDGE: Record<Rarity, string> = {
  common: "#b4bccb",
  uncommon: "#5fd46f",
  rare: "#4aa8ff",
  epic: "#b66bff",
  legendary: "#ffb824",
};

// ---------- Outils ----------

export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const f1 = (n: number) => n.toFixed(1);

const ngon = (n: number, r: number, rot: number, r2?: number) =>
  Array.from({ length: n }, (_, i) => {
    const a = ((rot + (360 / n) * i) * Math.PI) / 180;
    const rr = r2 && i % 2 ? r2 : r;
    return `${f1(60 + rr * Math.cos(a))},${f1(60 + rr * Math.sin(a))}`;
  }).join(" ");

/** La forme du cadre : cercle, carré arrondi, hexagone, octogone, soleil à pointes. */
function frameShape(rarity: Rarity, p: Record<string, unknown>) {
  switch (rarity) {
    case "common":
      return <circle cx="60" cy="60" r="56" {...p} />;
    case "uncommon":
      return <rect x="6" y="6" width="108" height="108" rx="30" {...p} />;
    case "rare":
      return <polygon points={ngon(6, 58, -90)} {...p} />;
    case "epic":
      return <polygon points={ngon(8, 58, -67.5)} {...p} />;
    case "legendary":
      return <polygon points={ngon(24, 58, -90, 51)} {...p} />;
  }
}

const PIECES: Record<string, ReactNode> = {
  pawn: (
    <>
      <circle cx="30" cy="15" r="9.5" />
      <path d="M19 52c1-10 5-17 11-21 6 4 10 11 11 21z" />
      <path d="M14 54h32" />
    </>
  ),
  knight: (
    <>
      <path d="M14 54h32" />
      <path d="M18 54c0-14 4-18 10-24-5 1-8 3-12 7l-6-4c3-8 9-18 22-20 10 2 18 12 18 28v13" />
    </>
  ),
  bishop: (
    <>
      <path d="M30 10c9 7 14 15 12 24-1 5-4 8-6 10H24c-2-2-5-5-6-10-2-9 3-17 12-24z" />
      <path d="M30 22v10M25 27h10M20 54h20" />
    </>
  ),
  rook: <path d="M14 54v-8h32v8zM18 46V30h24v16zM12 30V12h9v7h5v-7h8v7h5v-7h9v18z" />,
  queen: (
    <>
      <path d="M12 20l8 22 10-28 10 28 8-22-4 34H16z" />
      <circle cx="12" cy="18" r="2" />
      <circle cx="30" cy="12" r="2" />
      <circle cx="48" cy="18" r="2" />
    </>
  ),
  king: (
    <>
      <path d="M30 6v14M23 12h14" />
      <path d="M30 22c-10 0-16 7-14 15 1 5 5 8 7 11l-4 6h22l-4-6c2-3 6-6 7-11 2-8-4-15-14-15z" />
      <path d="M16 54h28" />
    </>
  ),
};

interface PieceProps {
  kind: string;
  cx: number;
  cy: number;
  s: number;
  fill: string;
  line: string;
  opacity?: number;
  flip?: boolean;
  dash?: boolean;
}

/** Silhouette pleine : contour d'encre épais dessous, remplissage et liseré dessus. */
function Pc({ kind, cx, cy, s, fill, line, opacity, flip, dash }: PieceProps) {
  const t = `translate(${cx} ${cy}) scale(${flip ? -s : s} ${s}) translate(-30 -30)`;
  const body = PIECES[kind] ?? PIECES.knight;
  return (
    <g opacity={opacity} strokeLinejoin="round" strokeLinecap="round">
      <g transform={t} fill={INK} stroke={INK} strokeWidth={10 / s}>
        {body}
      </g>
      <g transform={t} fill={fill} stroke={line} strokeWidth={2.8 / s} strokeDasharray={dash ? `${4 / s} ${4 / s}` : undefined}>
        {body}
      </g>
    </g>
  );
}

/** Un trait épais cerné d'encre (flèches, arcs). */
function Stroke({ d, color, w = 6 }: { d: string; color: string; w?: number }) {
  return (
    <>
      <path d={d} fill="none" stroke={INK} strokeWidth={w + 5} strokeLinecap="round" strokeLinejoin="round" />
      <path d={d} fill="none" stroke={color} strokeWidth={w} strokeLinecap="round" strokeLinejoin="round" />
    </>
  );
}

/** Une forme pleine cernée d'encre. */
function Solid({ d, fill, w = 5 }: { d: string; fill: string; w?: number }) {
  return <path d={d} fill={fill} stroke={INK} strokeWidth={w} strokeLinejoin="round" paintOrder="stroke" />;
}

interface Ctx {
  kind: string;
  enemy: boolean;
  seed: number;
  fill: string;
  line: string;
}

const CX = 60;
const CY = 58;
const S = 1.4;

type Scene = (c: Ctx) => ReactNode;

const SPARKLE = (x: number, y: number) => `M${x} ${y - 7}l2 5 5 2-5 2-2 5-2-5-5-2 5-2z`;
const CLOUDS: [number, number, number][] = [
  [24, 76, 15],
  [48, 84, 17],
  [74, 80, 16],
  [96, 74, 13],
  [60, 68, 14],
];

/** Une scène par glyphe nommé par le serveur (`GLYPHS`) : la pièce est le héros, le verbe agit dessus. */
export const SCENES: Record<string, Scene> = {
  // Prise dans la glace : pièce givrée, fissures, glaçons.
  snowflake: ({ kind }) => (
    <>
      <Pc kind={kind} cx={CX} cy={CY} s={S} fill="#dff8ff" line="#ffffff" />
      <path d={`M${CX - 8} ${CY - 20}l6 12-4 10M${CX + 10} ${CY - 6}l-8 8 6 10`} fill="none" stroke="#7fcff0" strokeWidth="2.2" />
      {[-18, 0, 16].map((dx, i) => (
        <path key={i} d={`M${CX + dx - 5} ${CY + 20}l5 ${12 + i * 3} 5-${12 + i * 3}z`} fill="#e6fbff" stroke={INK} strokeWidth="2.5" strokeLinejoin="round" />
      ))}
    </>
  ),
  // Sous un dôme protecteur.
  shield: ({ kind, fill, line }) => (
    <>
      <Stroke d={`M${CX - 40} ${CY + 26}a40 42 0 0180 0`} color="#9affc0" w={11} />
      <Pc kind={kind} cx={CX} cy={CY + 6} s={S * 0.88} fill={fill} line={line} />
    </>
  ),
  // Cachée : la pièce disparaît sous un voile sombre, seuls ses yeux brillent.
  veil: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX} cy={CY} s={S} fill={fill} line={line} />
      <path d="M22 108C20 70 30 30 60 22c30 8 40 48 38 86l-9-9-9 9-10-9-10 9-9-9-9 9z" fill="#2a1146" fillOpacity="0.92" stroke={INK} strokeWidth="5" strokeLinejoin="round" />
      <ellipse cx="48" cy="56" rx="5.5" ry="7" fill="#fff3a8" />
      <ellipse cx="72" cy="56" rx="5.5" ry="7" fill="#fff3a8" />
    </>
  ),
  // Une pièce en devient une autre : deux grandes flèches circulaires autour de la pièce.
  morph: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX} cy={CY + 2} s={S * 0.78} fill={fill} line={line} />
      <Stroke d="M26 50A38 38 0 0 1 84 26M84 26l-2-14M84 26l-14 2" color="#ffe08a" w={7} />
      <Stroke d="M94 66A38 38 0 0 1 36 90M36 90l2 14M36 90l14-2" color="#ffe08a" w={7} />
    </>
  ),
  // Le pion devient dame : une grande couronne d'or sacre la pièce.
  crown: ({ fill, line }) => (
    <>
      <Pc kind="pawn" cx={CX} cy={CY + 14} s={S * 0.95} fill={fill} line={line} />
      <Solid d="M28 58l-8-34 22 16 18-26 18 26 22-16-8 34z" fill="#ffd34d" w={5} />
      <path d="M30 66h60" stroke="#ffd34d" strokeWidth="6" strokeLinecap="round" />
      <circle cx="60" cy="40" r="4" fill="#ff5a5a" stroke={INK} strokeWidth="2" />
    </>
  ),
  // La pièce se délite en cubes.
  erase: ({ kind, seed, enemy, fill, line }) => {
    const r = rng(seed ^ 0x51ed);
    const holes: ReactNode[] = [];
    for (let y = 0; y < 8; y++)
      for (let x = 0; x < 8; x++) {
        const p = (y + 1) / 9 + r() * 0.3;
        if (p > 0.5 && r() < 0.85) holes.push(<rect key={`${x}${y}`} x={CX - 40 + x * 10} y={CY - 36 + y * 10} width="10" height="10" fill="black" />);
      }
    const parts = Array.from({ length: 7 }, (_, i) => (
      <rect key={i} x={f1(CX - 36 + r() * 72)} y={f1(CY + 10 + r() * 34)} width={f1(5 + r() * 5)} height={f1(5 + r() * 5)} fill={enemy ? "#c79bff" : "#f4f7ff"} stroke={INK} strokeWidth="2" />
    ));
    return (
      <>
        <mask id={`er${seed}`}>
          <rect width="120" height="120" fill="white" />
          {holes}
        </mask>
        <g mask={`url(#er${seed})`}>
          <Pc kind={kind} cx={CX} cy={CY - 2} s={S} fill={fill} line={line} />
        </g>
        {parts}
        <Stroke d="M30 30L90 90M90 30L30 90" color="#ff6a5a" w={9} />
      </>
    );
  },
  // Elle change de camp : même pièce, moitié sombre et moitié claire, flèche par-dessus.
  banner: ({ kind, seed }) => (
    <>
      <clipPath id={`bl${seed}`}>
        <rect x="0" y="0" width="60" height="120" />
      </clipPath>
      <clipPath id={`br${seed}`}>
        <rect x="60" y="0" width="60" height="120" />
      </clipPath>
      <g clipPath={`url(#br${seed})`}>
        <Pc kind={kind} cx={CX} cy={CY + 4} s={S} fill="#f4f7ff" line="#9fb0d8" />
      </g>
      <g clipPath={`url(#bl${seed})`}>
        <Pc kind={kind} cx={CX} cy={CY + 4} s={S} fill="#7a45c8" line="#f3e4ff" />
      </g>
      <Stroke d="M26 34A40 40 0 0 1 88 28" color="#fff1c2" w={6} />
      <Solid d="M84 12l18 14-22 8z" fill="#fff1c2" w={4} />
    </>
  ),
  // Elle disparaît d'ici et revient par un portail.
  portal: ({ kind, fill, line }) => (
    <>
      <ellipse cx="38" cy="60" rx="20" ry="40" fill="none" stroke={INK} strokeWidth="13" />
      <ellipse cx="38" cy="60" rx="20" ry="40" fill="none" stroke="#8ff0ff" strokeWidth="6" />
      <ellipse cx="38" cy="60" rx="9" ry="22" fill="#0d3a52" stroke="#bff6ff" strokeWidth="2.5" />
      <Pc kind={kind} cx={CX + 20} cy={CY + 2} s={S * 0.9} fill={fill} line={line} />
      <Stroke d="M30 96l-6 8M42 100l-4 8" color="#bff6ff" w={3} />
    </>
  ),
  // Un écho : la pièce et sa copie identique côte à côte, un « + » d'or entre elles.
  echo: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX - 24} cy={CY + 10} s={S * 0.72} fill={fill} line={line} />
      <Pc kind={kind} cx={CX + 24} cy={CY + 10} s={S * 0.72} fill={fill} line={line} />
      <Stroke d="M60 14v26M47 27h26" color="#ffd34d" w={7} />
    </>
  ),
  // Deux pièces échangent leur place.
  swap: ({ kind, fill, line, enemy }) => (
    <>
      <Pc kind={kind} cx={CX - 24} cy={CY + 4} s={S * 0.62} fill={fill} line={line} />
      <Pc kind="pawn" cx={CX + 24} cy={CY + 4} s={S * 0.62} fill={enemy ? "#f4f7ff" : "#7a45c8"} line={enemy ? "#9fb0d8" : "#f3e4ff"} />
      <Stroke d={`M${CX - 38} ${CY - 20}h66l-12-11M${CX + 28} ${CY - 20}l-12 11M${CX + 38} ${CY + 40}h-66l12 11M${CX - 28} ${CY + 40}l12-11`} color="#c9f6ff" w={7} />
    </>
  ),
  // Elle surgit d'un cercle lumineux, faisceau et étincelles.
  summon: ({ kind, fill, line }) => (
    <>
      <path d={`M${CX - 44} ${CY + 34}L${CX - 22} ${CY - 50}h44l22 84z`} fill="#fff6c8" fillOpacity="0.75" />
      <ellipse cx={CX} cy={CY + 34} rx="36" ry="9" fill="#fff3b8" stroke={INK} strokeWidth="4" />
      <Pc kind={kind} cx={CX} cy={CY - 2} s={S * 0.95} fill={fill} line={line} />
      {[[22, 28], [96, 34], [100, 76]].map(([x, y]) => (
        <path key={`${x}${y}`} d={SPARKLE(x, y)} fill="#fff7c2" stroke={INK} strokeWidth="2" />
      ))}
    </>
  ),
  // Elle revient : un ankh d'or derrière la pièce, nimbe et faisceau.
  ankh: ({ kind, fill, line }) => (
    <>
      <g transform="translate(60 56) scale(1.35) translate(-60 -66)">
        <path d="M60 70v34M40 84h40M60 70c-20-6-22-44 0-44s20 38 0 44z" fill="none" stroke={INK} strokeWidth="15" strokeLinecap="round" strokeLinejoin="round" />
        <path d="M60 70v34M40 84h40M60 70c-20-6-22-44 0-44s20 38 0 44z" fill="none" stroke="#ffd34d" strokeWidth="8" strokeLinecap="round" strokeLinejoin="round" />
      </g>
      <Pc kind={kind} cx={CX} cy={CY + 10} s={S * 0.8} fill={fill} line={line} />
    </>
  ),
  // Trêve : une colombe au-dessus de la pièce.
  dove: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX} cy={CY + 10} s={S * 0.92} fill={fill} line={line} />
      <g transform="translate(24 -2) scale(.72)">
        <Solid d="M10 60c20-4 30-18 44-34 6 14 4 30-6 40 16-2 30-14 40-30 6 24-6 48-34 56-20 6-40-2-48-14z" fill="#ffffff" w={6} />
        <circle cx="40" cy="50" r="2.4" fill={INK} />
      </g>
    </>
  ),
  // Une pièce et son reflet de part et d'autre d'un grand miroir.
  mirror: ({ kind, fill, line, enemy }) => (
    <>
      <Pc kind={kind} cx={CX - 30} cy={CY + 6} s={S * 0.68} fill={fill} line={line} />
      <Pc kind={kind} cx={CX + 30} cy={CY + 6} s={S * 0.68} fill={enemy ? "#f4f7ff" : "#7a45c8"} line={enemy ? "#9fb0d8" : "#f3e4ff"} flip opacity={0.8} />
      <Solid d="M50 10h20a6 6 0 016 6v88a6 6 0 01-6 6H50a6 6 0 01-6-6V16a6 6 0 016-6z" fill="#cfeeff" w={5} />
      <path d="M52 30l16-8M52 52l16-8M52 74l16-8" stroke="#fff" strokeWidth="4" strokeLinecap="round" />
    </>
  ),
  // Le bas de la pièce se noie dans des nuages épais.
  fog: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX} cy={CY - 6} s={S} fill={fill} line={line} />
      <g fill={INK}>
        {CLOUDS.map(([x, y, r]) => (
          <circle key={`${x}${y}`} cx={x} cy={y} r={r + 3} />
        ))}
      </g>
      <g fill="#eaf0ff">
        {CLOUDS.map(([x, y, r]) => (
          <circle key={`${x}${y}`} cx={x} cy={y} r={r} />
        ))}
      </g>
    </>
  ),
  // Silence : un haut-parleur barré devant la pièce.
  mute: ({ kind, fill, line }) => (
    <>
      <Pc kind={kind} cx={CX - 8} cy={CY + 6} s={S * 0.92} fill={fill} line={line} />
      <g transform="translate(40 -2) scale(.95)">
        <Solid d="M10 40h18l26-22v74L28 70H10z" fill="#fff1c2" w={7} />
        <Stroke d="M62 36l30 40M92 36L62 76" color="#ff7a7a" w={9} />
      </g>
    </>
  ),
  // Expansion de domaine : le roi cerné par des angles de domaine, des fous aux coins.
  domain: ({ fill, line }) => (
    <>
      <Stroke d="M12 36V12h24M84 12h24v24M108 84v24H84M36 108H12V84" color="#ffd34d" w={9} />
      <Pc kind="king" cx={CX} cy={CY + 2} s={S * 0.95} fill={fill} line={line} />
      <Pc kind="bishop" cx={26} cy={30} s={0.5} fill="#f4f7ff" line="#9fb0d8" />
      <Pc kind="bishop" cx={94} cy={90} s={0.5} fill="#f4f7ff" line="#9fb0d8" />
    </>
  ),
};

/** Les glyphes que le serveur peut nommer. */
export const GLYPH_NAMES = Object.keys(SCENES);

// ---------- Le sigil décoratif ----------

const FOLDS: Record<string, number> = {
  snowflake: 6, shield: 4, veil: 3, morph: 7, crown: 5, erase: 5, banner: 3, portal: 2, echo: 4,
  swap: 2, summon: 8, ankh: 6, dove: 5, mirror: 2, fog: 3, mute: 4, domain: 8,
};
const STYLES: Record<string, "straight" | "curve" | "zig" | "ray" | "wave" | "spiral"> = {
  snowflake: "straight", shield: "curve", veil: "wave", morph: "spiral", crown: "ray", erase: "zig", banner: "zig", portal: "curve", echo: "straight",
  swap: "curve", summon: "ray", ankh: "wave", dove: "curve", mirror: "zig", fog: "wave", mute: "zig", domain: "ray",
};

/** Le chemin d'un bras du sigil (en coordonnées centrées) et la pointe : tirés de la graine, symétriques par rotation. */
export function sigilArm(glyph: string, seed: number): { d: string; tip: [number, number] } {
  const r = rng(seed);
  const style = STYLES[glyph] ?? "straight";
  const segs = 2 + Math.floor(r() * 3);
  const L = 40 / segs;
  const pts: [number, number][] = [[0, 0]];
  let ang = 0;
  let rad = 0;
  for (let i = 0; i < segs; i++) {
    rad += L;
    let da = 0;
    if (style === "zig") da = (i % 2 ? 1 : -1) * 0.5;
    if (style === "wave") da = Math.sin(i * 1.7) * 0.5;
    if (style === "spiral") da = 0.35;
    if (style === "curve") da = 0.25;
    da += (r() - 0.5) * 0.25;
    ang += da;
    pts.push([rad * Math.cos(ang - Math.PI / 2), rad * Math.sin(ang - Math.PI / 2)]);
  }
  let d = "M" + pts.map(([x, y]) => `${f1(x)} ${f1(y)}`).join("L");
  const nb = 1 + Math.floor(r() * 3);
  for (let b = 0; b < nb; b++) {
    const i = 1 + Math.floor(r() * (pts.length - 1));
    const [x, y] = pts[i];
    const len = 8 + r() * 10;
    const a = Math.atan2(y, x);
    for (const s of [0.7, -0.7]) d += `M${f1(x)} ${f1(y)}L${f1(x + len * Math.cos(a + s))} ${f1(y + len * Math.sin(a + s))}`;
  }
  return { d, tip: pts[pts.length - 1] };
}

/** Le sigil, rogné à l'anneau extérieur du médaillon pour laisser le centre à la pièce. */
function Sigil({ glyph, seed, color, id }: { glyph: string; seed: number; color: string; id: string }) {
  const n = FOLDS[glyph] ?? 5;
  const { d, tip } = sigilArm(glyph, seed);
  const arms = Array.from({ length: n }, (_, k) => (
    <g key={k} transform={`rotate(${(360 / n) * k})`}>
      <path d={d} fill="none" stroke={INK} strokeWidth="11" strokeLinecap="round" strokeLinejoin="round" />
      <path d={d} fill="none" stroke={color} strokeWidth="6" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx={f1(tip[0])} cy={f1(tip[1])} r="4.5" fill={color} stroke={INK} strokeWidth="3" />
    </g>
  ));
  return (
    <>
      <clipPath id={`sg${id}`}>
        <path fillRule="evenodd" d="M60 8a52 52 0 1 0 .01 0zM60 32a28 28 0 1 1-.01 0z" />
      </clipPath>
      <g opacity="0.5" clipPath={`url(#sg${id})`}>
        <g transform="translate(60 60)">{arms}</g>
      </g>
    </>
  );
}

// ---------- Médaillon ----------

/** L'arc d'un anneau, de midi dans le sens horaire : `fraction` du tour. */
function arc(r: number, fraction: number): string {
  if (fraction >= 0.999) return `M60 ${60 - r}a${r} ${r} 0 1 1 0 ${2 * r}a${r} ${r} 0 1 1 0 ${-2 * r}`;
  const a1 = -Math.PI / 2 + fraction * Math.PI * 2;
  const x = 60 + r * Math.cos(a1);
  const y = 60 + r * Math.sin(a1);
  return `M60 ${60 - r}A${r} ${r} 0 ${fraction > 0.5 ? 1 : 0} 1 ${f1(x)} ${f1(y)}`;
}

/** L'anneau de durée : une part de tour égale à plies / 8. */
function Sweep({ plies }: { plies: number }) {
  const W = 10;
  const frac = Math.min(plies, 8) / 8;
  return (
    <>
      <circle cx="60" cy="60" r="54" fill="none" stroke={INK} strokeWidth={W + 3.5} />
      <circle cx="60" cy="60" r="54" fill="none" stroke="#000" strokeOpacity="0.35" strokeWidth={W} />
      <path d={arc(54, frac)} fill="none" stroke="#ffd34d" strokeWidth={W} data-plies={plies} />
    </>
  );
}

const MARK_PATHS: Record<string, [string, boolean]> = {
  check: ["M22 90l7 13H15zM22 95v4", false],
  free: ["M24 89l-6 9h4l-3 9 7-10h-4z", true],
  safe: ["M22 88l8 3v7c0 5-4 9-8 11-4-2-8-6-8-11v-7z", false],
};

/** La règle la plus marquante, en pastille crème en bas à gauche. */
function Chip({ mark }: { mark: string }) {
  const [d, filled] = MARK_PATHS[mark] ?? MARK_PATHS.check;
  return (
    <>
      <circle cx="22" cy="98" r="16" fill="#fff1c2" stroke={INK} strokeWidth="4" />
      <path d={d} fill={filled ? INK : "none"} stroke={INK} strokeWidth="2.6" strokeLinejoin="round" />
    </>
  );
}

/** La zone concernée : un halo (une pièce), une bande (une rangée) ou un damier (tout le plateau), en creux pour ne pas délaver le fond. */
function Zone({ zone }: { zone: string }) {
  if (zone === "row")
    return (
      <>
        <rect x="0" y="40" width="120" height="36" fill="#000" fillOpacity="0.38" />
        <path d="M0 40h120M0 76h120" stroke="#fff" strokeOpacity="0.9" strokeWidth="5" />
      </>
    );
  if (zone === "board") {
    const tiles: ReactNode[] = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 4; x++) if ((x + y) % 2) tiles.push(<rect key={`${x}${y}`} x={x * 30} y={y * 30} width="30" height="30" fill="#000" fillOpacity="0.4" />);
    return <>{tiles}</>;
  }
  return <circle cx="60" cy="58" r="36" fill="#fff" fillOpacity="0.28" stroke="#fff" strokeOpacity="0.8" strokeWidth="4" />;
}

const FRAME_SCALE = 0.88;

/** Les couches d'une icône forgée (à placer dans un `<svg viewBox="0 0 120 120">`). */
export function ForgedLayers({ def }: { def: ForgedDef | undefined }) {
  if (!def) return <circle cx="60" cy="60" r="50" stroke="currentColor" strokeWidth="3" opacity="0.35" />;
  const { icon, family, rarity } = def;
  return <ForgedBadge icon={icon} family={family} rarity={rarity} />;
}

/** Le médaillon d'une `IconSpec` (séparé de `ForgedDef` pour les planches de test). */
export function ForgedBadge({ icon, family, rarity }: { icon: IconSpec; family: Family; rarity: Rarity }) {
  const scene = SCENES[icon.glyph];
  const seed = icon.seed ?? 0;
  const id = String(seed);
  const enemy = icon.target === "enemy";
  const hue = GLYPH_HUE[icon.glyph] ?? 0;
  const [c0, c1] = FAMILY_BG[family].map((c) => hueShift(c, hue));
  const edge = RARITY_EDGE[rarity];
  const legendary = rarity === "legendary";
  const ctx: Ctx = { kind: icon.piece ?? "knight", enemy, seed, fill: enemy ? "#7a45c8" : "#f4f7ff", line: enemy ? "#f3e4ff" : "#9fb0d8" };
  return (
    <>
      <g style={legendary ? { filter: `drop-shadow(0 0 3px ${edge})` } : undefined}>
        {frameShape(rarity, { fill: "#1a1e36", stroke: INK, strokeWidth: 9, strokeLinejoin: "round" })}
        {frameShape(rarity, { fill: "none", stroke: edge, strokeWidth: 5, strokeLinejoin: "round" })}
      </g>
      <g transform={`translate(60 60) scale(${FRAME_SCALE}) translate(-60 -60)`}>
        <defs>
          <radialGradient id={`ibg-${family}-${hue}`} cx="50%" cy="38%" r="70%">
            <stop offset="0" stopColor={c0} />
            <stop offset="1" stopColor={c1} />
          </radialGradient>
          <clipPath id={`imc${id}`}>
            <circle cx="60" cy="60" r="50" />
          </clipPath>
        </defs>
        <circle cx="60" cy="60" r="50" fill={`url(#ibg-${family}-${hue})`} />
        <g clipPath={`url(#imc${id})`}>
          <Zone zone={icon.zone ?? "one"} />
          <Sigil glyph={icon.glyph} seed={seed} color={hueShift(FAMILY_ACCENT[family], hue)} id={id} />
          {scene ? scene(ctx) : <path d="M60 24a36 36 0 100 72 36 36 0 000-72zM52 52c0-12 16-12 16 0 0 8-8 8-8 18M60 82v2" />}
        </g>
        {icon.plies !== undefined && <Sweep plies={icon.plies} />}
        {icon.mark && <Chip mark={icon.mark} />}
      </g>
    </>
  );
}
