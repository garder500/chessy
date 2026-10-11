import { useEffect, useState } from "react";
import { useT } from "../i18n";
import type { ReportReason } from "../protocol";
import { store, useAppState } from "../store";
import { Sheet } from "./Sheet";
import { sameUser } from "./social";
import "./moderation.css";

/** `labelKey` : clé de dictionnaire, traduite au rendu. */
export const REPORT_REASONS: { id: ReportReason; labelKey: string }[] = [
  { id: "harassment", labelKey: "moderation.reason_harassment" },
  { id: "spam", labelKey: "moderation.reason_spam" },
  { id: "cheating", labelKey: "moderation.reason_cheating" },
  { id: "inappropriate_name", labelKey: "moderation.reason_inappropriate_name" },
  { id: "other", labelKey: "moderation.reason_other" },
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
  const t = useT();
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
          {t("moderation.unblock")}
        </button>
      ) : (
        <button type="button" className="btn sm ghost" disabled={!online} onClick={() => setConfirmBlock(true)}>
          {t("moderation.block")}
        </button>
      )}
      <button type="button" className="btn sm ghost" disabled={!online} onClick={() => setReport(true)}>
        {t("moderation.report")}
      </button>
      <BlockSheet open={confirmBlock} username={username} onClose={() => setConfirmBlock(false)} />
      <ReportSheet open={report} username={username} gameId={gameId} excerpt={excerpt} onClose={() => setReport(false)} />
    </span>
  );
}

/** Confirmation avant de bloquer : l'effet est large (amitié, demandes, défis, chat) et le joueur n'en est pas informé. */
function BlockSheet({ open, username, onClose }: { open: boolean; username: string; onClose: () => void }) {
  const t = useT();
  return (
    <Sheet open={open} title={t("moderation.block_title", { username })} onClose={onClose}>
      <p className="sheet-sub">{t("moderation.block_body", { username })}</p>
      <div className="mod-confirm">
        <button
          type="button"
          className="btn danger solid block"
          onClick={() => {
            store.blockUser(username);
            onClose();
          }}
        >
          {t("moderation.block_confirm", { username })}
        </button>
        <button type="button" className="btn ghost block" onClick={onClose}>
          {t("moderation.cancel")}
        </button>
      </div>
    </Sheet>
  );
}

/** Choix du motif puis envoi du signalement ; l'adversaire n'en est jamais informé. */
function ReportSheet({ open, username, gameId, excerpt, onClose }: Props & { open: boolean; onClose: () => void }) {
  const t = useT();
  const [reason, setReason] = useState<ReportReason>("harassment");
  const [attach, setAttach] = useState(true);
  return (
    <Sheet open={open} title={t("moderation.report_title", { username })} onClose={onClose}>
      <p className="sheet-sub">{t("moderation.report_sub", { username })}</p>
      <div className="mod-reasons" role="radiogroup" aria-label={t("moderation.reason_aria")}>
        {REPORT_REASONS.map((r) => (
          <button key={r.id} type="button" role="radio" aria-checked={reason === r.id} className={`sg${reason === r.id ? " on" : ""}`} onClick={() => setReason(r.id)}>
            {t(r.labelKey)}
          </button>
        ))}
      </div>
      {excerpt && (
        <label className="mod-attach">
          <input type="checkbox" checked={attach} onChange={(e) => setAttach(e.target.checked)} />
          <span>{t("moderation.attach")}</span>
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
        {t("moderation.send")}
      </button>
    </Sheet>
  );
}
