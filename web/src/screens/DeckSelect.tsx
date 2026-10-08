import { useEffect, useState } from "react";
import type { DeckSelectInfo, SkillId } from "../protocol";
import { skillInfo } from "../skills";
import { store } from "../store";
import { initialOf } from "../ui/NavBar";
import { SkillCard } from "./SkillCard";
import "./deck.css";

export function DeckSelect({ info }: { info: DeckSelectInfo }) {
  const [picked, setPicked] = useState<SkillId[]>([]);
  const [left, setLeft] = useState(info.seconds);

  useEffect(() => {
    window.scrollTo(0, 0);
    setPicked([]);
    setLeft(info.seconds);
    const timer = setInterval(() => setLeft((s) => Math.max(0, s - 1)), 1000);
    return () => clearInterval(timer);
  }, [info.game_id, info.seconds]);

  const classic = info.deck.filter((s) => !skillInfo(s).unique);
  const unique = info.deck.filter((s) => skillInfo(s).unique);

  const toggle = (skill: SkillId) =>
    setPicked((cur) =>
      cur.includes(skill) ? cur.filter((s) => s !== skill) : cur.length < info.max_picks ? [...cur, skill] : cur,
    );

  const opp = info.opponent;
  const isBot = opp.bot === true;
  const oppName = opp.username ?? (isBot ? "Sage" : "Invité");
  const share = info.seconds > 0 ? Math.max(0, Math.min(1, left / info.seconds)) : 0;
  const urgent = left <= 10;

  const ready = picked.length >= Math.min(info.max_picks, classic.length);
  const cta = ready ? "Prêt" : picked.length === 0 ? "Jouer sans compétence" : `Jouer avec ${picked.length} compétence${picked.length > 1 ? "s" : ""}`;

  return (
    <main className="dk-page">
      <header className="dk-top">
        <button type="button" className="dk-back" aria-label={isBot ? "Annuler" : "Quitter"} onClick={() => store.send({ type: "leave_deck_select" })}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M15 5l-7 7 7 7" />
          </svg>
        </button>
        <h1 className="dk-title">Vos compétences</h1>
        <span className={`chip num dk-timer-chip${urgent ? " urgent" : ""}`} role="timer" aria-label={`Temps restant : ${left} secondes`}>
          0:{String(left).padStart(2, "0")}
        </span>
      </header>
      <div className="dk-bar" aria-hidden="true">
        <i className={urgent ? "urgent" : ""} style={{ transform: `scaleX(${share})` }} />
      </div>

      <div className="dk-body">
        <div className="dk-opp" aria-label="Adversaire">
          <span className="avatar">{initialOf(oppName)}</span>
          <span className="dk-opp-txt">
            <strong>{isBot ? oppName : `Contre ${oppName}`}</strong>
            <span className="meta dk-opp-meta">
              {isBot ? "Partie d'entraînement" : `${opp.elo !== null ? `${opp.elo} · ` : ""}${info.rated ? "Classée" : "Amicale"}`} · vous jouez les {info.you === "white" ? "blancs" : "noirs"}
            </span>
          </span>
        </div>

        <section aria-labelledby="dk-classic">
          <p id="dk-classic" className="dk-label">
            Choisissez-en {info.max_picks} dans votre deck <strong data-testid="pick-count">· {picked.length}/{info.max_picks}</strong>
          </p>
          <div className="dk-grid">
            {classic.map((skill) => {
              const rank = picked.indexOf(skill);
              return (
                <SkillCard
                  key={skill}
                  skill={skill}
                  selected={rank >= 0}
                  order={rank >= 0 ? rank + 1 : undefined}
                  disabled={info.submitted || (rank < 0 && picked.length >= info.max_picks)}
                  onClick={() => toggle(skill)}
                />
              );
            })}
          </div>
        </section>

        {unique.length > 0 && (
          <section aria-labelledby="dk-unique" className="dk-unique">
            <p id="dk-unique" className="dk-label">
              Compétence unique <span className="muted">· toujours incluse</span>
            </p>
            <div className="dk-grid">
              {unique.map((skill) => (
                <SkillCard key={skill} skill={skill} locked />
              ))}
            </div>
          </section>
        )}
      </div>

      <footer className="dk-foot">
        {info.submitted ? (
          <p className="dk-wait" role="status">
            Sélection envoyée. En attente de l'adversaire…
          </p>
        ) : (
          <button type="button" className="btn pri block" onClick={() => store.send({ type: "select_deck", skills: picked })}>
            {cta}
          </button>
        )}
      </footer>
    </main>
  );
}
