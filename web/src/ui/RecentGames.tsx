import { useT } from "../i18n";
import type { RecentGame } from "../protocol";
import { colorCap, kindLabel } from "../replay/frames";
import { hrefFor } from "../router";
import { formatDelta, reasonText, relativeTime, RESULT_LABEL } from "./social";
import "./recentgames.css";

interface Props {
  games: RecentGame[];
  /** Limite d'affichage (par défaut : tout). */
  limit?: number;
}

/** Liste des dernières parties d'un joueur : résultat, adversaire, variation d'Elo, motif, date. */
export function RecentGames({ games, limit }: Props) {
  const t = useT();
  const shown = limit ? games.slice(0, limit) : games;
  if (shown.length === 0) return <p className="rg-empty muted">{t("games.recent_empty")}</p>;
  return (
    <ul className="rg-list">
      {shown.map((g) => (
        <li key={g.game_id} className="rg-row">
          <span className={`rg-result ${g.result}`}>{RESULT_LABEL[g.result]}</span>
          <span className="rg-main">
            <span className="rg-opp">
              {t("games.vs")}{" "}
              {g.opponent ? (
                <a href={hrefFor({ name: "profile", param: g.opponent })}>{g.opponent}</a>
              ) : (
                <span className="muted">{t("replay.seat_guest")}</span>
              )}
            </span>
            <span className="rg-meta">
              {reasonText(g.reason, g.result)} · {colorCap(g.color)} · {relativeTime(g.at)}
            </span>
          </span>
          <span className="rg-side">
            <span className={`rg-delta mono ${g.elo_delta !== null && g.elo_delta > 0 ? "up" : g.elo_delta !== null && g.elo_delta < 0 ? "down" : ""}`} aria-label={g.rated ? t("games.delta_aria", { delta: formatDelta(g.elo_delta) }) : undefined}>
              {g.rated ? formatDelta(g.elo_delta) : "—"}
            </span>
            <span className="tag">{kindLabel("duel", g.rated)}</span>
            <a className="rg-replay" href={hrefFor({ name: "replay", param: g.game_id })} aria-label={g.opponent ? t("games.review_aria", { opponent: g.opponent }) : t("games.recent_review_guest")}>
              {t("games.review")}
            </a>
          </span>
        </li>
      ))}
    </ul>
  );
}
