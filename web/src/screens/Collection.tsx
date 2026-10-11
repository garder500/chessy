import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { type CampaignChapterView, type CampaignView, withLiveForge } from "../campaign";
import { useT } from "../i18n";
import { skillEntry } from "../catalog";
import type { BossForgeInfo, MySkills } from "../protocol";
import { readToken, useAppState } from "../store";
import { BossForge } from "./campaign/BossForge";
import { OwnedSkills } from "./collection/OwnedSkills";
import { SkillJournal } from "./collection/SkillJournal";
import { filterHistory, historyStats } from "./collectionData";
import "./collection.css";

type Status = "loading" | "ready" | "error";
type Section = "owned" | "journal";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "owned", label: "collection.section_owned" },
  { id: "journal", label: "collection.section_journal" },
];

/** Compétences forgées par un boss et pas encore placées : la révélation interrompue se rejoue d'ici. */
function usePendingForges(): BossForgeInfo[] {
  const { account, bossForge } = useAppState();
  const accountId = account && !account.guest ? account.player_id : null;
  const [chapters, setChapters] = useState<CampaignChapterView[]>([]);
  useEffect(() => {
    if (accountId === null) return;
    const ctl = new AbortController();
    api
      .campaign(readToken() ?? "", ctl.signal)
      .then((res) => setChapters((res as CampaignView).chapters))
      .catch(() => {});
    return () => ctl.abort();
  }, [accountId]);
  return withLiveForge(chapters, bossForge).flatMap((c) => (c.boss_forge?.state === "pending" ? [c.boss_forge] : []));
}

function PendingForges({ forges, onOpen }: { forges: BossForgeInfo[]; onOpen: (chapter: number) => void }) {
  return (
    <>
      {forges.map((f) => (
        <p key={f.chapter} className="card co-empty">
          Forgée en attente · chapitre {f.chapter + 1}{" "}
          <button type="button" className="btn sm pri" onClick={() => onOpen(f.chapter)}>
            Révéler
          </button>
        </p>
      ))}
    </>
  );
}

/** Page `#/collection` : les compétences du deck et le journal de ce qui a été obtenu, forgé ou perdu. */
export function Collection() {
  const t = useT();
  // `forged` change quand la définition d'une compétence forgée arrive : relance le rendu des fiches.
  const { deck } = useAppState();
  const pendingForges = usePendingForges();
  const [forgeChapter, setForgeChapter] = useState<number | null>(null);
  const closeForge = useCallback(() => setForgeChapter(null), []);
  const [data, setData] = useState<MySkills | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [section, setSection] = useState<Section>("owned");
  const ctl = useRef<AbortController | null>(null);

  const load = useCallback(async () => {
    ctl.current?.abort();
    const c = new AbortController();
    ctl.current = c;
    try {
      const res = await api.mySkills(readToken() ?? "", c.signal);
      if (c.signal.aborted) return;
      setData(res);
      setStatus("ready");
    } catch {
      if (!c.signal.aborted) setStatus((s) => (s === "ready" ? s : "error"));
    }
  }, []);

  // Recharge quand le deck change (une récompense vient d'être réclamée).
  const deckKey = deck.join(",");
  useEffect(() => {
    void load();
    return () => ctl.current?.abort();
  }, [load, deckKey]);

  const entries = useMemo(() => data?.entries ?? [], [data]);
  const stats = useMemo(() => historyStats(entries), [entries]);
  const ownedIds = data?.deck ?? deck;
  const owned = useMemo(() => new Set<string>(ownedIds), [ownedIds]);
  const legendary = useMemo(
    () => filterHistory(entries, "forged").filter((e) => skillEntry(e.skill).rarity === "legendary").length,
    [entries],
  );

  return (
    <main className="co-page">
      <header className="co-head">
        <p className="eyebrow">{t("collection.eyebrow")}</p>
        <h1 className="co-title">{t("collection.title")}</h1>
        <p className="co-lead muted">
          {t("collection.lead")}
        </p>
      </header>

      <section className="co-stats" aria-label={t("collection.summary_aria")}>
        <Stat label={t("collection.stat_forged")} value={stats.forged} hint={legendary > 0 ? t("collection.stat_legendary", { count: legendary }) : undefined} />
        <Stat label={t("collection.stat_obtained")} value={stats.obtained} />
        <Stat label={t("collection.stat_lost")} value={stats.lost} />
        <Stat label={t("collection.stat_in_deck")} value={owned.size} hint={t("collection.stat_of_seven")} />
      </section>

      <PendingForges forges={pendingForges} onOpen={setForgeChapter} />
      {forgeChapter !== null && <BossForge chapter={forgeChapter} initial={pendingForges.find((f) => f.chapter === forgeChapter)} onClose={closeForge} />}

      <div className="seg co-filter" role="group" aria-label={t("collection.section_aria")}>
        {SECTIONS.map((s) => (
          <button key={s.id} type="button" aria-pressed={section === s.id} className={section === s.id ? "on" : ""} onClick={() => setSection(s.id)}>
            {t(s.label)}
          </button>
        ))}
      </div>

      {status === "loading" && <p className="co-empty muted">{t("collection.loading")}</p>}
      {status === "error" && (
        <p className="co-empty card" role="alert">
          {t("collection.error")}{" "}
          <button type="button" className="btn sm ghost" onClick={() => void load()}>
            {t("collection.retry")}
          </button>
        </p>
      )}
      {status === "ready" && section === "owned" && <OwnedSkills deck={ownedIds} entries={entries} />}
      {status === "ready" && section === "journal" && <SkillJournal entries={entries} owned={owned} />}
    </main>
  );
}

function Stat({ label, value, hint }: { label: string; value: number; hint?: string }) {
  return (
    <div className="co-stat">
      <span className="co-stat-n mono">{value}</span>
      <span className="co-stat-l">{label}</span>
      {hint && <span className="co-stat-h muted">{hint}</span>}
    </div>
  );
}
