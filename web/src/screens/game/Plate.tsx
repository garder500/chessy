import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { capturedPieces, evalShare, formatClock, materialBalance, materialOf, remainingMs } from "../../game/logic";
import type { Clock as ClockState, Color, Piece, SkillId, StateView } from "../../protocol";
import { skillInfo } from "../../skills";
import { PieceIcon, SkillArt } from "../../ui/SkillArt";
import { initialOf } from "../../ui/NavBar";

/** Horloge interpolée côté client entre deux messages du serveur. */
export function ClockFace({ clock, color, stamp, label }: { clock: ClockState; color: Color; stamp: number; label: string }) {
  const running = clock.running === color;
  const [now, setNow] = useState(() => performance.now());

  useEffect(() => {
    if (!running) return;
    setNow(performance.now());
    const timer = setInterval(() => setNow(performance.now()), 100);
    return () => clearInterval(timer);
  }, [running, clock]);

  const ms = remainingMs(clock, color, running ? now - stamp : 0);
  const cls = ["gm-clock", "mono", running ? "on" : "", ms < 30_000 ? "low" : ""].filter(Boolean).join(" ");
  return (
    <span className={cls} role="timer" aria-label={label}>
      {formatClock(ms)}
    </span>
  );
}

interface PlateProps {
  name: string;
  elo: number | null;
  color: Color;
  board: (Piece | null)[];
  /** Pièces du camp adverse à `color` actuellement sur son banc (elles ne sont pas des captures). */
  rivalBench?: Piece[];
  /** Son propre banc (nous seulement : celui de l'adversaire est caché). */
  ownBench?: Piece[];
  clock: ClockState;
  stamp: number;
  active: boolean;
  /** Compétences utilisées par ce joueur. */
  used: SkillId[];
  /** Compétences encore disponibles (adversaire : nombre seulement). */
  remaining: number;
  disconnected?: boolean;
  you?: boolean;
  /** Adversaire IA : étiquette « IA » et Elo présenté comme un niveau. */
  bot?: boolean;
  /** Faux en solo : l'horloge n'est ni affichée ni interpolée. */
  clockEnabled?: boolean;
  clockLabel: string;
}

export function Plate({ name, elo, color, board, ownBench, rivalBench, clock, stamp, active, used, remaining, disconnected, you, bot, clockEnabled = true, clockLabel }: PlateProps) {
  const t = useT();
  // Les pièces que ce joueur a prises sont les pièces manquantes de l'autre camp.
  const taken = capturedPieces(board, color === "white" ? "black" : "white", rivalBench);
  const lead = materialOf(board, color, ownBench) - materialOf(board, color === "white" ? "black" : "white", rivalBench);
  return (
    <div className={`gm-plate card${active ? " turn" : ""}`}>
      <span className={`avatar${you ? " solid" : ""}`}>{initialOf(name)}</span>
      <div className="gm-plate-main">
        <div className="gm-plate-name">
          <strong>{name}</strong>
          {elo !== null && (
            <span className="mono muted">
              {bot ? "· " : ""}
              {elo}
            </span>
          )}
          {bot && <span className="tag">{t("game.tag_ai")}</span>}
          {disconnected && <span className="tag">{t("game.tag_disconnected")}</span>}
          {active && <span className="tag gm-turn-tag">{t("game.tag_turn")}</span>}
        </div>
        <div className="gm-plate-sub">
          <span className="gm-taken" aria-label={taken.length ? t("game.taken", { count: taken.length }) : t("game.taken_none")}>
            {taken.map((kind, i) => (
              <PieceIcon key={i} kind={kind} size={15} />
            ))}
            {lead > 0 && <span className="mono gm-lead">+{lead}</span>}
          </span>
          <span className="gm-used" aria-label={t("game.used_aria")}>
            {used.map((s) => (
              <span key={s} className="gm-used-ico" title={t("game.used_title", { name: skillInfo(s).name })}>
                <SkillArt id={s} size={20} />
              </span>
            ))}
            {remaining > 0 && (
              <span className="mono muted gm-remaining">
                {t("game.remaining", { count: remaining })}
              </span>
            )}
          </span>
        </div>
      </div>
      {clockEnabled && <ClockFace clock={clock} color={color} stamp={stamp} label={clockLabel} />}
    </div>
  );
}

/** Barre d'évaluation matérielle, du point de vue du joueur (sa part en bas). */
export function EvalBar({ view }: { view: StateView }) {
  const t = useT();
  const balance = materialBalance(view.board, view.you, view.benched);
  const share = evalShare(balance);
  const text = balance === 0 ? t("game.eval_even") : balance > 0 ? t("game.eval_you", { n: balance }) : t("game.eval_opp", { n: -balance });
  return (
    <div className="gm-eval" role="img" aria-label={t("game.eval_aria", { text })} title={text}>
      <i style={{ height: `${Math.round(share * 100)}%` }} className={view.you} />
    </div>
  );
}
