import { totalLabel } from "../../campaign";

interface Props {
  totalStars: number;
  maxStars: number;
  title: string | null;
}

/** Titre de la page, barre de progression globale et dernier titre gagné. */
export function CampaignHeader({ totalStars, maxStars, title }: Props) {
  return (
    <div className="cp-progress">
      <div className="cp-progress-main">
        <div className="cp-progress-row">
          <span>Progression</span>
          <span className="mono cp-total">{totalLabel(totalStars, maxStars)}</span>
        </div>
        <div className="cp-bar" role="progressbar" aria-valuemin={0} aria-valuemax={maxStars} aria-valuenow={totalStars}>
          <span style={{ width: `${(totalStars / maxStars) * 100}%` }} />
        </div>
      </div>
      <div className="cp-title-badge">
        <span className="lab">Titre</span>
        <strong>{title ?? "Aucun pour l'instant"}</strong>
      </div>
    </div>
  );
}
