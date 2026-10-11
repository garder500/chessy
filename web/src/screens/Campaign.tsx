import { useEffect, useState } from "react";
import { api, apiErrorText } from "../api";
import { type CampaignChapterView, type CampaignLevelView, type CampaignView, totalLabel, withLiveForge } from "../campaign";
import { hrefFor } from "../router";
import { readToken, useAppState } from "../store";
import { ChapterSection } from "./campaign/ChapterSection";
import { LevelSheet } from "./campaign/LevelSheet";
import "./campaign.css";

type Loaded = { status: "loading" } | { status: "error"; text: string } | ({ status: "ready" } & CampaignView);

interface Props {
  /** Révélation de la compétence forgée d'un boss (branchée par l'écran de révélation). */
  onReveal?: (chapter: number) => void;
}

/** Page `#/campaign` : les chapitres, leurs niveaux avec leurs étoiles, et le boss. */
export function Campaign({ onReveal }: Props) {
  const { account, connection, soloPending, deck, bossForge } = useAppState();
  const accountId = account?.player_id ?? null;
  const isAccount = !!account && !account.guest;
  const [loaded, setLoaded] = useState<Loaded>({ status: "loading" });
  const [picked, setPicked] = useState<{ chapter: number; level: number } | null>(null);

  // On attend le `welcome` : c'est lui qui fixe le jeton de session.
  useEffect(() => {
    if (accountId === null || !isAccount) return;
    const ctl = new AbortController();
    api
      .campaign(readToken() ?? "", ctl.signal)
      .then((res) => setLoaded({ status: "ready", ...(res as CampaignView) }))
      .catch((e) => {
        if (!ctl.signal.aborted) setLoaded({ status: "error", text: apiErrorText(e) });
      });
    return () => ctl.abort();
  }, [accountId, isAccount]);

  const chapters = loaded.status === "ready" ? withLiveForge(loaded.chapters, bossForge) : [];
  const pickedChapter = chapters.find((c) => c.chapter === picked?.chapter);
  const pickedLevel = pickedChapter?.levels.find((l) => l.level === picked?.level) ?? null;
  const pick = (chapter: CampaignChapterView) => (level: CampaignLevelView) => setPicked({ chapter: chapter.chapter, level: level.level });

  return (
    <main className="cp-page">
      <header className="cp-head">
        <p className="eyebrow">Mode solo</p>
        <h1 className="cp-title">Campagne</h1>
        {loaded.status === "ready" && <p className="mono cp-total">{totalLabel(loaded.total_stars, loaded.max_stars)}</p>}
        <p className="muted">Affrontez Sage avec des mains imposées. Trois étoiles par niveau : victoire, objectif, défi.</p>
      </header>
      {account && !isAccount && (
        <p className="muted">
          <a className="link" href={hrefFor({ name: "auth" })}>
            Créez un compte
          </a>{" "}
          pour jouer la campagne.
        </p>
      )}
      {isAccount && loaded.status === "loading" && <p className="muted">Chargement…</p>}
      {isAccount && loaded.status === "error" && <p className="cp-error" role="alert">{loaded.text}</p>}
      {chapters.map((chapter) => (
        <ChapterSection key={chapter.chapter} chapter={chapter} onPick={pick(chapter)} onReveal={onReveal} />
      ))}
      {pickedChapter && (
        <LevelSheet key={`${picked?.chapter}-${picked?.level}`} chapter={pickedChapter} level={pickedLevel} ownDeck={deck} connected={connection === "open"} pending={soloPending} onClose={() => setPicked(null)} />
      )}
    </main>
  );
}
