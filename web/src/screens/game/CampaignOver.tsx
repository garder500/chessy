import { useState } from "react";
import { levelLabel, levelName, nextOpenLevel, readHand, levelById, starLines, starCount } from "../../campaign";
import { RARITY_LABEL } from "../../forged";
import { useT } from "../../i18n";
import type { CampaignLevel, CampaignResult } from "../../protocol";
import { navigate } from "../../router";
import { store, useAppState } from "../../store";
import { Star } from "../../ui/Stars";
import { CampaignForge } from "../CampaignForge";

/** Ce qu'une partie de campagne a rapporté : les trois étoiles, ce qui a été réussi, l'avancement. */
export function CampaignSummary({ result, level }: { result: CampaignResult; level: CampaignLevel | undefined }) {
  const t = useT();
  const won = result.earned > 0;
  const earned = starCount(result.earned);
  return (
    <div className="cp-over" role="group" aria-label={t("campaign.over_aria")}>
      {level && (
        <p className="cp-over-lvl">
          {levelLabel(level)} · {levelName(level)}
        </p>
      )}
      <div className="cp-over-stars" aria-hidden="true">
        {[1, 2, 4].map((bit, i) => (
          <span key={bit} className={`cp-over-star${result.earned & bit ? " on" : ""}`} style={{ animationDelay: `${0.5 + i * 0.28}s` }}>
            <Star on={!!(result.earned & bit)} size={44} />
          </span>
        ))}
      </div>
      <p className="cp-over-count" role="status">
        {won ? t("campaign.over_stars", { count: earned }) : t("campaign.over_lost")}
        {won && result.gained > 0 ? ` · ${t("campaign.over_gained", { count: result.gained })}` : ""}
      </p>
      {level && won && (
        <ul className="cp-star-list cp-over-list">
          {starLines(level, result.earned).map((l) => (
            <li key={l.kind} className={l.got ? "got" : ""}>
              <Star on={l.got} size={18} />
              <span>{l.text}</span>
              <span className="sr-only">{l.got ? t("campaign.earned") : t("campaign.not_earned")}</span>
            </li>
          ))}
        </ul>
      )}
      {result.boss_opened && <p className="chip cp-over-chip">{t("campaign.over_boss_open")}</p>}
      {result.forge && <p className="chip cp-over-chip forge">{t("campaign.over_forge", { rarity: RARITY_LABEL[result.forge.min] })}</p>}
    </div>
  );
}

/** Boutons de fin de niveau : forge due, niveau suivant ou rejouer, retour à la carte. */
export function CampaignActions({ result }: { result: CampaignResult }) {
  const t = useT();
  const { campaign: levels, deck } = useAppState();
  const [forgeOpen, setForgeOpen] = useState(false);
  const level = levelById(levels, result.level);
  const won = result.earned > 0;
  const next = levels ? nextOpenLevel(levels, result.level) : undefined;
  const boss = result.forge && levels ? levels.find((l) => l.boss && l.chapter === result.forge!.chapter && l.forge_pending) : undefined;

  // Un niveau où le joueur choisit sa main repart avec la dernière main si elle existe, sinon on passe par la carte.
  const play = (target: CampaignLevel) => {
    if (!target.choose) return store.startCampaign(target.id);
    const hand = readHand(deck);
    if (hand.length > 0) return store.startCampaign(target.id, hand);
    store.leaveGame();
    navigate({ name: "campaign", param: String(target.id) });
  };
  const toMap = () => {
    store.leaveGame();
    navigate({ name: "campaign", param: String(result.level) });
  };

  const main = won && next && !boss ? next : level;
  return (
    <>
      {boss && (
        <button type="button" className="btn pri block" onClick={() => setForgeOpen(true)}>
          {t("campaign.forge_claim")}
        </button>
      )}
      <div className="rs-pair">
        {main && (
          <button type="button" className={`btn${boss ? "" : " pri"}`} onClick={() => play(main)}>
            {main === next ? t("campaign.next_level") : won ? t("campaign.replay") : t("campaign.retry")}
          </button>
        )}
        <button type="button" className="btn" onClick={toMap}>
          {t("campaign.back_to_map")}
        </button>
      </div>
      {won && next && main === next && level && (
        <button type="button" className="link" onClick={() => play(level)}>
          {t("campaign.replay")}
        </button>
      )}
      {forgeOpen && boss && <CampaignForge boss={boss} deck={deck} onClose={() => setForgeOpen(false)} />}
    </>
  );
}
