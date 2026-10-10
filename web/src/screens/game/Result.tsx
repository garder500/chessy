import { lazy, Suspense, useEffect, useRef } from "react";
import { levelById } from "../../campaign";
import type { CampaignResult, Color, EloChange, Outcome, PlacementView } from "../../protocol";
import { useT } from "../../i18n";
import { formatDelta, resultFor, resultHeadline } from "../../outcome";
import { navigate } from "../../router";
import { store, useAppState } from "../../store";
import { Beam } from "../../ui/Beam";
import { Confetti } from "../../ui/Confetti";
import { CountUp } from "../../ui/CountUp";
import { HeroPiece } from "../../ui/HeroPiece";
import { CampaignActions, CampaignSummary } from "./CampaignOver";
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
  /** Niveau de campagne : étoiles gagnées ; pas de revanche, la suite se lance d'ici. */
  campaign?: CampaignResult | null;
  rematch: "none" | "offered" | "received";
  /** Une récompense attend d'être choisie (victoire classée). */
  reward: boolean;
  gameId: string;
  onReward: () => void;
  onHide: () => void;
}

/** Fin de partie plein écran : la pièce sous le faisceau, le titre, l'Elo, puis l'étape suivante (récompense, revanche, analyse). */
export function Result({ outcome, you, rated, solo = false, elo, placement = null, campaign = null, rematch, reward, gameId, onReward, onHide }: Props) {
  const t = useT();
  const { title, reason } = resultHeadline(outcome, you);
  const result = resultFor(outcome, you);
  const delta = elo ? elo.you_after - elo.you_before : null;
  const first = useRef<HTMLElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);

  const levels = useAppState().campaign;
  const campaignLevel = campaign ? levelById(levels, campaign.level) : undefined;
  const placementLeft = placement ? placement.total - placement.done : 0;
  const cadence = campaign ? t("game.cad_campaign") : placement ? t("game.cad_placement", { done: placement.done, total: placement.total }) : solo ? t("game.mode_training") : t(rated ? "game.cad_rated" : "game.cad_friendly");
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
        <button type="button" className="rs-x" aria-label={t("game.close_home_aria")} onClick={leave}>
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
          {campaign ? (
            <CampaignSummary result={campaign} level={campaignLevel} />
          ) : placement ? (
            placement.elo != null ? (
              <span className="card rs-elo">
                <CountUp className="num rs-elo-n" to={placement.elo} />
                <span className="rs-delta">{t("game.elo_estimated")}</span>
              </span>
            ) : (
              <span className="muted rs-none">
                {t("game.placement_hidden", { done: placement.done, total: placement.total })}
              </span>
            )
          ) : !solo && rated && elo && delta !== null ? (
            <span className="card rs-elo">
              <CountUp className="num rs-elo-n" to={elo.you_after} />
              <span className={`rs-delta ${delta > 0 ? "up" : delta < 0 ? "down" : ""}`}>{formatDelta(delta)}</span>
            </span>
          ) : (
            <span className="muted rs-none">
              {t(solo ? "game.none_solo" : rated ? "game.none_rated" : "game.none_friendly")}
            </span>
          )}
          {reward && (
            <span className="card rs-reward">
              <span className="hex" style={{ width: 22, height: 25, background: "var(--rar-legendary)" }} aria-hidden="true" />
              {t("game.reward_pending")}
            </span>
          )}
        </div>
        {result === "loss" && rated && !solo && <p className="muted rs-lost">{t("game.lost_hint")}</p>}

        <div className="rs-act">
          {reward && (
            <button type="button" className="btn pri block" onClick={onReward}>
              {t("game.choose_reward")}
            </button>
          )}
          {campaign ? (
            <CampaignActions result={campaign} />
          ) : placement ? (
            <div className="rs-pair">
              {placementLeft > 0 && (
                <button type="button" className="btn pri" onClick={() => store.startPlacement()}>
                  {t("game.next_game")}
                </button>
              )}
              <button type="button" className={`btn${placementLeft > 0 ? "" : " pri"}`} onClick={() => analyse("analyse")}>
                {t("game.analyse")}
              </button>
            </div>
          ) : rematch === "received" ? (
            <>
              <p className="rs-msg">{t("game.rematch_received")}</p>
              <div className="rs-pair">
                <button type="button" className="btn pri" onClick={() => store.respondRematch(true)}>
                  {t("game.accept")}
                </button>
                <button type="button" className="btn" onClick={() => store.respondRematch(false)}>
                  {t("game.decline")}
                </button>
              </div>
            </>
          ) : (
            <div className="rs-pair">
              <button type="button" className={`btn${reward ? "" : " pri"}`} disabled={rematch === "offered"} onClick={() => store.requestRematch()}>
                {rematch === "offered" ? (solo ? t("game.rematch_new") : t("game.rematch_offered")) : t("game.rematch")}
              </button>
              <button type="button" className="btn" onClick={() => analyse("analyse")}>
                {t("game.analyse")}
              </button>
            </div>
          )}
          <div className="rs-links">
            <button type="button" className="link" onClick={onHide}>
              {t("game.review_board")}
            </button>
            <span aria-hidden="true">·</span>
            <button type="button" className="link" onClick={() => analyse()}>
              {t("game.review_game")}
            </button>
            <span aria-hidden="true" className="rs-home-sep">
              ·
            </span>
            <button type="button" className="link rs-home" onClick={leave}>
              {t("game.go_home")}
            </button>
          </div>
        </div>
      </main>
    </div>
  );
}
