import { useT } from "../../i18n";

interface Props<F extends string> {
  filters: readonly { id: F; label: string }[];
  value: F;
  onChange: (filter: F) => void;
  ariaLabel: string;
  counts?: Record<F, number>;
}

/** Filtres secondaires d'une section (puces), avec un compteur optionnel. */
export function FilterChips<F extends string>({ filters, value, onChange, ariaLabel, counts }: Props<F>) {
  const t = useT();
  return (
    <div className="co-chips" role="group" aria-label={ariaLabel}>
      {filters.map((f) => (
        <button key={f.id} type="button" aria-pressed={value === f.id} className={`btn sm ${value === f.id ? "pri" : "ghost"}`} onClick={() => onChange(f.id)}>
          {t(f.label)}
          {counts && <span className="mono co-chip-n">{counts[f.id]}</span>}
        </button>
      ))}
    </div>
  );
}
