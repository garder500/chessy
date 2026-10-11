import type { CampaignContext } from "../../protocol";
import { bannerModel } from "./objectiveBanner";

/** Objectif, défi et compteur de coups du niveau de campagne en cours. */
export function ObjectiveBanner({ campaign, ply }: { campaign: CampaignContext; ply: number }) {
  const { objective, challenge, counter } = bannerModel(campaign, ply);
  if (!objective && !challenge && !counter) return null;
  return (
    <div className="gm-objective" role="status" aria-label="Objectif du niveau">
      {counter && (
        <strong className={`gm-objective-count mono${counter.exceeded ? " over" : ""}`}>
          Coup {counter.move} / {counter.limit}
        </strong>
      )}
      <span className="gm-objective-text">
        {objective}
        {challenge && <span className="muted"> · {challenge}</span>}
      </span>
    </div>
  );
}
