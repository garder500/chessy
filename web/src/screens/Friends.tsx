import { useT } from "../i18n";
import { ChallengeSheet } from "../ui/ChallengeSheet";
import { useEffect, useMemo, useState } from "react";
import type { FriendInfo, UserResult } from "../protocol";
import { hrefFor } from "../router";
import { store, useAppState } from "../store";
import { EloChart } from "../ui/EloChart";
import { RecentGames } from "../ui/RecentGames";
import { StatTile } from "../ui/StatTile";
import { ModerationActions } from "../ui/Moderation";
import { initialOf, presenceLabel, RELATION_LABEL, sortFriends, winRate } from "../ui/social";
import { tierOf } from "../ui/tier";
import { useProfile } from "../ui/useProfile";
import "./friends.css";

export function Friends() {
  const t = useT();
  const { account, friends, connection } = useAppState();
  const guest = !account || account.guest;

  if (account && guest) {
    return (
      <main className="frd-page">
        <div className="card frd-guest">
          <p className="eyebrow">{t("friends.guest_eyebrow")}</p>
          <h1>{t("friends.guest_title")}</h1>
          <p className="muted">
            {t("friends.guest_text")}
          </p>
          <a className="btn pri frd-guest-btn" href={hrefFor({ name: "auth" })}>
            {t("friends.guest_cta")}
          </a>
        </div>
      </main>
    );
  }

  return <FriendsBody online={connection === "open"} friends={friends} />;
}

function FriendsBody({ online, friends }: { online: boolean; friends: ReturnType<typeof useAppState>["friends"] }) {
  const t = useT();
  const sorted = useMemo(() => sortFriends(friends.friends), [friends.friends]);
  const [selected, setSelected] = useState<string | null>(null);
  const current = sorted.find((f) => f.username === selected) ?? sorted[0] ?? null;

  return (
    <main className="frd-page">
      <header className="frd-head">
        <p className="eyebrow">{t("friends.eyebrow")}</p>
        <h1 className="frd-title">{t("friends.title")}</h1>
      </header>
      {/* Téléphone : la liste, puis le détail de l'ami choisi à sa place, avec un retour vers la liste. */}
      <div className={`frd-grid${selected !== null && current ? " has-sel" : ""}`}>
        <aside className="frd-side card" aria-label={t("friends.aria_list")}>
          <AddFriend online={online} />
          <Requests incoming={friends.incoming} outgoing={friends.outgoing} online={online} />
          <section className="frd-sec" aria-labelledby="frd-list-h">
            <h2 id="frd-list-h" className="frd-sec-h">
              {t("friends.my_friends")} <span className="mono frd-count">{sorted.length}</span>
            </h2>
            {sorted.length === 0 ? (
              <p className="frd-empty muted">{t("friends.empty")}</p>
            ) : (
              <ul className="frd-list">
                {sorted.map((f) => (
                  <li key={f.username}>
                    <FriendRow friend={f} active={current?.username === f.username} onSelect={() => {
                        setSelected(f.username);
                        if (window.matchMedia("(max-width: 960px)").matches) window.scrollTo({ top: 0 });
                      }}
                    />
                  </li>
                ))}
              </ul>
            )}
          </section>
        </aside>

        <section className="frd-detail" aria-label={t("friends.aria_detail")}>
          {current ? (
            <>
              <button type="button" className="btn ghost sm frd-back" onClick={() => setSelected(null)}>
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true" focusable="false">
                  <path d="M15 5l-7 7 7 7" />
                </svg>
                {t("friends.my_friends")}
              </button>
              <Detail key={current.username} friend={current} online={online} />
            </>
          ) : (
            <div className="card frd-placeholder">
              <p className="muted">{t("friends.select_hint")}</p>
            </div>
          )}
        </section>
      </div>
    </main>
  );
}

// ---- recherche / ajout --------------------------------------------------------

function AddFriend({ online }: { online: boolean }) {
  const t = useT();
  const { userResults } = useAppState();
  const [query, setQuery] = useState("");
  const [sent, setSent] = useState<Set<string>>(new Set());
  const q = query.trim();
  const active = q.length >= 2;

  useEffect(() => {
    if (!active) return;
    const timer = setTimeout(() => store.send({ type: "user_search", query: q }), 300);
    return () => clearTimeout(timer);
  }, [q, active]);

  const fresh = active && userResults && userResults.query.trim().toLowerCase() === q.toLowerCase();
  const users = fresh ? userResults.users : null;

  function request(u: UserResult) {
    store.send({ type: "friend_request", username: u.username });
    setSent((s) => new Set(s).add(u.username));
  }
  function accept(u: UserResult) {
    store.send({ type: "friend_respond", username: u.username, accept: true });
  }

  return (
    <section className="frd-sec" aria-labelledby="frd-add-h">
      <h2 id="frd-add-h" className="frd-sec-h">
        {t("friends.add_title")}
      </h2>
      <label className="frd-sr" htmlFor="frd-search">
        {t("friends.search_label")}
      </label>
      <input
        id="frd-search"
        className="input frd-search"
        type="search"
        placeholder={t("friends.search_placeholder")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        autoComplete="off"
        autoCapitalize="none"
        spellCheck={false}
        maxLength={32}
        aria-describedby="frd-search-status"
      />
      <div id="frd-search-status" aria-live="polite" className="frd-search-status">
        {query && !active && <p className="frd-hint">{t("friends.search_min")}</p>}
        {active && !users && <p className="frd-hint">{t("friends.searching")}</p>}
        {users && users.length === 0 && <p className="frd-hint">{t("friends.no_match", { q })}</p>}
      </div>
      {users && users.length > 0 && (
        <ul className="frd-results">
          {users.map((u) => {
            const relation = u.relation === "none" && sent.has(u.username) ? "outgoing" : u.relation;
            return (
              <li key={u.username} className="frd-result">
                <span className="avatar">{initialOf(u.username)}</span>
                <span className="frd-who">
                  <a className="frd-name" href={hrefFor({ name: "profile", param: u.username })}>
                    {u.username}
                  </a>
                  <span className="mono frd-elo">{u.elo}</span>
                </span>
                {relation === "none" ? (
                  <button type="button" className="btn sm" onClick={() => request(u)} disabled={!online}>
                    {t("friends.add")}
                  </button>
                ) : relation === "incoming" ? (
                  <button type="button" className="btn sm" onClick={() => accept(u)} disabled={!online}>
                    {t("friends.accept")}
                  </button>
                ) : (
                  <span className="tag">{RELATION_LABEL[relation]}</span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

// ---- demandes ----------------------------------------------------------------

function Requests({
  incoming,
  outgoing,
  online,
}: {
  incoming: { username: string; elo: number }[];
  outgoing: { username: string }[];
  online: boolean;
}) {
  const t = useT();
  if (incoming.length === 0 && outgoing.length === 0) return null;
  return (
    <section className="frd-sec" aria-labelledby="frd-req-h">
      <h2 id="frd-req-h" className="frd-sec-h">
        {t("friends.requests")} <span className="mono frd-count">{incoming.length + outgoing.length}</span>
      </h2>
      <ul className="frd-list">
        {incoming.map((r) => (
          <li key={r.username} className="frd-req">
            <span className="avatar">{initialOf(r.username)}</span>
            <span className="frd-who">
              <a className="frd-name" href={hrefFor({ name: "profile", param: r.username })}>
                {r.username}
              </a>
              <span className="mono frd-elo">{r.elo}</span>
            </span>
            <span className="frd-req-btns">
              <button
                type="button"
                className="btn sm"
                onClick={() => store.send({ type: "friend_respond", username: r.username, accept: true })}
                disabled={!online}
                aria-label={t("friends.accept_request_aria", { name: r.username })}
              >
                {t("friends.accept")}
              </button>
              <button
                type="button"
                className="btn sm ghost"
                onClick={() => store.send({ type: "friend_respond", username: r.username, accept: false })}
                disabled={!online}
                aria-label={t("friends.decline_request_aria", { name: r.username })}
              >
                {t("friends.decline")}
              </button>
            </span>
          </li>
        ))}
        {outgoing.map((r) => (
          <li key={r.username} className="frd-req">
            <span className="avatar">{initialOf(r.username)}</span>
            <span className="frd-who">
              <span className="frd-name">{r.username}</span>
            </span>
            <span className="tag">{t("friends.pending")}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

// ---- liste ---------------------------------------------------------------------

function FriendRow({ friend: f, active, onSelect }: { friend: FriendInfo; active: boolean; onSelect: () => void }) {
  useT();
  return (
    <button type="button" className={`frd-row${active ? " on" : ""}`} onClick={onSelect} aria-pressed={active}>
      <span className="frd-av">
        <span className="avatar">{initialOf(f.username)}</span>
        <span className={`presence ${f.presence} frd-dot`} aria-hidden="true" />
      </span>
      <span className="frd-who">
        <span className="frd-name">{f.username}</span>
        <span className="frd-sub">{presenceLabel(f.presence, f.last_seen)}</span>
      </span>
      <span className="mono frd-elo">{f.elo}</span>
    </button>
  );
}

// ---- détail --------------------------------------------------------------------

function Detail({ friend, online }: { friend: FriendInfo; online: boolean }) {
  const t = useT();
  const { outgoingChallenge } = useAppState();
  const { state, reload } = useProfile(friend.username);
  const [confirming, setConfirming] = useState(false);
  const [sheet, setSheet] = useState(false);

  const pending = outgoingChallenge !== null && outgoingChallenge.toLowerCase() === friend.username.toLowerCase();
  const otherPending = outgoingChallenge !== null && !pending;
  const canChallenge = online && friend.presence === "online" && !outgoingChallenge;
  const why =
    friend.presence === "in_game"
      ? t("friends.why_in_game", { name: friend.username })
      : friend.presence === "offline"
        ? t("friends.why_offline", { name: friend.username })
        : otherPending
          ? t("friends.why_other_pending")
          : "";

  return (
    <div className="card frd-card">
      <div className="frd-card-head">
        <span className="frd-av">
          <span className="avatar frd-avatar-lg">{initialOf(friend.username)}</span>
          <span className={`presence ${friend.presence} frd-dot`} aria-hidden="true" />
        </span>
        <div className="frd-card-id">
          <h2 className="frd-card-name">{friend.username}</h2>
          <p className="frd-sub">
            <span className="tag">{tierOf(friend.elo).name}</span> {presenceLabel(friend.presence, friend.last_seen)}
          </p>
        </div>
        <div className="frd-actions">
          {pending ? (
            <>
              <button type="button" className="btn pri" disabled>
                {t("friends.challenge_sent")}
              </button>
              <button type="button" className="btn ghost" onClick={() => store.cancelChallenge()}>
                {t("friends.cancel")}
              </button>
            </>
          ) : (
            <>
              <button type="button" className="btn pri" disabled={!canChallenge} onClick={() => setSheet(true)}>
                {t("friends.challenge")}
              </button>
              {friend.presence === "in_game" && friend.game_id && (
                <a className="btn" href={hrefFor({ name: "watch", param: friend.game_id })}>
                  {t("friends.watch")}
                </a>
              )}
            </>
          )}
        </div>
      </div>
      {why && !pending && <p className="frd-why muted">{why}</p>}

      {state.status === "loading" && <div className="frd-skel" aria-busy="true" aria-label={t("friends.loading_profile")} />}
      {state.status === "error" && (
        <div className="frd-err" role="alert">
          <p>{state.message}</p>
          <button type="button" className="btn sm" onClick={reload}>
            {t("friends.retry")}
          </button>
        </div>
      )}
      {state.status === "notfound" && <p className="muted">{t("friends.profile_gone")}</p>}
      {state.status === "ready" && (
        <>
          <div className="frd-stats">
            <StatTile label="Elo" value={state.profile.elo} />
            <StatTile label={t("friends.stat_games")} value={state.profile.games} />
            <StatTile
              label={t("friends.stat_wins")}
              value={winRate(state.profile.wins, state.profile.games) === null ? "—" : `${winRate(state.profile.wins, state.profile.games)} %`}
              hint={t("friends.record_hint", { w: state.profile.wins, d: state.profile.draws, l: state.profile.losses })}
            />
            <StatTile label={t("friends.stat_rank")} value={state.profile.rank ? `#${state.profile.rank}` : "—"} />
          </div>
          <h3 className="frd-sub-h">{t("friends.elo_history")}</h3>
          <EloChart history={state.profile.history} />
          <h3 className="frd-sub-h">{t("friends.recent_games")}</h3>
          <RecentGames games={state.profile.recent} limit={5} />
          <a className="frd-full" href={hrefFor({ name: "profile", param: friend.username })}>
            {t("friends.full_profile")}
          </a>
        </>
      )}

      <ChallengeSheet friend={sheet ? friend : null} onClose={() => setSheet(false)} />

      <div className="frd-remove">
        {confirming ? (
          <div className="frd-confirm" role="alertdialog" aria-label={t("friends.remove_aria", { name: friend.username })}>
            <span>{t("friends.remove_confirm", { name: friend.username })}</span>
            <button
              type="button"
              className="btn sm danger"
              onClick={() => {
                store.send({ type: "friend_remove", username: friend.username });
                setConfirming(false);
              }}
              disabled={!online}
              autoFocus
            >
              {t("friends.confirm")}
            </button>
            <button type="button" className="btn sm ghost" onClick={() => setConfirming(false)}>
              {t("friends.cancel")}
            </button>
          </div>
        ) : (
          <button type="button" className="btn sm ghost danger" onClick={() => setConfirming(true)} disabled={!online}>
            {t("friends.remove")}
          </button>
        )}
        {!confirming && <ModerationActions username={friend.username} />}
      </div>
    </div>
  );
}
