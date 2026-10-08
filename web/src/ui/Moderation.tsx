import { useEffect, useState } from "react";
import type { ReportReason } from "../protocol";
import { store, useAppState } from "../store";
import { Sheet } from "./Sheet";
import { sameUser } from "./social";
import "./moderation.css";

export const REPORT_REASONS: { id: ReportReason; label: string }[] = [
  { id: "harassment", label: "Harcèlement ou insultes" },
  { id: "spam", label: "Spam" },
  { id: "cheating", label: "Triche" },
  { id: "inappropriate_name", label: "Pseudo inapproprié" },
  { id: "other", label: "Autre" },
];

interface Props {
  username: string;
  /** Partie liée au signalement, le cas échéant. */
  gameId?: string;
  /** Derniers messages du chat, joints au signalement si le joueur le souhaite. */
  excerpt?: string;
}

/** Bloquer / débloquer et signaler un joueur (comptes seulement : un invité n'a rien à bloquer). */
export function ModerationActions({ username, gameId, excerpt }: Props) {
  const { account, blocked, connection } = useAppState();
  const [report, setReport] = useState(false);
  const [confirmBlock, setConfirmBlock] = useState(false);
  const online = connection === "open";
  const member = !!account && !account.guest;
  useEffect(() => {
    if (member && online) store.loadBlocks();
  }, [member, online, account?.player_id]);
  if (!account || account.guest || sameUser(account.username, username)) return null;
  const isBlocked = blocked.some((b) => sameUser(b, username));

  return (
    <span className="mod-actions">
      {isBlocked ? (
        <button type="button" className="btn sm ghost" disabled={!online} onClick={() => store.unblockUser(username)}>
          Débloquer
        </button>
      ) : (
        <button type="button" className="btn sm ghost" disabled={!online} onClick={() => setConfirmBlock(true)}>
          Bloquer
        </button>
      )}
      <button type="button" className="btn sm ghost" disabled={!online} onClick={() => setReport(true)}>
        Signaler
      </button>
      <BlockSheet open={confirmBlock} username={username} onClose={() => setConfirmBlock(false)} />
      <ReportSheet open={report} username={username} gameId={gameId} excerpt={excerpt} onClose={() => setReport(false)} />
    </span>
  );
}

/** Confirmation avant de bloquer : l'effet est large (amitié, demandes, défis, chat) et le joueur n'en est pas informé. */
function BlockSheet({ open, username, onClose }: { open: boolean; username: string; onClose: () => void }) {
  return (
    <Sheet open={open} title={`Bloquer ${username} ?`} onClose={onClose}>
      <p className="sheet-sub">
        Ses messages, ses demandes d'ami et ses défis ne vous parviendront plus, et il sera retiré de vos amis. Vous pouvez toujours jouer
        contre lui. {username} n'en est pas informé, et vous pourrez le débloquer à tout moment dans Réglages.
      </p>
      <div className="mod-confirm">
        <button
          type="button"
          className="btn danger solid block"
          onClick={() => {
            store.blockUser(username);
            onClose();
          }}
        >
          Bloquer {username}
        </button>
        <button type="button" className="btn ghost block" onClick={onClose}>
          Annuler
        </button>
      </div>
    </Sheet>
  );
}

/** Choix du motif puis envoi du signalement ; l'adversaire n'en est jamais informé. */
function ReportSheet({ open, username, gameId, excerpt, onClose }: Props & { open: boolean; onClose: () => void }) {
  const [reason, setReason] = useState<ReportReason>("harassment");
  const [attach, setAttach] = useState(true);
  return (
    <Sheet open={open} title={`Signaler ${username}`} onClose={onClose}>
      <p className="sheet-sub">Le signalement est transmis à la modération. {username} n'en est pas informé.</p>
      <div className="mod-reasons" role="radiogroup" aria-label="Motif">
        {REPORT_REASONS.map((r) => (
          <button key={r.id} type="button" role="radio" aria-checked={reason === r.id} className={`sg${reason === r.id ? " on" : ""}`} onClick={() => setReason(r.id)}>
            {r.label}
          </button>
        ))}
      </div>
      {excerpt && (
        <label className="mod-attach">
          <input type="checkbox" checked={attach} onChange={(e) => setAttach(e.target.checked)} />
          <span>Joindre les derniers messages de la partie</span>
        </label>
      )}
      <button
        type="button"
        className="btn pri block"
        onClick={() => {
          store.reportUser(username, reason, gameId, excerpt && attach ? excerpt : undefined);
          onClose();
        }}
      >
        Envoyer le signalement
      </button>
    </Sheet>
  );
}
