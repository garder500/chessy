// Conversion des positions enregistrées (`Frame`) et des vues de spectateur (`SpectatorView`) en
// `StateView`, la seule forme que comprend le plateau (`PhaserBoard`). Fonctions pures, sans React.

import type {
  Clock,
  Color,
  Frame,
  GameKind,
  GameRecord,
  OpponentInfo,
  Outcome,
  Seat,
  SkillSlot,
  SpectatorView,
  StateView,
} from "../protocol";
import { t } from "../i18n";
import { resultFor } from "../outcome";

export const opposite = (c: Color): Color => (c === "white" ? "black" : "white");

/** Nom du camp dans la langue courante (« blancs » / « noirs » en français), lu à l'usage. */
export const COLOR_FR: Record<Color, string> = {
  get white() {
    return t("replay.color_white");
  },
  get black() {
    return t("replay.color_black");
  },
};

/** Nom du camp en début de libellé (« Blancs » / « Noirs »). */
export const colorCap = (c: Color): string => t(c === "white" ? "replay.white_cap" : "replay.black_cap");

/** Nom affichable d'un joueur : pseudo, « IA » pour un bot, « Invité » sinon. */
export function seatName(seat: Seat): string {
  return seat.username ?? (seat.bot ? t("replay.seat_bot") : t("replay.seat_guest"));
}

export function opponentInfo(seat: Seat): OpponentInfo {
  return { username: seat.username, elo: seat.elo, guest: seat.username === null && !seat.bot, bot: seat.bot };
}

/** Camp occupé par `username` dans la partie, ou `null` s'il n'y joue pas (comparaison insensible à la casse). */
export function colorOf(game: { white: Seat; black: Seat }, username: string | null | undefined): Color | null {
  if (!username) return null;
  const u = username.toLowerCase();
  if (game.white.username?.toLowerCase() === u) return "white";
  if (game.black.username?.toLowerCase() === u) return "black";
  return null;
}

/** Orientation par défaut du plateau : le camp du joueur connecté, sinon le camp humain d'une partie solo, sinon les blancs. */
export function defaultOrientation(game: { white: Seat; black: Seat }, username: string | null | undefined): Color {
  const mine = colorOf(game, username);
  if (mine) return mine;
  if (game.white.bot && !game.black.bot) return "black";
  return "white";
}

/** Son de fin de partie pour `me` : victoire, défaite ou nulle ; `null` si la partie n'est pas finie. */
export function endSound(outcome: Outcome, me: Color): "game_win" | "game_lose" | "game_draw" | null {
  const result = resultFor(outcome, me);
  if (result === null) return null;
  return result === "win" ? "game_win" : result === "loss" ? "game_lose" : "game_draw";
}

/** Une partie enregistrée est rejouable si elle contient au moins sa position initiale. */
export function isReplayable(record: Pick<GameRecord, "frames"> | null | undefined): boolean {
  return !!record && Array.isArray(record.frames) && record.frames.length > 0;
}

/** Dernier indice de position (une position de plus que d'actions). */
export function lastIndex(record: Pick<GameRecord, "frames">): number {
  return Math.max(0, record.frames.length - 1);
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/** Compétences d'un camp : tout le deck avec l'état d'usage à cette position. */
function slotsOf(deck: string[], used: string[]): SkillSlot[] {
  return deck.map((skill) => ({ skill: skill as SkillSlot["skill"], used: used.includes(skill) }));
}

const NO_CLOCK: Clock = { white_ms: 0, black_ms: 0, running: null };

/**
 * Vue plateau d'une position enregistrée. `orientation` devient `you` (le plateau se tourne vers ce camp).
 * Lecture seule : ni coups ni cibles de compétence. Les pièges et le banc des deux camps sont visibles
 * (la partie est finie, l'enregistrement contient toute l'information).
 */
export function frameToView(record: GameRecord, frame: Frame, orientation: Color): StateView {
  const other = opposite(orientation);
  return {
    game_id: record.game_id,
    clock: NO_CLOCK,
    clock_enabled: false,
    rated: record.rated,
    opponent: opponentInfo(record[other]),
    draw_offer: "none",
    ply_count: record.plies,
    you: orientation,
    ply: frame.ply,
    to_move: frame.to_move,
    in_check: frame.in_check,
    board: frame.board,
    moves: [],
    skill_options: [],
    my_skills: slotsOf(record.loadouts[orientation], frame.used[orientation]),
    opponent_skills: { total: record.loadouts[other].length, used: frame.used[other] },
    effects: frame.effects,
    traps: frame.traps.map((t) => t.square),
    benched: frame.benched.map((b) => b.piece),
    terrain: frame.terrain,
    outcome: frame.outcome,
    events: frame.events,
    opponent_connected: true,
  };
}

/**
 * Vue d'exploration. `display` est ce que voit le plateau (orienté comme le replay) ; `play` sert aux
 * clics : `you` y est le camp au trait, avec les coups et les cibles de compétence du serveur.
 */
export function exploreViews(
  record: GameRecord,
  frame: Frame,
  orientation: Color,
  moves: StateView["moves"],
  skillOptions: StateView["skill_options"],
): { display: StateView; play: StateView } {
  const display = frameToView(record, frame, orientation);
  const play: StateView = {
    ...frameToView(record, frame, frame.to_move),
    moves,
    skill_options: skillOptions,
  };
  return { display, play };
}

/** `running` peut être un camp, un booléen (« l'horloge tourne ») ou `null`. */
export function normalizeClock(clock: SpectatorView["clock"], toMove: Color, over: boolean): Clock {
  let running: Color | null = null;
  if (!over) {
    if (clock.running === "white" || clock.running === "black") running = clock.running;
    else if (clock.running === true) running = toMove;
  }
  return { white_ms: clock.white_ms, black_ms: clock.black_ms, running };
}

/** Vue plateau d'une retransmission. Le spectateur ne connaît ni les decks, ni les pièges, ni le banc. */
export function spectatorToView(view: SpectatorView, orientation: Color): StateView {
  const other = opposite(orientation);
  const over = view.outcome.type !== "ongoing";
  return {
    game_id: view.game_id,
    clock: normalizeClock(view.clock, view.to_move, over),
    clock_enabled: view.clock_enabled,
    rated: view.rated,
    opponent: opponentInfo(view[other]),
    draw_offer: "none",
    ply_count: view.ply,
    you: orientation,
    ply: view.ply,
    to_move: view.to_move,
    in_check: view.in_check,
    board: view.board,
    moves: [],
    skill_options: [],
    my_skills: view.used[orientation].map((skill) => ({ skill, used: true })),
    opponent_skills: { total: view.used[other].length, used: view.used[other] },
    effects: view.effects ?? [],
    traps: [],
    benched: [],
    terrain: view.terrain ?? [],
    outcome: view.outcome,
    events: view.events ?? [],
    opponent_connected: true,
    spectators: view.spectators,
  };
}

/** Valeurs par défaut pour un serveur qui omettrait des champs optionnels de `SpectatorView`. */
export function normalizeSpectatorView(view: SpectatorView): SpectatorView {
  return {
    ...view,
    effects: view.effects ?? [],
    terrain: view.terrain ?? [],
    events: view.events ?? [],
    used: { white: view.used?.white ?? [], black: view.used?.black ?? [] },
    spectators: view.spectators ?? 0,
    delay_ms: view.delay_ms ?? 0,
    clock_enabled: view.clock_enabled ?? true,
  };
}

/** Étiquette de type de partie : « Solo » prime, puis classée / amicale selon `rated`. */
export function kindLabel(kind: GameKind, rated: boolean): string {
  if (kind === "solo") return t("replay.kind_solo");
  return t(rated ? "replay.kind_ranked" : "replay.kind_friendly");
}
