import { lazy, Suspense, useEffect, useRef } from "react";
import type { Color, EloChange, Outcome, PlacementView } from "../../protocol";
import { formatDelta, resultFor, resultHeadline } from "../../outcome";
import { navigate } from "../../router";
import { store } from "../../store";
import { Beam } from "../../ui/Beam";
import { Confetti } from "../../ui/Confetti";
import { CountUp } from "../../ui/CountUp";
import { HeroPiece } from "../../ui/HeroPiece";
import "./result.css";

const HeroPiece3D = lazy(() => import("../../ui/HeroPiece3D"));

interface Props {
  outcome: Outcome;
  you: Color;
  rated: boolean;
  /** Partie contre l'IA : ni Elo ni récompense, revanche immédiate. */
  solo?: boolean;
  elo: EloChange | null;
  /** Partie d'évaluation : pas de revanche, la suivante se lance d'ici. */
  placement?: PlacementView | null;
  rematch: "none" | "offered" | "received";
  /** Une récompense attend d'être choisie (victoire classée). */
  reward: boolean;
  gameId: string;
  onReward: () => void;
  onHide: () => void;
}

/** Fin de partie plein écran : la pièce sous le faisceau, le titre, l'Elo, puis l'étape suivante (récompense, revanche, analyse). */
export function Result({ outcome, you, rated, solo = false, elo, placement = null, rematch, reward, gameId, onReward, onHide }: Props) {
  const { title, reason } = resultHeadline(outcome, you);
  const result = resultFor(outcome, you);
  const delta = elo ? elo.you_after - elo.you_before : null;
  const first = useRef<HTMLElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);

  const placementLeft = placement ? placement.total - placement.done : 0;
  const cadence = placement ? `Évaluation · ${placement.done}/${placement.total}` : solo ? "Entraînement" : `${rated ? "Classée" : "Amicale"} · 10 min`;
  const leave = () => store.leaveGame();
  const analyse = (sub?: "analyse") => {
    store.leaveGame();
    navigate({ name: "replay", param: gameId, ...(sub ? { sub } : {}) });
  };

  return (
    <div className={`rs ${result ?? ""}`} role="dialog" aria-modal="true" aria-labelledby="rs-title">
      {result === "win" && !solo && <Confetti />}
      {result !== "loss" && <Beam width={560} height={470} />}
      <header className="rs-top">
        <button type="button" className="rs-x" aria-label="Fermer et revenir à l'accueil" onClick={leave}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
        <span className="lab rs-cad">{cadence}</span>
      </header>

      <div className="rs-hero">
        <Suspense fallback={<HeroPiece kind={result === "draw" ? "pawn" : "king"} className="rs-piece" />}>
          <HeroPiece3D kind={result === "draw" ? "pawn" : "king"} className="rs-piece" />
        </Suspense>
      </div>

      <main className="rs-main">
        <h1 id="rs-title" className="rs-title" tabIndex={-1} ref={first as React.RefObject<HTMLHeadingElement>}>
          {title}
        </h1>
        <p className="rs-reason">{reason}</p>

        <div className="rs-chips">
          {placement ? (
            placement.elo != null ? (
              <span className="card rs-elo">
                <CountUp className="num rs-elo-n" to={placement.elo} />
                <span className="rs-delta">Elo estimé</span>
              </span>
            ) : (
              <span className="muted rs-none">
                Partie d'évaluation {placement.done}/{placement.total} : l'Elo de l'adversaire reste caché, votre Elo sera estimé après la dernière.
              </span>
            )
          ) : !solo && rated && elo && delta !== null ? (
            <span className="card rs-elo">
              <CountUp className="num rs-elo-n" to={elo.you_after} />
              <span className={`rs-delta ${delta > 0 ? "up" : delta < 0 ? "down" : ""}`}>{formatDelta(delta)}</span>
            </span>
          ) : (
            <span className="muted rs-none">
              {solo ? "Partie d'entraînement : ni Elo ni récompense en jeu." : rated ? "Partie classée non comptabilisée : ni Elo ni récompense." : "Partie amicale : ni Elo ni récompense en jeu."}
            </span>
          )}
          {reward && (
            <span className="card rs-reward">
              <span className="hex" style={{ width: 22, height: 25, background: "var(--rar-legendary)" }} aria-hidden="true" />
              Récompense à choisir
            </span>
          )}
        </div>
        {result === "loss" && rated && !solo && <p className="muted rs-lost">Battez votre adversaire en classée pour reprendre une compétence perdue.</p>}

        <div className="rs-act">
          {reward && (
            <button type="button" className="btn pri block" onClick={onReward}>
              Choisir ma récompense
            </button>
          )}
          {placement ? (
            <div className="rs-pair">
              {placementLeft > 0 && (
                <button type="button" className="btn pri" onClick={() => store.startPlacement()}>
                  Partie suivante
                </button>
              )}
              <button type="button" className={`btn${placementLeft > 0 ? "" : " pri"}`} onClick={() => analyse("analyse")}>
                Analyser
              </button>
            </div>
          ) : rematch === "received" ? (
            <>
              <p className="rs-msg">L'adversaire propose une revanche.</p>
              <div className="rs-pair">
                <button type="button" className="btn pri" onClick={() => store.respondRematch(true)}>
                  Accepter
                </button>
                <button type="button" className="btn" onClick={() => store.respondRematch(false)}>
                  Refuser
                </button>
              </div>
            </>
          ) : (
            <div className="rs-pair">
              <button type="button" className={`btn${reward ? "" : " pri"}`} disabled={rematch === "offered"} onClick={() => store.requestRematch()}>
                {rematch === "offered" ? (solo ? "Nouvelle partie…" : "Revanche proposée…") : "Revanche"}
              </button>
              <button type="button" className="btn" onClick={() => analyse("analyse")}>
                Analyser
              </button>
            </div>
          )}
          <div className="rs-links">
            <button type="button" className="link" onClick={onHide}>
              Revoir le plateau
            </button>
            <span aria-hidden="true">·</span>
            <button type="button" className="link" onClick={() => analyse()}>
              Revoir la partie
            </button>
            <span aria-hidden="true" className="rs-home-sep">
              ·
            </span>
            <button type="button" className="link rs-home" onClick={leave}>
              Retour à l'accueil
            </button>
          </div>
        </div>
      </main>
    </div>
  );
}
