import type { CSSProperties } from "react";
import { type CampaignLevelView, countStars, isLocked } from "../../campaign";
import { percent, type MapLayout, type NodePoint } from "../../campaignMap";
import { Stars } from "../../ui/Stars";
import { LockIcon } from "./LockIcon";

interface Props {
  level: CampaignLevelView;
  point: NodePoint;
  layout: MapLayout;
  selected: boolean;
  /** Étoiles du chapitre et seuil du boss : la jauge de la porte. */
  gate: { stars: number; required: number } | null;
  onPick: () => void;
}

type NodeState = "won" | "current" | "locked";

function stateOf(level: CampaignLevelView): NodeState {
  if (isLocked(level)) return "locked";
  return level.best[0] ? "won" : "current";
}

function ariaLabel(level: CampaignLevelView, state: NodeState, gate: Props["gate"]): string {
  const name = level.boss ? `Boss, ${level.name}` : `Niveau ${level.level + 1}, ${level.name}`;
  if (state === "locked") return `${name}, verrouillé${gate ? ` : ${gate.stars} étoiles sur ${gate.required}` : ""}`;
  if (state === "current") return `${name}, à jouer`;
  const won = countStars(level.best);
  return `${name}, ${won} étoile${won > 1 ? "s" : ""} sur 3`;
}

function Medal({ level, state }: { level: CampaignLevelView; state: NodeState }) {
  if (state === "locked" && !level.boss) return <LockIcon size={20} />;
  return <>{level.boss ? <span className="cp-crown">♛</span> : level.level + 1}</>;
}

/** Un niveau posé sur la carte : conquis, à jouer (« vous êtes ici ») ou verrouillé. */
export function MapNode({ level, point, layout, selected, gate, onPick }: Props) {
  const state = stateOf(level);
  const style = { left: percent(point[0], layout.width), top: percent(point[1], layout.height) } as CSSProperties;
  return (
    <button
      type="button"
      className={`cp-node ${state}${level.boss ? " boss" : ""}${selected ? " selected" : ""}`}
      style={style}
      aria-label={ariaLabel(level, state, gate)}
      aria-current={state === "current" ? "step" : undefined}
      aria-disabled={state === "locked" || undefined}
      aria-pressed={selected}
      onClick={onPick}
    >
      <span className="cp-medal">
        <Medal level={level} state={state} />
      </span>
      <span className="cp-node-name">{level.boss ? `Boss · ${level.name}` : level.name}</span>
      {state === "won" && <Stars stars={level.best} size={14} />}
      {state === "current" && <span className="cp-here">Vous êtes ici</span>}
      {level.boss && gate && state === "locked" && (
        <span className="cp-gauge" aria-hidden="true">
          <span className="cp-gauge-bar"><span style={{ width: percent(Math.min(gate.stars, gate.required), gate.required) }} /></span>
          <span className="mono">{gate.stars} / {gate.required} ★</span>
        </span>
      )}
    </button>
  );
}
