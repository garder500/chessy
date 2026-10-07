import Phaser from "phaser";
import { exceedsDragThreshold, type Highlights } from "../interaction";
import type { ActiveEffect, Color, EffectKind, GameEvent, Piece, PieceKind, SkillId, SkillTarget, Square, StateView, Terrain } from "../protocol";
import { FX, drawArrow, drawDashedArrow, drawEffectMark, drawDashedRing, drawHalo, drawHexRing, drawRune, drawShield, drawStrings, effectColor } from "./fx";
import { skillEntry } from "../catalog";
import { pieceArtAssets, pieceArtKey } from "./pieceArt";
import { isForgedId } from "../forged";
import { actionKey, turnsLeft } from "./logic";
import { accentColor, boardTheme, getTheme, hexToNum, pieceSet, premoveColor, type ThemeSettings } from "../theme";
import { dragLift, dragScale, dragThreshold, markBoost, squareAtPoint, touchSlop } from "./touch";
import {
  BOARD_PX,
  FRAME,
  PIECE_TEX,
  ROCK_H,
  ROCK_KEY,
  SIZE,
  TILE,
  drawBoard,
  drawPiece,
  drawRock,
  drawStonePiece,
  pieceKey,
  stoneKey,
} from "./textures";

export const BOARD_SIZE = SIZE;

/** Un premove de la file, tel que dessiné ; `failed` : celui qui n'est plus jouable. */
export interface PremoveMark {
  from: Square;
  to: Square;
  failed?: boolean;
}

const MOVE_MS = 260;
const PIECE_SCALE = 78 / PIECE_TEX;
const KINDS: PieceKind[] = ["pawn", "knight", "bishop", "rook", "queen", "king"];
const MONO = '"Geist Mono Variable", "Geist Mono", ui-monospace, monospace';

// Couleurs du thème courant (voir `BoardScene.setTheme`) ; valeurs par défaut = accent bleu.
let ACCENT = 0x8fb4ff;
let PREMOVE = hexToNum(premoveColor("blue"));
const TEXT = 0xe9ebef;
const DANGER = 0xee8272;

type Sprite = Phaser.GameObjects.Image;

/** Ce que les événements de la dernière action racontent, indexé pour la mise à jour des pièces. */
interface EventCtx {
  skill: SkillId | null;
  caster: Color | null;
  teleported: Set<Square>;
  clonedFrom: Map<number, Square>;
  removed: Set<number>;
  captured: Set<number>;
  spawned: Set<number>;
  benched: Set<number>;
  unbenched: Set<number>;
  vanished: Set<number>;
  switched: Set<number>;
  pushed: Map<number, { from: Square; to: Square }>;
  saved: Map<number, { from: Square; to: Square }>;
  rotated: Set<Square>;
  trapSet: Set<Square>;
  terrain: Set<Square>;
}

function analyse(events: GameEvent[]): EventCtx {
  const ctx: EventCtx = {
    skill: null,
    caster: null,
    teleported: new Set(),
    clonedFrom: new Map(),
    removed: new Set(),
    captured: new Set(),
    spawned: new Set(),
    benched: new Set(),
    unbenched: new Set(),
    vanished: new Set(),
    switched: new Set(),
    pushed: new Map(),
    saved: new Map(),
    rotated: new Set(),
    trapSet: new Set(),
    terrain: new Set(),
  };
  for (const e of events) {
    switch (e.type) {
      case "skill_used":
        ctx.skill = e.skill;
        ctx.caster = e.color;
        break;
      case "teleported":
        ctx.teleported.add(e.to);
        break;
      case "cloned":
        ctx.clonedFrom.set(e.piece.id, e.from);
        break;
      case "removed":
        ctx.removed.add(e.piece.id);
        break;
      case "captured":
        ctx.captured.add(e.piece.id);
        break;
      case "spawned":
        ctx.spawned.add(e.piece.id);
        break;
      case "benched":
        ctx.benched.add(e.piece.id);
        break;
      case "unbenched":
        ctx.unbenched.add(e.piece.id);
        break;
      case "vanished":
        ctx.vanished.add(e.piece.id);
        break;
      case "switched":
        ctx.switched.add(e.piece.id);
        break;
      case "pushed":
        ctx.pushed.set(e.piece, { from: e.from, to: e.to });
        break;
      case "saved":
        ctx.saved.set(e.piece, { from: e.from, to: e.to });
        break;
      case "rotated":
        for (const m of e.moves) ctx.rotated.add(m.to);
        break;
      case "trap_set":
        ctx.trapSet.add(e.square);
        break;
      case "terrain":
        for (const s of e.squares) ctx.terrain.add(s);
        break;
      default:
        break;
    }
  }
  return ctx;
}

interface Mark {
  box: Phaser.GameObjects.Container;
  piece: number;
  kind: EffectKind | "temp";
  label?: Phaser.GameObjects.Text;
}

interface TerrainEntry {
  box: Phaser.GameObjects.Container;
  label: Phaser.GameObjects.Text;
}

type Flicker = "mirage" | "ghost" | "temp" | null;

/**
 * Renders the board and animates changes. It holds no game logic: clicks are
 * reported as squares and the owner decides what they mean.
 */
export class BoardScene extends Phaser.Scene {
  onSquare: (square: Square) => void = () => {};

  private view: StateView | null = null;
  private highlight: Highlights = { selectable: [], selected: null, targets: [] };
  private ready = false;

  private orientation: Color = "white";
  private gameId: string | null = null;
  private lastKey: string | null = null;
  private boardImage: Phaser.GameObjects.Image | null = null;
  private highlightLayer!: Phaser.GameObjects.Graphics;
  private bestLayer!: Phaser.GameObjects.Graphics;
  private sprites = new Map<number, Sprite>();
  private placed = new Map<number, Square>();
  private flicker = new Map<number, Flicker>();
  private marks = new Map<string, Mark>();
  private traps = new Map<Square, Phaser.GameObjects.Graphics>();
  private terrain = new Map<Square, TerrainEntry>();
  /** Meilleur coup de Mind Reading, affiché jusqu'au prochain coup. */
  private best: { ply: number; from: Square; to: Square } | null = null;

  // ---- thème, glisser-déposer, premove (voir docs/spec-v4.md §4) ----
  /** Cette case peut-elle être glissée ? Renseigné par `PhaserBoard`. */
  canDrag: (square: Square) => boolean = () => false;
  /** Début d'un glisser : le propriétaire « prend la pièce en main » (cases légales). */
  onDragStart: (square: Square) => void = () => {};
  /** Relâchement : `to` est null hors du plateau. Renvoie `snap` (la pièce se pose) ou `return` (elle revient). */
  onDrop: (from: Square, to: Square | null) => "snap" | "return" = () => "return";
  /** Clic droit ou appui long : annule tous les premoves. */
  onCancelPremove: () => void = () => {};

  private theme: ThemeSettings = getTheme();
  private boardKeys = new Set<string>();
  private premove: PremoveMark[] = [];
  private premoveLabels: Phaser.GameObjects.Text[] = [];
  private premoveLayer!: Phaser.GameObjects.Graphics;
  private dragShadow!: Phaser.GameObjects.Graphics;
  private press: { square: Square; x: number; y: number; id: number | null } | null = null;
  private drag: { id: number; from: Square; sprite: Sprite; hover: Square | null } | null = null;
  private pressTimer: Phaser.Time.TimerEvent | null = null;
  private longPressed = false;

  constructor() {
    super("board");
  }

  preload() {
    for (const [key, url] of pieceArtAssets()) this.load.image(key, url);
  }

  create() {
    this.ensurePieceTextures(this.theme.pieces);
    for (const color of ["white", "black"] as Color[]) {
      const stone = this.textures.createCanvas(stoneKey(color, "pawn"), PIECE_TEX, PIECE_TEX);
      if (stone) {
        drawStonePiece(stone.getSourceImage() as HTMLCanvasElement, "pawn", color);
        stone.refresh();
      }
    }
    const rock = this.textures.createCanvas(ROCK_KEY, TILE, ROCK_H);
    if (rock) {
      drawRock(rock.getSourceImage() as HTMLCanvasElement);
      rock.refresh();
    }
    this.highlightLayer = this.add.graphics().setDepth(1);
    this.bestLayer = this.add.graphics().setDepth(9);
    this.premoveLayer = this.add.graphics().setDepth(8.5);
    this.dragShadow = this.add.graphics().setDepth(19);
    this.input.mouse?.disableContextMenu();
    this.game.canvas.style.touchAction = "none";
    this.input.on("pointerdown", (pointer: Phaser.Input.Pointer) => this.handleDown(pointer));
    this.input.on("pointermove", (pointer: Phaser.Input.Pointer) => this.handleMove(pointer));
    this.input.on("pointerup", (pointer: Phaser.Input.Pointer) => this.handleUp(pointer));
    this.input.on("pointerupoutside", (pointer: Phaser.Input.Pointer) => this.handleUp(pointer));
    // Le plateau change d'échelle (rotation, redimensionnement) : les repères de cases s'adaptent (voir `markBoost`).
    const onResize = () => {
      if (this.ready) this.drawHighlights();
    };
    this.scale.on("resize", onResize);
    this.events.once("shutdown", () => {
      this.scale.off("resize", onResize);
      this.clearPress();
    });
    this.ready = true;
    this.applyTheme(null);
    this.render();
  }

  setView(view: StateView | null) {
    this.view = view;
    if (this.ready) this.render();
  }

  setHighlights(highlight: Highlights) {
    this.highlight = highlight;
    if (this.ready) this.drawHighlights();
  }

  /** Thème de plateau, jeu de pièces, accent, mode de déplacement, animations : appliqué sans recharger. */
  setTheme(theme: ThemeSettings) {
    const prev = this.theme;
    this.theme = theme;
    if (this.ready) this.applyTheme(prev);
  }

  private applyTheme(prev: ThemeSettings | null) {
    const theme = this.theme;
    ACCENT = hexToNum(accentColor(theme.accent));
    PREMOVE = hexToNum(premoveColor(theme.accent));
    this.tweens.timeScale = theme.reduceMotion ? 20 : 1;
    if (!prev || prev.pieces !== theme.pieces) {
      this.ensurePieceTextures(theme.pieces);
      if (prev) this.retextureAll();
    }
    if (prev && prev.board !== theme.board && this.view) this.drawBoard();
    // Le cadre reprend la surface du mode clair/sombre : redessiné une fois le mode appliqué au document.
    else if (prev && prev.mode !== theme.mode && this.view) this.time.delayedCall(0, () => this.view && this.drawBoard());
    if (prev && !this.dragEnabled && this.drag) this.cancelDrag();
    this.drawHighlights();
    this.drawPremove();
  }

  private get dragEnabled() {
    return this.theme.move === "drag";
  }

  private ensurePieceTextures(setId: ThemeSettings["pieces"]) {
    const set = pieceSet(setId);
    for (const color of ["white", "black"] as Color[]) {
      for (const kind of KINDS) {
        const key = pieceKey(color, kind, setId);
        if (this.textures.exists(key)) continue;
        const tex = this.textures.createCanvas(key, PIECE_TEX, PIECE_TEX);
        if (!tex) continue;
        const art = setId === "cburnett" ? this.textures.get(pieceArtKey(color, kind)) : null;
        if (art && art.key !== "__MISSING") (tex.getSourceImage() as HTMLCanvasElement).getContext("2d")!.drawImage(art.getSourceImage() as HTMLImageElement, 0, 0, PIECE_TEX, PIECE_TEX);
        else drawPiece(tex.getSourceImage() as HTMLCanvasElement, kind, color, color === "white" ? set.white : set.black);
        tex.refresh();
      }
    }
  }

  private retextureAll() {
    const byId = new Map<number, Piece>();
    for (const p of this.view?.board ?? []) if (p) byId.set(p.id, p);
    for (const [id, sprite] of this.sprites) {
      const piece = byId.get(id);
      if (piece) sprite.setTexture(this.textureFor(piece));
    }
  }

  /**
   * File de premoves (cases d'origine et d'arrivée teintées, flèches pointillées numérotées dans l'ordre).
   * Un premove `failed` est dessiné en rouge et toute la couche clignote avant que la file ne soit vidée.
   */
  setPremove(list: PremoveMark[] | null) {
    const was = this.premove.some((m) => m.failed);
    this.premove = list ?? [];
    if (!this.ready) return;
    this.drawPremove();
    const failed = this.premove.some((m) => m.failed);
    const targets = [this.premoveLayer, ...this.premoveLabels];
    this.tweens.killTweensOf(targets);
    for (const t of targets) t.setAlpha(1);
    if (failed && !was) {
      this.tweens.add({ targets, alpha: 0.08, duration: 110, yoyo: true, repeat: 2 });
    }
  }

  private drawPremove() {
    const g = this.premoveLayer;
    g.clear();
    for (const label of this.premoveLabels) label.destroy();
    this.premoveLabels = [];
    const list = this.premove;
    if (list.length === 0) return;
    const bad = hexToNum("#ff4d3d");
    const squares = new Set<Square>();
    for (const m of list) {
      squares.add(m.from);
      squares.add(m.to);
    }
    for (const square of squares) {
      const { x, y } = this.cell(square);
      const color = list.some((m) => m.failed && (m.from === square || m.to === square)) ? bad : PREMOVE;
      g.fillStyle(color, 0.4).fillRect(x, y, TILE, TILE);
      g.lineStyle(3, color, 0.95).strokeRect(x + 2, y + 2, TILE - 4, TILE - 4);
    }
    list.forEach((m, i) => {
      const color = m.failed ? bad : PREMOVE;
      const a = this.center(m.from);
      const b = this.center(m.to);
      drawDashedArrow(g, a.x, a.y, b.x, b.y, color);
      // Pastille d'ordre dans le coin de la case d'arrivée (seulement quand il y a plusieurs premoves).
      if (list.length < 2) return;
      const { x, y } = this.cell(m.to);
      const cx = x + TILE - 15;
      const cy = y + 15;
      g.fillStyle(0x0e0f12, 0.92).fillCircle(cx, cy, 12);
      g.lineStyle(2, color, 1).strokeCircle(cx, cy, 12);
      this.premoveLabels.push(
        this.add
          .text(cx, cy, String(i + 1), { fontFamily: MONO, fontSize: "14px", color: "#ffffff", fontStyle: "700" })
          .setOrigin(0.5)
          .setDepth(8.6),
      );
    });
  }

  // ---- entrée : clic, glisser-déposer, appui long ----------------------------------------------

  private idAt(square: Square): number | null {
    for (const [id, sq] of this.placed) if (sq === square) return id;
    return null;
  }

  private clearPress() {
    this.pressTimer?.remove(false);
    this.pressTimer = null;
    this.press = null;
  }

  private handleDown(pointer: Phaser.Input.Pointer) {
    if (pointer.rightButtonDown()) {
      this.onCancelPremove();
      return;
    }
    const square = this.squareAt(pointer.x, pointer.y, pointer.wasTouch);
    if (square === null) return;
    this.longPressed = false;
    this.pressTimer?.remove(false);
    this.pressTimer = this.premove.length > 0
      ? this.time.delayedCall(550, () => {
          this.pressTimer = null;
          if (!this.drag) {
            this.longPressed = true;
            this.press = null;
            this.onCancelPremove();
          }
        })
      : null;
    const id = this.idAt(square);
    if (this.dragEnabled && id !== null && this.canDrag(square)) {
      // Clic ou glisser ? On attend le relâchement ou le dépassement du seuil de 4 px.
      this.press = { square, x: pointer.x, y: pointer.y, id };
      return;
    }
    this.press = null;
    this.onSquare(square);
  }

  private handleMove(pointer: Phaser.Input.Pointer) {
    const square = this.squareAt(pointer.x, pointer.y, pointer.wasTouch);
    const hot = square !== null && (this.highlight.selectable.includes(square) || this.highlight.targets.includes(square));
    this.game.canvas.style.cursor = this.drag ? "grabbing" : hot ? "pointer" : "default";
    if (this.press && !this.drag && exceedsDragThreshold(pointer.x - this.press.x, pointer.y - this.press.y, dragThreshold(pointer.wasTouch))) {
      this.pressTimer?.remove(false);
      this.pressTimer = null;
      this.startDrag(this.press, pointer);
    }
    if (this.drag) this.updateDrag(pointer);
  }

  private handleUp(pointer: Phaser.Input.Pointer) {
    this.pressTimer?.remove(false);
    this.pressTimer = null;
    if (this.drag) {
      this.finishDrag(pointer);
      return;
    }
    const press = this.press;
    this.press = null;
    if (press && !this.longPressed) this.onSquare(press.square);
  }

  private startDrag(press: { square: Square; id: number | null }, pointer: Phaser.Input.Pointer) {
    const sprite = press.id !== null ? this.sprites.get(press.id) : undefined;
    this.press = null;
    if (!sprite || press.id === null) {
      this.onSquare(press.square);
      return;
    }
    this.tweens.killTweensOf(sprite);
    this.drag = { id: press.id, from: press.square, sprite, hover: press.square };
    sprite.setDepth(20).setScale(PIECE_SCALE * dragScale(pointer.wasTouch)).setAlpha(1);
    this.onDragStart(press.square);
    this.updateDrag(pointer);
  }

  private updateDrag(pointer: Phaser.Input.Pointer) {
    const drag = this.drag;
    if (!drag) return;
    const x = Phaser.Math.Clamp(pointer.x, FRAME, SIZE - FRAME);
    const y = Phaser.Math.Clamp(pointer.y, FRAME, SIZE - FRAME);
    // La pièce est soulevée au-dessus du doigt ; son ombre reste plus bas.
    drag.sprite.setPosition(x, y - dragLift(pointer.wasTouch));
    this.dragShadow.clear().fillStyle(0x000000, 0.34).fillEllipse(x + 3, y + 24, 56, 16);
    const hover = this.squareAt(pointer.x, pointer.y, pointer.wasTouch);
    if (hover !== drag.hover) {
      drag.hover = hover;
      this.drawHighlights();
    }
  }

  private finishDrag(pointer: Phaser.Input.Pointer) {
    const drag = this.drag;
    if (!drag) return;
    const to = this.squareAt(pointer.x, pointer.y, pointer.wasTouch);
    this.drag = null;
    this.dragShadow.clear();
    this.game.canvas.style.cursor = "default";
    const verdict = this.onDrop(drag.from, to);
    const { sprite } = drag;
    const settle = () => {
      sprite.setDepth(2);
      if (this.sprites.get(drag.id) === sprite) sprite.setScale(PIECE_SCALE);
    };
    if (verdict === "snap" && to !== null) {
      const t = this.center(to);
      this.tweens.add({ targets: sprite, x: t.x, y: t.y, scale: PIECE_SCALE, duration: 70, ease: "Quad.Out", onComplete: settle });
      // Filet de sécurité : si le serveur refuse le coup, la pièce ne reste pas sur une case fausse.
      this.time.delayedCall(1500, () => {
        const home = this.placed.get(drag.id);
        if (this.sprites.get(drag.id) !== sprite || home === undefined || this.tweens.isTweening(sprite)) return;
        const c = this.center(home);
        if (Math.abs(sprite.x - c.x) > 1 || Math.abs(sprite.y - c.y) > 1) this.slideHome(drag.id, sprite);
      });
    } else {
      this.slideHome(drag.id, sprite);
    }
    this.drawHighlights();
  }

  private slideHome(id: number, sprite: Sprite) {
    const home = this.placed.get(id);
    if (home === undefined) return;
    const c = this.center(home);
    this.tweens.killTweensOf(sprite);
    this.tweens.add({
      targets: sprite,
      x: c.x,
      y: c.y,
      scale: PIECE_SCALE,
      duration: 170,
      ease: "Cubic.Out",
      onComplete: () => sprite.setDepth(2),
    });
  }

  /** Abandonne un glisser en cours (nouvelle position, thème, désactivation) : la pièce rentre. */
  private cancelDrag() {
    const drag = this.drag;
    if (!drag) return;
    this.drag = null;
    this.dragShadow.clear();
    this.slideHome(drag.id, drag.sprite);
  }

  /** Where each piece currently stands, for tests and debugging. */
  debug() {
    return {
      orientation: this.orientation,
      pieces: [...this.placed.entries()].map(([id, square]) => ({ id, square, texture: this.sprites.get(id)?.texture.key })),
      highlight: this.highlight,
      badges: [...this.marks.keys()],
      traps: [...this.traps.keys()],
      terrain: [...this.terrain.keys()],
      best: this.best,
      theme: this.theme,
      premove: this.premove,
      dragging: this.drag ? { id: this.drag.id, from: this.drag.from, hover: this.drag.hover } : null,
    };
  }

  update(time: number) {
    // Les marqueurs d'effet suivent leur pièce pendant qu'elle glisse et respirent doucement.
    for (const mark of this.marks.values()) {
      const sprite = this.sprites.get(mark.piece);
      if (!sprite) continue;
      const pulse = 0.78 + 0.22 * Math.sin(time / 380);
      const moving = this.tweens.isTweening(sprite);
      mark.box.setPosition(sprite.x, sprite.y).setAlpha(moving ? sprite.alpha * pulse : pulse);
      if (mark.kind === "color_loan") mark.box.setRotation(0.035 * Math.sin(time / 520));
    }
    // Les pièces fantomatiques vacillent ; une animation en cours garde la main sur l'opacité.
    for (const [id, mode] of this.flicker) {
      const sprite = this.sprites.get(id);
      if (!sprite || !mode || this.tweens.isTweening(sprite)) continue;
      if (mode === "mirage") {
        const glitch = Math.floor(time / 90 + id * 7) % 23 === 0;
        sprite.setAlpha((0.5 + 0.1 * Math.sin(time / 260 + id)) * (glitch ? 0.4 : 1));
      } else if (mode === "ghost") {
        sprite.setAlpha(0.42 + 0.06 * Math.sin(time / 420 + id));
      } else {
        sprite.setAlpha(0.86 + 0.14 * Math.sin(time / 170 + id));
      }
    }
    for (const [square, rune] of this.traps) rune.setAlpha(0.55 + 0.25 * Math.sin(time / 600 + square));
  }

  // ---- geometry ----------------------------------------------------------

  private cell(square: Square): { x: number; y: number } {
    const file = square % 8;
    const rank = Math.floor(square / 8);
    const white = this.orientation === "white";
    return { x: FRAME + (white ? file : 7 - file) * TILE, y: FRAME + (white ? 7 - rank : rank) * TILE };
  }

  private center(square: Square): { x: number; y: number } {
    const { x, y } = this.cell(square);
    return { x: x + TILE / 2, y: y + TILE / 2 };
  }

  /** Case sous un point du canvas ; au doigt, le cadre compte pour la case de bord voisine (voir `touchSlop`). */
  private squareAt(px: number, py: number, touch = false): Square | null {
    return squareAtPoint(px, py, this.orientation, touchSlop(touch));
  }

  private boardCenter() {
    return { x: FRAME + BOARD_PX / 2, y: FRAME + BOARD_PX / 2 };
  }

  // ---- drawing -----------------------------------------------------------

  private render() {
    const view = this.view;
    if (!view) {
      this.clearPieces();
      this.gameId = null;
      this.lastKey = null;
      this.best = null;
      this.highlightLayer.clear();
      this.bestLayer.clear();
      return;
    }
    const fresh = view.game_id !== this.gameId || view.you !== this.orientation;
    if (fresh) {
      this.clearPieces();
      this.gameId = view.game_id;
      this.orientation = view.you;
      this.lastKey = null;
      this.best = null;
      this.drawBoard();
    }
    // Les effets ne se jouent qu'une fois par action (Mind Reading/Control ne changent pas le demi-coup).
    const key = actionKey(view);
    const advanced = !fresh && key !== this.lastKey;
    this.lastKey = key;
    if (this.drag && (fresh || advanced)) this.cancelDrag();
    const ctx = analyse(advanced ? view.events : []);
    this.reconcilePieces(view, !fresh, ctx);
    this.reconcileTerrain(view, !fresh, ctx);
    this.reconcileTraps(view, !fresh, ctx);
    this.reconcileMarks(view);
    // Après une reconnexion, le dernier `best_move` encore valable (même demi-coup) est reaffiché.
    this.trackBest(view, advanced || fresh);
    if (advanced) this.playEffects(view, ctx);
    this.drawHighlights();
  }

  private drawBoard() {
    const colors = boardTheme(this.theme.board);
    const key = `board-${this.orientation}-${this.theme.board}`;
    if (!this.textures.exists(key)) {
      const tex = this.textures.createCanvas(key, SIZE, SIZE);
      if (tex) {
        drawBoard(tex.getSourceImage() as HTMLCanvasElement, this.orientation, colors);
        tex.refresh();
        this.boardKeys.add(key);
      }
    }
    this.boardImage?.destroy();
    this.boardImage = this.add.image(0, 0, key).setOrigin(0).setDepth(0);
    // Les plateaux d'un ancien thème ne servent plus : on libère leur texture.
    for (const old of this.boardKeys) {
      if (old === key) continue;
      this.boardKeys.delete(old);
      if (this.textures.exists(old)) this.textures.remove(old);
    }
  }

  private drawHighlights() {
    const g = this.highlightLayer;
    g.clear();
    this.bestLayer.clear();
    const view = this.view;
    if (!view) return;
    // Plateau réduit (téléphone) : les repères de cases s'agrandissent pour rester lisibles.
    // (`displayScale` de Phaser = taille de dessin / taille affichée : on l'inverse.)
    const boost = markBoost(1 / this.scale.displayScale.x);

    // Dernière position : cases touchées par la dernière action.
    for (const e of view.events) {
      for (const square of touchedSquares(e)) {
        const { x, y } = this.cell(square);
        g.fillStyle(ACCENT, 0.3).fillRect(x, y, TILE, TILE);
        g.lineStyle(2, ACCENT, 0.7).strokeRect(x + 1, y + 1, TILE - 2, TILE - 2);
      }
    }
    if (view.in_check) {
      const king = view.board.findIndex((p) => p?.kind === "king" && p.color === view.to_move);
      if (king >= 0) {
        const { x, y } = this.center(king);
        g.fillStyle(DANGER, 0.22).fillCircle(x, y, TILE * 0.62);
        g.fillStyle(DANGER, 0.3).fillCircle(x, y, TILE * 0.42);
        g.lineStyle(3, DANGER, 0.9).strokeCircle(x, y, TILE * 0.44);
      }
    }
    for (const square of this.highlight.selectable) {
      if (square === this.highlight.selected) continue;
      if (view.board[square]) {
        const { x, y } = this.cell(square);
        g.lineStyle(2, TEXT, 0.55).strokeRect(x + 5, y + 5, TILE - 10, TILE - 10);
      } else {
        // Case vide à choisir (Trap, Geomancy, Mirage) : un petit repère discret.
        const { x, y } = this.center(square);
        g.fillStyle(TEXT, 0.12).fillCircle(x, y, 15 * boost);
        g.lineStyle(2 * boost, TEXT, 0.6).strokeCircle(x, y, 15 * boost);
      }
    }
    if (this.highlight.selected !== null) {
      const { x, y } = this.cell(this.highlight.selected);
      if (boost > 1) {
        // Petit plateau : un blanc translucide se perd sur les cases claires, on prend la couleur d'accent.
        g.fillStyle(ACCENT, 0.4).fillRect(x, y, TILE, TILE);
        g.lineStyle(3 * boost, ACCENT, 1).strokeRect(x + 2, y + 2, TILE - 4, TILE - 4);
      } else {
        g.fillStyle(TEXT, 0.2).fillRect(x, y, TILE, TILE);
        g.lineStyle(3, TEXT, 0.95).strokeRect(x + 2, y + 2, TILE - 4, TILE - 4);
      }
    }
    for (const square of this.highlight.targets) {
      const { x, y } = this.center(square);
      if (view.board[square]) {
        g.lineStyle(6, DANGER, 0.9).strokeCircle(x, y, TILE / 2 - 5);
      } else {
        g.fillStyle(0x0e0f12, 0.6).fillCircle(x, y, 13 * boost);
        g.lineStyle(2.5 * boost, TEXT, 0.95).strokeCircle(x, y, 13 * boost);
        g.fillStyle(TEXT, 0.95).fillCircle(x, y, 4 * boost);
      }
    }
    const hover = this.drag?.hover;
    if (hover !== null && hover !== undefined && hover !== this.drag?.from && this.highlight.targets.includes(hover)) {
      const { x, y } = this.cell(hover);
      g.fillStyle(ACCENT, 0.32).fillRect(x, y, TILE, TILE);
      g.lineStyle(4, ACCENT, 0.95).strokeRect(x + 2, y + 2, TILE - 4, TILE - 4);
    }
    if (this.best && view.ply === this.best.ply) {
      const a = this.center(this.best.from);
      const b = this.center(this.best.to);
      drawArrow(this.bestLayer, a.x, a.y, b.x, b.y, ACCENT);
    }
  }

  /** Mind Reading : la flèche reste jusqu'au prochain coup (changement de demi-coup). */
  private trackBest(view: StateView, advanced: boolean) {
    if (advanced) {
      const e = view.events.find((x): x is Extract<GameEvent, { type: "best_move" }> => x.type === "best_move");
      if (e) {
        this.best = { ply: view.ply, from: e.from, to: e.to };
        this.bestLayer.setAlpha(0);
        this.tweens.killTweensOf(this.bestLayer);
        this.tweens.add({ targets: this.bestLayer, alpha: 1, duration: 420, delay: 500 });
        return;
      }
    }
    if (this.best && this.best.ply !== view.ply) this.best = null;
  }

  private textureFor(piece: Piece): string {
    if (piece.wall && this.textures.exists(stoneKey(piece.color, piece.kind))) return stoneKey(piece.color, piece.kind);
    return pieceKey(piece.color, piece.kind, this.theme.pieces);
  }

  private makeSprite(piece: Piece, x: number, y: number): Sprite {
    return this.add.image(x, y, this.textureFor(piece)).setScale(PIECE_SCALE).setDepth(2);
  }

  private clearPieces() {
    this.sprites.forEach((s) => {
      this.tweens.killTweensOf(s);
      s.destroy();
    });
    this.marks.forEach((m) => {
      this.tweens.killTweensOf(m.box);
      m.box.destroy();
    });
    this.traps.forEach((t) => t.destroy());
    this.terrain.forEach((t) => {
      this.tweens.killTweensOf(t.box);
      t.box.destroy();
    });
    this.sprites.clear();
    this.placed.clear();
    this.flicker.clear();
    this.marks.clear();
    this.traps.clear();
    this.terrain.clear();
  }

  /** Applique le style de la pièce (teinte, vacillement). */
  private restyle(sprite: Sprite, piece: Piece, view: StateView) {
    const invisible = view.effects.some((e) => e.piece === piece.id && e.kind === "invisible");
    let mode: Flicker = null;
    if (piece.mirage) {
      mode = "mirage";
      sprite.setTint(FX.mirage);
    } else if (invisible) {
      mode = "ghost";
      sprite.setTint(0xcfe0ff);
    } else {
      sprite.clearTint();
      if (piece.temp) mode = "temp";
    }
    this.flicker.set(piece.id, mode);
    if (!mode && !this.tweens.isTweening(sprite) && sprite.alpha < 1) sprite.setAlpha(1);
  }

  /**
   * Brings the sprites in line with the board. Pieces keep their id across
   * moves, so a moved piece slides, a new id pops in and a vanished one fades.
   */
  private reconcilePieces(view: StateView, animate: boolean, ctx: EventCtx) {
    const present = new Set<number>();
    view.board.forEach((piece, square) => {
      if (!piece) return;
      present.add(piece.id);
      let sprite = this.sprites.get(piece.id);
      const target = this.center(square);

      if (!sprite) {
        const from = animate ? ctx.clonedFrom.get(piece.id) : undefined;
        const origin = from !== undefined ? this.center(from) : target;
        sprite = this.makeSprite(piece, origin.x, origin.y);
        this.sprites.set(piece.id, sprite);
        this.placed.set(piece.id, square);
        this.restyle(sprite, piece, view);
        if (animate) this.enter(sprite, piece, target, ctx, from !== undefined);
        return;
      }

      this.restyle(sprite, piece, view);
      const tex = this.textureFor(piece);
      if (this.placed.get(piece.id) !== square) {
        this.placed.set(piece.id, square);
        this.tweens.killTweensOf(sprite);
        if (animate) this.relocate(sprite, piece, square, target, ctx);
        else sprite.setPosition(target.x, target.y);
      }
      if (sprite.texture.key !== tex) this.retexture(sprite, tex, piece, animate, ctx);
    });

    for (const [id, sprite] of this.sprites) {
      if (present.has(id)) continue;
      this.sprites.delete(id);
      this.placed.delete(id);
      this.flicker.delete(id);
      this.tweens.killTweensOf(sprite);
      if (!animate) {
        sprite.destroy();
      } else if (ctx.benched.has(id)) {
        // Banc : la pièce s'élève dans un rayon de lumière.
        this.tweens.add({
          targets: sprite,
          y: sprite.y - 54,
          alpha: 0,
          scale: PIECE_SCALE * 0.7,
          duration: 560,
          ease: "Cubic.In",
          onComplete: () => sprite.destroy(),
        });
      } else if (ctx.vanished.has(id)) {
        this.glitchOut(sprite);
      } else if (ctx.removed.has(id)) {
        this.dissolve(sprite);
      } else if (ctx.captured.has(id)) {
        this.tweens.add({
          targets: sprite,
          alpha: 0,
          scale: PIECE_SCALE * 1.25,
          duration: 280,
          delay: MOVE_MS * 0.6,
          ease: "Quad.In",
          onComplete: () => sprite.destroy(),
        });
      } else {
        this.tweens.add({ targets: sprite, alpha: 0, scale: PIECE_SCALE * 0.4, duration: MOVE_MS, onComplete: () => sprite.destroy() });
      }
    }
  }

  /** Entrée d'une nouvelle pièce : copie qui se détache, surgissement, descente du banc… */
  private enter(sprite: Sprite, piece: Piece, t: { x: number; y: number }, ctx: EventCtx, cloned: boolean) {
    const S = PIECE_SCALE;
    if (cloned) {
      // Dédoublement : la copie se détache de l'original et glisse vers sa case.
      sprite.setAlpha(0.2).setDepth(9);
      this.tweens.add({ targets: sprite, alpha: 1, duration: 200 });
      this.tweens.add({
        targets: sprite,
        x: t.x,
        y: t.y,
        duration: 420,
        delay: 120,
        ease: "Cubic.InOut",
        onComplete: () => sprite.setDepth(2),
      });
    } else if (ctx.unbenched.has(piece.id)) {
      // Retour du banc : la pièce redescend dans la lumière.
      sprite.setPosition(t.x, t.y - 56).setAlpha(0).setDepth(10);
      this.tweens.add({ targets: sprite, y: t.y, alpha: 1, duration: 560, ease: "Cubic.Out", onComplete: () => sprite.setDepth(2) });
    } else if (ctx.spawned.has(piece.id)) {
      if (piece.wall) {
        // Un mur surgit du sol.
        sprite.setPosition(t.x, t.y + 34).setScale(S, S * 0.3).setAlpha(0);
        this.tweens.add({
          targets: sprite,
          y: t.y,
          scaleY: S,
          alpha: 1,
          duration: 420,
          delay: (piece.id % 3) * 110,
          ease: "Back.Out",
        });
      } else if (ctx.skill === "godhelp") {
        sprite.setPosition(t.x, t.y - 130).setAlpha(0).setDepth(10);
        this.tweens.add({ targets: sprite, y: t.y, alpha: 1, duration: 520, delay: 160, ease: "Bounce.Out", onComplete: () => sprite.setDepth(2) });
      } else if (ctx.skill === "terminator") {
        // La copie se déplie comme un reflet.
        sprite.setScale(0, S).setAlpha(0.2);
        this.tweens.add({ targets: sprite, scaleX: S, alpha: 1, duration: 520, delay: 300, ease: "Back.Out" });
      } else if (piece.mirage) {
        sprite.setScale(S * 1.3).setAlpha(0);
        this.tweens.add({ targets: sprite, scale: S, alpha: 0.55, duration: 560, ease: "Cubic.Out" });
      } else {
        sprite.setScale(S * 0.2).setAlpha(0);
        this.tweens.add({ targets: sprite, scale: S, alpha: 1, duration: 340, ease: "Back.Out" });
      }
    } else {
      sprite.setScale(S * 0.2).setAlpha(0);
      this.tweens.add({ targets: sprite, scale: S, alpha: 1, duration: 300, ease: "Back.Out" });
    }
  }

  /** Déplacement d'une pièce déjà présente, avec l'animation propre à l'événement qui l'explique. */
  private relocate(sprite: Sprite, piece: Piece, square: Square, target: { x: number; y: number }, ctx: EventCtx) {
    const S = PIECE_SCALE;
    const saved = ctx.saved.get(piece.id);
    const pushed = ctx.pushed.get(piece.id);
    if (saved) {
      // Celestial : la pièce s'efface dans une lumière dorée et se reforme sur sa case de départ.
      this.tweens.add({
        targets: sprite,
        alpha: 0,
        scale: S * 0.3,
        duration: 260,
        onComplete: () => {
          sprite.setPosition(target.x, target.y);
          this.tweens.add({ targets: sprite, alpha: 1, scale: S, duration: 520, delay: 160, ease: "Back.Out" });
        },
      });
    } else if (pushed) {
      // Force Field : l'attaquant arrive sur la case puis est repoussé.
      const hit = this.center(pushed.from);
      sprite.setDepth(10);
      this.tweens.add({
        targets: sprite,
        x: hit.x,
        y: hit.y,
        scale: S,
        duration: MOVE_MS,
        ease: "Cubic.InOut",
        onComplete: () => {
          this.tweens.add({
            targets: sprite,
            x: target.x,
            y: target.y,
            duration: 420,
            ease: "Back.Out",
            onComplete: () => sprite.setDepth(2),
          });
        },
      });
    } else if (ctx.rotated.has(square)) {
      this.arcMove(sprite, target);
    } else if (ctx.teleported.has(square)) {
      this.tweens.add({
        targets: sprite,
        alpha: 0,
        scale: S * 0.3,
        duration: 180,
        onComplete: () => {
          sprite.setPosition(target.x, target.y);
          this.tweens.add({ targets: sprite, alpha: 1, scale: S, duration: 260, ease: "Back.Out" });
        },
      });
    } else {
      sprite.setDepth(10);
      // `scale` : une pièce lâchée juste avant cette animation (glisser) a pu être interrompue en cours de réduction.
      this.tweens.add({
        targets: sprite,
        x: target.x,
        y: target.y,
        scale: S,
        duration: MOVE_MS,
        ease: "Cubic.InOut",
        onComplete: () => sprite.setDepth(2),
      });
    }
  }

  /** Changement d'apparence : retournement (Switch), fondu enchaîné (Morph, Evolve, Tornado, contrôle). */
  private retexture(sprite: Sprite, tex: string, piece: Piece, animate: boolean, ctx: EventCtx) {
    if (!animate) {
      sprite.setTexture(tex);
      return;
    }
    if (ctx.switched.has(piece.id)) {
      this.tweens.add({
        targets: sprite,
        scaleX: 0,
        duration: 180,
        ease: "Quad.In",
        onComplete: () => {
          sprite.setTexture(tex);
          this.tweens.add({ targets: sprite, scaleX: PIECE_SCALE, duration: 320, ease: "Back.Out" });
        },
      });
      return;
    }
    const ghost = this.add
      .image(sprite.x, sprite.y, sprite.texture.key)
      .setScale(sprite.scaleX, sprite.scaleY)
      .setDepth(sprite.depth + 0.1)
      .setAlpha(sprite.alpha);
    this.tweens.add({ targets: ghost, alpha: 0, scale: ghost.scaleX * 1.12, duration: 520, onComplete: () => ghost.destroy() });
    sprite.setTexture(tex).setAlpha(0.2);
    this.tweens.add({ targets: sprite, alpha: 1, duration: 520 });
  }

  /** Tornado : la pièce suit un arc de cercle autour du centre du plateau. */
  private arcMove(sprite: Sprite, target: { x: number; y: number }) {
    const c = this.boardCenter();
    const a0 = Math.atan2(sprite.y - c.y, sprite.x - c.x);
    const r0 = Math.hypot(sprite.x - c.x, sprite.y - c.y);
    const a1 = Math.atan2(target.y - c.y, target.x - c.x);
    const r1 = Math.hypot(target.x - c.x, target.y - c.y);
    let da = a1 - a0;
    while (da > Math.PI) da -= Math.PI * 2;
    while (da < -Math.PI) da += Math.PI * 2;
    const state = { t: 0 };
    sprite.setDepth(10);
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 820,
      ease: "Cubic.InOut",
      onUpdate: () => {
        const a = a0 + da * state.t;
        const r = r0 + (r1 - r0) * state.t;
        sprite.setPosition(c.x + Math.cos(a) * r, c.y + Math.sin(a) * r);
      },
      onComplete: () => {
        sprite.setPosition(target.x, target.y);
        sprite.setDepth(2);
      },
    });
  }

  // ---- terrain, pièges ---------------------------------------------------------

  private reconcileTerrain(view: StateView, animate: boolean, ctx: EventCtx) {
    const wanted = new Set<Square>();
    for (const t of view.terrain) {
      wanted.add(t.square);
      let entry = this.terrain.get(t.square);
      if (!entry) {
        entry = this.makeTerrain(t);
        this.terrain.set(t.square, entry);
        if (animate && ctx.terrain.has(t.square)) {
          entry.box.setScale(1, 0.05).setAlpha(0.2);
          this.tweens.add({ targets: entry.box, scaleY: 1, alpha: 1, duration: 520, delay: 120, ease: "Back.Out" });
          const c = this.center(t.square);
          this.puffs(c.x, c.y + 26, FX.stone, 9, 140);
        }
      }
      entry.label.setText(String(turnsLeft(t.expires_at, view.ply)));
    }
    for (const [square, entry] of this.terrain) {
      if (wanted.has(square)) continue;
      this.terrain.delete(square);
      const { box } = entry;
      if (!animate) {
        box.destroy();
        continue;
      }
      const c = this.center(square);
      this.puffs(c.x, c.y + 26, FX.stone, 7);
      this.tweens.add({ targets: box, scaleY: 0.05, alpha: 0, duration: 420, ease: "Quad.In", onComplete: () => box.destroy() });
    }
  }

  private makeTerrain(t: Terrain): TerrainEntry {
    const { x, y } = this.cell(t.square);
    const box = this.add.container(x + TILE / 2, y + TILE - 2).setDepth(1.6);
    const owner = t.owner === "white" ? TEXT : 0x7a8191;
    const rim = this.add.graphics();
    rim.fillStyle(0x0e0f12, 0.35).fillRect(-TILE / 2 + 2, -TILE + 4, TILE - 4, TILE - 6);
    rim.lineStyle(3, owner, 0.85).strokeRect(-TILE / 2 + 3, -TILE + 5, TILE - 6, TILE - 8);
    const img = this.add.image(0, 0, ROCK_KEY).setOrigin(0.5, 1);
    const pill = this.add.graphics();
    pill.fillStyle(0x0e0f12, 0.85).fillCircle(TILE / 2 - 17, -TILE + 17, 11);
    pill.lineStyle(2, owner, 0.9).strokeCircle(TILE / 2 - 17, -TILE + 17, 11);
    const label = this.add.text(TILE / 2 - 17, -TILE + 17, "", { fontFamily: MONO, fontSize: "13px", color: "#e9ebef", fontStyle: "600" }).setOrigin(0.5);
    box.add([rim, img, pill, label]);
    return { box, label };
  }

  /** Pièges du joueur : runes discrètes (le serveur n'envoie que les siennes). */
  private reconcileTraps(view: StateView, animate: boolean, ctx: EventCtx) {
    const wanted = new Set(view.traps);
    for (const square of wanted) {
      if (this.traps.has(square)) continue;
      const { x, y } = this.center(square);
      const g = this.add.graphics().setDepth(1.2).setPosition(x, y);
      drawRune(g);
      this.traps.set(square, g);
      if (animate && ctx.trapSet.has(square)) {
        g.setScale(1.7).setAlpha(0);
        this.tweens.add({ targets: g, scale: 1, alpha: 0.8, duration: 420, ease: "Cubic.Out" });
        this.rings(x, y, FX.rune, 8, 40, 460);
      }
    }
    for (const [square, g] of this.traps) {
      if (wanted.has(square)) continue;
      this.traps.delete(square);
      g.destroy();
    }
  }

  // ---- marqueurs persistants (effets, copies temporaires) ----------------------

  private reconcileMarks(view: StateView) {
    const wanted = new Map<string, { piece: number; kind: Mark["kind"]; label: string | null }>();
    for (const effect of view.effects as ActiveEffect[]) {
      if (!this.sprites.has(effect.piece)) continue;
      const label = effect.kind === "vanish" ? String(turnsLeft(effect.expires_at, view.ply)) : null;
      wanted.set(`${effect.piece}:${effect.kind}`, { piece: effect.piece, kind: effect.kind, label });
    }
    for (const piece of view.board) {
      if (piece?.temp && this.sprites.has(piece.id)) wanted.set(`${piece.id}:temp`, { piece: piece.id, kind: "temp", label: null });
    }
    for (const [key, want] of wanted) {
      const existing = this.marks.get(key);
      if (existing) {
        existing.label?.setText(want.label ?? "");
        continue;
      }
      const sprite = this.sprites.get(want.piece)!;
      const mark = this.makeMark(want.kind, want.label);
      mark.box.setPosition(sprite.x, sprite.y).setAlpha(0);
      this.tweens.add({ targets: mark.box, alpha: 1, duration: 260 });
      this.marks.set(key, { ...mark, piece: want.piece, kind: want.kind });
    }
    for (const [key, mark] of this.marks) {
      if (wanted.has(key)) continue;
      this.tweens.killTweensOf(mark.box);
      mark.box.destroy();
      this.marks.delete(key);
    }
  }

  private makeMark(kind: Mark["kind"], label: string | null) {
    const g = this.add.graphics();
    if (kind === "temp") {
      drawDashedRing(g, FX.gold, 36, 18, 3);
      for (let i = 0; i < 4; i++) {
        const a = (Math.PI / 2) * i + Math.PI / 4;
        g.fillStyle(FX.gold, 0.9).fillCircle(Math.cos(a) * 36, Math.sin(a) * 36, 2.6);
      }
    } else {
      drawEffectMark(g, kind);
    }
    const box = this.add.container(0, 0, [g]).setDepth(11);
    let text: Phaser.GameObjects.Text | undefined;
    if (kind === "vanish") {
      const pill = this.add.graphics();
      pill.fillStyle(0x0e0f12, 0.88).fillCircle(26, -26, 10);
      pill.lineStyle(2, FX.glitch, 0.9).strokeCircle(26, -26, 10);
      text = this.add.text(26, -26, label ?? "", { fontFamily: MONO, fontSize: "12px", color: "#9fe7ff", fontStyle: "600" }).setOrigin(0.5);
      box.add([pill, text]);
    }
    return { box, label: text };
  }

  // ---- effets de compétences -------------------------------------------------

  private playEffects(view: StateView, ctx: EventCtx) {
    for (const e of view.events) {
      switch (e.type) {
        case "skill_used":
          this.castFlourish(e.skill, e.target, e.color);
          break;
        case "teleported": {
          // Portail : anneaux violets qui se referment au départ et s'ouvrent à l'arrivée.
          const a = this.center(e.from);
          const b = this.center(e.to);
          this.rings(a.x, a.y, FX.portal, 46, 6, 460);
          this.rings(a.x, a.y, FX.portal, 30, 4, 460, 80);
          this.rings(b.x, b.y, FX.portal, 6, 46, 520, 260);
          this.rings(b.x, b.y, FX.portal, 4, 30, 520, 340);
          break;
        }
        case "effect_added": {
          const sprite = this.sprites.get(e.piece);
          if (!sprite) break;
          const pos = this.placed.get(e.piece);
          const at = pos !== undefined ? this.center(pos) : { x: sprite.x, y: sprite.y };
          this.effectBurst(e.effect, at.x, at.y);
          break;
        }
        case "rolled_back":
          this.reverseTrail(e.from, e.to);
          break;
        case "cloned": {
          const a = this.center(e.from);
          this.rings(a.x, a.y, FX.clone, 14, 44, 420);
          this.rings(a.x, a.y, FX.clone, 8, 30, 420, 100);
          break;
        }
        case "swapped":
          this.destinyThread(e.a, e.b, ctx.skill === "transposition" ? FX.field : FX.thread);
          break;
        case "removed": {
          const c = this.center(e.square);
          this.rings(c.x, c.y, FX.dust, 10, 40, 380);
          break;
        }
        case "captured": {
          const c = this.center(e.square);
          this.rings(c.x, c.y, TEXT, 8, TILE * 0.6, 340, MOVE_MS * 0.6);
          break;
        }
        case "spawned": {
          const c = this.center(e.square);
          if (e.piece.wall) this.puffs(c.x, c.y + 24, FX.stone, 8);
          else if (e.piece.mirage) {
            this.rings(c.x, c.y, FX.mirage, 40, 8, 520);
            this.rings(c.x, c.y, FX.mirage, 6, 44, 520, 300);
          } else if (ctx.skill === "godhelp") {
            this.beam(e.square, FX.gold, 160);
            this.rings(c.x, c.y, FX.gold, 6, 56, 560, 560);
            this.cameraShake(180, 0.003, 600);
          } else if (ctx.skill === "terminator") {
            this.rings(c.x, c.y, FX.attack, 8, 48, 520, 300);
          } else {
            this.rings(c.x, c.y, FX.clone, 8, 44, 460);
          }
          break;
        }
        case "transformed": {
          const c = this.center(e.square);
          const color = ctx.skill === "evolve" ? FX.gold : FX.morph;
          this.rings(c.x, c.y, color, 10, 50, 560);
          this.rings(c.x, c.y, color, 6, 34, 560, 120);
          if (ctx.skill === "evolve") this.ascend(e.square, FX.gold);
          else this.swirl(c.x, c.y, FX.morph);
          break;
        }
        case "switched": {
          const c = this.center(e.square);
          this.rings(c.x, c.y, FX.attack, 44, 8, 420);
          this.rings(c.x, c.y, ACCENT, 8, 46, 480, 260);
          break;
        }
        case "rotated":
          this.tornadoSwirl();
          break;
        case "trap_sprung":
          this.trapBurst(e.square);
          break;
        case "benched": {
          this.beam(e.square, FX.trail, 0);
          const c = this.center(e.square);
          this.rings(c.x, c.y, FX.trail, 40, 8, 420);
          break;
        }
        case "unbenched": {
          const c = this.center(e.square);
          this.beam(e.square, FX.trail, 0);
          this.rings(c.x, c.y, FX.trail, 6, 46, 520, 380);
          break;
        }
        case "pushed": {
          const hit = this.center(e.from);
          this.hexBurst(hit.x, hit.y, 180);
          this.cameraShake(160, 0.004, 200);
          break;
        }
        case "saved": {
          const a = this.center(e.from);
          const b = this.center(e.to);
          this.haloBurst(a.x, a.y);
          this.beam(e.to, FX.gold, 360);
          this.rings(b.x, b.y, FX.gold, 6, 52, 560, 520);
          break;
        }
        case "cancelled":
          this.rewind();
          break;
        case "terrain":
          this.cameraShake(240, 0.004, 100);
          break;
        case "global_effect": {
          // Armistice, brouillard, silence : un voile de couleur sur tout l'échiquier.
          const color = e.effect === "truce" ? FX.shield : e.effect === "fog" ? FX.dust : FX.glitch;
          this.flash(color, e.effect === "fog" ? 0.3 : 0.18, 640);
          const mid = SIZE / 2;
          this.rings(mid, mid, color, 20, SIZE * 0.55, 720);
          break;
        }
        case "vanished": {
          const c = this.center(e.square);
          this.rings(c.x, c.y, FX.glitch, 8, 42, 360);
          this.rings(c.x, c.y, FX.glitchAlt, 6, 34, 360, 90);
          break;
        }
        case "loan_ended": {
          const c = this.center(e.square);
          this.puppetSnap(c.x, c.y);
          this.rings(c.x, c.y, FX.puppet, 42, 8, 420);
          break;
        }
        default:
          break;
      }
    }
  }

  /** Effet distinct à chaque lancement de compétence, en plus de ce que racontent les événements. */
  private castFlourish(skill: SkillId, target: SkillTarget, caster: Color) {
    const sq = target.kind === "piece" || target.kind === "square" || target.kind === "spawn" ? target.square : null;
    const at = sq !== null ? this.center(sq) : null;
    switch (skill) {
      case "wall": {
        const rank = caster === "white" ? 2 : 5;
        for (let f = 0; f < 8; f++) {
          const c = this.center(rank * 8 + f);
          this.puffs(c.x, c.y + 22, FX.stone, 3, f * 45);
        }
        this.cameraShake(260, 0.003, 120);
        break;
      }
      case "mirage":
        if (at) this.rings(at.x, at.y, FX.mirage, 50, 10, 460);
        break;
      case "evolve":
        if (sq !== null) this.ascend(sq, FX.gold);
        break;
      case "switch":
        if (at) {
          this.rings(at.x, at.y, FX.attack, 56, 12, 420);
          this.rings(at.x, at.y, FX.attack, 40, 8, 420, 80);
        }
        break;
      case "mind":
        this.sweepLine();
        break;
      case "control":
        if (at) this.puppetDrop(at.x, at.y);
        break;
      case "morph":
        if (at) this.swirl(at.x, at.y, FX.morph);
        break;
      case "canceller":
        // Le rembobinage est joué par l'événement `cancelled`.
        break;
      case "tornado":
        // Le tourbillon est joué par l'événement `rotated`.
        break;
      case "invisibility":
        if (at) this.scan(at.x, at.y, FX.mirage);
        break;
      case "terminator":
        if (sq !== null) this.mirrorLine(sq);
        break;
      case "trap":
        if (at) this.rings(at.x, at.y, FX.rune, 40, 8, 380);
        break;
      case "bench":
        break;
      case "forcefield":
        break;
      case "transposition":
        break;
      case "queensac":
        this.flash(DANGER, 0.22, 520);
        this.cameraShake(260, 0.005, 0);
        break;
      case "temporal":
        if (at) this.clockSweep(at.x, at.y);
        break;
      case "geomancy":
        if (at) this.rings(at.x, at.y, FX.stone, 10, 80, 520);
        break;
      case "celestial":
        if (sq !== null) this.ascend(sq, FX.gold);
        break;
      case "godhelp":
        break;
      default:
        if (isForgedId(skill)) this.forgedFlourish(skill, at);
        break;
    }
  }

  /** Éclat d'une compétence forgée : ses anneaux prennent la couleur de sa famille, l'or pour une légendaire. */
  private forgedFlourish(skill: SkillId, at: { x: number; y: number } | null) {
    const entry = skillEntry(skill);
    const family: Record<string, number> = { attack: FX.attack, defense: FX.shield, mobility: FX.trail, control: FX.thread, create: FX.clone };
    const color = entry.rarity === "legendary" ? FX.gold : (family[entry.family] ?? FX.thread);
    const mid = SIZE / 2;
    const { x, y } = at ?? { x: mid, y: mid };
    this.rings(x, y, color, 10, at ? 54 : SIZE * 0.45, 480);
    this.rings(x, y, color, 6, at ? 36 : SIZE * 0.3, 480, 90);
    if (entry.rarity === "legendary") {
      this.flash(FX.gold, 0.16, 560);
      this.cameraShake(240, 0.003, 100);
    } else if (!at) {
      this.flash(color, 0.1, 420);
    }
  }

  /** Éclat de pose d'un effet persistant. */
  private effectBurst(kind: EffectKind, x: number, y: number) {
    switch (kind) {
      case "immune":
        this.shieldBurst(x, y);
        break;
      case "frozen":
        this.crystalBurst(x, y, FX.ice);
        break;
      case "locked":
        this.crystalBurst(x, y, FX.stone);
        break;
      case "forcefield":
        this.hexBurst(x, y);
        break;
      case "celestial":
        this.haloBurst(x, y);
        break;
      case "morphed":
        this.swirl(x, y, FX.morph);
        break;
      case "color_loan":
        this.rings(x, y, FX.puppet, 50, 14, 460);
        break;
      case "invisible":
        this.scan(x, y, FX.mirage);
        break;
      case "vanish":
        this.rings(x, y, FX.glitch, 6, 36, 360);
        break;
      default:
        this.rings(x, y, effectColor(kind), 10, 44, 420);
    }
  }

  /** Anneau qui s'élargit (ou se referme) en s'effaçant. */
  private rings(x: number, y: number, color: number, r0: number, r1: number, duration: number, delay = 0) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y).setAlpha(0);
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration,
      delay,
      ease: "Cubic.Out",
      onStart: () => g.setAlpha(1),
      onUpdate: () => {
        const r = r0 + (r1 - r0) * state.t;
        g.clear().lineStyle(4 * (1 - state.t) + 1, color, 0.9 * (1 - state.t * 0.85)).strokeCircle(0, 0, r);
        g.fillStyle(color, 0.16 * (1 - state.t)).fillCircle(0, 0, r);
      },
      onComplete: () => g.destroy(),
    });
  }

  /** Éclats de poussière qui montent (pierre, terre). */
  private puffs(x: number, y: number, color: number, count: number, delay = 0) {
    for (let i = 0; i < count; i++) {
      const size = 3 + Math.random() * 5;
      const dust = this.add.rectangle(x + (Math.random() - 0.5) * 50, y + (Math.random() - 0.5) * 14, size, size, color).setDepth(12).setAlpha(0);
      this.tweens.add({
        targets: dust,
        x: dust.x + (Math.random() - 0.5) * 40,
        y: dust.y - 14 - Math.random() * 34,
        alpha: { from: 0.9, to: 0 },
        angle: Math.random() * 160,
        duration: 520 + Math.random() * 380,
        delay: delay + Math.random() * 140,
        ease: "Cubic.Out",
        onStart: () => dust.setAlpha(0.9),
        onComplete: () => dust.destroy(),
      });
    }
  }

  private cameraShake(duration: number, intensity: number, delay: number) {
    if (this.theme.reduceMotion) return;
    this.time.delayedCall(delay, () => {
      if (this.sys.isActive()) this.cameras.main.shake(duration, intensity);
    });
  }

  private flash(color: number, alpha: number, duration: number) {
    const r = this.add.rectangle(SIZE / 2, SIZE / 2, SIZE, SIZE, color, alpha).setDepth(13);
    this.tweens.add({ targets: r, alpha: 0, duration, onComplete: () => r.destroy() });
  }

  private shieldBurst(x: number, y: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    drawShield(g, 1.5);
    g.setScale(0.4).setAlpha(0);
    this.tweens.add({ targets: g, scale: 1.25, alpha: { from: 1, to: 0 }, duration: 620, ease: "Cubic.Out", onComplete: () => g.destroy() });
    this.rings(x, y, FX.shield, 18, 54, 520);
  }

  private crystalBurst(x: number, y: number, color: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 640,
      ease: "Cubic.Out",
      onUpdate: () => {
        g.clear().lineStyle(3, color, 1 - state.t * 0.9);
        for (let i = 0; i < 6; i++) {
          const a = (Math.PI / 3) * i;
          const r0 = 10 + state.t * 14;
          const r1 = 22 + state.t * 28;
          g.lineBetween(Math.cos(a) * r0, Math.sin(a) * r0, Math.cos(a) * r1, Math.sin(a) * r1);
          const b = a + 0.5;
          g.lineBetween(Math.cos(a) * r1, Math.sin(a) * r1, Math.cos(b) * (r1 - 8), Math.sin(b) * (r1 - 8));
        }
      },
      onComplete: () => g.destroy(),
    });
  }

  /** Force Field : un anneau hexagonal qui s'élargit. */
  private hexBurst(x: number, y: number, delay = 0) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y).setAlpha(0);
    drawHexRing(g, 38);
    g.setScale(0.5);
    this.tweens.add({
      targets: g,
      scale: 1.6,
      alpha: { from: 1, to: 0 },
      angle: 30,
      duration: 640,
      delay,
      ease: "Cubic.Out",
      onStart: () => g.setAlpha(1),
      onComplete: () => g.destroy(),
    });
  }

  /** Celestial : halo doré qui s'épanouit. */
  private haloBurst(x: number, y: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    drawHalo(g, 34);
    g.setScale(0.4).setAlpha(0);
    this.tweens.add({ targets: g, scale: 1.5, angle: 40, alpha: { from: 1, to: 0 }, duration: 820, ease: "Cubic.Out", onComplete: () => g.destroy() });
    this.rings(x, y, FX.gold, 16, 60, 620);
  }

  /** Colonne de lumière qui tombe du haut du plateau jusqu'à une case. */
  private beam(square: Square, color: number, delay: number) {
    const { x, y } = this.center(square);
    const height = Math.max(40, y - FRAME + TILE / 2);
    const rect = this.add.rectangle(x, y + TILE / 2, 46, height, color, 0).setOrigin(0.5, 1).setDepth(11).setBlendMode(Phaser.BlendModes.ADD);
    this.tweens.add({
      targets: rect,
      fillAlpha: { from: 0, to: 0.55 },
      scaleX: { from: 0.4, to: 1 },
      duration: 240,
      delay,
      yoyo: true,
      hold: 140,
      ease: "Cubic.Out",
      onComplete: () => rect.destroy(),
    });
  }

  /** Chevrons qui s'élèvent au-dessus d'une case. */
  private ascend(square: Square, color: number) {
    const { x, y } = this.center(square);
    for (let i = 0; i < 3; i++) {
      const g = this.add.graphics().setDepth(12).setPosition(x, y + 18).setAlpha(0);
      g.lineStyle(4, color, 1).beginPath().moveTo(-12, 6).lineTo(0, -6).lineTo(12, 6).strokePath();
      this.tweens.add({
        targets: g,
        y: y - 46,
        alpha: { from: 1, to: 0 },
        duration: 620,
        delay: i * 130,
        onStart: () => g.setAlpha(1),
        onComplete: () => g.destroy(),
      });
    }
  }

  /** Points qui spiralent vers le centre d'une pièce (Morph). */
  private swirl(x: number, y: number, color: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 760,
      ease: "Cubic.InOut",
      onUpdate: () => {
        g.clear();
        for (let i = 0; i < 10; i++) {
          const a = (Math.PI * 2 * i) / 10 + state.t * Math.PI * 3;
          const r = 42 * (1 - state.t) + 6;
          g.fillStyle(color, 1 - state.t * 0.8).fillCircle(Math.cos(a) * r, Math.sin(a) * r, 3.2);
        }
      },
      onComplete: () => g.destroy(),
    });
  }

  /** Balayage horizontal qui monte le long d'une pièce (Invisibility). */
  private scan(x: number, y: number, color: number) {
    const line = this.add.rectangle(x, y + 34, 58, 3, color, 0.9).setDepth(12);
    this.tweens.add({ targets: line, y: y - 34, alpha: 0.15, duration: 560, ease: "Sine.InOut", yoyo: true, onComplete: () => line.destroy() });
    this.rings(x, y, color, 30, 46, 560);
  }

  /** Ligne verticale qui traverse tout le plateau (Mind Reading). */
  private sweepLine() {
    const mid = FRAME + BOARD_PX / 2;
    const glow = this.add.rectangle(FRAME, mid, 120, BOARD_PX, ACCENT, 0.14).setOrigin(1, 0.5).setDepth(12);
    const line = this.add.rectangle(FRAME, mid, 4, BOARD_PX, ACCENT, 0.9).setDepth(12);
    this.tweens.add({
      targets: [glow, line],
      x: FRAME + BOARD_PX + 120,
      duration: 820,
      ease: "Sine.InOut",
      onComplete: () => {
        glow.destroy();
        line.destroy();
      },
    });
  }

  /** Mind Control : les fils d'une marionnette descendent sur la pièce visée. */
  private puppetDrop(x: number, y: number) {
    const g = this.add.graphics().setDepth(12);
    drawStrings(g);
    g.setPosition(x, y - 70).setAlpha(0);
    this.tweens.add({ targets: g, y, alpha: 1, duration: 420, ease: "Cubic.Out" });
    this.tweens.add({ targets: g, alpha: 0, duration: 520, delay: 900, onComplete: () => g.destroy() });
    this.rings(x, y, FX.puppet, 48, 14, 520, 260);
  }

  /** Fin du prêt de Mind Control : les fils se coupent et retombent. */
  private puppetSnap(x: number, y: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    drawStrings(g);
    this.tweens.add({ targets: g, y: y + 46, alpha: 0, angle: 12, duration: 620, ease: "Quad.In", onComplete: () => g.destroy() });
  }

  /** Piège déclenché : éclat d'épines orange. */
  private trapBurst(square: Square) {
    const { x, y } = this.center(square);
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 560,
      ease: "Cubic.Out",
      onUpdate: () => {
        g.clear().lineStyle(4 * (1 - state.t) + 1, FX.rune, 1 - state.t * 0.85);
        for (let i = 0; i < 10; i++) {
          const a = (Math.PI / 5) * i;
          const long = i % 2 === 0;
          const r0 = 12 + state.t * 10;
          const r1 = (long ? 40 : 28) + state.t * 22;
          g.lineBetween(Math.cos(a) * r0, Math.sin(a) * r0, Math.cos(a) * r1, Math.sin(a) * r1);
        }
        g.strokeCircle(0, 0, 14 + state.t * 24);
      },
      onComplete: () => g.destroy(),
    });
    this.rings(x, y, FX.rune, 10, 54, 480);
  }

  /** Tornado : trois bras de spirale qui balaient le plateau. */
  private tornadoSwirl() {
    const c = this.boardCenter();
    const g = this.add.graphics().setDepth(12).setPosition(c.x, c.y);
    for (let arm = 0; arm < 3; arm++) {
      g.lineStyle(6, FX.trail, 0.7).beginPath();
      for (let r = 14; r <= 270; r += 6) {
        const a = (Math.PI * 2 * arm) / 3 + r / 52;
        g.lineTo(Math.cos(a) * r, Math.sin(a) * r);
      }
      g.strokePath();
    }
    g.setScale(0.3).setAlpha(0);
    this.tweens.add({
      targets: g,
      angle: 380,
      scale: 1.1,
      alpha: { from: 0.9, to: 0 },
      duration: 1100,
      ease: "Cubic.Out",
      onStart: () => g.setAlpha(0.9),
      onComplete: () => g.destroy(),
    });
  }

  /** Canceller : des anneaux se referment sur le centre, comme un rembobinage. */
  private rewind() {
    const c = this.boardCenter();
    for (let i = 0; i < 3; i++) this.rings(c.x, c.y, FX.portal, 300 - i * 40, 12, 720, i * 140);
    this.flash(FX.portal, 0.1, 700);
  }

  /** Temporal Distortion : une aiguille remonte le temps autour de la pièce. */
  private clockSweep(x: number, y: number) {
    const g = this.add.graphics().setDepth(12).setPosition(x, y);
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 780,
      ease: "Cubic.InOut",
      onUpdate: () => {
        const a = -Math.PI / 2 - state.t * Math.PI * 2;
        g.clear().lineStyle(3, FX.trail, 1 - state.t * 0.7).strokeCircle(0, 0, 38);
        g.lineBetween(0, 0, Math.cos(a) * 34, Math.sin(a) * 34);
        g.beginPath().arc(0, 0, 44, -Math.PI / 2, a, true).strokePath();
        for (let i = 0; i < 12; i++) {
          const t = (Math.PI / 6) * i;
          g.lineBetween(Math.cos(t) * 38, Math.sin(t) * 38, Math.cos(t) * 42, Math.sin(t) * 42);
        }
      },
      onComplete: () => g.destroy(),
    });
  }

  /** Terminator : réticule sur la cible et ligne pointillée vers sa case miroir. */
  private mirrorLine(square: Square) {
    const a = this.center(square);
    const mirror = (7 - Math.floor(square / 8)) * 8 + (square % 8);
    const b = this.center(mirror);
    const g = this.add.graphics().setDepth(12);
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    const steps = Math.max(1, Math.floor(len / 14));
    g.lineStyle(3, FX.attack, 0.9);
    for (let i = 0; i < steps; i += 2) {
      const t0 = i / steps;
      const t1 = Math.min(1, (i + 1) / steps);
      g.lineBetween(a.x + (b.x - a.x) * t0, a.y + (b.y - a.y) * t0, a.x + (b.x - a.x) * t1, a.y + (b.y - a.y) * t1);
    }
    g.lineStyle(3, FX.attack, 0.95).strokeCircle(a.x, a.y, 30).strokeCircle(b.x, b.y, 30);
    this.tweens.add({ targets: g, alpha: 0, duration: 520, delay: 520, onComplete: () => g.destroy() });
    this.rings(a.x, a.y, FX.attack, 54, 22, 420);
  }

  /** Rollback : chevrons qui remontent le trajet de la pièce, du point d'arrivée vers l'origine du trajet. */
  private reverseTrail(from: Square, to: Square) {
    const a = this.center(from);
    const b = this.center(to);
    const angle = Math.atan2(b.y - a.y, b.x - a.x);
    const steps = 5;
    for (let i = 0; i < steps; i++) {
      const t = 1 - i / (steps - 1); // du bout (to) vers le début (from) : sens inverse
      const g = this.add.graphics().setDepth(11).setPosition(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t).setRotation(angle).setAlpha(0);
      g.lineStyle(4, FX.trail, 1).beginPath().moveTo(-7, -9).lineTo(5, 0).lineTo(-7, 9).strokePath();
      this.tweens.add({
        targets: g,
        alpha: { from: 0.9, to: 0 },
        duration: 520,
        delay: i * 90,
        onStart: () => g.setAlpha(0.9),
        onComplete: () => g.destroy(),
      });
    }
    const line = this.add.graphics().setDepth(1.5);
    line.lineStyle(10, FX.trail, 0.25).lineBetween(a.x, a.y, b.x, b.y);
    this.tweens.add({ targets: line, alpha: 0, duration: 700, onComplete: () => line.destroy() });
  }

  /** Destiny Swapper / Transposition : deux fils croisés relient les pièces échangées. */
  private destinyThread(sqA: Square, sqB: Square, color: number) {
    const a = this.center(sqA);
    const b = this.center(sqB);
    const mx = (a.x + b.x) / 2;
    const my = (a.y + b.y) / 2;
    const nx = -(b.y - a.y);
    const ny = b.x - a.x;
    const len = Math.hypot(nx, ny) || 1;
    const bow = 38;
    const g = this.add.graphics().setDepth(11);
    const draw = (sign: number, progress: number) => {
      const cx = mx + (nx / len) * bow * sign;
      const cy = my + (ny / len) * bow * sign;
      g.beginPath().moveTo(a.x, a.y);
      const steps = 24;
      for (let i = 1; i <= steps * progress; i++) {
        const t = i / steps;
        const x = (1 - t) * (1 - t) * a.x + 2 * (1 - t) * t * cx + t * t * b.x;
        const y = (1 - t) * (1 - t) * a.y + 2 * (1 - t) * t * cy + t * t * b.y;
        g.lineTo(x, y);
      }
      g.strokePath();
    };
    const state = { t: 0 };
    this.tweens.add({
      targets: state,
      t: 1,
      duration: 380,
      onUpdate: () => {
        g.clear().lineStyle(3, color, 0.95);
        draw(1, state.t);
        draw(-1, state.t);
      },
      onComplete: () => this.tweens.add({ targets: g, alpha: 0, duration: 420, onComplete: () => g.destroy() }),
    });
    this.rings(a.x, a.y, color, 10, 36, 420, 200);
    this.rings(b.x, b.y, color, 10, 36, 420, 200);
  }

  /** Remover : la pièce se désagrège en poussière. */
  private dissolve(sprite: Sprite) {
    const { x, y } = sprite;
    this.tweens.add({ targets: sprite, alpha: 0, scale: PIECE_SCALE * 1.1, duration: 380, ease: "Quad.In", onComplete: () => sprite.destroy() });
    for (let i = 0; i < 16; i++) {
      const size = 3 + Math.random() * 4;
      const dust = this.add.rectangle(x + (Math.random() - 0.5) * 36, y + (Math.random() - 0.5) * 48, size, size, FX.dust).setDepth(12).setAlpha(0.9);
      this.tweens.add({
        targets: dust,
        x: dust.x + (Math.random() - 0.5) * 60,
        y: dust.y - 20 - Math.random() * 46,
        alpha: 0,
        angle: Math.random() * 180,
        duration: 600 + Math.random() * 400,
        delay: Math.random() * 160,
        ease: "Cubic.Out",
        onComplete: () => dust.destroy(),
      });
    }
  }

  /** Fin d'une pièce éphémère : disparition glitchée (décalages, teintes, tranches). */
  private glitchOut(sprite: Sprite) {
    const { x, y } = sprite;
    let n = 0;
    sprite.setDepth(10);
    this.time.addEvent({
      delay: 55,
      repeat: 9,
      callback: () => {
        n++;
        if (!sprite.active) return;
        if (n >= 10) {
          sprite.destroy();
          return;
        }
        sprite.setPosition(x + (Math.random() - 0.5) * 16, y + (Math.random() - 0.5) * 6);
        sprite.setAlpha(Math.random() < 0.5 ? 0.18 : 0.9);
        sprite.setTint(n % 2 ? FX.glitch : FX.glitchAlt);
        sprite.setScale(PIECE_SCALE * (1 + (Math.random() - 0.5) * 0.22), PIECE_SCALE * (1 + (Math.random() - 0.5) * 0.12));
        const bar = this.add
          .rectangle(x + (Math.random() - 0.5) * 20, y + (Math.random() - 0.5) * 60, 36 + Math.random() * 30, 3, n % 2 ? FX.glitchAlt : FX.glitch, 0.8)
          .setDepth(12);
        this.tweens.add({ targets: bar, alpha: 0, duration: 160, onComplete: () => bar.destroy() });
      },
    });
  }
}

function touchedSquares(e: GameEvent): Square[] {
  switch (e.type) {
    case "moved":
    case "teleported":
    case "rolled_back":
      return [e.from, e.to];
    case "cloned":
      return [e.to];
    case "spawned":
    case "unbenched":
    case "switched":
    case "transformed":
      return [e.square];
    case "swapped":
      return [e.a, e.b];
    case "removed":
    case "benched":
    case "vanished":
    case "loan_ended":
      return [e.square];
    case "pushed":
      return [e.from, e.to];
    case "saved":
      return [e.to];
    case "trap_sprung":
      return [e.square];

    default:
      return [];
  }
}

export { BOARD_PX };
