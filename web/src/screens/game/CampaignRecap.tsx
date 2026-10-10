import { STAR_LABELS } from "../../campaign";
import type { CampaignResult } from "../../protocol";
import { Stars } from "../../ui/Stars";

const BOSS_LEVEL = 6;

/** Étoiles de la partie (pleines) face aux meilleures déjà acquises (atténuées), et progression du chapitre. */
export function CampaignRecap({ campaign }: { campaign: CampaignResult }) {
  return (
    <div className="card rs-campaign">
      <Stars stars={campaign.stars} kept={campaign.best} size={28} />
      <p className="muted rs-campaign-labels">{STAR_LABELS.filter((_, i) => campaign.stars[i]).join(" · ") || "Aucune étoile cette fois"}</p>
      <p className="rs-campaign-chapter">
        {campaign.chapter_stars} ★ dans ce chapitre
        {campaign.boss_unlocked && campaign.level !== BOSS_LEVEL && <strong> · Boss débloqué</strong>}
      </p>
    </div>
  );
}
