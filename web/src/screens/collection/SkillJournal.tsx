import { useMemo, useState } from "react";
import { useT } from "../../i18n";
import type { SkillHistoryEntry } from "../../protocol";
import { describeEntry, filterJournal, groupByDay, JOURNAL_FILTERS, type JournalFilter } from "../collectionData";
import { FilterChips } from "./FilterChips";
import { SkillRow } from "./SkillRow";

/** Le journal chronologique, jour par jour : chaque gain et chaque perte dit d'où il vient. */
export function SkillJournal({ entries, owned }: { entries: readonly SkillHistoryEntry[]; owned: ReadonlySet<string> }) {
  const t = useT();
  const [filter, setFilter] = useState<JournalFilter>("all");
  const [open, setOpen] = useState<number | null>(null);
  const days = useMemo(() => groupByDay(filterJournal(entries, filter)), [entries, filter]);
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
    <>
      <FilterChips filters={JOURNAL_FILTERS} value={filter} onChange={setFilter} ariaLabel={t("collection.filter_aria")} />
      {days.length === 0 && <p className="co-empty card">{t(entries.length === 0 ? "collection.empty" : "collection.empty_filter")}</p>}
      {days.map((day) => (
        <section key={day.label} className="hl-day" aria-label={day.label}>
          <h2 className="hl-day-title">{day.label}</h2>
          <ul className="hl-list">
            {day.entries.map((e) => (
              <SkillRow
                key={e.id}
                skill={e.skill}
                note={describeEntry(e)}
                lost={e.change === "lost"}
                current={current.has(e.id)}
                at={e.at}
                open={open === e.id}
                onToggle={() => setOpen(open === e.id ? null : e.id)}
              />
            ))}
          </ul>
        </section>
      ))}
    </>
  );
}
