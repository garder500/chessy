// Nom et description d'une compétence forgée, écrits ici dans la langue de l'interface à partir de ses briques
// (`SkillDef::bricks()` et `name_parts`, envoyés par le serveur). Le texte français du serveur ne sert que de repli
// quand les briques manquent (vieux serveur) : voir `forgedName` / `forgedDescription`.
import { t } from "./i18n";
import type { ForgedBricks, ForgedDef } from "./forged";

/** Les actions que le client sait dire (les 17 effets du moteur) ; le rang donne l'index des noms. */
export const ACTIONS = [
  "freeze", "shield", "cloak", "morph", "promote", "remove", "convert", "teleport", "duplicate",
  "swap", "spawn", "revive", "truce", "mirror", "fog", "silence", "ambush",
] as const;
export type Action = (typeof ACTIONS)[number];

const isAction = (a: string): a is Action => (ACTIONS as readonly string[]).includes(a);

/** Nombre de noms par effet (`NOUNS` dans crates/chessy-engine/src/forge/identity.rs). */
export const NOUNS_PER_ACTION = 4;

const PROPER_VOWEL = /^[AEIOUY]/;

/** « Givre d'Alfen » : le nom commun de l'effet, puis le nom propre inventé (qui ne se traduit pas). */
export function forgedName(def: Pick<ForgedDef, "name" | "name_parts" | "bricks">): string {
  const parts = def.name_parts;
  const action = def.bricks?.action;
  if (!parts || !action || !isAction(action) || parts.noun < 0 || parts.noun >= NOUNS_PER_ACTION) return def.name;
  const noun = t(`fgtext.noun_${action}_${parts.noun}`);
  return t(PROPER_VOWEL.test(parts.proper) ? "fgtext.name_of_vowel" : "fgtext.name_of", { noun, proper: parts.proper });
}

const PIECES = ["pawn", "knight", "bishop", "rook", "queen"];

/** « pion, cavalier ou dame » (une pièce parmi elles) ou, au pluriel, « pions, cavaliers ou dames ». */
function kindsList(kinds: string[], plural: boolean): string {
  const names = kinds.map((k) => t(plural ? `fgtext.kinds_${k}` : `game.piece_lc_${k}`));
  const last = names[names.length - 1] ?? "";
  if (names.length <= 1) return last;
  return `${names.slice(0, -1).join(", ")} ${t("fgtext.join_or")} ${last}`;
}

/** Les briques décrivent-elles assez la compétence pour que le client l'écrive lui-même ? */
function readable(b: ForgedBricks | undefined): b is ForgedBricks & { side: string } {
  if (!b || !isAction(b.action) || typeof b.side !== "string" || !Array.isArray(b.constraints)) return false;
  if (b.action === "morph" && !(b.into && PIECES.includes(b.into))) return false;
  if (b.action === "swap" && b.side !== "own" && b.side !== "any") return false;
  const all = [...(b.kinds ?? []), ...(b.selector_kinds ?? []), ...(b.into ? [b.into] : [])];
  return all.every((k) => PIECES.includes(k));
}

/** Durée en coups : « 4 coups (2 de chaque camp) » ; `plies` compte les coups de chaque joueur. */
function duration(plies: number): string {
  return plies % 2 === 0
    ? t("fgtext.dur_even", { count: plies, half: plies / 2 })
    : t("fgtext.dur_odd", { count: plies });
}

function effectSentence(b: ForgedBricks & { side: string }): string {
  const dur = duration(b.plies ?? 0);
  // Retirer ou faire apparaître « un pion, cavalier ou dame » ; ramener « des pions, cavaliers ou dames ».
  const kinds = kindsList(b.kinds ?? [], b.action === "revive");
  const key =
    b.action === "morph" ? `morph_${b.side === "own" ? "own" : "enemy"}` : b.action === "swap" ? `swap_${b.side}` : b.action;
  return t(`fgtext.${key}`, { dur, kinds, into: b.into ? t(`fgtext.into_${b.into}`) : "" });
}

/** Description de la compétence dans la langue courante, du même contenu que celle du serveur. */
export function forgedDescription(def: Pick<ForgedDef, "description" | "bricks">): string {
  const b = def.bricks;
  if (!readable(b)) return def.description;
  const out = [effectSentence(b)];
  if (b.selector_kinds && b.selector_kinds.length > 0) {
    out.push(t("fgtext.sel_kinds", { kinds: kindsList(b.selector_kinds, true) }));
  }
  if (b.zone && b.zone !== "anywhere") out.push(t(`fgtext.zone_${b.zone}`));
  if (b.condition) out.push(t(`fgtext.cond_${b.condition}`));
  for (const c of b.constraints ?? []) out.push(t(`fgtext.cons_${c}`));
  if (b.action === "swap" && b.side === "any") out.push(t("fgtext.cons_forbid_check"));
  if (b.free_action) out.push(t("fgtext.free"));
  if ((b.max_uses ?? 1) > 1) out.push(t("fgtext.uses", { count: b.max_uses ?? 1 }));
  return out.join(" ");
}
