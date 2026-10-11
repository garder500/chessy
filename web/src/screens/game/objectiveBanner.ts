import type { CampaignContext } from "../../protocol";

/**
 * Numéro de coup (notation) que le joueur est en train de jouer. `ply` compte les tours passés : une compétence
 * qui coûte le tour avance le compteur, Mind Reading et Mind Control (gratuites) non. Blancs et noirs partagent
 * le numéro de leur paire, comme dans le replay.
 */
export function moveNumber(ply: number): number {
  return Math.floor(ply / 2) + 1;
}

export interface BannerModel {
  objective: string | null;
  challenge: string | null;
  /** « Coup N / limite », absent quand le niveau n'a pas de limite. */
  counter: { move: number; limit: number; exceeded: boolean } | null;
}

export function bannerModel(campaign: CampaignContext, ply: number): BannerModel {
  const move = moveNumber(ply);
  const limit = campaign.move_limit;
  return {
    objective: campaign.objective,
    challenge: campaign.challenge,
    // « avant le coup 30 » : à N = 30 il est déjà trop tard.
    counter: limit === null ? null : { move, limit, exceeded: move >= limit },
  };
}
