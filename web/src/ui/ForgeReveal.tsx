import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../catalog";
import { useT } from "../i18n";
import { RARITIES, RARITY_LABEL } from "../forged";
import { navigate } from "../router";
import { skillInfo } from "../skills";
import { store, useAppState } from "../store";
import { SkillArt } from "./SkillArt";
import { tileRarity } from "./tileRarity";
import "./forgeReveal.css";

const STAGE_W = 390;
const STAGE_H = 844;

/** Instants (en % de l'animation) où le curseur du tirage change de case : il ralentit puis se fige sur la rareté tirée. */
const SCAN_AT = [0, 1.91, 3.92, 6.19, 8.91, 12.26, 16.4, 21.54, 27.83, 35.47, 44.62, 55.47, 68.2, 82.98];

/** Keyframes du curseur : il parcourt les cinq cases en cercle et s'arrête sur `target`. */
function scanKeyframes(target: number): string {
  const n = SCAN_AT.length;
  const start = (((target - (n - 1)) % 5) + 5) % 5;
  const steps = SCAN_AT.map((t, i) => `${t}%{transform:translateX(${((start + i) % 5) * 68}px);animation-timing-function:step-end}`);
  return `@keyframes fr-scan-run{${steps.join("")}100%{transform:translateX(${target * 68}px)}}`;
}

/** Étincelles du marteau : [écart x, écart y] vers le haut, deux tailles et deux couleurs en alternance. */
const SPARKS = Array.from({ length: 14 }, (_, i) => {
  const a = Math.PI * (1.08 + (0.84 * i) / 13);
  const r = 70 + ((i * 37) % 80);
  return { x: Math.round(Math.cos(a) * r), y: Math.round(Math.sin(a) * r), size: 5 + (i % 3) * 2, hot: i % 3 !== 0 };
});

const SHARDS = Array.from({ length: 14 }, (_, i) => {
  const a = (i / 14) * Math.PI * 2;
  const r = 90 + ((i * 29) % 70);
  return { x: Math.round(Math.cos(a) * r), y: Math.round(Math.sin(a) * r), rot: (i % 2 ? 1 : -1) * (120 + ((i * 53) % 240)), w: 8 + (i % 3) * 4 };
});

const CONFETTI = Array.from({ length: 28 }, (_, i) => ({
  left: (i * 47 + 20) % 380,
  w: 10 + (i % 4) * 2,
  h: 10 + ((i * 3) % 4) * 3,
  r: (i % 2 ? 1 : -1) * (180 + ((i * 71) % 330)),
  dur: 2.4 + ((i * 13) % 10) / 10,
  delay: 5.9 + ((i * 7) % 5) / 10,
  rar: RARITIES[i % 5],
}));

/**
 * Révélation d'une compétence forgée (design « Forge animée ») : le forgeron frappe trois fois une pierre qui se fissure et chauffe,
 * un dernier coup la brise et un faisceau tombe ; la rareté est tirée sur cinq cases, son nom s'écrase à l'écran, puis la carte
 * apparaît. Dure environ 7 s ; « Passer » saute à la fin.
 */
export function ForgeReveal({ skill }: { skill: string }) {
  const t = useT();
  const info = skillInfo(skill);
  const rarity = tileRarity(skill);
  const idx = RARITIES.indexOf(rarity);
  const gradId = useId().replace(/:/g, "");
  const [skip, setSkip] = useState(false);
  const [scale, setScale] = useState(1);
  const ref = useRef<HTMLDivElement>(null);
  const { over } = useAppState();

  useEffect(() => {
    ref.current?.focus();
  }, []);
  // La scène est dessinée sur 390 × 844 : on l'ajuste à la fenêtre (téléphone, tablette ou ordinateur).
  useEffect(() => {
    const fit = () => setScale(Math.min(window.innerWidth / STAGE_W, window.innerHeight / STAGE_H, 1.25));
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, []);

  const close = (toCollection: boolean) => {
    store.dismissReveal();
    if (toCollection) {
      // L'écran de victoire est encore ouvert sous la révélation : on le quitte pour montrer la collection.
      if (over) store.leaveGame();
      navigate({ name: "collection" });
    }
  };

  return (
    <div
      className={`fr r-${rarity}${skip ? " skip" : ""}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby="fr-name"
      tabIndex={-1}
      ref={ref}
      onKeyDown={(e) => e.key === "Escape" && close(false)}
      style={{ "--s": scale } as CSSProperties}
    >
      <style>{scanKeyframes(idx)}</style>
      <div className="fr-stage">
        <div className="fr-ember" />
        <p className="fr-cap">{t("forge.smith_working")}</p>

        <div className="fr-shk">
          <div className="fr-rig">
            <svg className="fr-anvil" viewBox="0 0 260 100" width="260" height="100" aria-hidden="true">
              <defs>
                <linearGradient id={`an-${gradId}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stopColor="#59676a" />
                  <stop offset=".25" stopColor="#3a4647" />
                  <stop offset="1" stopColor="#1a2223" />
                </linearGradient>
              </defs>
              <path d="M26 0H212L252 18H222C212 38 196 44 176 46L186 76H222V100H40V76H82L92 46C62 42 42 32 32 18H0Z" fill={`url(#an-${gradId})`} />
              <path d="M26 1.5H212" stroke="var(--accent-ink)" strokeWidth="3" strokeLinecap="round" />
            </svg>
            <div className="fr-floor" />

            <div className="fr-stone hexc">
              <div className="hexc fr-stone-in" />
              <div className="hexc fr-stone-heat" />
              <span className="fr-q">?</span>
              <svg className="fr-cracks" viewBox="0 0 116 132" width="116" height="132" aria-hidden="true">
                <path d="M58 28L50 58L66 76L54 104" pathLength="100" style={{ animationDelay: ".9s" }} />
                <path d="M58 28L74 52L68 78L84 98" pathLength="100" style={{ animationDelay: "1.7s" }} />
                <path d="M40 60L58 66M78 70L96 62M58 104L72 124" pathLength="100" style={{ animationDelay: "2.4s" }} />
              </svg>
            </div>

            <svg className="fr-ham fx" width="260" height="140" viewBox="0 0 260 140" aria-hidden="true">
              <defs>
                <linearGradient id={`hm-${gradId}`} x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0" stopColor="#e3eae8" />
                  <stop offset=".45" stopColor="#8a9694" />
                  <stop offset="1" stopColor="#3a4443" />
                </linearGradient>
                <linearGradient id={`hj-${gradId}`} x1="0" y1="0" x2="1" y2="0">
                  <stop offset="0" stopColor="#0b6a57" />
                  <stop offset=".5" stopColor="#4cc9b0" />
                  <stop offset="1" stopColor="#0b6a57" />
                </linearGradient>
              </defs>
              <rect x="64" y="62" width="186" height="16" rx="3" fill="#2a3231" />
              <rect x="190" y="60" width="52" height="20" rx="3" fill={`url(#hj-${gradId})`} />
              <rect x="64" y="62" width="186" height="4" fill="rgba(255,255,255,.18)" />
              <polygon points="14,22 66,22 80,36 80,104 66,118 14,118 0,104 0,36" fill={`url(#hm-${gradId})`} stroke="#0b0f10" strokeWidth="2.5" strokeLinejoin="round" />
              <rect x="0" y="60" width="80" height="14" fill={`url(#hj-${gradId})`} />
              <polygon points="14,22 66,22 80,36 0,36" fill="rgba(255,255,255,.35)" />
            </svg>

            {SPARKS.map((s, i) => (
              <i key={i} className="fr-spark fx" style={{ width: s.size, height: s.size, "--x": `${s.x}px`, "--y": `${s.y}px`, background: s.hot ? "#ffd27a" : "var(--accent-ink)", boxShadow: `0 0 8px ${s.hot ? "#ffd27a" : "var(--accent-ink)"}` } as CSSProperties} />
            ))}
            {[0.9, 1.7, 2.4].map((d) => (
              <i key={d} className="fr-ringo fx" style={{ animationDelay: `${d}s` }} />
            ))}
            {SHARDS.map((s, i) => (
              <i key={i} className="fr-shard fx hexc" style={{ width: s.w, height: s.w * 1.15, "--x": `${s.x}px`, "--y": `${s.y}px`, "--r": `${s.rot}deg` } as CSSProperties} />
            ))}
            <i className="fr-ringo fr-ringo-big fx" />
            <i className="fr-beam fx" />
          </div>

          {[0.9, 1.7, 2.4].map((d) => (
            <i key={d} className="fr-flash fx" style={{ animationDelay: `${d}s` }} />
          ))}
          <i className="fr-flash fr-flash-big fx" />
        </div>

        <div className="fr-roll fx">
          <p className="fr-roll-t">{t("forge.rarity_drawn")}</p>
          <div className="fr-cells">
            <i className="fr-scan" />
            {RARITIES.map((r, i) => (
              <div key={r} className={`fr-cell r-${r}${i === idx ? " win" : ""}`}>
                <span className="hexc fr-cell-hex" />
                {RARITY_LABEL[r]}
              </div>
            ))}
          </div>
        </div>

        <div className="fr-title fx">
          <h1>{RARITY_LABEL[rarity]}</h1>
          <p>{t("forge.new_skill")} · {FAMILY_LABEL[info.family]}</p>
        </div>

        <div className="fr-cardwrap fx" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
          <div className="fr-tilebox hexc">
            <div className="hexc fr-tile-bg" />
            <span className="fr-tile-art">
              <SkillArt id={skill} size={84} />
            </span>
          </div>
          <p className="fr-gems" aria-hidden="true">
            {"◆".repeat(idx + 1)}
          </p>
          <h2 id="fr-name" className="fr-name">
            {info.name}
          </h2>
          <p className="fr-desc">{info.description}</p>
          {info.unique && <span className="fr-stamp fx">{t("forge.unique_world")}</span>}
        </div>

        {CONFETTI.map((c, i) => (
          <i key={i} className="fr-conf fx hexc" style={{ left: c.left, width: c.w, height: c.h, background: `var(--rar-${c.rar})`, "--r": `${c.r}deg`, animationDuration: `${c.dur}s`, animationDelay: `${c.delay}s` } as CSSProperties} />
        ))}

        <div className="fr-fin fx">
          <button type="button" className="btn pri block" onClick={() => close(true)}>
            {t("forge.add_to_deck")}
          </button>
          <button type="button" className="link" onClick={() => close(false)}>
            {t("forge.later")}
          </button>
        </div>
        <button type="button" className="link fr-skip fx" onClick={() => setSkip(true)}>
          {t("forge.skip")}
        </button>
      </div>
    </div>
  );
}
