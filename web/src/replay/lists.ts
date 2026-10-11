// Logique pure des listes : « Mes parties » (pagination, filtre) et « En direct » (tri, étiquettes).

import type { Color, GameSummary, LiveGame, Outcome, Seat } from "../protocol";
import { t } from "../i18n";
import { kindLabel, opposite, seatName } from "./frames";

// ---- Mes parties -------------------------------------------------------------

export type GamesFilter = "all" | "ranked" | "friendly" | "solo";

// `label` est un accesseur : traduit à la lecture, jamais au chargement du module.
export const GAMES_FILTERS: { id: GamesFilter; label: string }[] = [
  {
    id: "all",
    get label() {
      return t("replay.filter_all");
    },
  },
  {
    id: "ranked",
    get label() {
      return t("replay.filter_ranked");
    },
  },
  {
    id: "friendly",
    get label() {
      return t("replay.filter_friendly");
    },
  },
  {
    id: "solo",
    get label() {
      return t("replay.filter_solo");
    },
  },
];

export const GAMES_PAGE = 20;

export function matchesFilter(game: Pick<GameSummary, "kind" | "rated">, filter: GamesFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "ranked":
      return game.kind !== "solo" && game.rated;
    case "friendly":
      return game.kind !== "solo" && !game.rated;
    case "solo":
      return game.kind === "solo";
  }
}

export function filterGames(games: GameSummary[], filter: GamesFilter): GameSummary[] {
  return filter === "all" ? games : games.filter((g) => matchesFilter(g, filter));
}

export function countByFilter(games: GameSummary[]): Record<GamesFilter, number> {
  return {
    all: games.length,
    ranked: filterGames(games, "ranked").length,
    friendly: filterGames(games, "friendly").length,
    solo: filterGames(games, "solo").length,
  };
}

/** Ajoute une page au début de liste déjà chargée, sans doublon (une partie qui se termine décale les pages). */
export function mergePage(current: GameSummary[], page: GameSummary[], offset: number): GameSummary[] {
  if (offset === 0) return page;
  const seen = new Set(current.map((g) => g.game_id));
  return [...current, ...page.filter((g) => !seen.has(g.game_id))];
}

/** Reste-t-il des parties à charger ? Une page vide arrête la pagination même si `total` est inexact. */
export function hasMore(loaded: number, total: number, lastPageSize: number): boolean {
  return lastPageSize > 0 && loaded < total;
}

/** L'adversaire du demandeur dans une partie. */
export function opponentSeat(game: Pick<GameSummary, "white" | "black" | "color">): Seat {
  return game[opposite(game.color)];
}

/** Libellé de l'adversaire : pseudo, « IA (niveau 1400) » ou « Invité ». */
export function opponentLabel(game: Pick<GameSummary, "white" | "black" | "color">): string {
  const seat = opponentSeat(game);
  if (seat.bot) return seat.elo !== null ? t("replay.bot_level", { elo: seat.elo }) : t("replay.seat_bot");
  return seatName(seat);
}

/** Message d'une liste vide selon le filtre actif. */
export function emptyGamesText(filter: GamesFilter, loaded: number): string {
  if (loaded === 0) return t("replay.empty_none");
  switch (filter) {
    case "ranked":
      return t("replay.empty_ranked");
    case "friendly":
      return t("replay.empty_friendly");
    case "solo":
      return t("replay.empty_solo");
    default:
      return t("replay.empty_default");
  }
}

/** Variation d'Elo seulement pour une partie classée qui compte. */
export function showsDelta(game: Pick<GameSummary, "kind" | "rated" | "elo_delta">): boolean {
  return game.kind !== "solo" && game.rated && game.elo_delta !== null;
}

// ---- En direct ---------------------------------------------------------------

/** Elo moyen des deux joueurs ; un bot ou un invité sans Elo compte pour ce qui est connu. */
export function averageElo(game: Pick<LiveGame, "white" | "black">): number {
  const values = [game.white.elo, game.black.elo].filter((e): e is number => e !== null);
  return values.length ? values.reduce((a, b) => a + b, 0) / values.length : 0;
}

/** Elo moyen décroissant, puis partie la plus ancienne d'abord (même ordre que le serveur). */
export function sortLive(games: LiveGame[]): LiveGame[] {
  return [...games].sort(
    (a, b) =>
      averageElo(b) - averageElo(a) ||
      (a.started_at < b.started_at ? -1 : a.started_at > b.started_at ? 1 : 0) ||
      a.game_id.localeCompare(b.game_id),
  );
}

export const liveTag = (game: Pick<LiveGame, "kind" | "rated">): string => kindLabel(game.kind, game.rated);

export function plyText(ply: number): string {
  return ply === 0 ? t("replay.ply_none") : t("replay.ply", { count: ply });
}

export function spectatorsText(n: number): string {
  if (n <= 0) return t("replay.spectators_none");
  return t("replay.spectators", { count: n });
}

/** « Retransmission différée de 30 s », ou `null` en direct strict. */
export function delayText(delayMs: number): string | null {
  if (!(delayMs > 0)) return null;
  const s = Math.round(delayMs / 1000);
  return s >= 120 ? t("replay.delay_min", { n: Math.round(s / 60) }) : t("replay.delay_s", { n: s });
}

/** Qui gagne : texte de résultat neutre pour un spectateur. */
export function winnerText(winner: Color | null): string {
  if (winner === null) return t("replay.winner_draw");
  return t(winner === "white" ? "replay.winner_white" : "replay.winner_black");
}

const NEUTRAL_REASON: Record<string, string> = {
  checkmate: "replay.neutral_checkmate",
  resignation: "replay.neutral_resignation",
  timeout: "replay.neutral_timeout",
  agreed_draw: "replay.neutral_draw_agreed",
  draw_agreed: "replay.neutral_draw_agreed",
  stalemate: "replay.neutral_stalemate",
  fifty_moves: "replay.neutral_fifty_moves",
  repetition: "replay.neutral_repetition",
  insufficient_material: "replay.neutral_insufficient_material",
  disconnect: "replay.neutral_disconnect",
};

/** Motif de fin sans point de vue (« Abandon », « Échec et mat »). */
export function neutralReason(reason: string): string {
  const key = NEUTRAL_REASON[reason];
  return key ? t(key) : reason;
}

/** Vainqueur d'une issue : un camp, `null` pour une nulle, `undefined` si la partie continue. */
export function outcomeWinner(outcome: Outcome): Color | null | undefined {
  if (outcome.type === "ongoing") return undefined;
  return "winner" in outcome ? outcome.winner : null;
}

/** « Victoire des noirs · Abandon », ou « Partie nulle · Pat ». Vide si la partie continue. */
export function resultLine(outcome: Outcome, reason: string): string {
  const winner = outcomeWinner(outcome);
  if (winner === undefined) return "";
  return `${winnerText(winner)} · ${neutralReason(reason || outcome.type)}`;
}
