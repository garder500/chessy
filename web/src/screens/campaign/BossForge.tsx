import { useEffect, useRef, useState, type ReactNode } from "react";
import { loadForged } from "../../forged";
import type { BossForgeInfo, SkillId } from "../../protocol";
import { skillInfo } from "../../skills";
import { store, useAppState } from "../../store";
import { ForgeReveal } from "../../ui/ForgeReveal";
import { SkillCard } from "../SkillCard";
import { bossForgeStep, claimMsg, FOLLOW_NOTE, legendaryNote, markRevealed, placeMsg, wasRevealed } from "./bossForge";
import "../reward.css";
import "./bossForge.css";

interface Props {
  chapter: number;
  /** État connu avant tout message `boss_forge` (liste REST, fin de partie). */
  initial?: BossForgeInfo | null;
  onClose: () => void;
}

function Shell({ onClose, children, foot }: { onClose: () => void; children: ReactNode; foot: ReactNode }) {
  const dialog = useRef<HTMLDivElement>(null);
  useEffect(() => {
    dialog.current?.focus();
  }, []);
  return (
    <div className="rw bf" role="dialog" aria-modal="true" aria-labelledby="bf-title" tabIndex={-1} ref={dialog}>
      <header className="rw-top">
        <button type="button" className="rw-back" aria-label="Fermer" onClick={onClose}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M15 5l-7 7 7 7" />
          </svg>
        </button>
        <h2 id="bf-title" className="rw-title">
          Forge du boss
        </h2>
      </header>
      <div className="rw-body bf-body">{children}</div>
      <footer className="rw-foot">{foot}</footer>
    </div>
  );
}

function Notes({ info }: { info: BossForgeInfo }) {
  return (
    <>
      <p className="muted bf-note">{FOLLOW_NOTE}</p>
      {legendaryNote(info) && <p className="muted bf-note">{legendaryNote(info)}</p>}
    </>
  );
}

function ReplaceChoice({ info, deck, onClose }: { info: BossForgeInfo; deck: SkillId[]; onClose: () => void }) {
  const [replace, setReplace] = useState<SkillId | null>(null);
  const name = info.skill ? skillInfo(info.skill).name : "";
  return (
    <Shell
      onClose={onClose}
      foot={
        <>
          <button type="button" className="btn pri block" disabled={replace === null} onClick={() => store.send(placeMsg(info.chapter, replace))}>
            {replace === null ? "Choisissez la compétence à remplacer" : `Remplacer ${skillInfo(replace).name}`}
          </button>
          <button type="button" className="link" onClick={onClose}>
            Plus tard
          </button>
        </>
      }
    >
      <p className="muted rw-sub">
        Votre deck est plein (<span className="mono">{deck.length}/7</span>) : choisissez la compétence que {name} remplacera. « Plus tard » la garde en attente.
      </p>
      <Notes info={info} />
      <div className="rw-opts" role="radiogroup" aria-label="Compétence à remplacer">
        {deck.map((skill) => (
          <SkillCard key={skill} skill={skill} radio selected={replace === skill} onClick={() => setReplace(skill)} />
        ))}
      </div>
    </Shell>
  );
}

function Waiting({ chapter, onClose }: { chapter: number; onClose: () => void }) {
  return (
    <Shell
      onClose={onClose}
      foot={
        <button type="button" className="btn pri block" onClick={() => store.send(claimMsg(chapter))}>
          Récupérer la forge
        </button>
      }
    >
      <p className="rw-sub" role="status">
        La forge travaille…
      </p>
    </Shell>
  );
}

/** Forge de la compétence offerte par un boss : attente, révélation, puis placement dans le deck (ou « Plus tard »). */
export function BossForge({ chapter, initial = null, onClose }: Props) {
  const { bossForge, deck } = useAppState();
  const info = bossForge?.chapter === chapter ? bossForge : initial;
  const [seenNow, setSeenNow] = useState(false);
  const [loadedSkill, setLoadedSkill] = useState<SkillId | null>(null);
  const placing = useRef(false);
  const step = bossForgeStep(info, seenNow || wasRevealed(info));
  const skill = info?.skill ?? null;

  useEffect(() => {
    if (skill) void loadForged([skill]).then(() => setLoadedSkill(skill));
  }, [skill]);
  useEffect(() => {
    if (step === "none") onClose();
  }, [step, onClose]);
  useEffect(() => {
    if (step !== "place" || placing.current) return;
    placing.current = true;
    store.send(placeMsg(chapter, null));
  }, [step, chapter]);

  if (!info || step === "none") return null;
  if (step === "wait") return <Waiting chapter={chapter} onClose={onClose} />;
  if (loadedSkill !== skill || !skill) return <Shell onClose={onClose} foot={null}><p className="rw-sub" role="status">Chargement…</p></Shell>;
  if (step === "reveal") {
    // Les boutons de ForgeReveal (fermer, ajouter, Échap) closent la révélation : on repart de là.
    const revealed = () => {
      markRevealed(info);
      setSeenNow(true);
    };
    return (
      <div className="bf-reveal" onClick={(e) => (e.target as Element).closest(".fr-fin") && revealed()} onKeyDown={(e) => e.key === "Escape" && revealed()}>
        <ForgeReveal skill={skill} />
      </div>
    );
  }
  if (step === "choose") return <ReplaceChoice info={info} deck={deck} onClose={onClose} />;
  return (
    <Shell onClose={onClose} foot={null}>
      <p className="rw-sub" role="status">
        Placement de la compétence…
      </p>
    </Shell>
  );
}
