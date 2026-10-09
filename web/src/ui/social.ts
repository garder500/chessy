// Petites fonctions pures partagées par les écrans sociaux (classement, amis, profil).
import { intlLocale, t } from "../i18n";
import type { Presence, RecentGame, Relation } from "../protocol";

export function initialOf(name: string): string {
  return (name.trim().charAt(0) || "?").toUpperCase();
}

/** Pourcentage de victoires arrondi, ou null sans partie. */
export function winRate(wins: number, games: number): number | null {
  return games > 0 ? Math.round((wins / games) * 100) : null;
}

/** Interprète les horodatages serveur : RFC 3339, ou SQLite « AAAA-MM-JJ HH:MM:SS » (UTC). */
export function parseServerDate(value: string): Date | null {
  if (!value) return null;
  let v = value.trim();
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}/.test(v)) v = v.replace(" ", "T");
  if (/T\d{2}:\d{2}(:\d{2}(\.\d+)?)?$/.test(v)) v += "Z";
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? null : d;
}

const dateFmt = () => new Intl.DateTimeFormat(intlLocale(), { day: "numeric", month: "long", year: "numeric" });
const monthFmt = () => new Intl.DateTimeFormat(intlLocale(), { month: "long", year: "numeric" });

/** « à l'instant », « il y a 5 min », « hier », « il y a 3 j », sinon date complète. */
export function relativeTime(value: string, now: Date = new Date()): string {
  const d = parseServerDate(value);
  if (!d) return "";
  const diff = Math.max(0, now.getTime() - d.getTime());
  const min = Math.floor(diff / 60_000);
  if (min < 1) return t("social.time_now");
  if (min < 60) return t("social.time_min", { n: min });
  const h = Math.floor(min / 60);
  if (h < 24) return t("social.time_hour", { n: h });
  const days = Math.floor(h / 24);
  if (days === 1) return t("social.time_yesterday");
  if (days < 7) return t("social.time_day", { n: days });
  return dateFmt().format(d);
}

export function memberSince(value: string): string {
  const d = parseServerDate(value);
  return d ? t("social.member_since", { date: monthFmt().format(d) }) : "";
}

const REASONS = new Set([
  "checkmate",
  "resignation",
  "timeout",
  "agreed_draw",
  "stalemate",
  "fifty_moves",
  "repetition",
  "insufficient_material",
  "disconnect",
]);

export function reasonText(reason: string, result: RecentGame["result"]): string {
  return REASONS.has(reason) ? t(`social.reason_${reason}_${result}`) : reason;
}

/** Libellés lus au moment de l'accès (donc traduits dans la langue courante). */
export const RESULT_LABEL: Record<RecentGame["result"], string> = {
  get win() {
    return t("social.result_win");
  },
  get loss() {
    return t("social.result_loss");
  },
  get draw() {
    return t("social.result_draw");
  },
};

export function formatDelta(delta: number | null): string {
  if (delta === null) return "—";
  if (delta === 0) return "±0";
  return delta > 0 ? `+${delta}` : `−${Math.abs(delta)}`;
}

export function presenceLabel(presence: Presence, lastSeen: string | null, now: Date = new Date()): string {
  if (presence === "online") return t("social.presence_online");
  if (presence === "in_game") return t("social.presence_in_game");
  const seen = lastSeen ? relativeTime(lastSeen, now) : "";
  return seen ? t("social.presence_offline_seen", { seen }) : t("social.presence_offline");
}

const PRESENCE_RANK: Record<Presence, number> = { online: 0, in_game: 1, offline: 2 };

/** Tri de la liste d'amis : en ligne, en partie, hors ligne, puis pseudo. */
export function sortFriends<T extends { username: string; presence: Presence }>(friends: T[]): T[] {
  return [...friends].sort(
    (a, b) => PRESENCE_RANK[a.presence] - PRESENCE_RANK[b.presence] || a.username.localeCompare(b.username, intlLocale()),
  );
}

export const RELATION_LABEL: Record<Relation, string> = {
  get none() {
    return "";
  },
  get friend() {
    return t("social.relation_friend");
  },
  get incoming() {
    return t("social.relation_incoming");
  },
  get outgoing() {
    return t("social.relation_outgoing");
  },
  get self() {
    return t("social.relation_self");
  },
};

export function sameUser(a: string | null | undefined, b: string | null | undefined): boolean {
  return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}
