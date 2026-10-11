// Durée de partie au choix (Courte, Moyenne, Longue) : mémorisée, envoyée avec la file, la salle ou le défi.
import { useSyncExternalStore } from "react";
import { t as tr } from "./i18n";
import type { TimeControl } from "./protocol";

// `label` et `short` sont des accesseurs : le texte est traduit à la lecture (jamais au chargement du module).
export const TIMES: { id: TimeControl; label: string; short: string; minutes: number }[] = [
  {
    id: "short",
    get label() {
      return tr("time.short_label");
    },
    get short() {
      return tr("time.short_short");
    },
    minutes: 5,
  },
  {
    id: "medium",
    get label() {
      return tr("time.medium_label");
    },
    get short() {
      return tr("time.medium_short");
    },
    minutes: 15,
  },
  {
    id: "long",
    get label() {
      return tr("time.long_label");
    },
    get short() {
      return tr("time.long_short");
    },
    minutes: 30,
  },
];

const KEY = "chessy.time";
const listeners = new Set<() => void>();

export function readTime(): TimeControl {
  try {
    const v = localStorage.getItem(KEY);
    return TIMES.some((t) => t.id === v) ? (v as TimeControl) : "short";
  } catch {
    return "short";
  }
}

let current: TimeControl = readTime();

export function setTime(t: TimeControl) {
  current = t;
  try {
    localStorage.setItem(KEY, t);
  } catch {
    // Mode privé : le choix ne survit pas au rechargement.
  }
  listeners.forEach((fn) => fn());
}

export function useTime(): TimeControl {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => current,
  );
}

export const timeInfo = (t: TimeControl) => TIMES.find((x) => x.id === t) ?? TIMES[0];
/** « Blitz · 5 min » pour la plus courte, « 15 min » pour les autres. */
export const timeText = (t: TimeControl) => (t === "short" ? tr("time.blitz") : tr("time.minutes", { minutes: timeInfo(t).minutes }));
