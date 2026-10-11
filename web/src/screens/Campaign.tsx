import { useCallback, useEffect, useState } from "react";
import { api, apiErrorText } from "../api";
import { type CampaignView, currentLevel, currentTitle, defaultSelection, lockReason, withLiveForge } from "../campaign";
import { hrefFor } from "../router";
import { readToken, useAppState } from "../store";
import { useCompact } from "../ui/useCompact";
import { BossForge } from "./campaign/BossForge";
import { CampaignHeader } from "./campaign/CampaignHeader";
import { ChapterMap } from "./campaign/ChapterMap";
import { ChapterRail } from "./campaign/ChapterRail";
import { LevelPanel, showPanel } from "./campaign/LevelPanel";
import "./campaign.css";
import "./campaign-map.css";

type Loaded = { status: "loading" } | { status: "error"; text: string } | ({ status: "ready" } & CampaignView);

/** Page `#/campaign` : la carte de progression, chapitre par chapitre, avec ses niveaux verrouillés. */
export function Campaign() {
  const { account, connection, soloPending, deck, bossForge } = useAppState();
  const compact = useCompact();
  const accountId = account?.player_id ?? null;
  const isAccount = !!account && !account.guest;
  const [loaded, setLoaded] = useState<Loaded>({ status: "loading" });
  const [picked, setPicked] = useState<{ chapter: number; level: number } | null>(null);
  const [forgeChapter, setForgeChapter] = useState<number | null>(null);
  const closeForge = useCallback(() => setForgeChapter(null), []);

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
  const selection = picked ?? (chapters.length > 0 ? defaultSelection(chapters) : null);
  const chapter = chapters.find((c) => c.chapter === selection?.chapter);
  const level = chapter?.levels.find((l) => l.level === selection?.level);

  return (
    <main className="cp-page">
      <header className="cp-head">
        <div>
          <p className="eyebrow">Mode solo</p>
          <h1 className="cp-title">Campagne</h1>
        </div>
        {loaded.status === "ready" && <CampaignHeader totalStars={loaded.total_stars} maxStars={loaded.max_stars} title={currentTitle(chapters)} />}
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
      {chapter && level && (
        <>
          <ChapterRail chapters={chapters} selected={chapter.chapter} onSelect={(c) => setPicked({ chapter: c, level: currentLevel(chapters[c]) })} />
          <div className="cp-board">
            <ChapterMap
              chapters={chapters}
              chapter={chapter}
              selectedLevel={level.level}
              compact={compact}
              onPick={(l) => {
                setPicked({ chapter: chapter.chapter, level: l });
                if (compact) showPanel();
              }}
              onReveal={setForgeChapter}
            />
            <LevelPanel
              key={`${chapter.chapter}-${level.level}`}
              chapter={chapter}
              level={level}
              lockReason={lockReason(chapters, chapter, level)}
              ownDeck={deck}
              connected={connection === "open"}
              pending={soloPending}
            />
          </div>
        </>
      )}
      {forgeChapter !== null && <BossForge chapter={forgeChapter} initial={chapters.find((c) => c.chapter === forgeChapter)?.boss_forge} onClose={closeForge} />}
    </main>
  );
}
