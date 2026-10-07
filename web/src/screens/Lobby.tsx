import { lazy, Suspense, useEffect, useState, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../catalog";
import { hrefFor } from "../router";
import { skillInfo } from "../skills";
import { store, type AppState } from "../store";
import { HeroPiece, ModeIcon, type HeroKind } from "../ui/HeroPiece";
import { initialOf } from "../ui/NavBar";
import { SkillArt } from "../ui/SkillArt";
import { sortFriends } from "../ui/social";
import { tileRarity } from "../ui/tileRarity";
import { UniqueBadge } from "../ui/UniqueBadge";
import { SoloPanel } from "./SoloPanel";
import "./lobby.css";

// three.js ne se charge qu'avec l'écran Jouer ; la pièce dessinée en SVG sert d'attente.
const HeroPiece3D = lazy(() => import("../ui/HeroPiece3D"));

const DECK_SLOTS = 7;

export type PlayMode = "ranked" | "friendly" | "private" | "ai";

const MODES: { id: PlayMode; label: string; piece: HeroKind }[] = [
  { id: "ranked", label: "Classée", piece: "king" },
  { id: "friendly", label: "Amicale", piece: "pawn" },
  { id: "private", label: "Salle privée", piece: "rook" },
  { id: "ai", label: "Contre l'IA", piece: "knight" },
];

const MODE_KEY = "chessy.playMode";

function readMode(): PlayMode | null {
  try {
    const v = localStorage.getItem(MODE_KEY);
    return MODES.some((m) => m.id === v) ? (v as PlayMode) : null;
  } catch {
    return null;
  }
}

/** Secondes écoulées depuis que `active` est devenu vrai. */
function useElapsed(active: boolean): number {
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

/** Statut affiché sous le pseudo dans « Votre groupe ». */
export function groupStatus(lobby: AppState["lobby"]): string {
  if (lobby.type === "queued") return "En recherche";
  if (lobby.type === "room_waiting") return "Salle ouverte";
  return "Prêt";
}

export function Lobby({ state }: { state: AppState }) {
  const { lobby, deck, account, friends, outgoingChallenge } = state;
  const isAccount = !!account && !account.guest;
  const [picked, setPicked] = useState<PlayMode | null>(readMode);
  // Un invité ne peut pas jouer en classée : son mode par défaut est l'amicale.
  const mode: PlayMode = picked ?? (isAccount ? "ranked" : "friendly");
  const [code, setCode] = useState("");
  const [copied, setCopied] = useState(false);
  const waiting = lobby.type !== "idle";
  const elapsed = useElapsed(waiting);
  const connected = state.connection === "open";

  const choose = (m: PlayMode) => {
    setPicked(m);
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {
      // Mode privé : le choix ne survit pas au rechargement.
    }
  };

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      // Presse-papiers indisponible : le code reste lisible à l'écran.
    }
  };

  // En attente, la pièce et le titre suivent la file réellement rejointe, pas l'onglet.
  const shown: PlayMode =
    lobby.type === "queued" ? (lobby.ranked ? "ranked" : "friendly") : lobby.type === "room_waiting" ? "private" : mode;
  const piece = MODES.find((m) => m.id === shown)!.piece;
  const elo = account?.elo ?? 1200;

  const online = sortFriends(friends.friends.filter((f) => f.presence !== "offline"));

  return (
    <main className="pl" data-mode={shown}>
      <div className="pl-modes" role="tablist" aria-label="Type de partie">
        {MODES.map((m) => (
          <button
            key={m.id}
            type="button"
            role="tab"
            aria-selected={shown === m.id}
            className="pl-mode"
            disabled={waiting && shown !== m.id}
            onClick={() => choose(m.id)}
          >
            <ModeIcon kind={m.piece} />
            {m.label}
          </button>
        ))}
      </div>

      <div className="pl-grid">
        <aside className="pl-side pl-left" aria-label="Votre groupe">
          <h2 className="pl-label">Votre groupe</h2>
          <div className="pl-me card">
            <span className="avatar pl-av">{initialOf(isAccount ? account?.username : "Invité")}</span>
            <span className="pl-me-txt">
              <strong>{isAccount ? account?.username : "Invité"}</strong>
              <span className="muted">
                {isAccount ? `Elo ${elo} · ` : ""}
                {groupStatus(lobby)}
              </span>
            </span>
          </div>
          {isAccount ? (
            <a className="pl-invite" href={hrefFor({ name: "friends" })}>
              <span className="pl-plus" aria-hidden="true">
                +
              </span>
              Inviter un ami
            </a>
          ) : (
            <a className="pl-invite" href={hrefFor({ name: "auth" })}>
              <span className="pl-plus" aria-hidden="true">
                +
              </span>
              Créer un compte
            </a>
          )}

          <div className="pl-deck-head">
            <h2 className="pl-label">Votre deck</h2>
            <span className="pl-count">
              {deck.length}/{DECK_SLOTS}
            </span>
          </div>
          <ul className="pl-deck">
            {Array.from({ length: DECK_SLOTS }, (_, i) => {
              const id = deck[i];
              if (!id) {
                return (
                  <li key={`empty-${i}`} className="slot empty">
                    Emplacement libre
                  </li>
                );
              }
              const info = skillInfo(id);
              return (
                <li key={id} className={`slot${info.unique ? " foil" : ""}`} style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties} title={info.description}>
                  <span className="slot-art" data-rar={tileRarity(id)}>
                    <SkillArt id={id} size={40} />
                    {info.unique && <UniqueBadge />}
                  </span>
                  <span className="pl-slot-txt">
                    <span className="slot-name">{info.name}</span>
                    <span className="eyebrow">{info.unique ? "Unique" : FAMILY_LABEL[info.family]}</span>
                  </span>
                </li>
              );
            })}
          </ul>
          <p className="muted pl-small">
            Avant chaque partie, choisissez trois compétences de ce deck. Gagnez une partie classée pour prendre une compétence à votre adversaire ou en
            forger une nouvelle.
          </p>
        </aside>

        <section className="pl-stage" aria-labelledby="pl-title">
          <div className="pl-beam" aria-hidden="true" />
          <div className="pl-head">
            <h1 id="pl-title" className="pl-title">
              {lobby.type === "queued" ? "Recherche…" : lobby.type === "room_waiting" ? "Salle privée" : MODES.find((m) => m.id === mode)!.label}
            </h1>
            <p className="pl-sub">
              {lobby.type === "queued"
                ? lobby.ranked
                  ? "File classée · adversaire de force proche"
                  : "File amicale"
                : lobby.type === "room_waiting"
                  ? "Partagez ce code avec votre adversaire"
                  : SUBTITLE[mode]}
            </p>
          </div>

          <div className="pl-hero">
            <Suspense fallback={<HeroPiece kind={piece} className="pl-piece" />}>
              <HeroPiece3D kind={piece} className="pl-piece" fast={waiting} />
            </Suspense>
            <div className="pl-floor" aria-hidden="true">
              <i />
              <i />
            </div>
          </div>

          <div className="pl-act">
            {lobby.type === "queued" && (
              <div className="pl-wait" role="status">
                <p className="pl-timer" aria-label={`Temps d'attente : ${elapsed} secondes`}>
                  {formatElapsed(elapsed)}
                </p>
                <div className="lb-bar" aria-hidden="true">
                  <i />
                </div>
                {lobby.ranked && <p className="muted pl-note">La plage d'Elo s'élargit peu à peu pendant l'attente.</p>}
                <button type="button" className="btn" onClick={() => store.send({ type: "leave_lobby" })}>
                  Annuler la recherche
                </button>
              </div>
            )}

            {lobby.type === "room_waiting" && (
              <div className="pl-wait" role="status">
                <p className="lb-code" data-testid="room-code" aria-label={`Code de salle ${lobby.code.split("").join(" ")}`}>
                  {lobby.code}
                </p>
                <div className="pl-row">
                  <button type="button" className="btn sm" onClick={() => copy(lobby.code)}>
                    {copied ? "Code copié" : "Copier le code"}
                  </button>
                  <button type="button" className="btn sm ghost" onClick={() => store.send({ type: "leave_lobby" })}>
                    Fermer la salle
                  </button>
                </div>
                <p className="pl-timer small">{formatElapsed(elapsed)}</p>
              </div>
            )}

            {lobby.type === "idle" && mode === "ranked" && !isAccount && (
              <>
                <a className="btn pri lb-main pl-cta" href={hrefFor({ name: "auth" })}>
                  Créer un compte
                </a>
                <p className="muted pl-note">Un compte est nécessaire pour jouer en classée, grimper au classement et défier vos amis.</p>
              </>
            )}

            {lobby.type === "idle" && mode === "ranked" && isAccount && (
              <>
                <button type="button" className="btn pri lb-main pl-cta" onClick={() => store.send({ type: "queue_join", ranked: true })}>
                  Trouver une partie
                </button>
                <p className="muted pl-note">Votre Elo ({elo}) et une compétence sont en jeu</p>
              </>
            )}

            {lobby.type === "idle" && mode === "friendly" && (
              <>
                <button type="button" className="btn pri lb-main pl-cta" onClick={() => store.send({ type: "queue_join", ranked: false })}>
                  Trouver une partie
                </button>
                <p className="muted pl-note">Sans enjeu : ni Elo ni compétence à gagner</p>
              </>
            )}

            {lobby.type === "idle" && mode === "private" && (
              <>
                <button type="button" className="btn pri lb-main pl-cta" onClick={() => store.send({ type: "create_room" })}>
                  Créer une salle
                </button>
                <form
                  className="pl-join"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (code.trim()) store.send({ type: "join_room", code: code.trim() });
                  }}
                >
                  <label className="sr-only" htmlFor="room-code">
                    Code de salle
                  </label>
                  <input
                    id="room-code"
                    className="input"
                    value={code}
                    onChange={(e) => setCode(e.target.value.toUpperCase())}
                    placeholder="Code de salle"
                    maxLength={8}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button type="submit" className="btn" disabled={!code.trim()}>
                    Rejoindre
                  </button>
                </form>
              </>
            )}

            {lobby.type === "idle" && mode === "ai" && <SoloPanel state={state} />}
          </div>
        </section>

        <aside className="pl-side pl-right" aria-labelledby="pl-friends">
          <h2 id="pl-friends" className="pl-label">
            Amis en ligne
          </h2>
          {!isAccount ? (
            <p className="muted pl-small">
              <a href={hrefFor({ name: "auth" })}>Créez un compte</a> pour ajouter des amis et les défier.
            </p>
          ) : online.length === 0 ? (
            <div className="pl-empty">
              <p className="muted pl-small">Aucun ami en ligne pour l'instant.</p>
              <a className="btn sm line" href={hrefFor({ name: "friends" })}>
                {friends.friends.length ? "Voir vos amis" : "Ajouter des amis"}
              </a>
            </div>
          ) : (
            <ul className="pl-friends">
              {online.map((f) => {
                const sent = outgoingChallenge?.toLowerCase() === f.username.toLowerCase();
                return (
                  <li key={f.username} className="pl-friend">
                    <span className={`presence ${f.presence}`} aria-hidden="true" />
                    <a className="pl-friend-name" href={hrefFor({ name: "profile", param: f.username })}>
                      {f.username}
                      <span className="sr-only">, {f.presence === "online" ? "en ligne" : "en partie"}</span>
                    </a>
                    <span className="pl-friend-elo muted">{f.elo}</span>
                    {f.presence === "in_game" ? (
                      f.game_id ? (
                        <a className="btn sm line" href={hrefFor({ name: "watch", param: f.game_id })}>
                          Regarder
                        </a>
                      ) : (
                        <span className="pl-busy muted">En partie</span>
                      )
                    ) : (
                      <button
                        type="button"
                        className="btn sm line"
                        disabled={!connected || waiting || (outgoingChallenge !== null && !sent) || sent}
                        onClick={() => store.send({ type: "challenge", username: f.username })}
                      >
                        {sent ? "Envoyé" : "Inviter"}
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          {friends.incoming.length > 0 && (
            <a className="pl-requests" href={hrefFor({ name: "friends" })}>
              {friends.incoming.length} demande{friends.incoming.length > 1 ? "s" : ""} d'ami en attente
            </a>
          )}
        </aside>
      </div>
    </main>
  );
}

const SUBTITLE: Record<PlayMode, string> = {
  ranked: "Adversaire de force proche",
  friendly: "Une partie sans enjeu",
  private: "Jouez avec un ami grâce à un code",
  ai: "Entraînement contre Sage",
};
