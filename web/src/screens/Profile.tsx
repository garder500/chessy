import { useEffect, useState } from "react";
import { t, useT } from "../i18n";
import { hrefFor, navigate } from "../router";
import { store, useAppState } from "../store";
import { ChallengeSheet } from "../ui/ChallengeSheet";
import { ModerationActions } from "../ui/Moderation";
import { EloChart } from "../ui/EloChart";
import { RecentGames } from "../ui/RecentGames";
import { StatTile } from "../ui/StatTile";
import { initialOf, memberSince, sameUser, winRate } from "../ui/social";
import { tierOf } from "../ui/tier";
import { useProfile } from "../ui/useProfile";
import { RecoveryCodeSettings } from "./RecoveryCode";
import { SettingsBody } from "./Settings";
import "./profile.css";
import "./settings.css";

interface Props {
  username: string;
}

function streakText(streak: number): string {
  if (streak === 0) return "—";
  return streak > 0 ? t("profile.streak_w", { n: streak }) : t("profile.streak_l", { n: Math.abs(streak) });
}

function streakHint(streak: number): string | undefined {
  if (streak === 0) return undefined;
  const n = Math.abs(streak);
  return streak > 0 ? t("profile.streak_wins", { count: n }) : t("profile.streak_losses", { count: n });
}

export function Profile({ username }: Props) {
  const t = useT();
  const { state, reload } = useProfile(username);
  const { account } = useAppState();
  const isMe = !!account && !account.guest && sameUser(account.username, username);

  if (state.status === "loading") {
    return (
      <main className="pf-page" aria-busy="true">
        <div className="pf-skel pf-skel-head" />
        <div className="pf-skel pf-skel-body" />
      </main>
    );
  }
  if (state.status === "notfound") {
    return (
      <main className="pf-page">
        <div className="card pf-state">
          <h1>{t("profile.notfound_title")}</h1>
          <p className="muted">{t("profile.notfound_text", { name: username })}</p>
          <a className="btn" href={hrefFor({ name: "ranking" })}>
            {t("profile.see_ranking")}
          </a>
        </div>
      </main>
    );
  }
  if (state.status === "error") {
    return (
      <main className="pf-page">
        <div className="card pf-state" role="alert">
          <h1>{t("profile.unavailable")}</h1>
          <p className="muted">{state.message}</p>
          <button type="button" className="btn" onClick={reload}>
            {t("profile.retry")}
          </button>
        </div>
        {isMe && <ProfileSettings />}
      </main>
    );
  }

  const p = state.profile;
  const rate = winRate(p.wins, p.games);
  const mine = !!account && !account.guest && sameUser(account.username, p.username);

  return (
    <main className="pf-page">
      <a className="pf-back" href={hrefFor({ name: "ranking" })}>
        {t("profile.back_ranking")}
      </a>
      <header className="pf-head card">
        <span className="avatar solid pf-avatar">{initialOf(p.username)}</span>
        <div className="pf-id">
          <h1 className="pf-name">{p.username}</h1>
          <p className="pf-meta">
            {p.placed === false ? <span className="tag">{t("profile.unplaced")}</span> : <span className="tag">{tierOf(p.elo).name}</span>}
            {p.rank && <span className="mono">{t("profile.rank", { rank: p.rank })}</span>}
            <span className="muted">{memberSince(p.created_at)}</span>
          </p>
        </div>
        <Relation username={p.username} />
        {!mine && <ModerationActions username={p.username} />}
      </header>

      <section className="pf-stats" aria-label={t("profile.stats_aria")}>
        <StatTile label="Elo" value={p.elo} hint={tierOf(p.elo).name} />
        <StatTile
          label={t("profile.stat_record")}
          value={`${p.wins} · ${p.draws} · ${p.losses}`}
          hint={rate === null ? t("profile.no_ranked_games") : t("profile.win_rate_hint", { rate, games: p.games })}
        />
        <StatTile label={t("profile.stat_peak")} value={p.peak_elo} />
        <StatTile label={t("profile.stat_streak")} value={streakText(p.streak)} hint={streakHint(p.streak)} />
      </section>

      <div className="pf-cols">
        <section className="card pf-card" aria-labelledby="pf-chart-h">
          <h2 id="pf-chart-h" className="pf-h">
            {t("profile.elo_history")}
          </h2>
          <EloChart history={p.history} />
        </section>
        <section className="card pf-card" aria-labelledby="pf-recent-h">
          <h2 id="pf-recent-h" className="pf-h">
            {t("profile.recent_games")}
          </h2>
          <RecentGames games={p.recent.slice(0, 10)} />
        </section>
      </div>

      {mine && <ProfileSettings />}
    </main>
  );
}

/** Réglages du compte, dans le profil : apparence, sons et jeu, puis mes parties et déconnexion. */
function ProfileSettings() {
  const t = useT();
  // `#/settings` ouvre le profil directement sur cette section.
  useEffect(() => {
    if (location.hash.startsWith("#/settings")) document.getElementById("reglages")?.scrollIntoView();
  }, []);
  return (
    <section id="reglages" className="pf-settings" aria-labelledby="pf-set-h">
      <h2 id="pf-set-h" className="pf-h pf-set-h">
        {t("profile.settings")}
      </h2>
      <SettingsBody />
      <RecoveryCodeSettings />
      <div className="card pf-card pf-q-foot">
        <a className="btn block" href={hrefFor({ name: "games" })}>
          {t("profile.my_games")}
        </a>
        <button
          type="button"
          className="btn block"
          onClick={() => {
            void store.logout();
            navigate({ name: "home" });
          }}
        >
          {t("profile.logout")}
        </button>
        <button
          type="button"
          className="btn block"
          onClick={() => {
            void store.logout(true);
            navigate({ name: "home" });
          }}
        >
          {t("profile.logout_all")}
        </button>
      </div>
    </section>
  );
}

/** Bouton d'action selon la relation du visiteur avec ce joueur. */
function Relation({ username }: { username: string }) {
  const t = useT();
  const { account, friends, outgoingChallenge, connection } = useAppState();
  const [requested, setRequested] = useState(false);
  const [sheet, setSheet] = useState(false);
  const online = connection === "open";

  if (!account) return null;
  if (account.guest) {
    return (
      <div className="pf-actions">
        <a className="btn" href={hrefFor({ name: "auth" })}>
          {t("profile.guest_add")}
        </a>
      </div>
    );
  }
  if (sameUser(account.username, username)) {
    return (
      <div className="pf-actions">
        <span className="tag">{t("profile.is_you")}</span>
      </div>
    );
  }

  const friend = friends.friends.find((f) => sameUser(f.username, username));
  if (friend) {
    const pending = sameUser(outgoingChallenge, username);
    const can = online && friend.presence === "online" && !outgoingChallenge;
    const reason =
      friend.presence === "in_game" ? t("profile.in_game") : friend.presence === "offline" ? t("profile.offline") : "";
    return (
      <div className="pf-actions">
        {pending ? (
          <>
            <button type="button" className="btn pri" disabled>
              {t("profile.challenge_sent")}
            </button>
            <button type="button" className="btn ghost" onClick={() => store.cancelChallenge()}>
              {t("profile.cancel")}
            </button>
          </>
        ) : (
          <>
            <button type="button" className="btn pri" disabled={!can} onClick={() => setSheet(true)}>
              {t("profile.challenge")}
            </button>
            <ChallengeSheet friend={sheet ? friend : null} onClose={() => setSheet(false)} />
          </>
        )}
        {reason && !pending && <span className="muted pf-reason">{reason}</span>}
      </div>
    );
  }

  if (friends.incoming.some((r) => sameUser(r.username, username))) {
    return (
      <div className="pf-actions">
        <button
          type="button"
          className="btn pri"
          disabled={!online}
          onClick={() => store.send({ type: "friend_respond", username, accept: true })}
        >
          {t("profile.accept_request")}
        </button>
        <button
          type="button"
          className="btn ghost"
          disabled={!online}
          onClick={() => store.send({ type: "friend_respond", username, accept: false })}
        >
          {t("profile.decline")}
        </button>
      </div>
    );
  }

  if (requested || friends.outgoing.some((r) => sameUser(r.username, username))) {
    return (
      <div className="pf-actions" aria-live="polite">
        <button type="button" className="btn" disabled>
          {t("profile.request_sent")}
        </button>
      </div>
    );
  }

  return (
    <div className="pf-actions">
      <button
        type="button"
        className="btn pri"
        disabled={!online}
        onClick={() => {
          store.send({ type: "friend_request", username });
          setRequested(true);
        }}
      >
        {t("profile.add_friend")}
      </button>
    </div>
  );
}
