// Noms de sons et classification (aucune dépendance Web Audio : utilisable dans les tests et côté UI).
import { t } from "../i18n";
import type { BuiltinSkillId, SkillId } from "../protocol";

export type SfxName =
  | "move"
  | "capture"
  | "check"
  | "castle"
  | "promote"
  | "illegal"
  | "your_turn"
  | "game_start"
  | "game_win"
  | "game_lose"
  | "game_draw"
  | "low_time"
  | "match_found"
  | "chat"
  | "friend_request"
  | "challenge"
  | "notice"
  | "ui_click"
  | "trap_sprung"
  | "shield"
  | "pushed"
  | "saved"
  | "vanish"
  | `skill_${SkillId}`;

export const SKILL_IDS: BuiltinSkillId[] = [
  "teleportation", "imune", "freeze", "rollback", "clone", "destiny_swapper", "remover", "wall", "mirage",
  "evolve", "switch", "mind", "control", "morph", "canceller", "tornado", "invisibility", "terminator",
  "trap", "bench", "forcefield", "transposition", "queensac", "temporal", "geomancy", "celestial", "godhelp",
];

export const skillSfx = (id: SkillId): SfxName => `skill_${id}`;

export const BASIC_SFX: SfxName[] = [
  "move", "capture", "check", "castle", "promote", "illegal", "your_turn", "game_start", "game_win", "game_lose",
  "game_draw", "low_time", "match_found", "chat", "friend_request", "challenge", "notice", "ui_click",
  "trap_sprung", "shield", "pushed", "saved", "vanish",
];

export const ALL_SFX: SfxName[] = [...BASIC_SFX, ...SKILL_IDS.map(skillSfx)];

/** Sons d'interface : désactivables avec le réglage `ui`. */
const UI_SET = new Set<SfxName>(["ui_click", "chat", "friend_request", "challenge", "notice", "match_found"]);
export const isUiSfx = (name: SfxName) => UI_SET.has(name);

/** Sons peu importants, abandonnés en premier quand toutes les voix sont prises. */
const LOW_SET = new Set<SfxName>(["ui_click", "low_time", "chat", "notice"]);
export const isLowPriority = (name: SfxName) => LOW_SET.has(name);

/** Nom lisible d'un effet sonore (clé `sfxnames.<nom>`) ; les sons de compétence prennent le nom de la compétence. */
export function sfxLabel(name: SfxName): string {
  return t(`sfxnames.${name}`);
}
