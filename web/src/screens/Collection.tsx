import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { useT } from "../i18n";
import { FAMILY_LABEL, skillEntry } from "../catalog";
import type { MySkills, SkillHistoryEntry } from "../protocol";
import { readToken, useAppState } from "../store";
import { RarityTag } from "../ui/RarityTag";
import { SkillArt } from "../ui/SkillArt";
import { SkillPreview } from "../ui/skillPreview";
import { relativeTime } from "../ui/social";
import { UniqueBadge } from "../ui/UniqueBadge";
import {
  classicNote,
  describeEntry,
  filterHistory,
  groupByDay,
  HISTORY_FILTERS,
  historyStats,
  RULES,
  uniqueNote,
  type HistoryFilter,
} from "./collectionData";
import "./collection.css";
import { tileRarity } from "../ui/tileRarity";

type Status = "loading" | "ready" | "error";

/** Page `#/collection` : l'historique des compétences obtenues, forgées ou perdues. */
export function Collection() {
  const t = useT();
  // `forged` change quand la définition d'une compétence forgée arrive : relance le rendu des fiches.
  const { deck } = useAppState();
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

      <div className="seg co-filter" role="group" aria-label={t("collection.filter_aria")}>
        {HISTORY_FILTERS.map((f) => (
          <button key={f.id} type="button" aria-pressed={filter === f.id} className={filter === f.id ? "on" : ""} onClick={() => setFilter(f.id)}>
            {t(f.label)}
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
      {status === "ready" && days.length === 0 && (
        <p className="co-empty card">
          {entries.length === 0
            ? t("collection.empty")
            : t("collection.empty_filter")}
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
  const t = useT();
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
            <span className={`hi-change ${lost ? "out" : "in"}`}>{lost ? t("collection.change_lost") : t("collection.change_gained")}</span> · {describeEntry(entry)}
          </span>
          <span className="hi-meta">
            <span className="eyebrow">{FAMILY_LABEL[info.family]}</span>
            {info.rarity ? <RarityTag rarity={info.rarity} /> : info.unique && <span className="tag foil-tag">{t("skills.unique_tag")}</span>}
            {current && <span className="tag co-deck">{t("collection.stat_in_deck")}</span>}
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
          <p className="muted hi-note">{info.unique ? uniqueNote() : classicNote()}</p>
        </div>
      )}
    </li>
  );
}
