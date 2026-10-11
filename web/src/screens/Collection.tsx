import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { type CampaignChapterView, type CampaignView, withLiveForge } from "../campaign";
import { FAMILY_LABEL, skillEntry } from "../catalog";
import type { BossForgeInfo, MySkills, SkillHistoryEntry } from "../protocol";
import { readToken, useAppState } from "../store";
import { BossForge } from "./campaign/BossForge";
import { RarityTag } from "../ui/RarityTag";
import { SkillArt } from "../ui/SkillArt";
import { SkillPreview } from "../ui/skillPreview";
import { relativeTime } from "../ui/social";
import { UniqueBadge } from "../ui/UniqueBadge";
import {
  CLASSIC_NOTE,
  describeEntry,
  filterHistory,
  groupByDay,
  HISTORY_FILTERS,
  historyStats,
  RULES,
  UNIQUE_NOTE,
  type HistoryFilter,
} from "./collectionData";
import "./collection.css";
import { tileRarity } from "../ui/tileRarity";

type Status = "loading" | "ready" | "error";

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

/** Page `#/collection` : l'historique des compétences obtenues, forgées ou perdues. */
export function Collection() {
  // `forged` change quand la définition d'une compétence forgée arrive : relance le rendu des fiches.
  const { deck } = useAppState();
  const pendingForges = usePendingForges();
  const [forgeChapter, setForgeChapter] = useState<number | null>(null);
  const closeForge = useCallback(() => setForgeChapter(null), []);
  const [data, setData] = useState<MySkills | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [filter, setFilter] = useState<HistoryFilter>("all");
  const [open, setOpen] = useState<number | null>(null);
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
  const days = useMemo(() => groupByDay(filterHistory(entries, filter)), [entries, filter]);
  const owned = useMemo(() => new Set<string>(data?.deck ?? deck), [data, deck]);
  const legendary = useMemo(
    () => filterHistory(entries, "forged").filter((e) => skillEntry(e.skill).rarity === "legendary").length,
    [entries],
  );
  // Seule la ligne la plus récente d'une compétence dit qu'elle est « dans votre deck ».
  const current = useMemo(() => {
    const seen = new Set<string>();
    const ids = new Set<number>();
    for (const e of entries) {
      if (seen.has(e.skill)) continue;
      seen.add(e.skill);
      if (e.change === "gained" && owned.has(e.skill)) ids.add(e.id);
    }
    return ids;
  }, [entries, owned]);

  return (
    <main className="co-page">
      <header className="co-head">
        <p className="eyebrow">Historique</p>
        <h1 className="co-title">Collection</h1>
        <p className="co-lead muted">
          Les compétences que vous avez obtenues ou forgées, et ce qu'il est advenu d'elles. Votre deck en contient 7 au maximum.
        </p>
      </header>

      <section className="co-stats" aria-label="Résumé">
        <Stat label="Forgées" value={stats.forged} hint={legendary > 0 ? `dont ${legendary} légendaire${legendary > 1 ? "s" : ""}` : undefined} />
        <Stat label="Obtenues" value={stats.obtained} />
        <Stat label="Perdues" value={stats.lost} />
        <Stat label="Dans votre deck" value={owned.size} hint="sur 7" />
      </section>

      <PendingForges forges={pendingForges} onOpen={setForgeChapter} />
      {forgeChapter !== null && <BossForge chapter={forgeChapter} initial={pendingForges.find((f) => f.chapter === forgeChapter)} onClose={closeForge} />}

      <div className="seg co-filter" role="group" aria-label="Filtrer l'historique">
        {HISTORY_FILTERS.map((f) => (
          <button key={f.id} type="button" aria-pressed={filter === f.id} className={filter === f.id ? "on" : ""} onClick={() => setFilter(f.id)}>
            {f.label}
          </button>
        ))}
      </div>

      {status === "loading" && <p className="co-empty muted">Chargement de l'historique…</p>}
      {status === "error" && (
        <p className="co-empty card" role="alert">
          Impossible de charger l'historique.{" "}
          <button type="button" className="btn sm ghost" onClick={() => void load()}>
            Réessayer
          </button>
        </p>
      )}
      {status === "ready" && days.length === 0 && (
        <p className="co-empty card">
          {entries.length === 0
            ? "Aucune compétence pour l'instant."
            : "Rien dans cette catégorie. Gagnez une partie classée pour forger ou prendre une compétence."}
        </p>
      )}

      {days.map((day) => (
        <section key={day.label} className="hi-day" aria-label={day.label}>
          <h2 className="hi-day-title">{day.label}</h2>
          <ul className="hi-list">
            {day.entries.map((e) => (
              <Row key={e.id} entry={e} current={current.has(e.id)} open={open === e.id} onToggle={() => setOpen(open === e.id ? null : e.id)} />
            ))}
          </ul>
        </section>
      ))}
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

function Row({ entry, current, open, onToggle }: { entry: SkillHistoryEntry; current: boolean; open: boolean; onToggle: () => void }) {
  const info = skillEntry(entry.skill);
  const lost = entry.change === "lost";
  const classes = ["hi-row", lost ? "lost" : "", info.unique ? "foil" : ""].filter(Boolean).join(" ");
  return (
    <li className={classes} style={{ ["--fam" as string]: `var(--fam-${info.family})` }}>
      <button type="button" className="hi-head" aria-expanded={open} onClick={onToggle}>
        <span className="hi-art" data-rar={tileRarity(entry.skill)}>
          <SkillArt id={entry.skill} size={46} />
          {info.unique && <UniqueBadge />}
        </span>
        <span className="hi-body">
          <span className="hi-name">{info.name}</span>
          <span className="hi-what">
            <span className={`hi-change ${lost ? "out" : "in"}`}>{lost ? "Perdue" : "Obtenue"}</span> · {describeEntry(entry)}
          </span>
          <span className="hi-meta">
            <span className="eyebrow">{FAMILY_LABEL[info.family]}</span>
            {info.rarity ? <RarityTag rarity={info.rarity} /> : info.unique && <span className="tag foil-tag">unique</span>}
            {current && <span className="tag co-deck">Dans votre deck</span>}
          </span>
        </span>
        <time className="hi-when mono muted" dateTime={entry.at}>
          {relativeTime(entry.at)}
        </time>
      </button>
      {open && (
        <div className="hi-detail">
          <p className="hi-rules">{(RULES as Record<string, string>)[entry.skill] ?? info.description}</p>
          <SkillPreview id={entry.skill} caption />
          <p className="muted hi-note">{info.unique ? UNIQUE_NOTE : CLASSIC_NOTE}</p>
        </div>
      )}
    </li>
  );
}
