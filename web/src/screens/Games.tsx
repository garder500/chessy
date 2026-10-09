import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, gameErrorText } from "../api";
import { useT } from "../i18n";
import type { GameSummary } from "../protocol";
import {
  countByFilter,
  emptyGamesText,
  filterGames,
  GAMES_FILTERS,
  GAMES_PAGE,
  hasMore,
  mergePage,
  opponentLabel,
  showsDelta,
  type GamesFilter,
} from "../replay/lists";
import { colorCap, kindLabel } from "../replay/frames";
import { hrefFor } from "../router";
import { readToken, useAppState } from "../store";
import { formatDelta, reasonText, relativeTime, RESULT_LABEL } from "../ui/social";
import "./live.css";

type Status = "loading" | "ready" | "error";

/** Page `#/games` : mes parties terminées, paginées, avec accès au replay et à l'analyse. */
export function Games() {
  const t = useT();
  const { account } = useAppState();
  const accountId = account?.player_id ?? null;
  const guest = account?.guest ?? false;

  const [filter, setFilter] = useState<GamesFilter>("all");
  const [games, setGames] = useState<GameSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [lastPage, setLastPage] = useState(GAMES_PAGE);
  const [status, setStatus] = useState<Status>("loading");
  const [more, setMore] = useState(false);
  const [error, setError] = useState("");
  const ctl = useRef<AbortController | null>(null);

  const load = useCallback(async (offset: number) => {
    const token = readToken();
    ctl.current?.abort();
    const c = new AbortController();
    ctl.current = c;
    if (offset === 0) setStatus("loading");
    else setMore(true);
    try {
      const res = await api.myGames(token ?? "", GAMES_PAGE, offset, c.signal);
      if (c.signal.aborted) return;
      setGames((cur) => mergePage(cur, res.games, offset));
      setTotal(res.total);
      setLastPage(res.games.length);
      setStatus("ready");
      setError("");
    } catch (e) {
      if (c.signal.aborted) return;
      setError(gameErrorText(e));
      if (offset === 0) setStatus("error");
    } finally {
      if (!c.signal.aborted) setMore(false);
    }
  }, []);

  // On attend le `welcome` : c'est lui qui fixe le jeton de session.
  useEffect(() => {
    if (accountId === null) return;
    void load(0);
    return () => ctl.current?.abort();
  }, [accountId, load]);

  const counts = useMemo(() => countByFilter(games), [games]);
  const shown = useMemo(() => filterGames(games, filter), [games, filter]);
  const canMore = hasMore(games.length, total, lastPage);

  return (
    <main className="lv-page">
      <header className="lv-head">
        <div>
          <p className="eyebrow">{t("games.eyebrow")}</p>
          <h1 className="lv-title">{t("games.title")}</h1>
        </div>
        <div className="seg lv-filter" role="group" aria-label={t("games.filter_aria")}>
          {GAMES_FILTERS.map((f) => (
            <button key={f.id} type="button" className={filter === f.id ? "on" : undefined} aria-pressed={filter === f.id} onClick={() => setFilter(f.id)}>
              {f.label}
              {status === "ready" && <span className="mono lv-count"> {counts[f.id]}</span>}
            </button>
          ))}
        </div>
      </header>

      {guest && (
        <p className="lv-warn" role="note">
          {t("games.guest_before")} <a href={hrefFor({ name: "auth" })}>{t("games.guest_link")}</a> {t("games.guest_after")}
        </p>
      )}

      {(accountId === null || status === "loading") && (
        <div className="lv-state card" role="status" aria-live="polite">
          <span className="rp-spinner" aria-hidden="true" />
          <p>{t("games.loading")}</p>
        </div>
      )}

      {accountId !== null && status === "error" && (
        <div className="lv-state card" role="alert">
          <p>{error}</p>
          <button type="button" className="btn" onClick={() => void load(0)}>
            {t("games.retry")}
          </button>
        </div>
      )}

      {accountId !== null && status === "ready" && (
        <>
          {shown.length === 0 ? (
            <div className="lv-state card">
              <p>{emptyGamesText(filter, games.length)}</p>
              {games.length === 0 && (
                <a className="btn" href="#/">
                  {t("games.play")}
                </a>
              )}
            </div>
          ) : (
            <ul className="gl-list card" aria-label={t("games.list_aria")}>
              {shown.map((g) => (
                <GameRow key={g.game_id} game={g} />
              ))}
            </ul>
          )}

          {error && (
            <p className="lv-warn" role="alert">
              {error}
            </p>
          )}
          <div className="gl-foot">
            <span className="muted" aria-live="polite">
              {t("games.loaded", { loaded: games.length, count: total })}
            </span>
            {canMore && (
              <button type="button" className="btn" disabled={more} onClick={() => void load(games.length)}>
                {t(more ? "games.loading_more" : "games.load_more")}
              </button>
            )}
          </div>
        </>
      )}
    </main>
  );
}

function GameRow({ game: g }: { game: GameSummary }) {
  const t = useT();
  const opponent = opponentLabel(g);
  return (
    <li className={`gl-row ${g.result}`}>
      <span className={`gl-result ${g.result}`}>{RESULT_LABEL[g.result]}</span>
      <span className="gl-main">
        <span className="gl-opp">
          {t("games.vs")} <strong>{opponent}</strong>
        </span>
        <span className="gl-meta">
          {reasonText(g.reason, g.result)} · {colorCap(g.color)} · {t("games.plies", { count: g.plies })} · {relativeTime(g.at)}
        </span>
      </span>
      <span className="gl-side">
        <span className="tag">{kindLabel(g.kind, g.rated)}</span>
        {showsDelta(g) && (
          <span className={`gl-delta mono ${(g.elo_delta ?? 0) > 0 ? "up" : (g.elo_delta ?? 0) < 0 ? "down" : ""}`} aria-label={t("games.delta_aria", { delta: formatDelta(g.elo_delta) })}>
            {formatDelta(g.elo_delta)}
          </span>
        )}
      </span>
      <span className="gl-actions">
        <a className="btn sm" href={hrefFor({ name: "replay", param: g.game_id })} aria-label={t("games.review_aria", { opponent })}>
          {t("games.review")}
        </a>
        <a className="btn sm ghost" href={hrefFor({ name: "replay", param: g.game_id, sub: "analyse" })} aria-label={t("games.analyse_aria", { opponent })}>
          {t("games.analyse")}
        </a>
      </span>
    </li>
  );
}
