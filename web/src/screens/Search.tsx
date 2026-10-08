import { useEffect, useMemo, useState } from "react";
import { CATALOG } from "../catalog";
import type { LobbyStatus } from "../protocol";
import { store } from "../store";
import { Beam } from "../ui/Beam";
import { HeroPiece } from "../ui/HeroPiece";
import "./search.css";

/** Secondes écoulées depuis que `active` est devenu vrai. */
export function useElapsed(active: boolean): number {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (!active) return;
    setSeconds(0);
    const timer = setInterval(() => setSeconds((s) => s + 1), 1000);
    return () => clearInterval(timer);
  }, [active]);
  return seconds;
}

export function formatElapsed(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/** Recherche d'un adversaire (file classée ou amicale) ou salle privée en attente : un seul écran, une seule décision (annuler). */
export function Search({ lobby }: { lobby: Exclude<LobbyStatus, { type: "idle" }> }) {
  const elapsed = useElapsed(true);
  const [copied, setCopied] = useState(false);
  // Un conseil tiré une fois par recherche : la description d'une compétence du catalogue.
  const tip = useMemo(() => CATALOG[Math.floor(Math.random() * CATALOG.length)], []);
  const room = lobby.type === "room_waiting";

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      // Presse-papiers indisponible : le code reste lisible à l'écran.
    }
  };
  const leave = () => store.send({ type: "leave_lobby" });

  return (
    <main className="sr">
      <Beam width={620} height={560} />
      <header className="sr-top">
        <button type="button" className="sr-back" onClick={leave} aria-label="Annuler et revenir">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M15 5l-7 7 7 7" />
          </svg>
        </button>
        <h1 className="sr-h1">Recherche</h1>
      </header>

      <div className="sr-mid" role="status">
        <div className="sr-radar" aria-hidden="true">
          <span className="sr-ping" />
          <span className="sr-ring" />
          <HeroPiece kind={room ? "rook" : "king"} className="sr-piece" />
        </div>
        {room ? (
          <>
            <p className="lb-code" data-testid="room-code" aria-label={`Code de salle ${lobby.code.split("").join(" ")}`}>
              {lobby.code}
            </p>
            <h2 className="sr-title">Salle privée</h2>
            <p className="sr-sub">Partagez ce code avec votre adversaire.</p>
            <button type="button" className="btn sm" onClick={() => copy(lobby.code)}>
              {copied ? "Code copié" : "Copier le code"}
            </button>
          </>
        ) : (
          <>
            <p className="num sr-timer" role="timer" aria-label={`Temps d'attente : ${elapsed} secondes`}>
              {formatElapsed(elapsed)}
            </p>
            <h2 className="sr-title">Recherche d'un adversaire</h2>
            <p className="sr-sub">{lobby.ranked ? "Classée · adversaire de force proche · 10 min + 3 s" : "Amicale · 10 min + 3 s"}</p>
            {lobby.ranked && <p className="sr-sub sr-fine">La plage d'Elo s'élargit peu à peu pendant l'attente.</p>}
          </>
        )}
        <div className="card sr-tip">
          <span className="hex sr-tip-hex" style={{ background: `var(--fam-${tip.family})` }} aria-hidden="true" />
          <span>
            <strong>Le saviez-vous ?</strong>
            <br />
            <span className="muted">
              {tip.name} : {tip.description}
            </span>
          </span>
        </div>
      </div>

      <div className="sr-act">
        <button type="button" className="btn block" onClick={leave}>
          {room ? "Fermer la salle" : "Annuler la recherche"}
        </button>
      </div>
    </main>
  );
}
