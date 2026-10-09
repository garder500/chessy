import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, apiErrorText } from "../api";
import { useT } from "../i18n";
import type { LeaderboardEntry } from "../protocol";
import { hrefFor } from "../router";
import { useAppState } from "../store";
import { initialOf, sameUser, winRate } from "../ui/social";
import { pointsToNextTier, tierOf, tierProgress } from "../ui/tier";
import { displayRank, filterEntries, missingFriends } from "./rankingLogic";
import type { RankingFilter } from "./rankingLogic";
import "./ranking.css";

const PAGE = 50;
const MAX_FRIEND_SCAN = 1000;

interface Data {
  entries: LeaderboardEntry[];
  total: number;
}

export function Ranking() {
  const t = useT();
  const { account, friends } = useAppState();
  const me = account && !account.guest ? account.username : null;
  const guest = !me;

  const [filter, setFilter] = useState<RankingFilter>("all");
  const [data, setData] = useState<Data>({ entries: [], total: 0 });
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [more, setMore] = useState(false);
  const [error, setError] = useState("");
  const aborter = useRef<AbortController | null>(null);

  const friendNames = useMemo(() => friends.friends.map((f) => f.username), [friends]);

  const load = useCallback(async (offset: number) => {
    aborter.current?.abort();
    const ctl = new AbortController();
    aborter.current = ctl;
    if (offset === 0) setStatus("loading");
    else setMore(true);
    try {
      const res = await api.leaderboard(PAGE, offset, ctl.signal);
      setData((d) => ({ total: res.total, entries: offset === 0 ? res.entries : [...d.entries, ...res.entries] }));
      setStatus("ready");
      setError("");
    } catch (e) {
      if (ctl.signal.aborted) return;
      setError(apiErrorText(e));
      if (offset === 0) setStatus("error");
      else setStatus("ready");
    } finally {
      if (!ctl.signal.aborted) setMore(false);
    }
  }, []);

  useEffect(() => {
    void load(0);
    return () => aborter.current?.abort();
  }, [load]);

  // Filtre « Amis » : on charge les pages suivantes tant que des amis manquent (plafonné).
  const needMore =
    filter === "friends" &&
    status === "ready" &&
    !more &&
    !error &&
    data.entries.length < data.total &&
    data.entries.length < MAX_FRIEND_SCAN &&
    missingFriends(data.entries, friendNames.concat(me ? [me] : [])).length > 0;
  useEffect(() => {
    if (needMore) void load(data.entries.length);
  }, [needMore, load, data.entries.length]);

  const shown = useMemo(
    () => filterEntries(data.entries, filter, friendNames, me),
    [data.entries, filter, friendNames, me],
  );
  const podium = shown.slice(0, 3);
  const rest = shown.slice(3);
  const canLoadMore = data.entries.length < data.total;
  const rowsFrom = 3;

  return (
    <main className="rk-page">
      <header className="rk-head">
        <div>
          <p className="eyebrow">{t("ranking.season")}</p>
          <h1 className="rk-title">{t("ranking.title")}</h1>
        </div>
        <div className="seg rk-filter" role="group" aria-label={t("ranking.filter_aria")}>
          <button type="button" aria-pressed={filter === "all"} className={filter === "all" ? "on" : ""} onClick={() => setFilter("all")}>
            {t("ranking.filter_all")}
          </button>
          <button
            type="button"
            aria-pressed={filter === "friends"}
            className={filter === "friends" ? "on" : ""}
            onClick={() => setFilter("friends")}
            disabled={guest}
            title={guest ? t("ranking.friends_guest_title") : undefined}
          >
            {t("ranking.filter_friends")}
          </button>
        </div>
      </header>

      {status === "loading" && <Skeleton />}

      {status === "error" && (
        <div className="rk-state card" role="alert">
          <p>{error || t("ranking.load_error")}</p>
          <button type="button" className="btn" onClick={() => void load(0)}>
            {t("ranking.retry")}
          </button>
        </div>
      )}

      {status === "ready" && shown.length === 0 && (
        <div className="rk-state card">
          {filter === "friends" && needMore ? (
            <p>{t("ranking.searching_friends")}</p>
          ) : filter === "friends" ? (
            <>
              <p>{t("ranking.no_friends_ranked")}</p>
              <a className="btn" href={hrefFor({ name: "friends" })}>
                {t("ranking.see_friends")}
              </a>
            </>
          ) : (
            <p>{t("ranking.empty")}</p>
          )}
        </div>
      )}

      {status === "ready" && shown.length > 0 && (
        <>
          <ol className="rk-podium" aria-label={t("ranking.podium")}>
            {[1, 0, 2].map((i) => {
              const e = podium[i];
              if (!e) return <li key={i} className="rk-pod-empty" aria-hidden="true" />;
              const rank = displayRank(e, i, filter);
              const mine = sameUser(e.username, me);
              return (
                <li key={e.username} className={`rk-pod rk-pod-${i + 1}${mine ? " me" : ""}`}>
                  <a href={hrefFor({ name: "profile", param: e.username })}>
                    <span className="rk-pod-rank mono">{rank}</span>
                    <span className={`avatar${i === 0 ? " solid" : ""} rk-pod-av`}>{initialOf(e.username)}</span>
                    <span className="rk-pod-name">
                      {e.username}
                      {mine && <span className="rk-you"> · {t("ranking.you")}</span>}
                    </span>
                    <span className="rk-pod-elo mono">{e.elo}</span>
                    <span className="tag">{tierOf(e.elo).name}</span>
                  </a>
                </li>
              );
            })}
          </ol>

          {rest.length > 0 && (
            <div className="rk-table-wrap card">
              <table className="rk-table">
                <caption className="rk-sr">{t("ranking.table_caption")}</caption>
                <thead>
                  <tr>
                    <th scope="col" className="c-rank">{t("ranking.col_rank")}</th>
                    <th scope="col">{t("ranking.col_player")}</th>
                    <th scope="col" className="c-num">Elo</th>
                    <th scope="col" className="c-vnd" title={t("ranking.col_wdl_title")}>{t("ranking.col_wdl")}</th>
                    <th scope="col" className="c-bar">{t("ranking.col_wins")}</th>
                    <th scope="col" className="c-tier">{t("ranking.col_tier")}</th>
                  </tr>
                </thead>
                <tbody>
                  {rest.map((e, k) => (
                    <Row key={e.username} entry={e} rank={displayRank(e, rowsFrom + k, filter)} mine={sameUser(e.username, me)} />
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {error && (
            <p className="field-msg rk-inline-err" role="alert">
              {error}
            </p>
          )}
          {canLoadMore && filter === "all" && (
            <div className="rk-more">
              <button type="button" className="btn" onClick={() => void load(data.entries.length)} disabled={more}>
                {more ? t("ranking.loading") : t("ranking.more")}
              </button>
              <span className="muted rk-count mono">
                {data.entries.length} / {data.total}
              </span>
            </div>
          )}
        </>
      )}

      <Footer account={account} guest={guest} />
    </main>
  );
}

function Row({ entry: e, rank, mine }: { entry: LeaderboardEntry; rank: number; mine: boolean }) {
  const t = useT();
  const rate = winRate(e.wins, e.games);
  const href = hrefFor({ name: "profile", param: e.username });
  return (
    <tr className={`rk-row${mine ? " me" : ""}`} onClick={() => (location.hash = href)}>
      <td className="c-rank mono">{rank}</td>
      <td>
        <a className="rk-player" href={href} onClick={(ev) => ev.stopPropagation()}>
          <span className="avatar">{initialOf(e.username)}</span>
          <span className="rk-name">
            {e.username}
            {mine && <span className="rk-you"> · {t("ranking.you")}</span>}
          </span>
        </a>
      </td>
      <td className="c-num mono">{e.elo}</td>
      <td className="c-vnd mono" aria-label={t("ranking.record_aria", { w: e.wins, d: e.draws, l: e.losses })}>
        {e.wins} · {e.draws} · {e.losses}
      </td>
      <td className="c-bar">
        <span className="rk-bar" role="img" aria-label={rate === null ? t("ranking.no_games") : t("ranking.win_pct_aria", { rate })}>
          <span style={{ width: `${rate ?? 0}%` }} />
        </span>
        <span className="rk-rate mono">{rate === null ? "—" : `${rate} %`}</span>
      </td>
      <td className="c-tier">
        <span className="tag">{tierOf(e.elo).name}</span>
      </td>
    </tr>
  );
}

function Footer({ account, guest }: { account: ReturnType<typeof useAppState>["account"]; guest: boolean }) {
  const t = useT();
  if (guest || !account) {
    return (
      <aside className="rk-foot" aria-label={t("ranking.your_position_aria")}>
        <div className="rk-foot-in">
          <p>{t("ranking.guest_note")}</p>
          <a className="btn pri sm" href={hrefFor({ name: "auth" })}>
            {t("ranking.create_account")}
          </a>
        </div>
      </aside>
    );
  }
  const next = pointsToNextTier(account.elo);
  const tier = tierOf(account.elo);
  return (
    <aside className="rk-foot" aria-label={t("ranking.your_position_aria")}>
      <div className="rk-foot-in">
        <div className="rk-foot-pos">
          <span className="eyebrow">{t("ranking.your_position")}</span>
          <span className="rk-foot-rank mono">{account.rank ? `#${account.rank}` : t("ranking.unranked")}</span>
        </div>
        <div className="rk-foot-me">
          <span className="avatar solid">{initialOf(account.username ?? "?")}</span>
          <span>
            <strong>{account.username}</strong>
            <span className="muted rk-foot-sub mono">
              {account.elo} · {tier.name}
            </span>
          </span>
        </div>
        <div className="rk-foot-next">
          <span className="rk-foot-label">
            {next ? (
              <>
                <span className="mono">{next.points}</span> {t("ranking.points_to_tier", { count: next.points, tier: next.tier.name })}
              </>
            ) : (
              t("ranking.max_tier")
            )}
          </span>
          <span className="rk-bar rk-bar-wide" role="img" aria-label={t("ranking.tier_progress_aria", { pct: Math.round(tierProgress(account.elo) * 100) })}>
            <span style={{ width: `${Math.round(tierProgress(account.elo) * 100)}%` }} />
          </span>
        </div>
      </div>
    </aside>
  );
}

function Skeleton() {
  const t = useT();
  return (
    <div className="rk-skel" aria-busy="true" aria-label={t("ranking.loading_aria")}>
      <div className="rk-skel-pod">
        <span /> <span /> <span />
      </div>
      {Array.from({ length: 6 }, (_, i) => (
        <div key={i} className="rk-skel-row" />
      ))}
    </div>
  );
}
