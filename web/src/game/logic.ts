// Logique pure de l'écran de jeu (horloges, matériel, journal), sans React ni Phaser.

import { t } from "../i18n";
import { skillName } from "../skills";
import { NO_PIECE, type Clock, type Color, type EffectKind, type GameEvent, type HistoryEntry, type Piece, type PieceKind, type Square, type StateView } from "../protocol";

// ---- horloges -----------------------------------------------------------------

/** Temps restant de `color` après `elapsedMs` écoulées depuis la réception de `clock`. */
export function remainingMs(clock: Clock, color: Color, elapsedMs: number): number {
  const base = color === "white" ? clock.white_ms : clock.black_ms;
  const left = clock.running === color ? base - Math.max(0, elapsedMs) : base;
  return Math.max(0, Math.round(left));
}

/** `9:59`, `0:42`, et le dixième de seconde sous 10 s (`0:07.3`). */
export function formatClock(ms: number): string {
  const total = Math.max(0, ms);
  const tenths = Math.floor(total / 100) % 10;
  const seconds = Math.floor(total / 1000);
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  const base = `${m}:${String(s).padStart(2, "0")}`;
  return total < 10_000 ? `${base}.${tenths}` : base;
}

// ---- matériel -----------------------------------------------------------------

export const PIECE_VALUE: Record<PieceKind, number> = { pawn: 1, knight: 3, bishop: 3, rook: 5, queen: 9, king: 0 };

const START_COUNT: Record<PieceKind, number> = { pawn: 8, knight: 2, bishop: 2, rook: 2, queen: 1, king: 1 };

export const CAPTURE_ORDER: PieceKind[] = ["queen", "rook", "bishop", "knight", "pawn"];

/** Pièces comptées dans le matériel : ni mirages ni copies temporaires. */
const counts = (p: Piece | null): p is Piece => !!p && !p.mirage && !p.temp;

/** `benched` : pièces de `color` mises de côté (Bench), qui comptent encore comme matériel. */
export function materialOf(board: (Piece | null)[], color: Color, benched: Piece[] = []): number {
  return [...board, ...benched].reduce((sum, p) => (counts(p) && p.color === color ? sum + PIECE_VALUE[p.kind] : sum), 0);
}

/** Avantage matériel de `color` (positif = en avance). `benched` : le banc de `color`. */
export function materialBalance(board: (Piece | null)[], color: Color, benched: Piece[] = []): number {
  return materialOf(board, color, benched) - materialOf(board, color === "white" ? "black" : "white");
}

/** Part (0..1) de la barre d'évaluation occupée par `color` : sigmoïde douce autour de 0,5. */
export function evalShare(balance: number): number {
  return 1 / (1 + Math.exp(-balance / 4));
}

/**
 * Pièces de `color` absentes de l'échiquier, de la plus forte à la plus faible.
 * Les pièces gagnées par promotion compensent les pions manquants.
 */
export function capturedPieces(board: (Piece | null)[], color: Color, benched: Piece[] = []): PieceKind[] {
  const count: Record<PieceKind, number> = { pawn: 0, knight: 0, bishop: 0, rook: 0, queen: 0, king: 0 };
  for (const p of [...board, ...benched]) if (counts(p) && p.color === color) count[p.kind]++;
  const lost = {} as Record<PieceKind, number>;
  let promoted = 0;
  for (const kind of CAPTURE_ORDER) {
    lost[kind] = Math.max(0, START_COUNT[kind] - count[kind]);
    if (kind !== "pawn") promoted += Math.max(0, count[kind] - START_COUNT[kind]);
  }
  lost.pawn = Math.max(0, lost.pawn - promoted);
  return CAPTURE_ORDER.flatMap((kind) => Array<PieceKind>(lost[kind]).fill(kind));
}

// ---- journal ------------------------------------------------------------------

/** Nom d'une pièce dans la langue courante ; `lower` : forme de milieu de phrase (l'allemand garde la majuscule). */
export function pieceName(kind: PieceKind, lower = false): string {
  return t(`game.${lower ? "piece_lc_" : "piece_"}${kind}`);
}

export function sqName(square: Square): string {
  return `${"abcdefgh"[square % 8]}${Math.floor(square / 8) + 1}`;
}

export interface LogLine {
  /** Numéro de demi-coup (`ply` de la position obtenue). */
  ply: number;
  actor: Color;
  kind: "move" | "skill";
  text: string;
  skill?: string;
  /** Identifiant de l'action (voir `actionKey`) : Mind Reading/Control laissent le `ply` inchangé. */
  key?: string;
}

/** Qui a joué : l'auteur de `skill_used` si présent, sinon l'inverse du trait actuel. */
export function actorOf(view: Pick<StateView, "events" | "to_move">): Color {
  for (const e of view.events) if (e.type === "skill_used") return e.color;
  return view.to_move === "white" ? "black" : "white";
}

/** Une ligne de journal pour la dernière action de `view`, ou `null` si elle n'a rien produit. */
export function describeAction(view: Pick<StateView, "events" | "board" | "ply" | "to_move">): LogLine | null {
  const { events, board } = view;
  if (events.length === 0) return null;
  const actor = actorOf(view);
  const skillEvent = events.find((e): e is Extract<GameEvent, { type: "skill_used" }> => e.type === "skill_used");
  const parts: string[] = [];
  const spawns = events.filter((e) => e.type === "spawned").length;
  let spawnNoted = false;
  for (const e of events) {
    // Wall fait surgir plusieurs pions d'un coup : une seule mention.
    if (e.type === "spawned" && spawns > 1) {
      if (!spawnNoted) parts.push(t("game.log_spawn_many", { count: spawns }));
      spawnNoted = true;
      continue;
    }
    parts.push(...describeEvent(e, board, skillEvent !== undefined));
  }
  const text = [...new Set(parts.filter(Boolean))].join(" · ");
  if (!text && !skillEvent) return null;
  return {
    ply: view.ply,
    actor,
    kind: skillEvent ? "skill" : "move",
    text: skillEvent ? `${skillName(skillEvent.skill)}${text ? ` · ${text}` : ""}` : text,
    skill: skillEvent?.skill,
  };
}

function describeEvent(e: GameEvent, board: (Piece | null)[], inSkill: boolean): string[] {
  const lc = (kind: PieceKind) => pieceName(kind, true);
  switch (e.type) {
    case "moved": {
      const kind = board[e.to]?.kind ?? "pawn";
      return [t("game.log_move", { piece: pieceName(kind), from: sqName(e.from), to: sqName(e.to) })];
    }
    case "captured":
      return [t("game.log_capture", { piece: lc(e.piece.kind) })];
    case "promoted":
      return [t("game.log_promotion", { piece: lc(e.to) })];
    case "castled":
      return [t("game.log_castle")];
    case "teleported":
      return [t("game.log_teleport", { from: sqName(e.from), to: sqName(e.to) })];
    case "cloned":
      return [t("game.log_clone", { from: sqName(e.from), to: sqName(e.to) })];
    case "swapped":
      return [t("game.log_swap", { a: sqName(e.a), b: sqName(e.b) })];
    case "removed":
      return [t("game.log_removed", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "rolled_back":
      return [t("game.log_rollback", { from: sqName(e.from), to: sqName(e.to) })];
    case "effect_added":
      return [effectName(e.effect) ?? t("game.effect_default")];
    case "skill_used":
      return inSkill ? [] : [skillName(e.skill)];
    case "spawned":
      return [t("game.log_spawned", { piece: pieceLabel(e.piece), sq: sqName(e.square) })];
    case "transformed":
      return [t("game.log_transformed", { sq: sqName(e.square), piece: lc(e.kind) })];
    case "switched":
      return [t("game.log_switched", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "rotated":
      return [t("game.log_rotated", { count: e.moves.length })];
    case "trap_set":
      return [t("game.log_trap_set", { sq: sqName(e.square) })];
    case "trap_sprung":
      return [t("game.log_trap_sprung", { sq: sqName(e.square) })];
    case "benched":
      return [t("game.log_benched", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "unbenched":
      return [t("game.log_unbenched", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "pushed":
      return [t("game.log_pushed", { from: sqName(e.from), to: sqName(e.to) })];
    case "saved":
      return [t("game.log_saved", { sq: sqName(e.to) })];
    case "best_move":
      return [
        e.promo
          ? t("game.log_best_move_promo", { from: sqName(e.from), to: sqName(e.to), piece: lc(e.promo) })
          : t("game.log_best_move", { from: sqName(e.from), to: sqName(e.to) }),
      ];
    case "cancelled":
      return [t("game.log_cancelled", { skill: skillName(e.skill) })];
    case "terrain":
      return [t("game.log_terrain", { count: e.squares.length })];
    case "global_effect":
      return [effectName(e.effect) ?? ""];
    case "vanished":
      return [t("game.log_vanished", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "ambushed":
      return [t("game.log_ambushed", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    case "loan_ended":
      return [t("game.log_loan_ended", { piece: lc(e.piece.kind), sq: sqName(e.square) })];
    default:
      return [];
  }
}

const EFFECT_KINDS: readonly EffectKind[] = ["immune", "frozen", "invisible", "forcefield", "celestial", "locked", "morphed", "color_loan", "vanish", "truce", "fog", "silenced", "domain"];

/** Libellé d'un effet, ou `undefined` pour un type inconnu. */
function effectName(kind: EffectKind): string | undefined {
  return EFFECT_KINDS.includes(kind) ? t(`game.effect_${kind}`) : undefined;
}

function pieceLabel(p: Piece): string {
  const base = pieceName(p.kind);
  return p.mirage ? t("game.label_mirage", { piece: base }) : p.wall ? t("game.label_wall", { piece: base }) : p.temp ? t("game.label_temp", { piece: base }) : base;
}

/**
 * Empreinte de la dernière action : change à chaque coup ou compétence, y compris Mind Reading
 * et Mind Control qui laissent le `ply` inchangé, mais pas quand le serveur renvoie la même position
 * (reprise après reconnexion).
 */
export function actionKey(view: Pick<StateView, "ply" | "events" | "my_skills">): string {
  const uses = view.my_skills.reduce((n, s) => n + (s.uses ?? (s.used ? 1 : 0)), 0);
  return `${view.ply}|${uses}|${JSON.stringify(view.events)}`;
}

/** Durée restante d'un effet ou d'un terrain, en tours complets (un tour = 2 demi-coups). */
export function turnsLeft(expiresAt: number, ply: number): number {
  return Math.max(0, Math.ceil((expiresAt - ply) / 2));
}

/** Un effet de partie entière (armistice, brouillard, silence), pour le bandeau de la partie. */
export interface Ambient {
  kind: EffectKind;
  label: string;
  /** Intitulé court (avant les deux-points du libellé). */
  name: string;
  /** Tours complets restants. */
  turns: number;
}

/** Les effets qui ne concernent aucune pièce, tels que `me` les vit. */
export function ambientEffects(view: Pick<StateView, "effects" | "ply" | "you">): Ambient[] {
  const out: Ambient[] = [];
  for (const e of view.effects ?? []) {
    if (e.piece !== NO_PIECE) continue;
    const turns = turnsLeft(e.expires_at, view.ply);
    if (turns === 0) continue;
    const mine = e.owner === view.you;
    if (e.kind === "truce") out.push({ kind: e.kind, label: t("game.amb_truce"), name: t("game.amb_truce_name"), turns });
    else if (e.kind === "fog") out.push({ kind: e.kind, label: t("game.amb_fog"), name: t("game.amb_fog_name"), turns });
    else if (e.kind === "domain") {
      out.push({
        kind: e.kind,
        label: t(mine ? "game.amb_domain_own" : "game.amb_domain_opp"),
        name: t(mine ? "game.amb_domain_own_name" : "game.amb_domain_opp_name"),
        turns,
      });
    } else if (e.kind === "silenced") {
      out.push({ kind: e.kind, label: t(mine ? "game.amb_silenced_own" : "game.amb_silenced_opp"), name: t("game.amb_silenced_name"), turns });
    }
  }
  return out;
}

/** Compétence lancée lors de la dernière action, s'il y en a une. */
export function launchOf(view: Pick<StateView, "events">): { color: Color; skill: string } | null {
  for (const e of view.events) if (e.type === "skill_used") return { color: e.color, skill: e.skill };
  return null;
}

/**
 * Journal reconstruit à partir de l'historique envoyé à la reprise de la partie (rechargement de la page).
 * Chaque ligne reçoit une clé propre à sa place dans l'historique : Mind Reading/Control laissent le `ply`
 * inchangé, deux lignes peuvent donc partager le même demi-coup.
 */
export function logFromHistory(history: HistoryEntry[], max = 200): LogLine[] {
  const lines: LogLine[] = [];
  history.forEach((entry, i) => {
    // `describeAction` ne lit que les cases d'arrivée des coups : un plateau creux suffit.
    const board: (Piece | null)[] = Array.from({ length: 64 }, () => null);
    const mover = entry.to_move === "white" ? "black" : "white";
    for (const { square, kind } of entry.landed) board[square] = { id: -1, kind, color: mover };
    const line = describeAction({ events: entry.events, board, ply: entry.ply, to_move: entry.to_move });
    if (line) lines.push({ ...line, key: `h${i}` });
  });
  return lines.slice(-max);
}

/** Ajoute `line` au journal sans doublon (par `key` si présente, sinon par demi-coup). */
export function appendLog(log: LogLine[], line: LogLine | null, max = 200): LogLine[] {
  if (!line) return log;
  const same = (l: LogLine) => (line.key !== undefined ? l.key === line.key : l.key === undefined && l.ply === line.ply);
  if (log.some(same)) return log;
  return [...log, line].slice(-max);
}
