import { useEffect, useRef } from "react";
import { useT } from "../i18n";
import { store } from "../store";
import { initialOf } from "./NavBar";

/** Défi reçu d'un ami : accepter ou refuser (il expire côté serveur après 60 s). */
export function ChallengeModal({ challenge }: { challenge: { username: string; elo: number } }) {
  const t = useT();
  const accept = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    accept.current?.focus();
  }, []);

  return (
    <div className="md-backdrop">
      <div
        className="md card"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="challenge-title"
        onKeyDown={(e) => e.key === "Escape" && store.respondChallenge(false)}
      >
        <p className="eyebrow">{t("challenge.received")}</p>
        <div className="md-who">
          <span className="avatar">{initialOf(challenge.username)}</span>
          <div>
            <h2 id="challenge-title" className="md-title">
              {t("challenge.challenges_you", { name: challenge.username })}
            </h2>
            <p className="muted">
              <span className="mono">{challenge.elo}</span> {t("challenge.elo_friendly")}
            </p>
          </div>
        </div>
        <div className="md-actions">
          <button type="button" className="btn ghost" onClick={() => store.respondChallenge(false)}>
            {t("challenge.decline")}
          </button>
          <button type="button" className="btn pri" ref={accept} onClick={() => store.respondChallenge(true)}>
            {t("challenge.accept")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Rappel d'un défi envoyé, en attente de réponse. */
export function OutgoingChallenge({ username }: { username: string }) {
  const t = useT();
  return (
    <div className="out-challenge card" role="status">
      <span>
        {t("challenge.sent_to_pre")}<strong>{username}</strong>{t("challenge.sent_to_post")}
      </span>
      <button type="button" className="btn sm ghost" onClick={() => store.cancelChallenge()}>
        {t("challenge.cancel")}
      </button>
    </div>
  );
}
