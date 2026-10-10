import { challengeText, levelLabel, objectiveText } from "../../campaign";
import { useT } from "../../i18n";
import type { CampaignBanner } from "../../protocol";
import { Star } from "../../ui/Stars";

/** Bandeau au-dessus du plateau : l'objectif et le défi du niveau de campagne (voir `campaign.ts`). */
export function CampaignBar({ banner }: { banner: CampaignBanner }) {
  const t = useT();
  const chapter = Math.floor(banner.level / 10);
  const index = banner.level % 10;
  const label = levelLabel({ chapter, index, boss: index === 7 });
  return (
    <section className="cp-bar-game" aria-label={t("campaign.bar_aria", { level: label })}>
      <div>
        <b>{t("campaign.bar_objective")}</b>
        <Star on={false} size={16} />
        <span>{objectiveText(banner.objective)}</span>
      </div>
      <div>
        <b>{t("campaign.bar_challenge")}</b>
        <Star on={false} size={16} />
        <span>{challengeText(banner.challenge)}</span>
      </div>
    </section>
  );
}
