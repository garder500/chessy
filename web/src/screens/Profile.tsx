import { useState, useSyncExternalStore } from "react";
import { hrefFor, navigate } from "../router";
import { sfx } from "../sound";
import { store, useAppState } from "../store";
import { BOARD_THEMES, setTheme, useTheme, type ColorMode } from "../theme";
import { EloChart } from "../ui/EloChart";
import { RecentGames } from "../ui/RecentGames";
import { StatTile } from "../ui/StatTile";
import { initialOf, memberSince, sameUser, winRate } from "../ui/social";
import { tierOf } from "../ui/tier";
import { useProfile } from "../ui/useProfile";
import "./profile.css";

interface Props {
  username: string;
}

function streakText(streak: number): string {
  if (streak === 0) return "—";
  return streak > 0 ? `${streak} V` : `${Math.abs(streak)} D`;
}

function streakHint(streak: number): string | undefined {
  if (streak === 0) return undefined;
  const n = Math.abs(streak);
  return streak > 0 ? `${n} victoire${n > 1 ? "s" : ""} de suite` : `${n} défaite${n > 1 ? "s" : ""} de suite`;
}

export function Profile({ username }: Props) {
  const { state, reload } = useProfile(username);
  const { account } = useAppState();

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
          <h1>Joueur introuvable</h1>
          <p className="muted">Aucun joueur ne s'appelle « {username} ».</p>
          <a className="btn" href={hrefFor({ name: "ranking" })}>
            Voir le classement
          </a>
        </div>
      </main>
    );
  }
  if (state.status === "error") {
    return (
      <main className="pf-page">
        <div className="card pf-state" role="alert">
          <h1>Profil indisponible</h1>
          <p className="muted">{state.message}</p>
          <button type="button" className="btn" onClick={reload}>
            Réessayer
          </button>
        </div>
      </main>
    );
  }

  const p = state.profile;
  const rate = winRate(p.wins, p.games);
  const mine = !!account && !account.guest && sameUser(account.username, p.username);

  return (
    <main className="pf-page">
      <a className="pf-back" href={hrefFor({ name: "ranking" })}>
        ← Classement
      </a>
      <header className="pf-head card">
        <span className="avatar solid pf-avatar">{initialOf(p.username)}</span>
        <div className="pf-id">
          <h1 className="pf-name">{p.username}</h1>
          <p className="pf-meta">
            <span className="tag">{tierOf(p.elo).name}</span>
            {p.rank && <span className="mono">Rang #{p.rank}</span>}
            <span className="muted">{memberSince(p.created_at)}</span>
          </p>
        </div>
        <Relation username={p.username} />
      </header>

      <section className="pf-stats" aria-label="Statistiques">
        <StatTile label="Elo" value={p.elo} hint={tierOf(p.elo).name} />
        <StatTile
          label="Record V · N · D"
          value={`${p.wins} · ${p.draws} · ${p.losses}`}
          hint={rate === null ? "Aucune partie classée" : `${rate} % de victoires sur ${p.games}`}
        />
        <StatTile label="Pic d'Elo" value={p.peak_elo} />
        <StatTile label="Série" value={streakText(p.streak)} hint={streakHint(p.streak)} />
      </section>

      <div className="pf-cols">
        <section className="card pf-card" aria-labelledby="pf-chart-h">
          <h2 id="pf-chart-h" className="pf-h">
            Évolution de l'Elo
          </h2>
          <EloChart history={p.history} />
        </section>
        <section className="card pf-card" aria-labelledby="pf-recent-h">
          <h2 id="pf-recent-h" className="pf-h">
            Dernières parties
          </h2>
          <RecentGames games={p.recent.slice(0, 10)} />
        </section>
      </div>

      {mine && <QuickSettings />}
    </main>
  );
}

const MODES: { id: ColorMode; label: string }[] = [
  { id: "system", label: "Système" },
  { id: "light", label: "Clair" },
  { id: "dark", label: "Sombre" },
];

/** Réglages du quotidien, à portée de l'avatar : apparence, plateau, sons, déconnexion. Le reste vit dans « Réglages ». */
function QuickSettings() {
  const theme = useTheme();
  const snd = useSyncExternalStore(sfx.subscribe, sfx.getSettings);
  return (
    <section className="card pf-card pf-quick" aria-labelledby="pf-quick-h">
      <h2 id="pf-quick-h" className="pf-h">
        Réglages
      </h2>
      <div className="pf-q-row">
        <span className="pf-q-l">Thème</span>
        <div className="segs pf-q-segs" role="radiogroup" aria-label="Apparence">
          {MODES.map((m) => (
            <button key={m.id} type="button" role="radio" aria-checked={theme.mode === m.id} className={`sg${theme.mode === m.id ? " on" : ""}`} onClick={() => setTheme({ mode: m.id })}>
              {m.label}
            </button>
          ))}
        </div>
      </div>
      <div className="pf-q-row">
        <span className="pf-q-l">Plateau</span>
        <div className="pf-boards" role="radiogroup" aria-label="Plateau">
          {BOARD_THEMES.map((b) => (
            <button
              key={b.id}
              type="button"
              role="radio"
              aria-checked={theme.board === b.id}
              aria-label={b.label}
              title={b.label}
              className={`pf-board${theme.board === b.id ? " on" : ""}`}
              style={{ background: `repeating-conic-gradient(${b.dark} 0 25%, ${b.light} 0 50%) 0 0 / 50% 50%` }}
              onClick={() => setTheme({ board: b.id })}
            />
          ))}
        </div>
      </div>
      <div className="pf-q-row">
        <span className="pf-q-l">Sons</span>
        <button type="button" role="switch" aria-checked={snd.enabled} aria-label="Sons activés" className="st-switch" data-sfx="off" onClick={() => sfx.setSettings({ enabled: !snd.enabled })}>
          <span />
        </button>
      </div>
      <div className="pf-q-foot">
        <a className="link" href={hrefFor({ name: "settings" })}>
          Tous les réglages
        </a>
        <a className="link" href={hrefFor({ name: "games" })}>
          Mes parties
        </a>
        <button
          type="button"
          className="btn block"
          onClick={() => {
            void store.logout();
            navigate({ name: "home" });
          }}
        >
          Se déconnecter
        </button>
      </div>
    </section>
  );
}

/** Bouton d'action selon la relation du visiteur avec ce joueur. */
function Relation({ username }: { username: string }) {
  const { account, friends, outgoingChallenge, connection } = useAppState();
  const [requested, setRequested] = useState(false);
  const online = connection === "open";

  if (!account) return null;
  if (account.guest) {
    return (
      <div className="pf-actions">
        <a className="btn" href={hrefFor({ name: "auth" })}>
          Créer un compte pour l'ajouter
        </a>
      </div>
    );
  }
  if (sameUser(account.username, username)) {
    return (
      <div className="pf-actions">
        <span className="tag">C'est vous</span>
      </div>
    );
  }

  const friend = friends.friends.find((f) => sameUser(f.username, username));
  if (friend) {
    const pending = sameUser(outgoingChallenge, username);
    const can = online && friend.presence === "online" && !outgoingChallenge;
    const reason =
      friend.presence === "in_game" ? "En partie" : friend.presence === "offline" ? "Hors ligne" : "";
    return (
      <div className="pf-actions">
        {pending ? (
          <>
            <button type="button" className="btn pri" disabled>
              Défi envoyé
            </button>
            <button type="button" className="btn ghost" onClick={() => store.cancelChallenge()}>
              Annuler
            </button>
          </>
        ) : (
          <button type="button" className="btn pri" disabled={!can} onClick={() => store.send({ type: "challenge", username: friend.username })}>
            Défier
          </button>
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
          Accepter la demande
        </button>
        <button
          type="button"
          className="btn ghost"
          disabled={!online}
          onClick={() => store.send({ type: "friend_respond", username, accept: false })}
        >
          Refuser
        </button>
      </div>
    );
  }

  if (requested || friends.outgoing.some((r) => sameUser(r.username, username))) {
    return (
      <div className="pf-actions" aria-live="polite">
        <button type="button" className="btn" disabled>
          Demande envoyée
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
        Ajouter en ami
      </button>
    </div>
  );
}
