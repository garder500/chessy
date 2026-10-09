/** Petite couronne dorée marquant une compétence unique (à placer dans un conteneur positionné). */
import { useT } from "../i18n";

export function UniqueBadge({ label }: { label?: string }) {
  const t = useT();
  label ??= t("skills.unique_badge");
  return (
    <span className="foil-badge" role="img" aria-label={label} title={label}>
      <svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true" focusable="false">
        <path d="M2.2 12.6L1.4 5.4l3.5 2.7L8 2.8l3.1 5.3 3.5-2.7-.8 7.2z" fill="currentColor" />
        <path d="M2.6 14.4h10.8" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" fill="none" />
      </svg>
    </span>
  );
}
