// Géométrie et réglages tactiles du plateau, sans Phaser ni DOM : testables seuls.
// Le plateau est dessiné dans un canvas de SIZE × SIZE (cadre + 8 × 8 cases) que Phaser met à l'échelle du conteneur.

import type { Color, Square } from "../protocol";
import { DRAG_THRESHOLD } from "../interaction";
import { FRAME, SIZE, TILE } from "./textures";

/** Seuil de glisser pour un doigt : un appui tremble de plusieurs pixels, 4 px (souris) transformeraient chaque tap en glisser. */
export const TOUCH_DRAG_THRESHOLD = 10;

/** Taille minimale confortable d'une case au doigt, en pixels CSS (recommandation Apple / Material : 44 px, 40 px au pire). */
export const MIN_TOUCH_TILE = 40;

/** Seuil de glisser (en pixels du canvas) selon le type de pointeur. */
export function dragThreshold(touch: boolean): number {
  return touch ? TOUCH_DRAG_THRESHOLD : DRAG_THRESHOLD;
}

/** Hauteur (pixels du canvas) dont la pièce glissée est soulevée au-dessus du point de contact : le doigt ne la cache pas. */
export function dragLift(touch: boolean): number {
  return touch ? 46 : 12;
}

/** Agrandissement de la pièce glissée. */
export function dragScale(touch: boolean): number {
  return touch ? 1.3 : 1.14;
}

/** Marge, en pixels du canvas, où un appui tactile compte encore pour la case de bord la plus proche (le cadre n'est pas une zone morte). */
export const TOUCH_SLOP = 24;
export function touchSlop(touch: boolean): number {
  return touch ? TOUCH_SLOP : 0;
}

/** Côté d'une case en pixels CSS pour un plateau affiché sur `boardCssPx` de large (cadre compris). */
export function tileCssSize(boardCssPx: number): number {
  return (boardCssPx * TILE) / SIZE;
}

/** Largeur d'affichage minimale du plateau pour que les cases atteignent `MIN_TOUCH_TILE`. */
export function minBoardCssWidth(minTile = MIN_TOUCH_TILE): number {
  return Math.ceil((minTile * SIZE) / TILE);
}

/**
 * Facteur d'agrandissement des repères de cases (cibles, cases à choisir) quand le canvas est affiché réduit :
 * 1 à partir de ~0,75 (bureau), jusqu'à 1,5 sur un très petit plateau, pour que le point reste visible à l'écran.
 */
export function markBoost(displayScale: number): number {
  if (!(displayScale > 0)) return 1;
  return Math.min(1.5, Math.max(1, 0.75 / displayScale));
}

/** Convertit un point de la page en coordonnées du canvas (espace de dessin), quelle que soit la mise à l'échelle CSS. */
export function toCanvasPoint(clientX: number, clientY: number, rect: { left: number; top: number; width: number; height: number }): { x: number; y: number } {
  return { x: ((clientX - rect.left) * SIZE) / rect.width, y: ((clientY - rect.top) * SIZE) / rect.height };
}

/**
 * Case sous le point `(px, py)` du canvas, ou `null` hors du plateau. `slop` étend le plateau d'autant de pixels de canvas :
 * un point dans cette marge (le cadre) revient à la case de bord la plus proche.
 */
export function squareAtPoint(px: number, py: number, orientation: Color, slop = 0): Square | null {
  const lo = FRAME - slop;
  const hi = FRAME + TILE * 8 + slop;
  if (px < lo || px >= hi || py < lo || py >= hi) return null;
  const col = Math.min(7, Math.max(0, Math.floor((px - FRAME) / TILE)));
  const row = Math.min(7, Math.max(0, Math.floor((py - FRAME) / TILE)));
  const white = orientation === "white";
  return (white ? 7 - row : row) * 8 + (white ? col : 7 - col);
}
