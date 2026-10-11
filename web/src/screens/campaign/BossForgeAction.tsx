import type { BossForgeInfo } from "../../protocol";
import { store } from "../../store";

interface Props {
  chapter: number;
  forge: BossForgeInfo | null;
  /** Ouvre la révélation de la compétence forgée ; branchée par l'écran qui la porte. */
  onReveal: (chapter: number) => void;
}

export function BossForgeAction({ chapter, forge, onReveal }: Props) {
  if (forge?.state === "forging") {
    return (
      <button type="button" className="btn pri cp-forge-btn" onClick={() => store.send({ type: "boss_forge_claim", chapter })}>
        Récupérer la forge
      </button>
    );
  }
  if (forge?.state === "pending") {
    return (
      <button type="button" className="btn pri cp-forge-btn" onClick={() => onReveal(chapter)}>
        Révéler
      </button>
    );
  }
  return null;
}
