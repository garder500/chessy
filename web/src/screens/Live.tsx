import { useEffect, useState } from "react";
import { api, apiErrorText } from "../api";
import { useT } from "../i18n";
import type { LiveGame, Seat } from "../protocol";
import { COLOR_FR, seatName } from "../replay/frames";
import { liveTag, plyText, sortLive, spectatorsText } from "../replay/lists";
import { hrefFor } from "../router";
import { readToken } from "../store";
import { initialOf } from "../ui/social";
import "./live.css";

/** Rafraîchissement de la liste (docs/spec-v4.md §5). */
export const LIVE_POLL_MS = 5000;

export function Live() {
  const t = useT();
  const [games, setGames] = useState<LiveGame[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const ctl = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const run = async () => {
      // Onglet masqué : on saute ce tour, la liste se remettra à jour au retour.
      if (!document.hidden) {
        try {
          const res = await api.live(50, readToken(), ctl.signal);
          if (ctl.signal.aborted) return;
          setGames(sortLive(res.games ?? []));
          setError(null);
        } catch (e) {
          if (ctl.signal.aborted) return;
          setError(apiErrorText(e));
        }
      }
      if (!ctl.signal.aborted) timer = setTimeout(run, LIVE_POLL_MS);
    };
    void run();
    return () => {
      ctl.abort();
      clearTimeout(timer);
    };
  }, [attempt]);

  return (
    <main className="lv-page">
      <header className="lv-head">
        <div>
          <p className="eyebrow">{t("live.eyebrow")}</p>
          <h1 className="lv-title">{t("live.title")}</h1>
        </div>
        <p className="muted lv-sub" aria-live="polite">
          {games ? t("live.count", { count: games.length }) : ""}
        </p>
      </header>

      {games === null && !error && (
        <div className="lv-state card" role="status" aria-live="polite">
          <span className="rp-spinner" aria-hidden="true" />
          <p>{t("live.loading")}</p>
        </div>
      )}

      {games === null && error && (
        <div className="lv-state card" role="alert">
          <p>{error}</p>
          <button type="button" className="btn" onClick={() => setAttempt((n) => n + 1)}>
            {t("live.retry")}
          </button>
        </div>
      )}

      {games !== null && error && (
        <p className="lv-warn" role="status">
          {t("live.refresh_failed", { error })}
        </p>
      )}

      {games !== null && games.length === 0 && (
        <div className="lv-state card">
          <p>{t("live.empty_title")}</p>
          <p className="muted">{t("live.empty_sub")}</p>
          <a className="btn" href="#/">
            {t("live.play")}
          </a>
        </div>
      )}

      {games !== null && games.length > 0 && (
        <ul className="lv-list">
          {games.map((g) => (
            <LiveCard key={g.game_id} game={g} />
          ))}
        </ul>
      )}
    </main>
  );
}

function Player({ seat, side }: { seat: Seat; side: "white" | "black" }) {
  const t = useT();
  const name = seatName(seat);
  return (
    <span className="lv-player">
      <span className={`lv-side ${side}`} aria-hidden="true" />
      <span className="avatar sm" aria-hidden="true">
        {seat.bot ? t("replay.seat_bot") : initialOf(name)}
      </span>
      <span className="lv-name">
        <strong>{name}</strong>
        <span className="mono muted">{seat.elo !== null ? (seat.bot ? t("live.level", { elo: seat.elo }) : seat.elo) : "—"}</span>
      </span>
      <span className="sr-only"> ({COLOR_FR[side]})</span>
    </span>
  );
}

function LiveCard({ game: g }: { game: LiveGame }) {
  const t = useT();
  return (
    <li className="lv-card card">
      <div className="lv-players">
        <Player seat={g.white} side="white" />
        <span className="lv-vs muted" aria-hidden="true">
          {t("live.vs")}
        </span>
        <Player seat={g.black} side="black" />
      </div>
      <div className="lv-meta">
        <span className={`tag lv-tag ${g.kind === "solo" ? "solo" : g.rated ? "ranked" : ""}`}>{liveTag(g)}</span>
        <span className="muted">{plyText(g.ply)}</span>
        <span className="muted">{spectatorsText(g.spectators)}</span>
      </div>
      <a className="btn sm lv-watch" href={hrefFor({ name: "watch", param: g.game_id })} aria-label={t("live.watch_aria", { white: seatName(g.white), black: seatName(g.black) })}>
        {t("live.watch")}
      </a>
    </li>
  );
}
