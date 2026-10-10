// Les compétences forgées (`forged_<n>`) n'ont pas de fiche écrite à la main : le serveur décrit chacune
// (nom, description, famille, rareté, icône, son). Ce module les garde en mémoire, repère les identifiants
// inconnus dans ce que le client reçoit et va chercher leur définition (`GET /api/skills/forged`).
import type { Family } from "./catalog";
import { t } from "./i18n";
import type { ForgedSkillId } from "./protocol";

export type Rarity = "common" | "uncommon" | "rare" | "epic" | "legendary";

export const RARITIES: Rarity[] = ["common", "uncommon", "rare", "epic", "legendary"];

/** Libellé de chaque rareté, traduit à chaque lecture (propriétés dynamiques). */
export const RARITY_LABEL: Record<Rarity, string> = Object.defineProperties(
  {} as Record<Rarity, string>,
  Object.fromEntries(RARITIES.map((r) => [r, { enumerable: true, get: () => t(`forge.rarity_${r}`) }])),
);

/** Ce que le client dessine (voir `ui/forgedIcon.tsx`) : le signe de l'effet ; la rareté et la famille viennent de la définition. */
export interface IconSpec {
  glyph: string;
  /** Anciens champs, envoyés par le serveur mais plus dessinés : l'icône n'est qu'un signe. */
  piece?: "pawn" | "knight" | "bishop" | "rook" | "queen";
  badge?: "short" | "long" | "forever";
}

/** Ce que le client joue (voir `sound/forgedRecipe.ts`). */
export interface SoundSpec {
  effect: number;
  degree: number;
  timbre: number;
  length: number;
}

/** Vue plate de la définition (`SkillDef::bricks()`), lue par l'animation. Absente tant que le serveur ne l'envoie pas. */
export interface ForgedBricks {
  action: string;
  zone?: string;
  kinds?: string[];
  plies?: number | null;
  permanent?: boolean;
  /** `own`, `enemy`, `any` ou `none` : qui l'effet touche. */
  side?: string;
  /** Les types de pièce auxquels le sélecteur limite l'effet (vide : tous ceux que l'effet permet). */
  selector_kinds?: string[];
  /** Ce en quoi une métamorphose transforme la pièce. */
  into?: string;
  condition?: string | null;
  constraints?: string[];
  max_uses?: number;
  free_action?: boolean;
}

/** Les deux morceaux d'un nom : lequel des quatre noms de l'effet, et le nom propre inventé. */
export interface NameParts {
  noun: number;
  proper: string;
}

/** Style de la marque de durée : les mêmes trois états que les badges d'icônes. */
export function durationStyle(b: Pick<ForgedBricks, "plies" | "permanent">): "short" | "long" | "forever" {
  if (b.permanent) return "forever";
  return (b.plies ?? 0) > 2 ? "long" : "short";
}

export interface ForgedDef {
  id: ForgedSkillId;
  /** Nom français du serveur : repli quand `name_parts` ou les briques manquent. */
  name: string;
  name_parts?: NameParts;
  /** Description française du serveur : repli quand les briques manquent. */
  description: string;
  family: Family;
  rarity: Rarity;
  unique: boolean;
  redundant: boolean;
  max_uses: number;
  icon: IconSpec;
  sound: SoundSpec;
  bricks?: ForgedBricks;
}

const FORGED_ID = /forged_\d+/g;
const FORGED_ONLY = /^forged_\d+$/;
/** Au plus autant d'identifiants par requête que le serveur en accepte. */
const BATCH = 64;

export const isForgedId = (id: string): id is ForgedSkillId => FORGED_ONLY.test(id);

const defs = new Map<string, ForgedDef>();
/** Requêtes en cours, par identifiant : on ne redemande pas ce qui arrive déjà. */
const inflight = new Map<string, Promise<void>>();
/** Identifiants que le serveur ne connaît pas : inutile de les redemander. */
const unknown = new Set<string>();
/** Échecs réseau récents (ms) : on réessaie, mais pas à chaque message reçu. */
const failedAt = new Map<string, number>();
const RETRY_MS = 5000;
const listeners = new Set<() => void>();
let version = 0;

export const forgedDef = (id: string): ForgedDef | undefined => defs.get(id);

/** Change à chaque définition reçue : de quoi relancer un rendu. */
export const forgedVersion = () => version;

export function onForgedChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function rememberForged(list: ForgedDef[]): void {
  if (list.length === 0) return;
  for (const def of list) {
    defs.set(def.id, def);
    failedAt.delete(def.id);
  }
  version++;
  for (const fn of [...listeners]) fn();
}

const settled = (id: string) => defs.has(id) || unknown.has(id);

/** Les identifiants forgés cités dans un texte (message JSON du serveur) dont on ne sait rien encore. */
export function unknownForged(text: string, now = Date.now()): ForgedSkillId[] {
  const found = new Set<string>();
  for (const m of text.matchAll(FORGED_ID)) {
    const id = m[0];
    if (settled(id) || inflight.has(id)) continue;
    const failed = failedAt.get(id);
    if (failed !== undefined && now - failed < RETRY_MS) continue;
    found.add(id);
  }
  return [...found] as ForgedSkillId[];
}

type Fetcher = (url: string) => Promise<{ ok: boolean; json(): Promise<unknown> }>;

async function fetchBatches(ids: string[], fetcher: Fetcher): Promise<void> {
  for (let i = 0; i < ids.length; i += BATCH) {
    const batch = ids.slice(i, i + BATCH);
    const numbers = batch.map((id) => id.slice("forged_".length)).join(",");
    try {
      const res = await fetcher(`/api/skills/forged?ids=${numbers}`);
      if (!res.ok) throw new Error("bad status");
      const body = (await res.json()) as { skills?: ForgedDef[] };
      const skills = body.skills ?? [];
      rememberForged(skills);
      for (const id of batch) if (!defs.has(id)) unknown.add(id);
    } catch {
      const now = Date.now();
      for (const id of batch) failedAt.set(id, now);
    }
  }
}

/** Va chercher les définitions manquantes ; la promesse se résout quand elles sont arrivées (ou ont échoué). */
export function loadForged(ids: string[], fetcher: Fetcher = (u) => fetch(u)): Promise<void> {
  const need = ids.filter((id) => isForgedId(id) && !settled(id));
  const fresh = need.filter((id) => !inflight.has(id));
  if (fresh.length > 0) {
    const run = fetchBatches(fresh, fetcher).finally(() => {
      for (const id of fresh) inflight.delete(id);
    });
    for (const id of fresh) inflight.set(id, run);
  }
  return Promise.all(need.map((id) => inflight.get(id))).then(() => undefined);
}

/** À appeler avec chaque message reçu : charge ce qui manque. */
export function noticeForged(text: string): void {
  const missing = unknownForged(text);
  if (missing.length > 0) void loadForged(missing);
}

/** Pour les tests. */
export function resetForged(): void {
  defs.clear();
  inflight.clear();
  unknown.clear();
  failedAt.clear();
  version = 0;
}
