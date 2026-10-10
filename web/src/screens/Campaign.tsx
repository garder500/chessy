import { useEffect, useState } from "react";
import { api, apiErrorText } from "../api";
import type { CampaignChapter, CampaignLevel } from "../protocol";
import { readToken, useAppState } from "../store";
import { ChapterSection } from "./campaign/ChapterSection";
import { LevelSheet } from "./campaign/LevelSheet";
import "./campaign.css";

type Loaded = { status: "loading" } | { status: "error"; text: string } | { status: "ready"; chapters: CampaignChapter[] };

/** Page `#/campaign` : les chapitres, leurs niveaux avec leurs étoiles, et le boss. */
export function Campaign() {
  const { account, connection, soloPending } = useAppState();
  const accountId = account?.player_id ?? null;
  const [loaded, setLoaded] = useState<Loaded>({ status: "loading" });
  const [picked, setPicked] = useState<{ chapter: number; level: number } | null>(null);

  // On attend le `welcome` : c'est lui qui fixe le jeton de session.
  useEffect(() => {
    if (accountId === null) return;
    const ctl = new AbortController();
    api
      .campaign(readToken() ?? "", ctl.signal)
      .then((res) => setLoaded({ status: "ready", chapters: res.chapters }))
      .catch((e) => {
        if (!ctl.signal.aborted) setLoaded({ status: "error", text: apiErrorText(e) });
      });
    return () => ctl.abort();
  }, [accountId]);

  const chapters = loaded.status === "ready" ? loaded.chapters : [];
  const pickedChapter = chapters.find((c) => c.chapter === picked?.chapter);
  const pickedLevel = pickedChapter?.levels.find((l) => l.level === picked?.level) ?? null;
  const pick = (chapter: CampaignChapter) => (level: CampaignLevel) => setPicked({ chapter: chapter.chapter, level: level.level });

  return (
    <main className="cp-page">
      <header className="cp-head">
        <p className="eyebrow">Mode solo</p>
        <h1 className="cp-title">Campagne</h1>
        <p className="muted">Affrontez Sage avec des decks imposés. Trois étoiles par niveau : victoire, objectif, défi.</p>
      </header>
      {loaded.status === "loading" && <p className="muted">Chargement…</p>}
      {loaded.status === "error" && <p className="cp-error" role="alert">{loaded.text}</p>}
      {chapters.map((chapter) => (
        <ChapterSection key={chapter.chapter} chapter={chapter} onPick={pick(chapter)} />
      ))}
      {pickedChapter && (
        <LevelSheet chapter={pickedChapter} level={pickedLevel} connected={connection === "open"} pending={soloPending} onClose={() => setPicked(null)} />
      )}
    </main>
  );
}
