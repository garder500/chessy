import { store, useAppState } from "../../store";

const RESULTS = [
  { result: "win", label: "Gagner (dev)" },
  { result: "all_stars", label: "Gagner 3 étoiles (dev)" },
  { result: "loss", label: "Perdre (dev)" },
] as const;

/** Raccourcis de dev : termine la partie de campagne en cours par une victoire, une victoire à 3 étoiles ou une défaite. */
export function DevFinish() {
  const { campaignGame } = useAppState();
  if (!import.meta.env.DEV || !campaignGame) return null;
  return (
    <div className="gm-row">
      {RESULTS.map(({ result, label }) => (
        <button key={result} type="button" className="btn sm" onClick={() => store.send({ type: "dev_finish", result })}>
          {label}
        </button>
      ))}
    </div>
  );
}
