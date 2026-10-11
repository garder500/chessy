import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { useT } from "../i18n";
import { CATALOG } from "../catalog";
import type { LobbyStatus } from "../protocol";
import { store } from "../store";
import { timeText, useTime } from "../time";
import { Beam } from "../ui/Beam";
import { HeroPiece } from "../ui/HeroPiece";
import "./search.css";

// three.js n'est chargé qu'avec l'écran Jouer ; la silhouette SVG sert d'attente.
const HeroPiece3D = lazy(() => import("../ui/HeroPiece3D"));

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
  const t = useT();
  const elapsed = useElapsed(true);
  const time = useTime();
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
        <button type="button" className="sr-back" onClick={leave} aria-label={t("search.back_aria")}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M15 5l-7 7 7 7" />
          </svg>
        </button>
        <h1 className="sr-h1">{t("search.title")}</h1>
      </header>

      <div className="sr-mid" role="status">
        <div className="sr-radar" aria-hidden="true">
          <span className="sr-ping" />
          <span className="sr-ping sr-ping-2" />
          <span className="sr-ring" />
          <Suspense fallback={<HeroPiece kind={room ? "rook" : "king"} className="sr-piece" />}>
            <HeroPiece3D kind={room ? "rook" : "king"} className="sr-piece" fast />
          </Suspense>
        </div>
        {room ? (
          <>
            <p className="lb-code" data-testid="room-code" aria-label={t("search.room_code_aria", { code: lobby.code.split("").join(" ") })}>
              {lobby.code}
            </p>
            <h2 className="sr-title">{t("search.private_room")}</h2>
            <p className="sr-sub">{t("search.share_code")}</p>
            <button type="button" className="btn sm" onClick={() => copy(lobby.code)}>
              {copied ? t("search.code_copied") : t("search.copy_code")}
            </button>
          </>
        ) : (
          <>
            <p className="num sr-timer" role="timer" aria-label={t("search.timer_aria", { count: elapsed })}>
              {formatElapsed(elapsed)}
            </p>
            <h2 className="sr-title">{t("search.finding")}</h2>
            <p className="sr-sub">{lobby.ranked ? t("search.sub_ranked", { time: timeText(time) }) : t("search.sub_friendly", { time: timeText(time) })}</p>
            {lobby.ranked && <p className="sr-sub sr-fine">{t("search.elo_widens")}</p>}
          </>
        )}
        <div className="card sr-tip">
          <span className="hex sr-tip-hex" style={{ background: `var(--fam-${tip.family})` }} aria-hidden="true" />
          <span>
            <strong>{t("search.did_you_know")}</strong>
            <br />
            <span className="muted">
              {tip.name} : {tip.description}
            </span>
          </span>
        </div>
      </div>

      <div className="sr-act">
        <button type="button" className="btn block" onClick={leave}>
          {room ? t("search.close_room") : t("search.cancel_search")}
        </button>
      </div>
    </main>
  );
}
