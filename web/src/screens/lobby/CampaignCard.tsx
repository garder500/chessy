import { useEffect, useState } from "react";
import { api } from "../../api";
import { BOSS_LEVEL, type CampaignChapterView, type CampaignView, totalLabel } from "../../campaign";
import { hrefFor } from "../../router";
import { readToken } from "../../store";

/** Premier chapitre ouvert dont le boss n'est pas encore vaincu ; à défaut, le dernier ouvert. */
function currentChapter(chapters: CampaignChapterView[]): number {
  const open = chapters.filter((c) => c.available);
  const ongoing = open.find((c) => !c.levels.find((l) => l.level === BOSS_LEVEL)?.best[0]);
  return (ongoing ?? open[open.length - 1])?.chapter ?? 0;
}

interface Props {
  /** Identifiant du compte une fois le `welcome` reçu : il fixe le jeton de session. */
  accountId: string | null;
  isAccount: boolean;
}

/** Entrée de la campagne dans l'accueil : chapitre en cours et total d'étoiles. */
export function CampaignCard({ accountId, isAccount }: Props) {
  const [view, setView] = useState<CampaignView | null>(null);

  useEffect(() => {
    if (accountId === null || !isAccount) return;
    const ctl = new AbortController();
    api
      .campaign(readToken() ?? "", ctl.signal)
      .then((res) => setView(res as CampaignView))
      .catch(() => undefined);
    return () => ctl.abort();
  }, [accountId, isAccount]);

  if (!isAccount) {
    return (
      <a className="jp-campaign off" href={hrefFor({ name: "auth" })} aria-label="Campagne : créez un compte">
        <strong>Campagne</strong>
        <span className="muted">Créez un compte</span>
      </a>
    );
  }
  return (
    <a className="jp-campaign" href={hrefFor({ name: "campaign" })}>
      <strong>Campagne</strong>
      <span className="muted">
        {view ? (
          <>
            Chapitre {currentChapter(view.chapters) + 1} · <span className="num">{totalLabel(view.total_stars, view.max_stars)}</span>
          </>
        ) : (
          "Chargement…"
        )}
      </span>
    </a>
  );
}
