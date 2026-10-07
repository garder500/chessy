// Logique pure de l'écran de jeu (horloges, matériel, journal), sans React ni Phaser.

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

export const PIECE_FR: Record<PieceKind, string> = {
  pawn: "Pion",
  knight: "Cavalier",
  bishop: "Fou",
  rook: "Tour",
  queen: "Dame",
  king: "Roi",
};

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
  for (const e of events) {
    // Wall fait surgir plusieurs pions d'un coup : une seule mention.
    if (e.type === "spawned" && spawns > 1) {
      if (!parts.some((p) => p.startsWith(`${spawns} pièces`))) parts.push(`${spawns} pièces apparaissent`);
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
  switch (e.type) {
    case "moved": {
      const kind = board[e.to]?.kind ?? "pawn";
      return [`${PIECE_FR[kind]} ${sqName(e.from)}–${sqName(e.to)}`];
    }
    case "captured":
      return [`prend ${PIECE_FR[e.piece.kind].toLowerCase()}`];
    case "promoted":
      return [`promotion en ${PIECE_FR[e.to].toLowerCase()}`];
    case "castled":
      return ["roque"];
    case "teleported":
      return [`${sqName(e.from)} vers ${sqName(e.to)}`];
    case "cloned":
      return [`copie ${sqName(e.from)} sur ${sqName(e.to)}`];
    case "swapped":
      return [`${sqName(e.a)} et ${sqName(e.b)} échangées`];
    case "removed":
      return [`${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)} retiré`];
    case "rolled_back":
      return [`retour ${sqName(e.from)} vers ${sqName(e.to)}`];
    case "effect_added":
      return [EFFECT_FR[e.effect] ?? "effet appliqué"];
    case "skill_used":
      return inSkill ? [] : [skillName(e.skill)];
    case "spawned":
      return [`${pieceLabel(e.piece)} apparaît en ${sqName(e.square)}`];
    case "transformed":
      return [`${sqName(e.square)} devient ${PIECE_FR[e.kind].toLowerCase()}`];
    case "switched":
      return [`${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)} change de camp`];
    case "rotated":
      return [`${e.moves.length} pièces tournent`];
    case "trap_set":
      return [`piège posé en ${sqName(e.square)}`];
    case "trap_sprung":
      return [`piège déclenché en ${sqName(e.square)}`];
    case "benched":
      return [`${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)} mis sur le banc`];
    case "unbenched":
      return [`${PIECE_FR[e.piece.kind].toLowerCase()} revient en ${sqName(e.square)}`];
    case "pushed":
      return [`attaquant repoussé ${sqName(e.from)}–${sqName(e.to)}`];
    case "saved":
      return [`pièce sauvée, retour en ${sqName(e.to)}`];
    case "best_move":
      return [`meilleur coup ${sqName(e.from)}–${sqName(e.to)}${e.promo ? ` (${PIECE_FR[e.promo].toLowerCase()})` : ""}`];
    case "cancelled":
      return [`${skillName(e.skill)} annulée`];
    case "terrain":
      return [`${e.squares.length} cases de roc`];
    case "global_effect":
      return [`${EFFECT_FR[e.effect]}`];
    case "vanished":
      return [`${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)} disparaît`];
    case "ambushed":
      return [`des fous frappent ${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)}`];
    case "loan_ended":
      return [`prêt terminé (${PIECE_FR[e.piece.kind].toLowerCase()} ${sqName(e.square)})`];
    default:
      return [];
  }
}

const EFFECT_FR: Record<EffectKind, string> = {
  immune: "pièce protégée",
  frozen: "pièce gelée",
  invisible: "pièce invisible",
  forcefield: "champ de force",
  celestial: "protection céleste",
  locked: "pion immobilisé",
  morphed: "pièce métamorphosée",
  color_loan: "pièce sous contrôle",
  vanish: "pièce éphémère",
  truce: "armistice",
  fog: "brouillard",
  silenced: "pouvoirs réduits au silence",
  domain: "expansion de domaine",
};

function pieceLabel(p: Piece): string {
  const base = PIECE_FR[p.kind];
  return p.mirage ? `${base} mirage` : p.wall ? `${base} mur` : p.temp ? `${base} temporaire` : base;
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
    if (e.kind === "truce") out.push({ kind: e.kind, label: "Armistice : plus de captures ni d'échecs", turns });
    else if (e.kind === "fog") out.push({ kind: e.kind, label: "Brouillard : vue limitée à deux cases", turns });
    else if (e.kind === "domain") {
      out.push({
        kind: e.kind,
        label: e.owner === view.you ? "Domaine : des fous frapperont la prochaine pièce qui vous met en échec" : "Domaine adverse : une pièce qui met son roi en échec sera frappée",
        turns,
      });
    } else if (e.kind === "silenced") {
      out.push({ kind: e.kind, label: e.owner === view.you ? "Silence : vous ne pouvez plus utiliser de compétence" : "Silence : l'adversaire ne peut plus utiliser de compétence", turns });
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
