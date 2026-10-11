import { useT } from "../i18n";
import { hrefFor } from "../router";
import type { FriendInfo } from "../protocol";
import { store, useAppState } from "../store";
import { setTime, TIMES, timeText, useTime } from "../time";
import { Sheet } from "./Sheet";
import { presenceLabel } from "./social";

/** Panneau « Défier X » : cadence au choix puis envoi du défi (partie amicale, ni Elo ni compétence en jeu). */
export function ChallengeSheet({ friend, onClose }: { friend: FriendInfo | null; onClose: () => void }) {
  const t = useT();
  const { outgoingChallenge, connection } = useAppState();
  const time = useTime();
  const connected = connection === "open";
  const sent = !!friend && outgoingChallenge?.toLowerCase() === friend.username.toLowerCase();
  const busy = outgoingChallenge !== null && !sent;
  const watch = friend?.presence === "in_game" && friend.game_id ? friend.game_id : null;
  const blocked = friend?.presence === "offline" || (friend?.presence === "in_game" && !watch);

  return (
    <Sheet open={!!friend} title={t("challenge.sheet_title", { name: friend?.username ?? "" })} onClose={onClose}>
      {friend && (
        <>
          <p className="sheet-sub">
            {presenceLabel(friend.presence, friend.last_seen)} · {t("challenge.sheet_sub", { elo: friend.elo })}
          </p>
          <div className="segs" role="radiogroup" aria-label={t("challenge.pace_aria")}>
            {TIMES.map((tm) => (
              <button key={tm.id} type="button" role="radio" aria-checked={time === tm.id} className={`sg${time === tm.id ? " on" : ""}`} onClick={() => setTime(tm.id)}>
                {tm.label}
              </button>
            ))}
          </div>
          <p className="sheet-sub">{timeText(time)}</p>
          {watch ? (
            <a className="btn pri block" href={hrefFor({ name: "watch", param: watch })}>
              {t("challenge.watch_game")}
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
              {sent ? t("challenge.sent") : busy ? t("challenge.other_pending") : blocked ? t("challenge.unavailable", { name: friend.username }) : t("challenge.send")}
            </button>
          )}
        </>
      )}
    </Sheet>
  );
}
