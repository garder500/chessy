import { store, useAppState } from "../../store";

/** Raccourcis de dev : termine la partie de campagne en cours par une victoire ou une défaite. */
export function DevFinish() {
  const { campaignGame } = useAppState();
  if (!import.meta.env.DEV || !campaignGame) return null;
  return (
    <div className="gm-row">
      <button type="button" className="btn sm" onClick={() => store.send({ type: "dev_finish", win: true })}>
        Gagner (dev)
      </button>
      <button type="button" className="btn sm" onClick={() => store.send({ type: "dev_finish", win: false })}>
        Perdre (dev)
      </button>
    </div>
  );
}
