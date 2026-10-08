import { describe, expect, it } from "vitest";
import { DRAG_THRESHOLD } from "../interaction";
import { FRAME, SIZE, TILE } from "./textures";
import {
  MIN_TOUCH_TILE,
  TOUCH_DRAG_THRESHOLD,
  TOUCH_SLOP,
  dragLift,
  dragScale,
  dragThreshold,
  markBoost,
  minBoardCssWidth,
  squareAtPoint,
  tileCssSize,
  toCanvasPoint,
  touchSlop,
} from "./touch";

/** Centre (en pixels du canvas) de la case `file` (0 = a) / `rank` (0 = rangée 1) vue des blancs. */
const whiteCentre = (file: number, rank: number) => ({ x: FRAME + file * TILE + TILE / 2, y: FRAME + (7 - rank) * TILE + TILE / 2 });

describe("réglages tactiles", () => {
  it("un doigt tolère plus de tremblement qu'une souris avant de devenir un glisser", () => {
    expect(dragThreshold(false)).toBe(DRAG_THRESHOLD);
    expect(dragThreshold(true)).toBe(TOUCH_DRAG_THRESHOLD);
    expect(dragThreshold(true)).toBeGreaterThan(dragThreshold(false));
  });

  it("la pièce glissée au doigt est plus soulevée et plus grande pour ne pas être cachée", () => {
    expect(dragLift(true)).toBeGreaterThan(dragLift(false));
    expect(dragScale(true)).toBeGreaterThan(dragScale(false));
  });

  it("la marge du cadre ne compte qu'au doigt", () => {
    expect(touchSlop(false)).toBe(0);
    expect(touchSlop(true)).toBe(TOUCH_SLOP);
  });
});

describe("taille des cases", () => {
  it("un plateau pleine largeur d'un téléphone (375 px) a des cases de plus de 43 px, 45 px sur 390 px", () => {
    expect(tileCssSize(375)).toBeGreaterThanOrEqual(43);
    expect(tileCssSize(390)).toBeGreaterThanOrEqual(45);
    expect(tileCssSize(SIZE)).toBe(TILE);
  });

  it("calcule la largeur minimale du plateau pour une taille de case donnée", () => {
    const w = minBoardCssWidth();
    expect(tileCssSize(w)).toBeGreaterThanOrEqual(MIN_TOUCH_TILE);
    expect(tileCssSize(w - 1)).toBeLessThan(MIN_TOUCH_TILE);
    expect(minBoardCssWidth(44)).toBeGreaterThan(w);
    // Un petit téléphone de 360 px reste au-dessus du seuil de 40 px.
    expect(360).toBeGreaterThanOrEqual(w);
  });

  it("les repères de cases grossissent sur un plateau réduit, jamais sur le bureau", () => {
    expect(markBoost(1)).toBe(1);
    expect(markBoost(0.9)).toBe(1);
    expect(markBoost(0.565)).toBeGreaterThan(1.2);
    expect(markBoost(0.565)).toBeLessThanOrEqual(1.5);
    expect(markBoost(0.1)).toBe(1.5);
    expect(markBoost(0)).toBe(1);
    expect(markBoost(Number.NaN)).toBe(1);
  });
});

describe("toCanvasPoint", () => {
  it("ramène un point de la page dans l'espace de dessin, quelle que soit l'échelle CSS", () => {
    const rect = { left: 10, top: 100, width: 375, height: 375 };
    expect(toCanvasPoint(10, 100, rect)).toEqual({ x: 0, y: 0 });
    const mid = toCanvasPoint(10 + 187.5, 100 + 187.5, rect);
    expect(mid.x).toBeCloseTo(SIZE / 2);
    expect(mid.y).toBeCloseTo(SIZE / 2);
    const end = toCanvasPoint(10 + 375, 100 + 375, rect);
    expect(end.x).toBeCloseTo(SIZE);
  });
});

describe("squareAtPoint", () => {
  it("trouve la case sous le doigt, côté blancs", () => {
    expect(squareAtPoint(whiteCentre(0, 0).x, whiteCentre(0, 0).y, "white")).toBe(0); // a1
    expect(squareAtPoint(whiteCentre(7, 7).x, whiteCentre(7, 7).y, "white")).toBe(63); // h8
    expect(squareAtPoint(whiteCentre(4, 1).x, whiteCentre(4, 1).y, "white")).toBe(12); // e2
    expect(squareAtPoint(whiteCentre(3, 3).x, whiteCentre(3, 3).y, "white")).toBe(27); // d4
  });

  it("retourne le plateau côté noirs", () => {
    // a1 est en haut à droite quand on joue les noirs.
    expect(squareAtPoint(FRAME + 7 * TILE + TILE / 2, FRAME + TILE / 2, "black")).toBe(0);
    // h8 est en bas à gauche.
    expect(squareAtPoint(FRAME + TILE / 2, FRAME + 7 * TILE + TILE / 2, "black")).toBe(63);
  });

  it("les limites des cases sont exactes", () => {
    expect(squareAtPoint(FRAME, FRAME + 7 * TILE, "white")).toBe(0);
    expect(squareAtPoint(FRAME + TILE - 0.01, FRAME + 7 * TILE + 0.01, "white")).toBe(0);
    expect(squareAtPoint(FRAME + TILE, FRAME + 7 * TILE, "white")).toBe(1);
  });

  it("hors du plateau (cadre compris) : rien à la souris", () => {
    expect(squareAtPoint(FRAME - 1, FRAME + 10, "white")).toBeNull();
    expect(squareAtPoint(FRAME + 8 * TILE, FRAME + 10, "white")).toBeNull();
    expect(squareAtPoint(FRAME + 10, FRAME - 1, "white")).toBeNull();
    expect(squareAtPoint(FRAME + 10, FRAME + 8 * TILE, "white")).toBeNull();
    expect(squareAtPoint(-5, -5, "white")).toBeNull();
  });

  it("au doigt, le cadre revient à la case de bord la plus proche", () => {
    const slop = touchSlop(true);
    // À gauche de la colonne a, rangée 4 : case a4.
    expect(squareAtPoint(FRAME - 10, whiteCentre(0, 3).y, "white", slop)).toBe(24);
    // Au-dessus de la rangée 8, colonne d : d8.
    expect(squareAtPoint(whiteCentre(3, 7).x, FRAME - 10, "white", slop)).toBe(59);
    // Coin bas-droit du cadre : h1.
    expect(squareAtPoint(FRAME + 8 * TILE + 10, FRAME + 8 * TILE + 10, "white", slop)).toBe(7);
    // Mais au-delà de la marge, on est vraiment hors du plateau (l'appui qui rate la pièce annule un glisser).
    expect(squareAtPoint(FRAME - slop - 1, whiteCentre(0, 3).y, "white", slop)).toBeNull();
    expect(squareAtPoint(FRAME + 8 * TILE + slop, FRAME + 10, "white", slop)).toBeNull();
  });
});
