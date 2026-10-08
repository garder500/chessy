import { hrefFor } from "../router";
import type { FriendInfo } from "../protocol";
import { store, useAppState } from "../store";
import { setTime, TIMES, timeText, useTime } from "../time";
import { Sheet } from "./Sheet";
import { presenceLabel } from "./social";

/** Panneau « Défier X » : cadence au choix puis envoi du défi (partie amicale, ni Elo ni compétence en jeu). */
export function ChallengeSheet({ friend, onClose }: { friend: FriendInfo | null; onClose: () => void }) {
  const { outgoingChallenge, connection } = useAppState();
  const time = useTime();
  const connected = connection === "open";
  const sent = !!friend && outgoingChallenge?.toLowerCase() === friend.username.toLowerCase();
  const busy = outgoingChallenge !== null && !sent;
  const watch = friend?.presence === "in_game" && friend.game_id ? friend.game_id : null;
  const blocked = friend?.presence === "offline" || (friend?.presence === "in_game" && !watch);

  return (
    <Sheet open={!!friend} title={`Défier ${friend?.username ?? ""}`} onClose={onClose}>
      {friend && (
        <>
          <p className="sheet-sub">
            {presenceLabel(friend.presence, friend.last_seen)} · {friend.elo} Elo · partie amicale : ni Elo ni compétence en jeu.
          </p>
          <div className="segs" role="radiogroup" aria-label="Cadence">
            {TIMES.map((t) => (
              <button key={t.id} type="button" role="radio" aria-checked={time === t.id} className={`sg${time === t.id ? " on" : ""}`} onClick={() => setTime(t.id)}>
                {t.label}
              </button>
            ))}
          </div>
          <p className="sheet-sub">{timeText(time)}</p>
          {watch ? (
            <a className="btn pri block" href={hrefFor({ name: "watch", param: watch })}>
              Regarder la partie
            </a>
          ) : (
            <button
              type="button"
              className="btn pri block"
              disabled={!connected || blocked || sent || busy}
              onClick={() => {
                store.send({ type: "challenge", username: friend.username, time });
                onClose();
              }}
            >
              {sent ? "Défi envoyé" : busy ? "Un autre défi est en attente" : blocked ? `${friend.username} est indisponible` : "Envoyer le défi"}
            </button>
          )}
        </>
      )}
    </Sheet>
  );
}
