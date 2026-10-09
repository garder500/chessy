import type { CSSProperties } from "react";
import { useT } from "../i18n";
import { RARITY_LABEL, type Rarity } from "../forged";

const GEMS: Record<Rarity, number> = { common: 1, uncommon: 2, rare: 3, epic: 4, legendary: 5 };

/** Étiquette de rareté d'une compétence forgée : le nombre de gemmes double la couleur. */
export function RarityTag({ rarity }: { rarity: Rarity }) {
  useT();
  return (
    <span className="tag rar-tag" style={{ "--rar": `var(--rar-${rarity})` } as CSSProperties}>
      <span className="gems" aria-hidden="true">
        {"◆".repeat(GEMS[rarity])}
      </span>
      {RARITY_LABEL[rarity]}
    </span>
  );
}
