import { useEffect, useRef, useState, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../catalog";
import { RARITIES, RARITY_LABEL } from "../forged";
import { navigate } from "../router";
import { skillInfo } from "../skills";
import { store } from "../store";
import { Confetti } from "./Confetti";
import { SkillArt } from "./SkillArt";
import { tileRarity } from "./tileRarity";
import "./forgeReveal.css";

const SHORT: Record<string, string> = { common: "Commune", uncommon: "Peu com.", rare: "Rare", epic: "Épique", legendary: "Légend." };

/**
 * Révélation d'une compétence forgée : trois coups de marteau (flash, onde, étincelles, secousse), tirage de rareté qui
 * se verrouille, flash, carte qui apparaît, tampon « unique ». Dure environ 6 s ; « Passer » saute à la fin.
 */
export function ForgeReveal({ skill }: { skill: string }) {
  const info = skillInfo(skill);
  const rarity = tileRarity(skill);
  const idx = RARITIES.indexOf(rarity);
  const [skip, setSkip] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, []);
  const close = (toCollection: boolean) => {
    store.dismissReveal();
    if (toCollection) navigate({ name: "collection" });
  };
  const bursts = [1.0, 1.7, 2.4].flatMap((t, k) =>
    Array.from({ length: 14 }, (_, i) => ({ a: i * (360 / 14) + k * 9, r: 80 + ((i * 13 + k * 7) % 50), dl: t })),
  );
  return (
    <div
      className={`fr r-${rarity}${skip ? " skip" : ""}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby="fr-name"
      tabIndex={-1}
      ref={ref}
      style={{ "--end": `${idx * 20}%` } as CSSProperties}
      onKeyDown={(e) => e.key === "Escape" && close(false)}
    >
      <div className="fr-seq">
        <div className="fr-forge" aria-hidden="true" />
        <div className="fr-beams" aria-hidden="true" />
        <p className="fr-cap">Le forgeron au travail</p>

        <svg className="fr-rune" viewBox="0 0 300 300" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth="1.5">
          <circle cx="150" cy="150" r="148" opacity=".7" />
          <g className="fr-a">
            <circle cx="150" cy="150" r="128" strokeDasharray="3 9" strokeWidth="2" />
            <path d="M150 12l7 12-7 12-7-12zM150 264l7 12-7 12-7-12zM12 150l12-7 12 7-12 7zM264 150l12-7 12 7-12 7z" fill="currentColor" stroke="none" />
          </g>
          <g className="fr-b">
            <path d="M150 52L235 200H65zM150 248L65 100h170z" opacity=".85" />
            <circle cx="150" cy="150" r="96" opacity=".5" />
          </g>
        </svg>
        <div className="fr-core" aria-hidden="true" />
        <svg className="fr-hammer" width="140" height="140" viewBox="0 0 140 140" aria-hidden="true" focusable="false">
          <defs>
            <linearGradient id="fr-st" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor="#d7e2f5" />
              <stop offset=".5" stopColor="#8393ad" />
              <stop offset="1" stopColor="#46526a" />
            </linearGradient>
          </defs>
          <path d="M104 120L40 56" stroke="#7a5b3a" strokeWidth="11" strokeLinecap="round" />
          <g transform="rotate(-45 40 56)">
            <rect x="6" y="34" width="68" height="44" rx="4" fill="url(#fr-st)" stroke="#0b0f18" strokeWidth="3" />
          </g>
        </svg>
        {[1, 1.7, 2.4].map((d) => (
          <div key={d} className="fr-ring" style={{ "--d": `${d}s` } as CSSProperties} aria-hidden="true" />
        ))}
        <div className="fr-ring fr-big" style={{ "--d": "4.6s" } as CSSProperties} aria-hidden="true" />
        <div className="fr-ring fr-big" style={{ "--d": "4.85s" } as CSSProperties} aria-hidden="true" />
        <div className="fr-bursts" aria-hidden="true">
          {bursts.map((b, i) => (
            <i key={i} style={{ "--a": `${b.a}deg`, "--r": `${b.r}px`, "--dl": `${b.dl}s` } as CSSProperties} />
          ))}
          {Array.from({ length: 24 }, (_, i) => (
            <i key={`f${i}`} style={{ "--a": `${i * 15}deg`, "--r": `${150 + ((i * 17) % 90)}px`, "--dl": "4.6s" } as CSSProperties} />
          ))}
        </div>
        {[1, 1.7, 2.4].map((d) => (
          <div key={d} className="fr-hf" style={{ "--d": `${d}s` } as CSSProperties} aria-hidden="true" />
        ))}
        <div className="fr-hf fr-hf-big" style={{ "--d": "4.6s" } as CSSProperties} aria-hidden="true" />

        <div className="fr-rou" aria-hidden="true">
          <p className="fr-rou-t">Rareté tirée par le forgeron</p>
          <div className="fr-cells">
            <i className="fr-scan" />
            {RARITIES.map((r, i) => (
              <div key={r} className={`fr-cell r-${r}${i === idx ? "" : " dim"}`}>
                <span className="fr-hex" />
                {SHORT[r]}
              </div>
            ))}
          </div>
        </div>

        <div className="fr-banner">
          <h1>{RARITY_LABEL[rarity]}</h1>
        </div>

        <div className="fr-card" style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
          <div className="fr-flip">
            <div className="fr-face fr-back">
              <svg width="110" height="110" viewBox="0 0 120 120" fill="none" stroke="#4a6296" strokeWidth="2" aria-hidden="true" focusable="false">
                <path d="M60 8L104 34v52L60 112 16 86V34z" />
                <path d="M60 30l26 15v30L60 90 34 75V45z" />
                <path d="M52 52a8 8 0 1116 0c0 8-8 8-8 16M60 78v2" stroke="var(--accent)" strokeWidth="4" strokeLinecap="round" />
              </svg>
            </div>
            <div className="fr-face fr-front">
              <div>
                <span className="fr-gems" aria-hidden="true">
                  {"◆".repeat(idx + 1)}
                </span>
                <span className="fr-tile" data-rar={rarity}>
                  <SkillArt id={skill} size={72} />
                </span>
                <h2 id="fr-name" className="fr-name">
                  {info.name}
                </h2>
                <p className="fr-fam">{FAMILY_LABEL[info.family]}</p>
                <p className="fr-desc">{info.description}</p>
              </div>
            </div>
          </div>
          {info.unique && <p className="fr-stamp">Unique au monde</p>}
        </div>

        <Confetti count={36} delay={4.7} />

        <div className="fr-fin">
          <button type="button" className="btn pri fr-go" onClick={() => close(true)}>
            Rejoindre l'arsenal
          </button>
          <button type="button" className="btn ghost sm" onClick={() => close(false)}>
            Plus tard
          </button>
        </div>
        <button type="button" className="btn sm ghost fr-skip" onClick={() => setSkip(true)}>
          Passer
        </button>
      </div>
    </div>
  );
}
