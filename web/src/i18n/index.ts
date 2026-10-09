import { useSyncExternalStore } from "react";

/** Langues de l'interface : la première est le repli quand une langue ou une clé manque. */
export const LANGS = ["en", "fr", "de", "es", "pt"] as const;
export type Lang = (typeof LANGS)[number];
export const FALLBACK_LANG: Lang = "en";
/** Nom de chaque langue dans sa propre langue (sélecteur des réglages). */
export const LANG_NAMES: Record<Lang, string> = {
  en: "English",
  fr: "Français",
  de: "Deutsch",
  es: "Español",
  pt: "Português",
};

export const LANG_KEY = "chessy.lang";

type Dict = Record<string, string>;

/**
 * Les traductions vivent dans `locales/<langue>/<espace>.json` ; la clé complète est `<espace>.<clé>`.
 * Les pluriels s'écrivent `<clé>_one` / `<clé>_other` (autres catégories Intl : `_zero`, `_two`, `_few`, `_many`)
 * et se choisissent via le paramètre `count`.
 */
const modules = import.meta.glob<Dict>("./locales/*/*.json", { eager: true, import: "default" });

function buildDicts(): Record<Lang, Dict> {
  const out = Object.fromEntries(LANGS.map((l) => [l, {} as Dict])) as Record<Lang, Dict>;
  for (const [path, mod] of Object.entries(modules)) {
    const m = /\.\/locales\/([^/]+)\/([^/]+)\.json$/.exec(path);
    if (!m || !(m[1] in out)) continue;
    for (const [k, v] of Object.entries(mod)) out[m[1] as Lang][`${m[2]}.${k}`] = v;
  }
  return out;
}

const dicts = buildDicts();

export function isLang(v: unknown): v is Lang {
  return typeof v === "string" && (LANGS as readonly string[]).includes(v);
}

/** Première langue prise en charge parmi les préférences du navigateur (`fr-CA` → `fr`), sinon l'anglais. */
export function detectLang(preferred: readonly string[] | undefined): Lang {
  for (const tag of preferred ?? []) {
    const base = tag.toLowerCase().split(/[-_]/)[0];
    if (isLang(base)) return base;
  }
  return FALLBACK_LANG;
}

function readStored(): Lang | null {
  try {
    const v = typeof localStorage !== "undefined" ? localStorage.getItem(LANG_KEY) : null;
    return isLang(v) ? v : null;
  } catch {
    return null;
  }
}

function browserLangs(): readonly string[] | undefined {
  if (typeof navigator === "undefined") return undefined;
  return navigator.languages?.length ? navigator.languages : navigator.language ? [navigator.language] : undefined;
}

let stored = readStored();
let current: Lang = stored ?? detectLang(browserLangs());
const listeners = new Set<() => void>();

function applyDocument() {
  if (typeof document !== "undefined") document.documentElement.lang = current;
}
applyDocument();

export function getLang(): Lang {
  return current;
}

/** `true` si l'utilisateur a choisi sa langue (sinon elle suit le navigateur). */
export function hasChosenLang(): boolean {
  return stored !== null;
}

/** Choisit une langue (mémorisée, prioritaire sur la détection) ; `null` revient à la détection automatique. */
export function setLang(lang: Lang | null): void {
  stored = lang;
  try {
    if (lang) localStorage.setItem(LANG_KEY, lang);
    else localStorage.removeItem(LANG_KEY);
  } catch {
    /* stockage indisponible : la langue ne vaut que pour cette session */
  }
  current = lang ?? detectLang(browserLangs());
  applyDocument();
  for (const l of listeners) l();
}

export function subscribeLang(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Langue courante, réactive. */
export function useLang(): Lang {
  return useSyncExternalStore(subscribeLang, getLang, getLang);
}

export type Params = Record<string, string | number>;

function lookup(lang: Lang, key: string, count: number | undefined): string | undefined {
  const d = dicts[lang];
  if (count !== undefined) {
    const cat = new Intl.PluralRules(lang).select(count);
    const plural = d[`${key}_${cat}`] ?? d[`${key}_other`];
    if (plural !== undefined) return plural;
  }
  return d[key];
}

/** Traduit `key` dans `lang` ; langue ou clé absente → anglais, puis la clé elle-même. `{nom}` est remplacé par `params.nom`. */
export function translate(lang: Lang, key: string, params?: Params): string {
  const count = typeof params?.count === "number" ? params.count : undefined;
  const raw = lookup(lang, key, count) ?? lookup(FALLBACK_LANG, key, count) ?? key;
  if (!params) return raw;
  return raw.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** Traduit dans la langue courante. À appeler au moment du rendu (ou de l'événement), pas au chargement du module. */
export function t(key: string, params?: Params): string {
  return translate(current, key, params);
}

/** Comme `t`, mais re-rend le composant quand la langue change. */
export function useT(): typeof t {
  useLang();
  return t;
}

/** Locale BCP 47 pour `Intl` / `toLocaleString`. */
export function intlLocale(): string {
  return current;
}

/** Test : clés d'une langue (pour vérifier la couverture). */
export function dictKeys(lang: Lang): string[] {
  return Object.keys(dicts[lang]);
}
