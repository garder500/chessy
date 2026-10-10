import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { FAMILY_LABEL } from "../catalog";
import {
  BOSS_GATE,
  CHAPTER_COUNT,
  CHAPTER_STARS,
  MAX_HAND,
  TOTAL_STARS,
  bossName,
  bossOf,
  chapterName,
  chapterOf,
  chapterOpen,
  chapterTotal,
  familyOfChapter,
  gateStars,
  levelById,
  levelLabel,
  levelName,
  lockedReason,
  markCampaignSeen,
  readHand,
  resumeLevel,
  starCount,
  starLines,
  toggleHand,
  totalStars,
  writeHand,
} from "../campaign";
import { RARITY_LABEL } from "../forged";
import { useT } from "../i18n";
import type { CampaignLevel, SkillId } from "../protocol";
import { navigate } from "../router";
import { skillInfo } from "../skills";
import { store, type AppState } from "../store";
import { Sheet } from "../ui/Sheet";
import { Star, Stars } from "../ui/Stars";
import { SkillArt } from "../ui/SkillArt";
import { tileRarity } from "../ui/tileRarity";
import { CampaignForge } from "./CampaignForge";
import "./campaign.css";

/** Position de chaque niveau sur la carte (viewBox 100 × 150) : le chemin serpente de bas en haut jusqu'au boss. */
const SPOTS: Record<number, [number, number]> = {
  1: [28, 134],
  2: [72, 113],
  3: [30, 92],
  4: [72, 71],
  5: [30, 50],
  6: [68, 31],
  7: [50, 9],
};

function Lock() {
  return (
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true" focusable="false">
      <rect x="5" y="11" width="14" height="9" rx="1.5" />
      <path d="M8 11V8a4 4 0 018 0v3" />
    </svg>
  );
}

function Crown() {
  return (
    <svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor" aria-hidden="true" focusable="false">
      <path d="M12 3l2.2 5.6L20 6l-2.2 11H6.2L4 6l5.8 2.6zM6 19.5h12V21H6z" />
    </svg>
  );
}

const chapterStyle = (chapter: number) => ({ "--fam": `var(--fam-${familyOfChapter(chapter)})` }) as CSSProperties;

function Map({ levels, chapter, selected, onSelect }: { levels: CampaignLevel[]; chapter: number; selected: number; onSelect: (id: number) => void }) {
  const t = useT();
  const items = chapterOf(levels, chapter);
  const boss = bossOf(levels, chapter);
  const spot = (l: CampaignLevel) => SPOTS[l.index];
  const gate = boss && !boss.unlocked && chapterOpen(levels, chapter);
  return (
    <div className="cp-map" style={chapterStyle(chapter)}>
      <svg className="cp-lines" viewBox="0 0 100 150" preserveAspectRatio="none" aria-hidden="true">
        {items.slice(0, -1).map((l, i) => {
          const next = items[i + 1];
          const [x1, y1] = spot(l);
          const [x2, y2] = spot(next);
          return <line key={l.id} x1={x1} y1={y1} x2={x2} y2={y2} className={next.unlocked ? "open" : "shut"} vectorEffect="non-scaling-stroke" />;
        })}
      </svg>
      {gate && (
        <span className="cp-gate chip" style={{ left: "50%", top: `${(22 / 150) * 100}%` }}>
          {t("campaign.boss_gate", { n: BOSS_GATE })}
        </span>
      )}
      {items.map((l) => {
        const [x, y] = spot(l);
        const done = starCount(l.stars);
        const state = !l.unlocked ? "locked" : done === 3 ? "full" : done > 0 ? "part" : "open";
        const label = `${levelLabel(l)} · ${levelName(l)} · ${l.unlocked ? t("campaign.stars_aria", { n: done }) : t("campaign.locked_aria")}`;
        return (
          <div key={l.id} className={`cp-spot${l.boss ? " boss" : ""}`} style={{ left: `${x}%`, top: `${(y / 150) * 100}%` }}>
            {l.boss && <span className="cp-boss-name">{t("campaign.boss_of", { name: bossName(chapter) })}</span>}
            <button
              type="button"
              className={`cp-node ${state}${selected === l.id ? " sel" : ""}`}
              aria-label={label}
              aria-current={selected === l.id ? "true" : undefined}
              onClick={() => onSelect(l.id)}
            >
              <span className="cp-node-in">{!l.unlocked ? <Lock /> : l.boss ? <Crown /> : <span className="num cp-node-n">{l.index}</span>}</span>
            </button>
            {l.unlocked && !l.boss && <Stars mask={l.stars} size={13} />}
            {l.unlocked && l.boss && <Stars mask={l.stars} size={13} />}
            {selected === l.id && !l.boss && <span className="cp-sel-tag">{t("campaign.level_tag", { n: l.index })}</span>}
          </div>
        );
      })}
    </div>
  );
}

/** Une compétence imposée ou choisie : hexagone de famille, nom et famille. */
function HandCard({ skill, order, onClick, pressed }: { skill: SkillId; order?: number; onClick?: () => void; pressed?: boolean }) {
  const info = skillInfo(skill);
  const content = (
    <>
      <span className="cp-skill-art" data-rar={tileRarity(skill)} style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}>
        <SkillArt id={skill} size={28} />
      </span>
      <span className="cp-skill-name">{info.name}</span>
      <span className="cp-skill-fam">{FAMILY_LABEL[info.family]}</span>
      {order !== undefined && <span className="cp-skill-n num">{order}</span>}
    </>
  );
  const style = { "--fam": `var(--fam-${info.family})` } as CSSProperties;
  return onClick ? (
    <button type="button" className={`cp-skill pick${pressed ? " on" : ""}`} style={style} aria-pressed={!!pressed} onClick={onClick}>
      {content}
    </button>
  ) : (
    <div className="cp-skill" style={style}>
      {content}
    </div>
  );
}

function Briefing({ levels, level, state, hand, onHand, onForge }: { levels: CampaignLevel[]; level: CampaignLevel; state: AppState; hand: SkillId[]; onHand: (h: SkillId[]) => void; onForge: () => void }) {
  const t = useT();
  const family = familyOfChapter(level.chapter);
  const deck = state.deck.filter((s) => !skillInfo(s).unique);
  const done = starCount(level.stars);
  const connected = state.connection === "open";
  const blocked = lockedReason(levels, level);
  const needHand = level.choose && hand.length === 0;
  const lines = starLines(level);
  const start = () => {
    if (level.choose) writeHand(hand);
    store.startCampaign(level.id, level.choose ? hand : []);
  };
  return (
    <section className="cp-brief card" style={chapterStyle(level.chapter)} aria-labelledby="cp-brief-title">
      <p className="cp-eyebrow">
        {level.boss ? t("campaign.boss_level", { n: level.chapter }) : t("campaign.brief_eyebrow", { chapter: level.chapter, index: level.index })} · {FAMILY_LABEL[family]}
      </p>
      <h2 id="cp-brief-title" className="cp-brief-title">
        {levelName(level)}
      </h2>
      <div className="cp-sage">
        <span className="avatar cp-sage-av" aria-hidden="true">
          S
        </span>
        <span className="cp-sage-txt">
          <strong>Sage</strong>
          <span className="muted">{level.boss ? t("campaign.sage_boss") : t("campaign.sage_level")}</span>
        </span>
        <span className="chip cp-elo">
          <span className="num">{level.elo}</span> Elo
        </span>
      </div>

      {level.boss && level.forge_min && (
        <p className="cp-forge-min">
          <span className="hex" style={{ width: 14, height: 16, background: `var(--rar-${level.forge_min})` }} aria-hidden="true" />
          {t("campaign.forge_min", { rarity: RARITY_LABEL[level.forge_min] })}
        </p>
      )}

      <h3 className="cp-h">{level.choose ? t("campaign.hand_choose", { n: hand.length, max: MAX_HAND }) : t("campaign.hand_imposed")}</h3>
      {level.choose ? (
        deck.length === 0 ? (
          <p className="muted cp-note">{t("campaign.hand_empty")}</p>
        ) : (
          <>
            <div className="cp-hand cp-hand-pick" role="group" aria-label={t("campaign.hand_aria")}>
              {deck.map((s) => (
                <HandCard key={s} skill={s} pressed={hand.includes(s)} order={hand.includes(s) ? hand.indexOf(s) + 1 : undefined} onClick={() => onHand(toggleHand(hand, s))} />
              ))}
            </div>
            <p className="muted cp-note">{t("campaign.hand_hint")}</p>
          </>
        )
      ) : (
        <div className="cp-hand">
          {level.hand.map((s) => (
            <HandCard key={s} skill={s} />
          ))}
        </div>
      )}

      <h3 className="cp-h">{t("campaign.enemy_skills")}</h3>
      {level.enemy.length === 0 ? (
        <p className="muted cp-note">{t("campaign.enemy_none")}</p>
      ) : (
        <p className="cp-enemy">
          {level.enemy.map((s) => (
            <span key={s} className="chip cp-enemy-chip" style={{ "--fam": `var(--fam-${skillInfo(s).family})` } as CSSProperties}>
              {skillInfo(s).name}
            </span>
          ))}
          {level.boss && <span className="muted cp-note">{t("campaign.signature")}</span>}
        </p>
      )}

      <h3 className="cp-h">
        {t("campaign.stars")} <span className="muted cp-h-n">{done} / 3</span>
      </h3>
      <ul className="cp-star-list">
        {lines.map((l) => (
          <li key={l.kind} className={l.got ? "got" : ""}>
            <Star on={l.got} size={18} />
            <span>{l.text}</span>
            <span className="sr-only">{l.got ? t("campaign.earned") : t("campaign.not_earned")}</span>
          </li>
        ))}
      </ul>

      {level.boss && level.forge_pending && (
        <div className="cp-owed" role="status">
          <span>{t("campaign.forge_owed")}</span>
          <button type="button" className="btn pri sm" onClick={onForge}>
            {t("campaign.forge_claim")}
          </button>
        </div>
      )}

      <div className="cp-cta">
        {blocked ? (
          <p className="cp-locked" role="status">
            <Lock />
            {blocked}
          </p>
        ) : (
          <button type="button" className="btn pri block lb-main" disabled={!connected || state.soloPending || needHand} onClick={start}>
            {state.soloPending ? t("campaign.starting") : done > 0 ? t("campaign.replay") : t("campaign.start")}
          </button>
        )}
        {!blocked && needHand && <p className="muted cp-note cp-cta-note">{t("campaign.hand_need")}</p>}
      </div>
    </section>
  );
}

function HowItWorks() {
  const t = useT();
  return (
    <div className="cp-how">
      <ol>
        {[1, 2, 3, 4, 5, 6].map((n) => (
          <li key={n}>
            <strong>{t(`campaign.how_${n}_t`)}</strong>
            <span className="muted">{t(`campaign.how_${n}_d`)}</span>
          </li>
        ))}
      </ol>
      <h3 className="cp-h">{t("campaign.rules_title")}</h3>
      <ul className="cp-rules">
        {[1, 2, 3, 4, 5].map((n) => (
          <li key={n}>{t(`campaign.rule_${n}`)}</li>
        ))}
      </ul>
    </div>
  );
}

export function Campaign({ state, level: levelParam }: { state: AppState; level?: string }) {
  const t = useT();
  const { campaign: levels, connection, account, deck } = state;
  const isGuest = !account || account.guest;
  const [forgeOpen, setForgeOpen] = useState(false);
  const [howOpen, setHowOpen] = useState(false);
  const [picked, setPicked] = useState<number | null>(null);
  const [hand, setHand] = useState<SkillId[]>(() => readHand(deck));

  useEffect(() => {
    markCampaignSeen();
  }, []);
  useEffect(() => {
    if (connection === "open") store.loadCampaign();
  }, [connection]);
  // Le deck change (forge, vol) : la main retire ce qui n'y est plus.
  useEffect(() => {
    setHand((h) => (h.every((s) => deck.includes(s)) ? h : h.filter((s) => deck.includes(s))));
  }, [deck]);

  const wanted = useMemo(() => {
    const n = Number(levelParam);
    return Number.isInteger(n) ? n : null;
  }, [levelParam]);
  const current = useMemo(() => (levels ? (levelById(levels, picked ?? wanted ?? -1) ?? resumeLevel(levels)) : undefined), [levels, picked, wanted]);

  const select = (id: number) => {
    setPicked(id);
    if (levelParam) navigate({ name: "campaign" });
  };

  if (!levels || !current) {
    return (
      <main className="cp-page">
        <h1 className="cp-title">{t("campaign.title")}</h1>
        <p className="muted" role="status">
          {connection === "closed" ? t("app.connectionLost") : t("campaign.loading")}
        </p>
      </main>
    );
  }

  const chapter = current.chapter;
  const gate = gateStars(levels, chapter);
  const owed = levels.find((l) => l.boss && l.forge_pending);
  const total = totalStars(levels);

  return (
    <main className="cp-page">
      <div className="cp-grid">
        <aside className="cp-rail" aria-label={t("campaign.chapters_aria")}>
          <div className="cp-head">
            <h1 className="cp-title">{t("campaign.title")}</h1>
            <span className="chip cp-total" aria-label={t("campaign.total_aria", { n: total, max: TOTAL_STARS })}>
              <Star on size={14} />
              <span className="num">{total}</span> / {TOTAL_STARS}
            </span>
          </div>
          <ol className="cp-chapters">
            {Array.from({ length: CHAPTER_COUNT }, (_, i) => i + 1).map((c) => {
              const open = chapterOpen(levels, c);
              const family = familyOfChapter(c);
              return (
                <li key={c}>
                  <button
                    type="button"
                    className={`cp-chap${c === chapter ? " on" : ""}${open ? "" : " shut"}`}
                    style={chapterStyle(c)}
                    aria-current={c === chapter ? "true" : undefined}
                    onClick={() => select(levels.find((l) => l.chapter === c && l.index === 1)!.id)}
                  >
                    <span className="num cp-chap-n">{c}</span>
                    <span className="cp-chap-txt">
                      <strong>{chapterName(c)}</strong>
                      <span className="meta">
                        {FAMILY_LABEL[family]} · {chapterTotal(levels, c)} / {CHAPTER_STARS} ★
                      </span>
                    </span>
                    {!open && <Lock />}
                  </button>
                </li>
              );
            })}
          </ol>
          <button type="button" className="link cp-how-link" onClick={() => setHowOpen(true)}>
            {t("campaign.how_title")}
          </button>
          {isGuest && <p className="muted cp-note">{t("campaign.guest_note")}</p>}
        </aside>

        <section className="cp-stage card" style={chapterStyle(chapter)} aria-labelledby="cp-chapter-title">
          <header className="cp-stage-head">
            <div>
              <p className="cp-eyebrow muted">{t("campaign.chapter_of", { n: chapter, max: CHAPTER_COUNT })}</p>
              <h2 id="cp-chapter-title" className="cp-stage-title">
                {chapterName(chapter)}
              </h2>
            </div>
            <div className="cp-gate-meter">
              <span className="meta">{t("campaign.gate_progress", { have: Math.min(gate, BOSS_GATE), need: BOSS_GATE, boss: bossName(chapter) })}</span>
              <span className="cp-bar" role="presentation">
                <i style={{ width: `${(Math.min(gate, BOSS_GATE) / BOSS_GATE) * 100}%` }} />
              </span>
            </div>
          </header>
          {owed && !forgeOpen && (
            <button type="button" className="cp-owed-banner" onClick={() => setForgeOpen(true)}>
              <span className="hex" style={{ width: 18, height: 20, background: "var(--rar-legendary)" }} aria-hidden="true" />
              {t("campaign.forge_owed")}
            </button>
          )}
          <Map levels={levels} chapter={chapter} selected={current.id} onSelect={select} />
        </section>

        <Briefing levels={levels} level={current} state={state} hand={hand} onHand={setHand} onForge={() => setForgeOpen(true)} />
      </div>

      {forgeOpen && owed && <CampaignForge boss={owed} deck={deck} onClose={() => setForgeOpen(false)} />}
      <Sheet open={howOpen} title={t("campaign.how_title")} onClose={() => setHowOpen(false)}>
        <HowItWorks />
      </Sheet>
    </main>
  );
}
