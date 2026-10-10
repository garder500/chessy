import { lazy, Suspense, useEffect, useState, type CSSProperties } from "react";
import { useT, t } from "../i18n";
import { hrefFor } from "../router";
import { campaignSeen, totalStars, TOTAL_STARS } from "../campaign";
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
import { Star } from "../ui/Stars";
import { Search } from "./Search";
import { SoloPanel } from "./SoloPanel";
import "./campaign.css";
import "./lobby.css";

// three.js ne se charge qu'avec l'écran Jouer ; la pièce dessinée en SVG sert d'attente.
const HeroPiece3D = lazy(() => import("../ui/HeroPiece3D"));

const DECK_SLOTS = 7;

export type PlayMode = "ranked" | "friendly";

// `label` et `sub` sont des clés de traduction (résolues au rendu).
const MODES: { id: PlayMode; label: string; sub: string; glow: string; piece: "king" | "pawn" }[] = [
  { id: "ranked", label: "lobby.mode_ranked", sub: "lobby.mode_ranked_sub", glow: "var(--accent)", piece: "king" },
  { id: "friendly", label: "lobby.mode_friendly", sub: "lobby.mode_friendly_sub", glow: "var(--rar-rare)", piece: "pawn" },
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
  const t = useT();
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
  // La carte Campagne montre les étoiles déjà gagnées : on les demande une fois la connexion ouverte.
  useEffect(() => {
    if (connected && !waiting) store.loadCampaign();
  }, [connected, waiting]);

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
  // Un compte dont l'Elo est encore celui de départ ; un serveur ancien ne le dit pas.
  const placement = isAccount ? account?.placement : undefined;
  const unplaced = !!placement && !placement.placed;
  const online = sortFriends(friends.friends.filter((f) => f.presence !== "offline"));
  const challenged = typeof sheet === "object" && sheet ? friends.friends.find((f) => f.username === sheet.friend) : undefined;
  const closeSheet = () => setSheet(null);

  return (
    <main className="jp" data-mode={mode}>
      <section className="jp-stage" aria-labelledby="jp-title">
        <Beam width={560} height={400} glow={current.glow} />
        {isAccount && (
          <span className="chip jp-elo">
            {unplaced ? (
              t("lobby.elo_unrated")
            ) : (
              <>
                <span className="num">{elo}</span> {t("lobby.elo_unit")}
              </>
            )}
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
            {t(current.label)}
          </h1>
          <p className="jp-sub">{t(current.sub)} · {timeText(time)}</p>
        </div>
      </section>

      <div className="jp-panel">
        {unplaced && placement && (
          <div className="jp-place card">
            <p>
              <strong>{t("lobby.elo_unrated")}</strong> · {t("lobby.placement_progress", { done: placement.done, total: placement.total })}
            </p>
            <p className="muted">
              {t("lobby.placement_blurb")}
            </p>
            <button type="button" className="btn pri" disabled={!connected || state.soloPending} onClick={() => store.startPlacement()}>
              {placement.done === 0 ? t("lobby.placement_start") : t("lobby.placement_next")}
            </button>
          </div>
        )}
        <h2 className="jp-panel-title">{t("lobby.new_game")}</h2>
        <div className="segs" role="radiogroup" aria-label={t("lobby.mode_aria")}>
          {MODES.map((m) => (
            <button key={m.id} type="button" role="radio" aria-checked={mode === m.id} className={`sg${mode === m.id ? " on" : ""}`} onClick={() => choose(m.id)}>
              {t(m.label)}
            </button>
          ))}
        </div>

        <div className="jp-times" role="radiogroup" aria-label={t("lobby.duration_aria")}>
          {TIMES.map((tm) => (
            <button key={tm.id} type="button" role="radio" aria-checked={time === tm.id} className={`sel${time === tm.id ? " on" : ""}`} onClick={() => setTime(tm.id)}>
              <strong className="jp-time-l">{tm.label}</strong>
              <span className="meta">{timeText(tm.id)}</span>
            </button>
          ))}
        </div>

        {isAccount ? (
          <div className="jp-friends" role="group" aria-label={t("lobby.friends_online_aria")}>
            <div className="jp-friends-head">
              <span className="jp-friends-hint">{online.length === 0 ? t("lobby.friends_none") : t("lobby.friends_hint")}</span>
              <a className="link" href={hrefFor({ name: "friends" })}>
                {online.length === 0 && friends.friends.length === 0 ? t("lobby.friends_add") : t("lobby.friends_all")}
              </a>
            </div>
            {online.length > 0 && (
              <div className="jp-friends-row">
                {online.slice(0, 6).map((f) => (
                  <button key={f.username} type="button" className="jp-friend" aria-label={t("lobby.challenge_aria", { name: f.username })} onClick={() => setSheet({ friend: f.username })}>
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
              {t("lobby.guest_create")}
            </a>{" "}
            {t("lobby.guest_rest")}
          </p>
        )}

        <div className="jp-act">
          {mode === "ranked" && !isAccount ? (
            <a className="btn pri block jp-cta" href={hrefFor({ name: "auth" })}>
              {t("lobby.create_account")}
            </a>
          ) : (
            <button type="button" className="btn pri block jp-cta" disabled={!connected} onClick={() => store.send({ type: "queue_join", ranked: mode === "ranked", time })}>
              {t("lobby.play", { time: TIMES.find((tm) => tm.id === time)!.short })}
            </button>
          )}
          <p className="jp-note">{mode === "ranked" ? t("lobby.note_ranked", { elo }) : t("lobby.note_friendly")}</p>
        </div>

        <a className="cp-entry" href={hrefFor({ name: "campaign" })}>
          <span className="cp-entry-hex" aria-hidden="true">
            <Star on size={22} />
          </span>
          <span className="cp-entry-txt">
            <strong>
              {t("campaign.title")}
              {!campaignSeen() && <span className="cp-new">{t("campaign.new")}</span>}
            </strong>
            <span className="muted">
              {state.campaign ? t("campaign.entry_progress", { n: totalStars(state.campaign), max: TOTAL_STARS }) : t("campaign.mode_sub")}
            </span>
          </span>
        </a>

        <a className="jp-deck" href={hrefFor({ name: "collection" })} aria-label={t("lobby.deck_aria")}>
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
            <strong>{t("lobby.deck_title")}</strong>
            <span className="muted">
              {deck.length}/{DECK_SLOTS} · {deck.slice(0, 3).map((id) => skillInfo(id).name).join(", ")}
            </span>
          </span>
        </a>

        <div className="jp-links">
          <button type="button" className="link" onClick={() => setSheet("room")}>
            {t("lobby.private_room")}
          </button>
          <span aria-hidden="true">·</span>
          <button type="button" className="link" onClick={() => setSheet("solo")}>
            {t("lobby.vs_ai")}
          </button>
        </div>
      </div>

      <ChallengeSheet friend={challenged ?? null} onClose={closeSheet} />

      <Sheet open={sheet === "room"} title={t("lobby.private_room")} onClose={closeSheet}>
        <p className="sheet-sub">{t("lobby.room_sub")}</p>
        <div className="segs" role="radiogroup" aria-label={t("lobby.duration_short_aria")}>
              {TIMES.map((tm) => (
                <button key={tm.id} type="button" role="radio" aria-checked={time === tm.id} className={`sg${time === tm.id ? " on" : ""}`} onClick={() => setTime(tm.id)}>
                  {tm.label}
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
          {t("lobby.room_create")}
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
            {t("lobby.room_code")}
          </label>
          <input id="room-code" className="input" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder={t("lobby.room_code")} maxLength={8} autoComplete="off" spellCheck={false} />
          <button type="submit" className="btn" disabled={!code.trim() || !connected}>
            {t("lobby.room_join")}
          </button>
        </form>
      </Sheet>

      <Sheet open={sheet === "solo"} title={t("lobby.vs_ai")} onClose={closeSheet}>
        <SoloPanel state={state} />
      </Sheet>
    </main>
  );
}

/** Statut affiché sous le pseudo dans « Votre groupe ». */
export function groupStatus(l: AppState["lobby"]): string {
  if (l.type === "queued") return t("lobby.status_searching");
  if (l.type === "room_waiting") return t("lobby.status_room_open");
  return t("lobby.status_ready");
}
