import { useId } from "react";

/** Pièces de l'écran Jouer : une par mode (roi = classée, pion = amicale, tour = salle privée, cavalier = contre l'IA). */
export type HeroKind = "king" | "pawn" | "rook" | "knight";

type Pt = [number, number];

/** Silhouette symétrique : demi-profil (écart au centre, hauteur) tourné autour de l'axe x = 100. */
function lathe(profile: Pt[]): string {
  const right = profile.map(([dx, y]) => `${100 + dx},${y}`);
  const left = [...profile].reverse().map(([dx, y]) => `${100 - dx},${y}`);
  return `M${right.join("L")}L${left.join("L")}Z`;
}

const BASE: Pt[] = [[50, 200], [70, 226], [80, 230], [80, 244], [88, 250], [88, 270]];

const KING = [
  "M90,0h20v14h16v18h-16v18h-20v-18h-16v-18h16z",
  lathe([[18, 50], [40, 60], [46, 72], [38, 98], [34, 112], [50, 120], [50, 132], [32, 138], [30, 154], ...BASE]),
];
const PAWN = [
  "M100,34a38,38 0 1 1 -0.01,0z",
  lathe([[22, 104], [42, 112], [42, 124], [26, 130], [28, 152], ...BASE]),
];
const ROOK = [
  "M36,30h26v20h20v-20h36v20h20v-20h26v62l-16,14h-96l-16,-14z",
  lathe([[48, 106], [42, 184], ...BASE]),
];
// Cavalier « chess-knight » de Skoll (game-icons.net, CC BY 3.0), le même que la mascotte.
const KNIGHT_512 =
  "M60.81 476.91h300v-60h-300v60zm233.79-347.3l13.94 7.39c31.88-43.62 61.34-31.85 61.34-31.85l-21.62 53 35.64 19 2.87 33 64.42 108.75-43.55 29.37s-26.82-36.39-39.65-43.66c-10.66-6-41.22-10.25-56.17-12l-67.54-76.91-12 10.56 37.15 42.31c-.13.18-.25.37-.38.57-35.78 58.17 23 105.69 68.49 131.78H84.14C93 85 294.6 129.61 294.6 129.61z";

function Shapes({ kind }: { kind: HeroKind }) {
  if (kind === "knight") {
    // Le tracé d'origine (512) est recadré sur la boîte 200 × 270 des autres pièces.
    return <path d={KNIGHT_512} transform="translate(-28 22) scale(0.52)" />;
  }
  const paths = kind === "king" ? KING : kind === "pawn" ? PAWN : ROOK;
  return (
    <>
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </>
  );
}

/** Grande pièce éclairée au centre de l'écran Jouer : dégradé d'accent tourné comme un objet poli. */
export function HeroPiece({ kind, className }: { kind: HeroKind; className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg className={className} viewBox="0 0 200 272" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id={`${id}-body`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" style={{ stopColor: "var(--accent-deep)" }} />
          <stop offset=".34" style={{ stopColor: "var(--accent)" }} />
          <stop offset=".5" style={{ stopColor: "color-mix(in srgb, var(--accent) 45%, #fff)" }} />
          <stop offset=".64" style={{ stopColor: "var(--accent)" }} />
          <stop offset="1" style={{ stopColor: "var(--accent-deep)" }} />
        </linearGradient>
        <linearGradient id={`${id}-shade`} x1="0" y1="0" x2="0" y2="1">
          <stop offset=".55" stopColor="#000" stopOpacity="0" />
          <stop offset="1" stopColor="#000" stopOpacity=".28" />
        </linearGradient>
      </defs>
      <g fill={`url(#${id}-body)`}>
        <Shapes kind={kind} />
      </g>
      <g fill={`url(#${id}-shade)`}>
        <Shapes kind={kind} />
      </g>
    </svg>
  );
}

/** Petite icône de mode (onglets de l'écran Jouer). */
export function ModeIcon({ kind }: { kind: HeroKind }) {
  return (
    <svg className="pl-mode-ico" viewBox="0 0 200 272" width="13" height="18" aria-hidden="true" focusable="false" fill="currentColor">
      <Shapes kind={kind} />
    </svg>
  );
}
