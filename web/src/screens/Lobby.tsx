import { lazy, Suspense, useState, type CSSProperties } from "react";
import { hrefFor } from "../router";
import { skillInfo } from "../skills";
import { store, type AppState } from "../store";
import { Beam } from "../ui/Beam";
import { HeroPiece } from "../ui/HeroPiece";
import { ChallengeSheet } from "../ui/ChallengeSheet";
import { Sheet } from "../ui/Sheet";
import { initialOf } from "../ui/NavBar";
import { SkillArt } from "../ui/SkillArt";
import { sortFriends } from "../ui/social";
import { tileRarity } from "../ui/tileRarity";
import { setTime, timeText, TIMES, useTime } from "../time";
import { Search } from "./Search";
import { SoloPanel } from "./SoloPanel";
import "./lobby.css";

// three.js ne se charge qu'avec l'écran Jouer ; la pièce dessinée en SVG sert d'attente.
const HeroPiece3D = lazy(() => import("../ui/HeroPiece3D"));

const DECK_SLOTS = 7;

export type PlayMode = "ranked" | "friendly";

const MODES: { id: PlayMode; label: string; sub: string; glow: string; piece: "king" | "pawn" }[] = [
  { id: "ranked", label: "Classée", sub: "Elo et compétences en jeu", glow: "var(--accent)", piece: "king" },
  { id: "friendly", label: "Amicale", sub: "Pour le plaisir, rien à perdre", glow: "var(--rar-rare)", piece: "pawn" },
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

export function Lobby({ state }: { state: AppState }) {
  const { lobby, deck, account, friends } = state;
  const isAccount = !!account && !account.guest;
  const [picked, setPicked] = useState<PlayMode | null>(readMode);
  // Un invité ne peut pas jouer en classée : son mode par défaut est l'amicale.
  const mode: PlayMode = picked ?? (isAccount ? "ranked" : "friendly");
  const [sheet, setSheet] = useState<"room" | "solo" | { friend: string } | null>(null);
  const [code, setCode] = useState("");
  const time = useTime();
  const connected = state.connection === "open";
  const waiting = lobby.type !== "idle";

  const choose = (m: PlayMode) => {
    setPicked(m);
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {
      // Mode privé : le choix ne survit pas au rechargement.
    }
  };

  // Une file ou une salle ouverte prend tout l'écran : le parcours continue sur « Recherche ».
  if (waiting) return <Search lobby={lobby} />;

  const current = MODES.find((m) => m.id === mode)!;
  const elo = account?.elo ?? 1200;
  const online = sortFriends(friends.friends.filter((f) => f.presence !== "offline"));
  const challenged = typeof sheet === "object" && sheet ? friends.friends.find((f) => f.username === sheet.friend) : undefined;
  const closeSheet = () => setSheet(null);

  return (
    <main className="jp" data-mode={mode}>
      <section className="jp-stage" aria-labelledby="jp-title">
        <Beam width={560} height={400} glow={current.glow} />
        {isAccount && (
          <span className="chip jp-elo">
            <span className="num">{elo}</span> Elo
          </span>
        )}
        <div className="jp-hero">
          <div className="jp-floor" aria-hidden="true">
            <i />
            <i />
          </div>
          <Suspense fallback={<HeroPiece kind={current.piece} className="jp-piece" />}>
            <HeroPiece3D kind={current.piece} className="jp-piece" />
          </Suspense>
        </div>
        <div className="jp-head">
          <h1 id="jp-title" className="jp-title">
            {current.label}
          </h1>
          <p className="jp-sub">{current.sub} · {timeText(time)}</p>
        </div>
      </section>

      <div className="jp-panel">
        <h2 className="jp-panel-title">Nouvelle partie</h2>
        <div className="segs" role="radiogroup" aria-label="Mode">
          {MODES.map((m) => (
            <button key={m.id} type="button" role="radio" aria-checked={mode === m.id} className={`sg${mode === m.id ? " on" : ""}`} onClick={() => choose(m.id)}>
              {m.label}
            </button>
          ))}
        </div>

        <div className="jp-times" role="radiogroup" aria-label="Durée de partie">
          {TIMES.map((t) => (
            <button key={t.id} type="button" role="radio" aria-checked={time === t.id} className={`sel${time === t.id ? " on" : ""}`} onClick={() => setTime(t.id)}>
              <strong className="jp-time-l">{t.label}</strong>
              <span className="meta">{t.id === "short" ? "Blitz · 5 min" : `${t.minutes} min`}</span>
            </button>
          ))}
        </div>

        {isAccount ? (
          <div className="jp-friends" role="group" aria-label="Amis en ligne">
            <div className="jp-friends-head">
              <span className="jp-friends-hint">{online.length === 0 ? "Aucun ami en ligne" : "Touchez un ami pour le défier"}</span>
              <a className="link" href={hrefFor({ name: "friends" })}>
                {online.length === 0 && friends.friends.length === 0 ? "Ajouter" : "Tous"}
              </a>
            </div>
            {online.length > 0 && (
              <div className="jp-friends-row">
                {online.slice(0, 6).map((f) => (
                  <button key={f.username} type="button" className="jp-friend" aria-label={`Défier ${f.username}`} onClick={() => setSheet({ friend: f.username })}>
                    <span className="avatar jp-av">
                      {initialOf(f.username)}
                      <span className={`presence ${f.presence}`} aria-hidden="true" />
                    </span>
                    <span className="jp-friend-name">{f.username}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        ) : (
          <p className="jp-guest">
            <a className="link" href={hrefFor({ name: "auth" })}>
              Créez un compte
            </a>{" "}
            pour jouer en classée et défier des amis.
          </p>
        )}

        <div className="jp-act">
          {mode === "ranked" && !isAccount ? (
            <a className="btn pri block jp-cta" href={hrefFor({ name: "auth" })}>
              Créer un compte
            </a>
          ) : (
            <button type="button" className="btn pri block jp-cta" disabled={!connected} onClick={() => store.send({ type: "queue_join", ranked: mode === "ranked", time })}>
              Jouer · {TIMES.find((t) => t.id === time)!.short}
            </button>
          )}
          <p className="jp-note">{mode === "ranked" ? `Votre Elo (${elo}) et une compétence sont en jeu` : "Sans enjeu : ni Elo ni compétence à gagner"}</p>
        </div>

        <a className="jp-deck" href={hrefFor({ name: "collection" })} aria-label="Votre deck">
          <span className="jp-deck-hex" aria-hidden="true">
            {Array.from({ length: DECK_SLOTS }, (_, i) => {
              const id = deck[i];
              if (!id) return <span key={i} className="hex jp-deck-empty" />;
              const info = skillInfo(id);
              return (
                <span key={id} className="jp-deck-slot" data-rar={tileRarity(id)} style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties} title={info.name}>
                  <SkillArt id={id} size={22} />
                </span>
              );
            })}
          </span>
          <span className="jp-deck-txt">
            <strong>Votre deck</strong>
            <span className="muted">
              {deck.length}/{DECK_SLOTS} · {deck.slice(0, 3).map((id) => skillInfo(id).name).join(", ")}
            </span>
          </span>
        </a>

        <div className="jp-links">
          <button type="button" className="link" onClick={() => setSheet("room")}>
            Salle privée
          </button>
          <span aria-hidden="true">·</span>
          <button type="button" className="link" onClick={() => setSheet("solo")}>
            Contre l'IA
          </button>
          <span aria-hidden="true">·</span>
          <a className="link" href={hrefFor({ name: "campaign" })}>
            Campagne
          </a>
        </div>
      </div>

      <ChallengeSheet friend={challenged ?? null} onClose={closeSheet} />

      <Sheet open={sheet === "room"} title="Salle privée" onClose={closeSheet}>
        <p className="sheet-sub">Jouez avec un ami grâce à un code.</p>
        <div className="segs" role="radiogroup" aria-label="Durée">
              {TIMES.map((t) => (
                <button key={t.id} type="button" role="radio" aria-checked={time === t.id} className={`sg${time === t.id ? " on" : ""}`} onClick={() => setTime(t.id)}>
                  {t.label}
                </button>
              ))}
            </div>

        <button
          type="button"
          className="btn pri block"
          disabled={!connected}
          onClick={() => {
            store.send({ type: "create_room", time });
            closeSheet();
          }}
        >
          Créer une salle
        </button>
        <form
          className="jp-join"
          onSubmit={(e) => {
            e.preventDefault();
            if (code.trim()) {
              store.send({ type: "join_room", code: code.trim() });
              closeSheet();
            }
          }}
        >
          <label className="sr-only" htmlFor="room-code">
            Code de salle
          </label>
          <input id="room-code" className="input" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="Code de salle" maxLength={8} autoComplete="off" spellCheck={false} />
          <button type="submit" className="btn" disabled={!code.trim() || !connected}>
            Rejoindre
          </button>
        </form>
      </Sheet>

      <Sheet open={sheet === "solo"} title="Contre l'IA" onClose={closeSheet}>
        <SoloPanel state={state} />
      </Sheet>
    </main>
  );
}

/** Statut affiché sous le pseudo dans « Votre groupe ». */
export function groupStatus(l: AppState["lobby"]): string {
  if (l.type === "queued") return "En recherche";
  if (l.type === "room_waiting") return "Salle ouverte";
  return "Prêt";
}
