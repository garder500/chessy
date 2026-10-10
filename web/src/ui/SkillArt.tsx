import { memo, useSyncExternalStore } from "react";
import { skillEntry, type Family } from "../catalog";
import { forgedDef, forgedVersion, isForgedId, onForgedChange } from "../forged";
import { ForgedLayers } from "./forgedIcon";

/**
 * Sprite SVG des 27 compétences (`sk-<id>`), dessinées au trait dans un viewBox 120x120.
 * Les traits héritent de `currentColor` : la couleur de famille vient de `color: var(--fam-…)`.
 * À monter une fois dans `App` ; `<SkillArt id="…" />` référence ensuite le symbole.
 */
export function SkillSprite() {
  return (
    <svg
      width="0"
      height="0"
      style={{ position: "absolute", overflow: "hidden" }}
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <defs>
        {/* Pièces simplifiées, boîte 60x60, réutilisables via <use>. */}
        <g id="p-pawn">
          <circle cx="30" cy="15" r="9.5" />
          <path d="M19 52c1-10 5-17 11-21 6 4 10 11 11 21z" />
          <path d="M14 54h32" />
        </g>
        <g id="p-knight">
          <path d="M14 54h32" />
          <path d="M18 54c0-14 4-18 10-24-5 1-8 3-12 7l-6-4c3-8 9-18 22-20 10 2 18 12 18 28v13" />
          <circle cx="30" cy="20" r="1.6" fill="currentColor" stroke="none" />
        </g>
        <g id="p-rook">
          <path d="M15 54h30M19 54V30h22v24M14 30h32M14 30V12h7v6h6v-6h6v6h6v-6h7v18" />
        </g>
        <g id="p-bishop">
          <path d="M30 10c9 7 14 15 12 24-1 5-4 8-6 10H24c-2-2-5-5-6-10-2-9 3-17 12-24z" />
          <path d="M30 22v10M25 27h10M20 54h20M22 44l-2 10M38 44l2 10" />
        </g>
        <g id="p-king">
          <path d="M30 6v14M23 12h14" />
          <path d="M30 22c-10 0-16 7-14 15 1 5 5 8 7 11l-4 6h22l-4-6c2-3 6-6 7-11 2-8-4-15-14-15z" />
          <path d="M16 54h28" />
        </g>
        <g id="p-queen">
          <path d="M12 20l8 22 10-28 10 28 8-22-4 34H16z" />
          <circle cx="12" cy="18" r="2" />
          <circle cx="30" cy="12" r="2" />
          <circle cx="48" cy="18" r="2" />
        </g>
      </defs>

      <symbol id="sk-teleportation" viewBox="0 0 120 120">
        <ellipse cx="34" cy="100" rx="22" ry="6" />
        <ellipse cx="86" cy="62" rx="22" ry="6" />
        <use href="#p-pawn" transform="translate(10 46) scale(.8)" strokeDasharray="2 6" />
        <use href="#p-pawn" transform="translate(62 8) scale(.8)" />
        <path d="M32 34q16-22 38-20" />
        <path d="M64 6l8 8-9 6" />
      </symbol>

      <symbol id="sk-imune" viewBox="0 0 120 120">
        <path d="M60 10l38 14v32c0 24-16 40-38 52C38 96 22 80 22 56V24z" />
        <use href="#p-pawn" transform="translate(30 26)" />
      </symbol>

      <symbol id="sk-freeze" viewBox="0 0 120 120">
        {[0, 60, 120].map((a) => (
          <g key={a} transform={`rotate(${a} 60 60)`}>
            <path d="M60 12v96M50 24l10 10 10-10M50 96l10-10 10 10" />
          </g>
        ))}
      </symbol>

      <symbol id="sk-rollback" viewBox="0 0 120 120">
        <path d="M94 36A40 40 0 1 0 100 66" />
        <path d="M96 16v22H74" />
        <use href="#p-pawn" transform="translate(34 32) scale(.86)" />
      </symbol>

      <symbol id="sk-clone" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(4 28) scale(.95)" />
        <use href="#p-pawn" transform="translate(58 28) scale(.95)" strokeDasharray="2 6" />
        <path d="M60 14v18M51 23h18" />
        <path d="M14 100h92" />
      </symbol>

      <symbol id="sk-destiny_swapper" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(0 6) scale(.8)" />
        <use href="#p-knight" transform="translate(70 6) scale(.8)" />
        <path d="M24 56C24 86 96 70 96 100" />
        <path d="M96 56C96 86 24 70 24 100" />
        <circle cx="24" cy="102" r="3" />
        <circle cx="96" cy="102" r="3" />
      </symbol>

      <symbol id="sk-remover" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(30 30)" strokeDasharray="1 6" />
        <circle cx="60" cy="60" r="46" />
        <path d="M27 93L93 27" />
      </symbol>

      <symbol id="sk-wall" viewBox="0 0 120 120">
        <path d="M14 54h92v52H14z" />
        <path d="M14 71h92M14 88h92M46 54v17M78 54v17M30 71v17M62 71v17M94 71v17M46 88v18M78 88v18" />
        <use href="#p-pawn" transform="translate(36 2) scale(.75)" />
      </symbol>

      <symbol id="sk-mirage" viewBox="0 0 120 120">
        <use href="#p-knight" transform="translate(20 8) scale(1.1)" strokeDasharray="2 6" />
        <path d="M10 94q12-8 25 0t25 0 25 0 25 0M22 106q10-6 19 0t19 0 19 0" />
        <path d="M96 22v10M91 27h10" />
      </symbol>

      <symbol id="sk-evolve" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(6 56) scale(.8)" />
        <use href="#p-queen" transform="translate(52 6) scale(1.05)" />
        {/* Flèche droite à 45 degrés, tête symétrique. */}
        <path d="M32 54L56 30" />
        <path d="M54.1 40.8L56 30l-10.8 1.9" />
        <path d="M54 106h56" />
      </symbol>

      <symbol id="sk-switch" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(2 12) scale(.8)" fill="currentColor" />
        <use href="#p-pawn" transform="translate(72 12) scale(.8)" />
        <path d="M22 86h76M34 74L22 86l12 12M86 74l12 12-12 12" />
      </symbol>

      <symbol id="sk-mind" viewBox="0 0 120 120">
        <path d="M8 60Q60 8 112 60 60 112 8 60z" />
        <circle cx="60" cy="60" r="17" />
        <circle cx="60" cy="60" r="6" fill="currentColor" />
        <path d="M60 4v10M24 18l6 8M96 18l-6 8" />
      </symbol>

      <symbol id="sk-control" viewBox="0 0 120 120">
        {/* Main + barre de manipulation, fils tendus vers un pion-marionnette aux bras levés. */}
        <path d="M60 3v7" />
        <rect x="22" y="10" width="76" height="9" rx="4.5" />
        <path d="M30 19L34 65M90 19L86 65M60 19v38" strokeWidth="2" />
        <use href="#p-pawn" transform="translate(31.5 52) scale(.95)" />
        <path d="M52 86L37 70M68 86l15-16" />
        <circle cx="34" cy="66" r="3" fill="currentColor" />
        <circle cx="86" cy="66" r="3" fill="currentColor" />
        <path d="M40 112h40" strokeDasharray="2 7" />
      </symbol>

      <symbol id="sk-morph" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(0 28) scale(.85)" />
        <use href="#p-knight" transform="translate(68 28) scale(.85)" />
        <path d="M50 58l8 8-8 8M60 58l8 8-8 8" />
        <path d="M10 100h100" />
      </symbol>

      <symbol id="sk-canceller" viewBox="0 0 120 120">
        <circle cx="60" cy="60" r="46" />
        <path d="M60 30l7 20 20 7-20 7-7 20-7-20-20-7 20-7z" />
        <path d="M27 93L93 27" />
      </symbol>

      <symbol id="sk-tornado" viewBox="0 0 120 120">
        <path d="M12 22Q60 6 108 22" />
        <path d="M22 44Q60 30 98 44" />
        <path d="M34 66Q60 54 86 66" />
        <path d="M46 86Q60 76 74 86" />
        <path d="M56 104h8" />
        <path d="M12 22q-2 8 10 22M108 22q2 8-10 22M22 44q0 8 12 22M98 44q0 8-12 22M34 66q2 8 12 20M86 66q-2 8-12 20" />
      </symbol>

      <symbol id="sk-invisibility" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(18 14) scale(1.4)" strokeDasharray="1 7" />
        <path d="M14 108h92" />
        <path d="M96 20v8M92 24h8M20 40v6M17 43h6" />
      </symbol>

      <symbol id="sk-terminator" viewBox="0 0 120 120">
        <circle cx="60" cy="60" r="38" />
        <path d="M60 8v22M60 90v22M8 60h22M90 60h22" />
        <use href="#p-pawn" transform="translate(37 38) scale(.76)" />
      </symbol>

      <symbol id="sk-trap" viewBox="0 0 120 120">
        {/* Piège à mâchoires (ours) : deux mâchoires dentées se referment sur un pion, plaque de détente au sol. */}
        <g transform="rotate(-9 20 98)">
          <path d="M20 98 A40 60 0 0 1 60 38 L60.0 48.0 L56.7 61.5 L50.1 50.4 L50.5 65.0 L41.2 57.5 L45.2 71.8 L34.1 68.6 L41.3 81.2 L29.6 82.5 L39.3 92.2 L28.0 98.0 z" />
        </g>
        <g transform="translate(120 0) scale(-1 1)">
          <g transform="rotate(-9 20 98)">
            <path d="M20 98 A40 60 0 0 1 60 38 L60.0 48.0 L56.7 61.5 L50.1 50.4 L50.5 65.0 L41.2 57.5 L45.2 71.8 L34.1 68.6 L41.3 81.2 L29.6 82.5 L39.3 92.2 L28.0 98.0 z" />
          </g>
        </g>
        <use href="#p-pawn" transform="translate(45 68) scale(.5)" />
        <rect x="8" y="98" width="104" height="10" rx="3" />
        <path d="M112 103h5" />
      </symbol>

      <symbol id="sk-bench" viewBox="0 0 120 120">
        <use href="#p-pawn" transform="translate(38 14) scale(.8)" />
        <path d="M10 66h100M20 66v34M100 66v34M10 78h100" />
      </symbol>

      <symbol id="sk-forcefield" viewBox="0 0 120 120">
        <circle cx="60" cy="60" r="36" />
        <use href="#p-pawn" transform="translate(36 36) scale(.8)" />
        <path d="M94 26l12-12M106 24V14h-10M26 26L14 14M14 24V14h10M94 94l12 12M106 96v10H96M26 94l-12 12M14 96v10h10" />
      </symbol>

      <symbol id="sk-transposition" viewBox="0 0 120 120">
        <rect x="10" y="44" width="36" height="36" rx="5" />
        <rect x="74" y="44" width="36" height="36" rx="5" />
        <use href="#p-pawn" transform="translate(17 51) scale(.4)" />
        <use href="#p-rook" transform="translate(81 51) scale(.4)" />
        <path d="M22 36Q60 6 98 36M90 28l10 8-12 4" />
        <path d="M98 88Q60 118 22 88M30 96l-10-8 12-4" />
      </symbol>

      <symbol id="sk-queensac" viewBox="0 0 120 120">
        {/* La dame est offerte sur l'autel : une épée plantée la transperce. */}
        <use href="#p-queen" transform="translate(27 34) scale(1.1)" />
        <path d="M18 96h84M24 96v14h72V96" />
        <path d="M56 22l4 52 4-52z" fill="currentColor" />
        <path d="M46 18h28M60 18V8" />
        <circle cx="60" cy="5" r="2.6" />
      </symbol>

      <symbol id="sk-temporal" viewBox="0 0 120 120">
        <path d="M32 12h56M32 108h56" />
        <path d="M38 12c0 30 22 36 22 48s-22 18-22 48M82 12c0 30-22 36-22 48s22 18 22 48" />
        <path d="M48 98q12-8 24 0M54 40h12M60 60v10" />
      </symbol>

      <symbol id="sk-geomancy" viewBox="0 0 120 120">
        <path d="M8 98L44 32l20 36 14-20 34 50z" />
        <path d="M34 50l10 6 10-8M70 62l8 6 8-8" />
        <path d="M8 98h104M20 110h80" />
      </symbol>

      <symbol id="sk-celestial" viewBox="0 0 120 120">
        <ellipse cx="60" cy="14" rx="18" ry="6" />
        <use href="#p-pawn" transform="translate(30 30) scale(.85)" />
        <path d="M14 20l12 14M106 20L94 34M60 24v-2" />
        <path d="M18 110h84" />
      </symbol>

      <symbol id="sk-godhelp" viewBox="0 0 120 120">
        <rect x="22" y="30" width="70" height="70" rx="12" />
        <circle cx="42" cy="50" r="4" fill="currentColor" />
        <circle cx="72" cy="50" r="4" fill="currentColor" />
        <circle cx="57" cy="65" r="4" fill="currentColor" />
        <circle cx="42" cy="80" r="4" fill="currentColor" />
        <circle cx="72" cy="80" r="4" fill="currentColor" />
        <path d="M100 8v18M91 17h18M10 14v10M5 19h10" />
      </symbol>
    </svg>
  );
}

interface Props {
  id: string;
  /** Taille en px (carré). Par défaut, remplit le conteneur. */
  size?: number;
  className?: string;
  /** Force une famille ; sinon celle du catalogue. */
  family?: Family;
}

/** Illustration d'une compétence ; la couleur suit la famille. */
export const SkillArt = memo(function SkillArt({ id, size, className, family }: Props) {
  // Une compétence forgée est décrite par le serveur : l'icône se redessine quand sa définition arrive.
  useSyncExternalStore(onForgedChange, forgedVersion, forgedVersion);
  const fam = family ?? skillEntry(id).family;
  return (
    <svg
      className={["skill-art", className].filter(Boolean).join(" ")}
      viewBox="0 0 120 120"
      width={size}
      height={size}
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ color: `var(--fam-${fam})` }}
    >
      {isForgedId(id) ? <ForgedLayers def={forgedDef(id)} /> : <use href={`#sk-${id}`} />}
    </svg>
  );
});

/** Petite pièce d'échecs au trait (pion, cavalier, fou, tour, dame, roi). */
export function PieceIcon({ kind, size = 16, className }: { kind: string; size?: number; className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 60 60"
      width={size}
      height={size}
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="4"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <use href={`#p-${kind}`} />
    </svg>
  );
}
