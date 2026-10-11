import { useT } from "../../i18n";
import { SPEEDS, speedLabel, type NavAction, type ReplayNav } from "../../replay/nav";

interface Props {
  nav: ReplayNav;
  disabled?: boolean;
  dispatch: (action: NavAction) => void;
}

const Icon = ({ d }: { d: string }) => (
  <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true" focusable="false" fill="currentColor">
    <path d={d} />
  </svg>
);

/** Début, précédent, lecture automatique, suivant, fin ; barre de progression ; vitesse. */
export function Controls({ nav, disabled = false, dispatch }: Props) {
  const t = useT();
  const atStart = nav.index <= 0;
  const atEnd = nav.index >= nav.max;
  return (
    <div className="rp-controls card" role="group" aria-label={t("replay.nav_aria")}>
      <div className="rp-transport">
        <button type="button" className="btn sm rp-ibtn" onClick={() => dispatch({ type: "first" })} disabled={disabled || atStart} aria-label={t("replay.first_aria")} aria-keyshortcuts="Home" title={t("replay.first_title")}>
          <Icon d="M4 4h2v12H4zM16 4v12L7 10z" />
        </button>
        <button type="button" className="btn sm rp-ibtn" onClick={() => dispatch({ type: "prev" })} disabled={disabled || atStart} aria-label={t("replay.nav_prev")} aria-keyshortcuts="ArrowLeft" title={t("replay.prev_title")}>
          <Icon d="M13 4v12L4 10z" />
        </button>
        <button
          type="button"
          className="btn sm pri rp-ibtn rp-play"
          onClick={() => dispatch({ type: "toggle" })}
          disabled={disabled || nav.max === 0}
          aria-label={t(nav.playing ? "replay.pause_aria" : "replay.play_aria")}
          aria-pressed={nav.playing}
          aria-keyshortcuts="Space"
          title={t(nav.playing ? "replay.pause_title" : "replay.play_title")}
        >
          {nav.playing ? <Icon d="M5 4h4v12H5zM11 4h4v12h-4z" /> : <Icon d="M6 3.5v13L16.5 10z" />}
        </button>
        <button type="button" className="btn sm rp-ibtn" onClick={() => dispatch({ type: "next" })} disabled={disabled || atEnd} aria-label={t("replay.nav_next")} aria-keyshortcuts="ArrowRight" title={t("replay.next_title")}>
          <Icon d="M7 4v12l9-6z" />
        </button>
        <button type="button" className="btn sm rp-ibtn" onClick={() => dispatch({ type: "last" })} disabled={disabled || atEnd} aria-label={t("replay.last_aria")} aria-keyshortcuts="End" title={t("replay.last_title")}>
          <Icon d="M14 4h2v12h-2zM4 4l9 6-9 6z" />
        </button>
      </div>

      <div className="rp-progress">
        <label className="sr-only" htmlFor="rp-range">
          {t("replay.position_label")}
        </label>
        <input
          id="rp-range"
          type="range"
          min={0}
          max={Math.max(1, nav.max)}
          value={nav.index}
          disabled={disabled || nav.max === 0}
          onChange={(e) => dispatch({ type: "goto", index: Number(e.target.value) })}
          aria-valuetext={t("replay.position_value", { index: nav.index, max: nav.max })}
          style={{ ["--rp-pct" as string]: `${nav.max ? (nav.index / nav.max) * 100 : 0}%` }}
        />
        <span className="mono muted rp-count" aria-hidden="true">
          {nav.index} / {nav.max}
        </span>
      </div>

      <div className="rp-speed" role="group" aria-label={t("replay.speed_aria")}>
        {SPEEDS.map((s) => (
          <button key={s} type="button" className={`rp-speed-btn${nav.speed === s ? " on" : ""}`} aria-pressed={nav.speed === s} onClick={() => dispatch({ type: "speed", speed: s })} disabled={disabled}>
            {speedLabel(s)}
          </button>
        ))}
      </div>
    </div>
  );
}
