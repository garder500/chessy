import "./stars.css";

/** Étoile pleine (gagnée) ou creuse. */
export function Star({ on, size = 16 }: { on: boolean; size?: number }) {
  return (
    <svg className={`cp-star${on ? " on" : ""}`} viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" focusable="false">
      <path d="M12 2.8l2.7 5.7 6.2.8-4.6 4.3 1.2 6.2L12 16.7l-5.5 3.1 1.2-6.2L3.1 9.3l6.2-.8z" />
    </svg>
  );
}

export function Stars({ mask, size }: { mask: number; size?: number }) {
  return (
    <span className="cp-stars" aria-hidden="true">
      <Star on={!!(mask & 1)} size={size} />
      <Star on={!!(mask & 2)} size={size} />
      <Star on={!!(mask & 4)} size={size} />
    </span>
  );
}
