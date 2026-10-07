// Textures du plateau et des pièces, générées sur canvas : aucune image externe.
import type { Color, PieceKind } from "../protocol";
import { luminance, mixHex } from "../theme";

export const TILE = 80;
export const FRAME = 24;
export const BOARD_PX = TILE * 8;
export const SIZE = BOARD_PX + FRAME * 2;

export interface BoardColors {
  light: string;
  dark: string;
  /** Plateau moderne : cases plates, sans grain de pierre. */
  flat?: boolean;
}

export const DEFAULT_BOARD: BoardColors = { light: "#cdd1d9", dark: "#69727f" };

/** Générateur pseudo-aléatoire déterministe : la pierre est identique à chaque partie. */
function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function shade(hex: string, amount: number): string {
  const n = parseInt(hex.slice(1), 16);
  const ch = (v: number) => Math.max(0, Math.min(255, Math.round(v + amount * 255)));
  return `rgb(${ch(n >> 16)},${ch((n >> 8) & 255)},${ch(n & 255)})`;
}

/** Cadre usiné + 64 cases de pierre + coordonnées dans les cases. */
export function drawBoard(canvas: HTMLCanvasElement, orientation: Color, colors: BoardColors = DEFAULT_BOARD) {
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d")!;
  if (colors.flat) return drawFlatBoard(ctx, orientation, colors);
  const rnd = mulberry32(1337);

  // Cadre : dégradé sombre, filet clair, chanfrein intérieur.
  const frame = ctx.createLinearGradient(0, 0, SIZE, SIZE);
  frame.addColorStop(0, "#23272e");
  frame.addColorStop(0.5, "#181b20");
  frame.addColorStop(1, "#1f2329");
  ctx.fillStyle = frame;
  ctx.fillRect(0, 0, SIZE, SIZE);
  ctx.strokeStyle = "#3b414b";
  ctx.lineWidth = 2;
  ctx.strokeRect(1, 1, SIZE - 2, SIZE - 2);
  ctx.strokeStyle = "rgba(255,255,255,0.05)";
  ctx.lineWidth = 1;
  ctx.strokeRect(6.5, 6.5, SIZE - 13, SIZE - 13);
  ctx.strokeStyle = "rgba(0,0,0,0.5)";
  ctx.strokeRect(FRAME - 3.5, FRAME - 3.5, BOARD_PX + 7, BOARD_PX + 7);

  ctx.font = '600 12px "Geist Mono Variable", "Geist Mono", ui-monospace, monospace';
  ctx.textBaseline = "top";

  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const x = FRAME + col * TILE;
      const y = FRAME + row * TILE;
      const file = orientation === "white" ? col : 7 - col;
      const rank = orientation === "white" ? 7 - row : row;
      const light = (file + rank) % 2 === 1;
      const base = light ? colors.light : colors.dark;

      const g = ctx.createLinearGradient(x, y, x + TILE, y + TILE);
      g.addColorStop(0, shade(base, 0.05));
      g.addColorStop(1, shade(base, -0.05));
      ctx.fillStyle = g;
      ctx.fillRect(x, y, TILE, TILE);

      // Grain de pierre : poussière claire et sombre.
      for (let i = 0; i < 260; i++) {
        const px = x + rnd() * TILE;
        const py = y + rnd() * TILE;
        const size = rnd() < 0.15 ? 2 : 1;
        ctx.fillStyle = rnd() < 0.5 ? "rgba(255,255,255,0.07)" : "rgba(0,0,0,0.09)";
        ctx.fillRect(px, py, size, size);
      }
      // Quelques veines fines.
      ctx.lineWidth = 1;
      for (let i = 0; i < 2; i++) {
        ctx.strokeStyle = rnd() < 0.5 ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.07)";
        ctx.beginPath();
        const sx = x + rnd() * TILE;
        const sy = y + rnd() * TILE;
        ctx.moveTo(sx, sy);
        ctx.bezierCurveTo(sx + rnd() * 30 - 15, sy + rnd() * 30, sx + rnd() * 40 - 20, sy + rnd() * 40, x + rnd() * TILE, y + rnd() * TILE);
        ctx.stroke();
      }
      // Chanfrein : lumière en haut à gauche, ombre en bas à droite.
      ctx.fillStyle = "rgba(255,255,255,0.10)";
      ctx.fillRect(x, y, TILE, 1);
      ctx.fillRect(x, y, 1, TILE);
      ctx.fillStyle = "rgba(0,0,0,0.22)";
      ctx.fillRect(x, y + TILE - 1, TILE, 1);
      ctx.fillRect(x + TILE - 1, y, 1, TILE);

      // Coordonnées dans les cases : rang sur la colonne de gauche, colonne sur la rangée du bas.
      ctx.fillStyle = luminance(base) > 0.35 ? "rgba(30,34,41,0.72)" : "rgba(246,247,250,0.8)";
      if (col === 0) {
        ctx.textAlign = "left";
        ctx.fillText(String(rank + 1), x + 5, y + 4);
      }
      if (row === 7) {
        ctx.textAlign = "right";
        ctx.fillText("abcdefgh"[file], x + TILE - 5, y + TILE - 17);
      }
    }
  }
  // Ombre intérieure du cadre sur les cases.
  const inner = ctx.createLinearGradient(0, FRAME, 0, FRAME + 10);
  inner.addColorStop(0, "rgba(0,0,0,0.35)");
  inner.addColorStop(1, "rgba(0,0,0,0)");
  ctx.fillStyle = inner;
  ctx.fillRect(FRAME, FRAME, BOARD_PX, 10);
}

/** Plateau moderne : cases plates, cadre sombre à filet cyan, repères dans les cases (sombres sur clair, clairs sur sombre). */
function cssVar(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

function drawFlatBoard(ctx: CanvasRenderingContext2D, orientation: Color, colors: BoardColors) {
  // Cadre Jade : la surface du thème courant, filet neutre (pas de lueur colorée).
  ctx.fillStyle = cssVar("--surface-2", "#1a2021");
  ctx.fillRect(0, 0, SIZE, SIZE);
  ctx.strokeStyle = cssVar("--line-3", "#43504f");
  ctx.lineWidth = 2;
  ctx.strokeRect(FRAME - 5, FRAME - 5, BOARD_PX + 10, BOARD_PX + 10);
  ctx.font = '700 17px "Barlow Condensed", system-ui, sans-serif';
  ctx.textBaseline = "top";
  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const x = FRAME + col * TILE;
      const y = FRAME + row * TILE;
      const file = orientation === "white" ? col : 7 - col;
      const rank = orientation === "white" ? 7 - row : row;
      const light = (file + rank) % 2 === 1;
      ctx.fillStyle = light ? colors.light : colors.dark;
      ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = light ? "#5a6f99" : "#eaf0fb";
      if (col === 0) {
        ctx.textAlign = "left";
        ctx.fillText(String(rank + 1), x + 6, y + 5);
      }
      if (row === 7) {
        ctx.textAlign = "right";
        ctx.fillText("abcdefgh"[file], x + TILE - 6, y + TILE - 20);
      }
    }
  }
}

// ---- pièces -------------------------------------------------------------------

const BASE = "M24 90H76Q80 90 80 86V81Q80 78 77 78H23Q20 78 20 81V86Q20 90 24 90Z";
const circle = (cx: number, cy: number, r: number) => `M${cx - r} ${cy}a${r} ${r} 0 1 1 ${2 * r} 0a${r} ${r} 0 1 1 ${-2 * r} 0Z`;

interface Shape {
  parts: string[];
  details: string[];
}

const SHAPES: Record<PieceKind, Shape> = {
  pawn: {
    parts: [BASE, "M37 78C39 68 43 62 44 54H56C57 62 61 68 63 78Z", "M35 56H65Q68 56 68 52Q68 47 64 47H36Q32 47 32 52Q32 56 35 56Z", circle(50, 33, 13)],
    details: [],
  },
  rook: {
    parts: [BASE, "M33 78L37 46H63L67 78Z", "M30 48V20H40V28H45V20H55V28H60V20H70V48Z"],
    details: ["M30 48H70", "M36 55H64"],
  },
  bishop: {
    parts: [
      BASE,
      "M38 78C40 70 44 66 44 58H56C56 66 60 70 62 78Z",
      "M36 62H64Q66 62 66 58Q66 54 63 54H37Q34 54 34 58Q34 62 36 62Z",
      "M50 14C62 24 69 38 62 50C60 53 58 54 56 54H44C42 54 40 53 38 50C31 38 38 24 50 14Z",
      circle(50, 11, 4),
    ],
    details: ["M50 26V38M44 32H56"],
  },
  knight: {
    parts: [
      BASE,
      "M31 78C31 64 39 58 44 48C38 50 33 54 27 59L21 52C25 42 31 34 39 26L37 15L46 21C48 19 52 17 56 17C70 20 79 34 79 54C79 66 74 72 74 78Z",
    ],
    details: ["M53 24C61 32 67 44 64 62", "M29 45L33 46"],
  },
  queen: {
    parts: [
      BASE,
      "M32 78L35 52L22 28L39 42L50 20L61 42L78 28L65 52L68 78Z",
      "M33 58H67Q70 58 70 54Q70 50 67 50H33Q30 50 30 54Q30 58 33 58Z",
      circle(22, 25, 4.5),
      circle(50, 16, 4.5),
      circle(78, 25, 4.5),
    ],
    details: [],
  },
  king: {
    parts: [
      BASE,
      "M36 78C36 68 38 62 34 52C31 46 38 40 50 40C62 40 69 46 66 52C62 62 64 68 64 78Z",
      "M35 46H65Q68 46 68 42Q68 38 64 38H36Q32 38 32 42Q32 46 35 46Z",
      "M47 5H53V14H62V20H53V32H47V20H38V14H47Z",
    ],
    details: [],
  },
};

/** Les pièces sont dessinées en 256 px ; le dessin vectoriel est mis à l'échelle. */
export const PIECE_TEX = 256;

/** Clé de texture : l'ensemble classique garde les clés d'origine. */
export function pieceKey(color: Color, kind: PieceKind, set = "classic"): string {
  return set === "classic" ? `piece-${color}-${kind}` : `piece-${set}-${color}-${kind}`;
}

interface PiecePalette {
  shadow: string;
  light: string;
  mid: string;
  dark: string;
  outline: string;
  glint: string;
  detail: string;
  eye: string;
}

/** Couleurs d'une pièce : ivoire/ébène d'origine, ou dégradé calculé depuis une teinte (jeux néon, or, braise). */
export function piecePalette(white: boolean, tint: string | null): PiecePalette {
  if (tint) {
    const bright = luminance(tint) > 0.4;
    return {
      shadow: mixHex(tint, "#000000", 0.45),
      light: mixHex(tint, "#ffffff", 0.4),
      mid: tint,
      dark: mixHex(tint, "#000000", 0.42),
      outline: mixHex(tint, "#000000", 0.82),
      glint: "rgba(255,255,255,0.65)",
      detail: bright ? "rgba(20,22,28,0.6)" : "rgba(255,255,255,0.4)",
      eye: bright ? "#1b1d23" : "#f5f6f8",
    };
  }
  return white
    ? { shadow: "#c8c4ba", light: "#fffdf8", mid: "#e6e2d8", dark: "#a7a399", outline: "#2b2d34", glint: "rgba(255,255,255,0.8)", detail: "rgba(43,45,52,0.7)", eye: "#2b2d34" }
    : { shadow: "#202127", light: "#666a77", mid: "#383b44", dark: "#15161a", outline: "#050506", glint: "rgba(255,255,255,0.22)", detail: "rgba(255,255,255,0.28)", eye: "#e9ebef" };
}

/** Pièce avec volume (dégradé latéral, liseré) et ombre portée cuite dans la texture. */
export function drawPiece(canvas: HTMLCanvasElement, kind: PieceKind, color: Color, tint: string | null = null) {
  canvas.width = PIECE_TEX;
  canvas.height = PIECE_TEX;
  const ctx = canvas.getContext("2d")!;
  const white = color === "white";
  const pal = piecePalette(white, tint);
  const s = 1.18 * (PIECE_TEX / 128);
  ctx.setTransform(s, 0, 0, s, (PIECE_TEX - 100 * s) / 2, 2 * (PIECE_TEX / 128));
  const shape = SHAPES[kind];
  const paths = shape.parts.map((d) => new Path2D(d));

  // Passe 1 : ombre portée de l'ensemble.
  ctx.shadowColor = "rgba(0,0,0,0.55)";
  ctx.shadowBlur = 7 * (PIECE_TEX / 128);
  ctx.shadowOffsetY = 4 * (PIECE_TEX / 128);
  ctx.fillStyle = pal.shadow;
  for (const p of paths) ctx.fill(p);
  ctx.shadowColor = "transparent";

  // Passe 2 : volume.
  const grad = ctx.createLinearGradient(22, 0, 80, 0);
  grad.addColorStop(0, pal.light);
  grad.addColorStop(0.45, pal.mid);
  grad.addColorStop(1, pal.dark);
  ctx.lineJoin = "round";
  for (const p of paths) {
    ctx.fillStyle = grad;
    ctx.fill(p);
    ctx.lineWidth = 2;
    ctx.strokeStyle = pal.outline;
    ctx.stroke(p);
  }
  // Reflet fin sur le côté éclairé.
  ctx.save();
  ctx.lineWidth = 1.2;
  ctx.strokeStyle = pal.glint;
  ctx.translate(1.5, 1.5);
  ctx.globalCompositeOperation = "source-atop";
  for (const p of paths.slice(1)) ctx.stroke(p);
  ctx.restore();

  ctx.lineWidth = 1.6;
  ctx.strokeStyle = pal.detail;
  for (const d of shape.details) ctx.stroke(new Path2D(d));
  if (kind === "knight") {
    ctx.fillStyle = pal.eye;
    ctx.fill(new Path2D(circle(56, 31, 2.4)));
  }
}

/** Pièce de pierre (pions-murs de Wall) : la silhouette de la pièce, recouverte de roche fissurée. */
export function stoneKey(color: Color, kind: PieceKind): string {
  return `piece-stone-${color}-${kind}`;
}

export function drawStonePiece(canvas: HTMLCanvasElement, kind: PieceKind, color: Color) {
  drawPiece(canvas, kind, color);
  const ctx = canvas.getContext("2d")!;
  const rnd = mulberry32(color === "white" ? 7 : 11);
  ctx.save();
  const k = PIECE_TEX / 128;
  ctx.setTransform(k, 0, 0, k, 0, 0);
  ctx.globalCompositeOperation = "source-atop";
  const g = ctx.createLinearGradient(20, 0, 110, 0);
  g.addColorStop(0, color === "white" ? "rgba(190,196,206,0.9)" : "rgba(120,126,138,0.9)");
  g.addColorStop(1, color === "white" ? "rgba(108,114,126,0.92)" : "rgba(52,56,66,0.92)");
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 128, 128);
  for (let i = 0; i < 420; i++) {
    ctx.fillStyle = rnd() < 0.5 ? "rgba(255,255,255,0.16)" : "rgba(0,0,0,0.22)";
    ctx.fillRect(rnd() * 128, rnd() * 128, rnd() < 0.2 ? 3 : 1.5, rnd() < 0.2 ? 3 : 1.5);
  }
  ctx.strokeStyle = "rgba(10,12,16,0.7)";
  ctx.lineWidth = 1.6;
  for (let i = 0; i < 4; i++) {
    let x = 36 + rnd() * 56;
    let y = 26 + rnd() * 60;
    ctx.beginPath();
    ctx.moveTo(x, y);
    for (let s = 0; s < 4; s++) {
      x += (rnd() - 0.5) * 16;
      y += 5 + rnd() * 8;
      ctx.lineTo(x, y);
    }
    ctx.stroke();
  }
  ctx.restore();
}

/** Pilier de roc du terrain de Geomancy : un bloc taillé à facettes qui déborde au-dessus de la case. */
export const ROCK_KEY = "terrain-rock";
export const ROCK_W = TILE;
export const ROCK_H = TILE + 30;

export function drawRock(canvas: HTMLCanvasElement) {
  canvas.width = ROCK_W;
  canvas.height = ROCK_H;
  const ctx = canvas.getContext("2d")!;
  const rnd = mulberry32(2024);
  const top = 6;
  const base = ROCK_H - 4;
  const pts: [number, number][] = [
    [6, base], [4, base - 28], [12, base - 52], [10, top + 34], [24, top + 10], [34, top], [48, top + 6], [58, top + 4],
    [68, top + 20], [72, top + 44], [74, base - 22], [72, base],
  ];
  // Ombre au sol.
  ctx.fillStyle = "rgba(0,0,0,0.35)";
  ctx.beginPath();
  ctx.ellipse(ROCK_W / 2, base + 1, 34, 6, 0, 0, Math.PI * 2);
  ctx.fill();
  // Corps du pilier.
  const body = ctx.createLinearGradient(8, 0, 74, 0);
  body.addColorStop(0, "#9aa1ac");
  body.addColorStop(0.5, "#6e7580");
  body.addColorStop(1, "#3f444e");
  ctx.fillStyle = body;
  ctx.beginPath();
  pts.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
  ctx.closePath();
  ctx.fill();
  ctx.save();
  ctx.clip();
  // Facettes : plans clairs et sombres.
  for (let i = 0; i < 9; i++) {
    const x = 6 + rnd() * 66;
    const y = top + rnd() * (base - top);
    ctx.fillStyle = rnd() < 0.5 ? "rgba(255,255,255,0.10)" : "rgba(0,0,0,0.16)";
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + 14 + rnd() * 18, y + (rnd() - 0.5) * 20);
    ctx.lineTo(x + (rnd() - 0.5) * 16, y + 22 + rnd() * 24);
    ctx.closePath();
    ctx.fill();
  }
  for (let i = 0; i < 320; i++) {
    ctx.fillStyle = rnd() < 0.5 ? "rgba(255,255,255,0.08)" : "rgba(0,0,0,0.12)";
    ctx.fillRect(rnd() * ROCK_W, rnd() * ROCK_H, 1.5, 1.5);
  }
  ctx.restore();
  ctx.lineJoin = "round";
  ctx.lineWidth = 2;
  ctx.strokeStyle = "#1b1e23";
  ctx.beginPath();
  pts.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
  ctx.closePath();
  ctx.stroke();
  ctx.strokeStyle = "rgba(255,255,255,0.22)";
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  ctx.moveTo(24, top + 12);
  ctx.lineTo(34, top + 2);
  ctx.lineTo(48, top + 8);
  ctx.stroke();
}
