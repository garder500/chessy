import { useEffect, useState, type CSSProperties } from "react";
import { bossName } from "../campaign";
import { RARITY_LABEL } from "../forged";
import { useT } from "../i18n";
import type { CampaignLevel, SkillId } from "../protocol";
import { skillInfo } from "../skills";
import { store } from "../store";
import { Sheet } from "../ui/Sheet";
import { SkillArt } from "../ui/SkillArt";
import { UniqueBadge } from "../ui/UniqueBadge";
import { tileRarity } from "../ui/tileRarity";

const MAX_DECK = 7;

/** La forge qu'un boss vaincu doit au joueur : rareté minimale, tirage, et la compétence à rendre quand le deck est plein. */
export function CampaignForge({ boss, deck, onClose }: { boss: CampaignLevel; deck: SkillId[]; onClose: () => void }) {
  const t = useT();
  const [replace, setReplace] = useState<SkillId | undefined>();
  // Le serveur forge hors de son verrou, ce qui prend un instant ; s'il refuse, on rend la main.
  const [forging, setForging] = useState(false);
  const full = deck.length >= MAX_DECK;
  const min = boss.forge_min ?? "common";
  useEffect(() => {
    if (!forging) return;
    const timer = setTimeout(() => setForging(false), 15_000);
    return () => clearTimeout(timer);
  }, [forging]);

  return (
    <Sheet open title={t("campaign.forge_title")} onClose={() => !forging && onClose()}>
      <div className="cp-forge">
        <p className="muted">{t("campaign.forge_sub", { boss: bossName(boss.chapter), rarity: RARITY_LABEL[min] })}</p>
        <ul className="cp-odds" aria-label={t("campaign.forge_odds_aria")}>
          {(boss.forge_odds ?? []).map((o) => (
            <li key={o.rarity} style={{ "--rar": `var(--rar-${o.rarity})` } as CSSProperties}>
              <span className="hex" aria-hidden="true" style={{ width: 14, height: 16, background: "var(--rar)" }} />
              <span>{RARITY_LABEL[o.rarity]}</span>
              <span className="num">{o.percent} %</span>
            </li>
          ))}
        </ul>
        {full && (
          <div className="rw-sec">
            <p className="rw-label" id="cp-replace">
              {t("campaign.forge_full", { n: deck.length })}
            </p>
            <div className="rw-replace" role="radiogroup" aria-labelledby="cp-replace">
              {deck.map((skill) => {
                const info = skillInfo(skill);
                const on = replace === skill;
                return (
                  <button
                    key={skill}
                    type="button"
                    role="radio"
                    aria-checked={on}
                    className={`slot rw-slot${on ? " on" : ""}${info.unique ? " foil" : ""}`}
                    style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}
                    disabled={forging}
                    onClick={() => setReplace(skill)}
                  >
                    <span className="slot-art" data-rar={tileRarity(skill)}>
                      <SkillArt id={skill} size={46} />
                      {info.unique && <UniqueBadge />}
                    </span>
                    <span className="slot-name">{info.name}</span>
                    <span className="skc-ring rw-ring" aria-hidden="true" />
                  </button>
                );
              })}
            </div>
          </div>
        )}
        <button
          type="button"
          className="btn pri block"
          disabled={forging || (full && !replace)}
          onClick={() => {
            setForging(true);
            store.claimCampaignForge(full ? replace : undefined);
          }}
        >
          {forging ? t("campaign.forge_forging") : t("campaign.forge_go")}
        </button>
        <p className="muted cp-note" role="status">
          {full && !replace ? t("campaign.forge_pick") : t("campaign.forge_keep")}
        </p>
      </div>
    </Sheet>
  );
}
