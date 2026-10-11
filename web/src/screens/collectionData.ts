// Règles complètes des compétences (source : docs/skills.md) et fonctions de l'historique de la collection.
import { CATALOG, type CatalogId } from "../catalog";
import { intlLocale, t } from "../i18n";
import type { SkillHistoryEntry, SkillId } from "../protocol";
import { parseServerDate } from "../ui/social";

/** Règles complètes de chaque compétence, traduites à chaque lecture (clés `skills.rules_<id>`). */
export const RULES: Record<CatalogId, string> = Object.defineProperties(
  {} as Record<CatalogId, string>,
  Object.fromEntries(CATALOG.map((c) => [c.id, { enumerable: true, get: () => t(`skills.rules_${c.id}`) }])),
);

/** Notes de bas de fiche (compétence unique / classique). */
export const uniqueNote = () => t("collection.note_unique");
export const classicNote = () => t("collection.note_classic");

export type HistoryFilter = "all" | "forged" | "obtained" | "lost";

/** Filtre du journal : les valeurs reprennent `SkillHistoryEntry.change`. `label` est une clé de traduction. */
export type JournalFilter = "all" | SkillHistoryEntry["change"];
export const JOURNAL_FILTERS: { id: JournalFilter; label: string }[] = [
  { id: "all", label: "collection.filter_all" },
  { id: "gained", label: "collection.journal_gained" },
  { id: "lost", label: "collection.journal_lost" },
];

export type OriginFilter = "all" | "forged" | "stolen" | "other";
export const ORIGIN_FILTERS: { id: OriginFilter; label: string }[] = [
  { id: "all", label: "collection.origin_all" },
  { id: "forged", label: "collection.origin_forged" },
  { id: "stolen", label: "collection.origin_stolen" },
  { id: "other", label: "collection.origin_other" },
];

/** Une compétence du deck et la dernière ligne qui l'a fait entrer (absente si l'historique n'en garde pas). */
export interface OwnedSkill {
  skill: SkillId;
  origin?: SkillHistoryEntry;
}

/** Le deck dans son ordre, chaque compétence avec son entrée la plus récente (`entries` : le plus récent d'abord). */
export function ownedSkills(deck: readonly SkillId[], entries: readonly SkillHistoryEntry[]): OwnedSkill[] {
  return deck.map((skill) => ({ skill, origin: entries.find((e) => e.skill === skill && e.change === "gained") }));
}

function originOf(owned: OwnedSkill): Exclude<OriginFilter, "all"> {
  const source = owned.origin?.source;
  return source === "forged" || source === "stolen" ? source : "other";
}

export function filterOwned(owned: readonly OwnedSkill[], filter: OriginFilter): OwnedSkill[] {
  return filter === "all" ? [...owned] : owned.filter((o) => originOf(o) === filter);
}

export function originCounts(owned: readonly OwnedSkill[]): Record<OriginFilter, number> {
  return Object.fromEntries(ORIGIN_FILTERS.map((f) => [f.id, filterOwned(owned, f.id).length])) as Record<OriginFilter, number>;
}

export function filterJournal(entries: readonly SkillHistoryEntry[], filter: JournalFilter): SkillHistoryEntry[] {
  return filter === "all" ? [...entries] : entries.filter((e) => e.change === filter);
}

/** Ce que raconte une ligne : « Forgée », « Volée à bob », « Prise par alice »… */
export function describeEntry(e: SkillHistoryEntry): string {
  const who = e.other ?? t("collection.anonymous_player");
  switch (e.source) {
    case "forged":
      return t("collection.src_forged");
    case "stolen":
      return t("collection.src_stolen", { who });
    case "won":
      return t("collection.src_won");
    case "starter":
      return t("collection.src_starter");
    case "refill":
      return t("collection.src_refill");
    case "earlier":
      return t("collection.src_earlier");
    case "taken":
      return e.other ? t("collection.src_taken_by", { who: e.other }) : t("collection.src_taken");
    case "replaced":
      return t("collection.src_replaced");
  }
}

/** Les lignes qui correspondent au filtre (l'ordre est conservé). */
export function filterHistory(entries: readonly SkillHistoryEntry[], filter: HistoryFilter): SkillHistoryEntry[] {
  return entries.filter((e) => {
    switch (filter) {
      case "all":
        return true;
      case "forged":
        return e.change === "gained" && e.source === "forged";
      case "obtained":
        return e.change === "gained" && e.source !== "forged";
      case "lost":
        return e.change === "lost";
    }
  });
}

export interface HistoryStats {
  forged: number;
  obtained: number;
  lost: number;
}

export function historyStats(entries: readonly SkillHistoryEntry[]): HistoryStats {
  return {
    forged: filterHistory(entries, "forged").length,
    obtained: filterHistory(entries, "obtained").length,
    lost: filterHistory(entries, "lost").length,
  };
}

/** Le jour d'une date serveur, en clair : « Aujourd'hui », « Hier » ou la date. */
export function dayLabel(value: string, now: Date = new Date()): string {
  const d = parseServerDate(value);
  if (!d) return t("collection.day_unknown");
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((start(now) - start(d)) / 86_400_000);
  if (days <= 0) return t("collection.day_today");
  if (days === 1) return t("collection.day_yesterday");
  return new Intl.DateTimeFormat(intlLocale(), { day: "numeric", month: "long", year: "numeric" }).format(d);
}

/** L'heure d'une date serveur (« 14:02 ») : le jour est déjà dans l'en-tête du groupe. */
export function clockTime(value: string): string {
  const d = parseServerDate(value);
  return d ? new Intl.DateTimeFormat(intlLocale(), { hour: "2-digit", minute: "2-digit" }).format(d) : "";
}

export interface HistoryDay {
  label: string;
  entries: SkillHistoryEntry[];
}

/** Regroupe par jour, en gardant l'ordre (le plus récent d'abord). */
export function groupByDay(entries: readonly SkillHistoryEntry[], now: Date = new Date()): HistoryDay[] {
  const days: HistoryDay[] = [];
  for (const e of entries) {
    const label = dayLabel(e.at, now);
    const last = days[days.length - 1];
    if (last && last.label === label) last.entries.push(e);
    else days.push({ label, entries: [e] });
  }
  return days;
}

