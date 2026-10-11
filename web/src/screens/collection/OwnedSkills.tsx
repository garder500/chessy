import { useMemo, useState } from "react";
import { useT } from "../../i18n";
import type { SkillHistoryEntry, SkillId } from "../../protocol";
import { describeEntry, filterOwned, ORIGIN_FILTERS, originCounts, ownedSkills, type OriginFilter } from "../collectionData";
import { FilterChips } from "./FilterChips";
import { SkillRow } from "./SkillRow";

/** Les compétences du deck, avec leur origine ; filtrables par façon de les avoir obtenues. */
export function OwnedSkills({ deck, entries }: { deck: readonly SkillId[]; entries: readonly SkillHistoryEntry[] }) {
  const t = useT();
  const [filter, setFilter] = useState<OriginFilter>("all");
  const [open, setOpen] = useState<SkillId | null>(null);
  const owned = useMemo(() => ownedSkills(deck, entries), [deck, entries]);
  const counts = useMemo(() => originCounts(owned), [owned]);
  const shown = filterOwned(owned, filter);

  return (
    <>
      <FilterChips filters={ORIGIN_FILTERS} value={filter} onChange={setFilter} counts={counts} ariaLabel={t("collection.origin_aria")} />
      {shown.length === 0 && <p className="co-empty card">{t(owned.length === 0 ? "collection.empty_owned" : "collection.empty_filter")}</p>}
      <ul className="hl-list">
        {shown.map(({ skill, origin }) => (
          <SkillRow
            key={skill}
            skill={skill}
            note={origin ? describeEntry(origin) : t("collection.src_earlier")}
            open={open === skill}
            onToggle={() => setOpen(open === skill ? null : skill)}
          />
        ))}
      </ul>
    </>
  );
}
