import type { BossForgeInfo, ClientMsg, SkillId } from "../../protocol";

/** Ce que l'écran de forge d'un boss montre, selon l'état serveur et la révélation déjà vue. */
export type BossForgeStep = "none" | "wait" | "reveal" | "choose" | "place";

export const FOLLOW_NOTE = "Elle vous suit en classée, où elle peut être perdue";

export function bossForgeStep(info: BossForgeInfo | null, revealed: boolean): BossForgeStep {
  if (!info || info.state === "placed") return "none";
  if (info.state === "forging") return "wait";
  if (!revealed) return "reveal";
  return info.deck_full ? "choose" : "place";
}

export function legendaryNote(info: BossForgeInfo): string | null {
  return info.legendary_unavailable ? "Plus aucune Légendaire disponible : Épique garantie" : null;
}

export function claimMsg(chapter: number): ClientMsg {
  return { type: "boss_forge_claim", chapter };
}

/** `replace` absent = le deck a de la place. */
export function placeMsg(chapter: number, replace: SkillId | null): ClientMsg {
  return replace === null ? { type: "boss_forge_place", chapter } : { type: "boss_forge_place", chapter, replace };
}

// Une révélation vue n'est pas rejouée tant que la page reste ouverte (« Plus tard » puis retour par la Collection).
const seen = new Set<string>();
const keyOf = (info: BossForgeInfo) => `${info.chapter}:${info.skill}`;

export const markRevealed = (info: BossForgeInfo) => void seen.add(keyOf(info));
export const wasRevealed = (info: BossForgeInfo | null) => !!info && seen.has(keyOf(info));
