import { BOSS_LEVEL, STAR_LABELS } from "../../campaign";
import type { CampaignResult } from "../../protocol";
import { Stars } from "../../ui/Stars";

/** Progression vers le boss, ou annonce de son déverrouillage. */
function BossStatus({ campaign }: { campaign: CampaignResult }) {
  if (campaign.boss_just_unlocked) return <strong> · Boss débloqué !</strong>;
  if (campaign.boss_unlocked) return null;
  return <span className="muted"> · Boss : {campaign.chapter_stars}/{campaign.boss_stars_required} ★</span>;
}

/** Étoiles de la partie (pleines) face aux meilleures déjà acquises (atténuées), et progression du chapitre. */
export function CampaignRecap({ campaign }: { campaign: CampaignResult }) {
  return (
    <div className="card rs-campaign">
      <Stars stars={campaign.stars} kept={campaign.best} size={28} boss={campaign.level === BOSS_LEVEL} />
      <p className="muted rs-campaign-labels">{STAR_LABELS.filter((_, i) => campaign.stars[i]).join(" · ") || "Aucune étoile cette fois"}</p>
      {campaign.title && <p className="rs-campaign-chapter"><strong>Titre obtenu : {campaign.title}</strong></p>}
      <p className="rs-campaign-chapter">
        {campaign.chapter_stars} ★ dans ce chapitre
        <BossStatus campaign={campaign} />
      </p>
    </div>
  );
}
