import { STAR_LABELS } from "../../campaign";
import type { CampaignResult, Outcome } from "../../protocol";
import { Stars } from "../../ui/Stars";

const MAX_STARS = 105;

/** En campagne, une nulle ne vaut pas une victoire : on dit pourquoi. */
const DRAW_EXPLANATIONS: Partial<Record<Outcome["type"], string>> = {
  stalemate: "Pat : aux échecs, c'est une nulle.",
  fifty_moves: "Règle des 50 coups : aux échecs, c'est une nulle.",
  repetition: "Triple répétition : aux échecs, c'est une nulle.",
  insufficient_material: "Matériel insuffisant : aux échecs, c'est une nulle.",
};

/** Progression vers le boss, ou annonce de son déverrouillage. */
function BossStatus({ campaign }: { campaign: CampaignResult }) {
  if (campaign.boss_just_unlocked) return <strong> · Boss débloqué !</strong>;
  if (campaign.boss_unlocked) return null;
  return <span className="muted"> · Boss : {campaign.chapter_stars}/{campaign.boss_stars_required} ★</span>;
}

function Total({ campaign }: { campaign: CampaignResult }) {
  return <p className="rs-campaign-chapter">{campaign.total_stars} / {MAX_STARS} ★ au total</p>;
}

function Briefing({ onBriefing }: { onBriefing: () => void }) {
  return (
    <button type="button" className="link" onClick={onBriefing}>
      Relire le briefing
    </button>
  );
}

interface Props {
  campaign: CampaignResult;
  outcome: Outcome;
  won: boolean;
  /** Retour à la fiche du niveau, qui porte le conseil. */
  onBriefing: () => void;
}

/** Victoire : étoiles gagnées (pleines) et déjà acquises (atténuées), cumul et titre. Sinon : pourquoi c'est un échec. */
export function CampaignRecap({ campaign, outcome, won, onBriefing }: Props) {
  if (!won) {
    return (
      <div className="card rs-campaign">
        <p>{DRAW_EXPLANATIONS[outcome.type] ?? (outcome.type === "resignation" ? "Un abandon compte comme une défaite, sans pénalité." : "Aucune étoile cette fois.")}</p>
        <Total campaign={campaign} />
        {campaign.hint_available && <Briefing onBriefing={onBriefing} />}
      </div>
    );
  }
  return (
    <div className="card rs-campaign">
      <Stars stars={campaign.stars} kept={campaign.best} size={28} />
      <p className="muted rs-campaign-labels">{STAR_LABELS.filter((_, i) => campaign.stars[i]).join(" · ") || "Aucune étoile de plus cette fois"}</p>
      {campaign.title && <p className="rs-campaign-chapter"><strong>Titre obtenu : {campaign.title}</strong></p>}
      <p className="rs-campaign-chapter">
        {campaign.chapter_stars} ★ dans ce chapitre
        <BossStatus campaign={campaign} />
      </p>
      <Total campaign={campaign} />
    </div>
  );
}
