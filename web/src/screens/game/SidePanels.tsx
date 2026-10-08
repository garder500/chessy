import { useEffect, useRef, useState, type CSSProperties } from "react";
import { PIECE_FR, type LogLine } from "../../game/logic";
import { drawPiece } from "../../game/textures";
import type { Piece, SkillSlot, StateView } from "../../protocol";
import { skillInfo } from "../../skills";
import { store, useAppState, type ChatLine } from "../../store";
import { ModerationActions } from "../../ui/Moderation";
import { SkillArt } from "../../ui/SkillArt";
import { UniqueBadge } from "../../ui/UniqueBadge";
import { useCompact } from "../../ui/useCompact";
import { tileRarity } from "../../ui/tileRarity";

interface SkillListProps {
  slots: SkillSlot[];
  view: StateView;
  myTurn: boolean;
  active: string | null;
  onToggle: (skill: SkillSlot["skill"]) => void;
}

/** Libellé d'état d'une compétence : « 2/3 usages » pour Mind Reading, sinon utilisée / cible / tour. */
export function slotStatus(slot: SkillSlot, myTurn: boolean, targets: number): string {
  if (slot.used) return "Utilisée";
  const max = slot.max_uses ?? 1;
  const left = max - (slot.uses ?? 0);
  const count = max > 1 ? `${left}/${max} usages` : "1 usage";
  if (!myTurn) return max > 1 ? count : "Tour adverse";
  if (targets === 0) return "Aucune cible";
  return count;
}

/** Pièce dessinée avec sa texture de plateau (banc). */
function PieceThumb({ piece, size }: { piece: Piece; size: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) drawPiece(ref.current, piece.kind, piece.color);
  }, [piece.kind, piece.color]);
  return <canvas ref={ref} className="gm-thumb" style={{ width: size, height: size }} aria-hidden="true" />;
}

/** Tiroir des pièces mises de côté par The Bench ; elles reviennent à votre prochain tour. */
export function BenchPanel({ pieces }: { pieces: Piece[] }) {
  if (pieces.length === 0) return null;
  return (
    <section className="gm-panel card gm-bench" aria-labelledby="gm-bench-h">
      <div className="gm-panel-head">
        <h2 id="gm-bench-h" className="gm-h">
          Sur le banc
        </h2>
        <span className="mono muted">{pieces.length}</span>
      </div>
      <ul className="gm-bench-row">
        {pieces.map((p) => (
          <li key={p.id} title={`${PIECE_FR[p.kind]} (revient à votre prochain tour)`}>
            <PieceThumb piece={p} size={48} />
            <span className="muted">{PIECE_FR[p.kind]}</span>
          </li>
        ))}
      </ul>
      <p className="muted gm-bench-note">Elles reviennent sur la case libre la plus proche.</p>
    </section>
  );
}

/** Les compétences du joueur : un clic lance le ciblage. */
export function SkillList({ slots, view, myTurn, active, onToggle }: SkillListProps) {
  const options = (skill: string) => view.skill_options.find((o) => o.skill === skill)?.targets.length ?? 0;
  return (
    <section className="gm-panel card" aria-labelledby="gm-skills-h">
      <div className="gm-panel-head">
        <h2 id="gm-skills-h" className="gm-h">
          Vos compétences
        </h2>
        <span className="mono muted">{slots.filter((s) => !s.used).length}/{slots.length}</span>
      </div>
      {slots.length === 0 && <p className="muted gm-empty">Aucune compétence dans cette partie.</p>}
      <ul className="gm-skills">
        {slots.map((slot, i) => {
          const info = skillInfo(slot.skill);
          const n = options(slot.skill);
          const usable = myTurn && !slot.used && n > 0;
          const status = slotStatus(slot, myTurn, n);
          const on = active === slot.skill;
          return (
            <li key={slot.skill}>
              <button
                type="button"
                className={`gm-skill${on ? " on" : ""}${slot.used ? " used" : ""}${info.unique ? " foil" : ""}`}
                style={{ "--fam": `var(--fam-${info.family})` } as CSSProperties}
                disabled={!usable}
                aria-pressed={on}
                aria-keyshortcuts={String(i + 1)}
                title={info.description}
                onClick={() => onToggle(slot.skill)}
              >
                <span className="gm-skill-art" data-rar={tileRarity(slot.skill)}>
                  <SkillArt id={slot.skill} size={36} />
                  {info.unique && <UniqueBadge />}
                </span>
                <span className="gm-skill-txt">
                  <span className="gm-skill-name">
                    {info.name}
                    {info.unique && <span className="tag">unique</span>}
                  </span>
                  <span className="gm-skill-status muted">{status}</span>
                  <span className="gm-skill-desc">{info.description}</span>
                </span>
                <span className="gm-skill-key mono" aria-hidden="true">
                  {i + 1}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * En-tête d'un panneau : titre et compteur. En mise en page compacte (téléphone) c'est un bouton qui replie le panneau
 * (le plateau reste ainsi à portée de pouce) et, replié, il affiche un aperçu de la dernière ligne.
 */
function PanelHead({ id, title, count, compact, open, onToggle, peek }: { id: string; title: string; count: number; compact: boolean; open: boolean; onToggle: () => void; peek?: string }) {
  if (!compact) {
    return (
      <div className="gm-panel-head">
        <h2 id={id} className="gm-h">
          {title}
        </h2>
        <span className="mono muted">{count}</span>
      </div>
    );
  }
  return (
    <h2 id={id} className="gm-fold-h">
      <button type="button" className="gm-fold" aria-expanded={open} onClick={onToggle}>
        <span className="gm-h">{title}</span>
        <span className="mono muted">{count}</span>
        <span className="gm-fold-chev" aria-hidden="true">
          {open ? "▴" : "▾"}
        </span>
        {!open && peek && <span className="gm-fold-peek muted">{peek}</span>}
      </button>
    </h2>
  );
}

export function Journal({ log, you, flat }: { log: LogLine[]; you: StateView["you"]; flat?: boolean }) {
  const end = useRef<HTMLLIElement>(null);
  const compact = useCompact() && !flat;
  const [fold, setFold] = useState(false);
  const open = !compact || fold;
  useEffect(() => {
    if (open) end.current?.scrollIntoView({ block: "nearest" });
  }, [log.length, open]);
  const last = log[log.length - 1];
  return (
    <section className={`gm-panel${flat ? "" : " card"} gm-journal${open ? "" : " folded"}`} aria-labelledby="gm-journal-h">
      <PanelHead id="gm-journal-h" title="Journal" count={log.length} compact={compact} open={open} onToggle={() => setFold(!fold)} peek={last ? `${last.actor === you ? "Vous" : "Adv."} : ${last.text}` : undefined} />
      <ol className="gm-log" hidden={!open}>
        {log.length === 0 && <li className="muted gm-empty">Aucune action pour l'instant.</li>}
        {log.map((line) => (
          <li key={line.key ?? line.ply} className={line.actor === you ? "me" : "opp"}>
            <span className="mono gm-log-n">{line.ply}</span>
            <span className="gm-log-who">{line.actor === you ? "Vous" : "Adv."}</span>
            <span className="gm-log-text">
              {line.skill && (
                <span className="gm-log-ico">
                  <SkillArt id={line.skill} size={16} />
                </span>
              )}
              {line.text}
            </span>
          </li>
        ))}
        <li ref={end} aria-hidden="true" />
      </ol>
    </section>
  );
}

export function Actions({ view, over }: { view: StateView; over: boolean }) {
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    setConfirm(false);
  }, [view.game_id, over]);
  if (over) return null;
  return (
    <section className="gm-panel card gm-actions" aria-label="Actions de partie">
      {view.draw_offer === "them" && (
        <div className="gm-banner" role="alert">
          <strong>Nulle proposée</strong>
          <span className="muted">L'adversaire vous propose la nulle.</span>
          <div className="gm-row">
            <button type="button" className="btn sm pri" onClick={() => store.respondDraw(true)}>
              Accepter
            </button>
            <button type="button" className="btn sm" onClick={() => store.respondDraw(false)}>
              Refuser
            </button>
          </div>
        </div>
      )}
      {confirm ? (
        <div className="gm-banner danger" role="alertdialog" aria-label="Confirmer l'abandon">
          <strong>Abandonner la partie ?</strong>
          <span className="muted">Votre adversaire sera déclaré vainqueur.</span>
          <div className="gm-row">
            <button type="button" className="btn sm danger" onClick={() => store.send({ type: "resign" })} autoFocus>
              Oui, abandonner
            </button>
            <button type="button" className="btn sm" onClick={() => setConfirm(false)}>
              Continuer
            </button>
          </div>
        </div>
      ) : (
        <div className="gm-row">
          <button type="button" className="btn sm danger" onClick={() => setConfirm(true)}>
            Résigner
          </button>
          <button type="button" className="btn sm" disabled={view.draw_offer !== "none"} onClick={() => store.offerDraw()}>
            {view.draw_offer === "you" ? "Nulle proposée" : "Proposer nulle"}
          </button>
        </div>
      )}
    </section>
  );
}

/** Remplace le chat contre l'IA : il n'y a personne à qui écrire. */
export function TrainingNote() {
  return (
    <section className="gm-panel card gm-training" aria-labelledby="gm-training-h">
      <h2 id="gm-training-h" className="gm-h">
        Entraînement contre l'IA
      </h2>
      <p className="muted gm-empty">Sans horloge, sans Elo en jeu et sans récompense : prenez le temps d'essayer vos compétences.</p>
    </section>
  );
}

const QUICK = ["Bien joué !", "Merci", "Bonne chance", "Oups…", "Belle compétence"];

export function Chat({ lines, flat }: { lines: ChatLine[]; flat?: boolean }) {
  const [text, setText] = useState("");
  const list = useRef<HTMLDivElement>(null);
  const compact = useCompact() && !flat;
  const [fold, setFold] = useState(false);
  const open = !compact || fold;
  useEffect(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines.length, open]);

  const send = (t: string) => {
    const clean = t.trim();
    if (clean) store.send({ type: "chat", text: clean.slice(0, 140) });
  };

  // Bloquer ou signaler l'adversaire (un invité n'a pas de pseudo : rien à viser).
  const { game } = useAppState();
  const opponent = game?.opponent.username ?? null;
  const excerpt = lines
    .slice(-10)
    .map((l) => `${l.mine ? "Vous" : "Adv."} : ${l.text}`)
    .join("\n");

  return (
    <section className={`gm-panel${flat ? "" : " card"} gm-chat${open ? "" : " folded"}`} aria-labelledby="gm-chat-h">
      {compact ? (
        <PanelHead id="gm-chat-h" title="Chat" count={lines.length} compact open={open} onToggle={() => setFold(!fold)} peek={lines.length ? `${lines[lines.length - 1].mine ? "Vous" : "Adv."} : ${lines[lines.length - 1].text}` : "Dites bonjour"} />
      ) : (
        <h2 id="gm-chat-h" className="gm-h">
          Chat
        </h2>
      )}
      <div className="gm-chat-list" ref={list} role="log" aria-live="polite" hidden={!open}>
        {lines.length === 0 && <p className="muted gm-empty">Dites bonjour avec une phrase rapide.</p>}
        {lines.map((l, i) => (
          <p key={i} className={`gm-msg ${l.mine ? "me" : "opp"}`}>
            <span className="sr-only">{l.mine ? "Vous : " : "Adversaire : "}</span>
            {l.text}
          </p>
        ))}
      </div>
      {opponent && open && (
        <div className="mod-chat-bar">
          <ModerationActions username={opponent} gameId={game?.game_id} excerpt={lines.some((l) => !l.mine) ? excerpt : undefined} />
        </div>
      )}
      <div className="gm-quick" hidden={!open}>
        {QUICK.map((q) => (
          <button key={q} type="button" className="gm-chip" onClick={() => send(q)}>
            {q}
          </button>
        ))}
      </div>
      <form
        className="gm-chat-form"
        hidden={!open}
        onSubmit={(e) => {
          e.preventDefault();
          send(text);
          setText("");
        }}
      >
        <label className="sr-only" htmlFor="gm-chat-input">
          Message
        </label>
        <input id="gm-chat-input" className="input" value={text} maxLength={140} placeholder="Votre message" onChange={(e) => setText(e.target.value)} autoComplete="off" />
        <button type="submit" className="btn sm" disabled={!text.trim()}>
          Envoyer
        </button>
      </form>
    </section>
  );
}

/** Proposition de nulle de l'adversaire : visible sans ouvrir de menu. */
export function DrawBanner({ view }: { view: StateView }) {
  if (view.draw_offer !== "them") return null;
  return (
    <div className="gm-banner" role="alert">
      <strong>Nulle proposée</strong>
      <span className="muted">L'adversaire vous propose la nulle.</span>
      <div className="gm-row">
        <button type="button" className="btn sm pri" onClick={() => store.respondDraw(true)}>
          Accepter
        </button>
        <button type="button" className="btn sm" onClick={() => store.respondDraw(false)}>
          Refuser
        </button>
      </div>
    </div>
  );
}

/** Contenu du panneau « Plus » (téléphone) : proposer la nulle, abandonner avec confirmation. */
export function Options({ view, onClose }: { view: StateView; onClose: () => void }) {
  const [confirm, setConfirm] = useState(false);
  if (confirm) {
    return (
      <div className="gm-opts" role="alertdialog" aria-label="Confirmer l'abandon">
        <h3 className="sheet-title">Abandonner la partie ?</h3>
        <p className="sheet-sub">
          {view.rated ? "Vous perdez de l'Elo et votre adversaire peut vous prendre une compétence." : "Votre adversaire sera déclaré vainqueur."}
        </p>
        <button type="button" className="btn danger solid block" onClick={() => store.send({ type: "resign" })} autoFocus>
          Abandonner
        </button>
        <button type="button" className="btn block" onClick={() => setConfirm(false)}>
          Continuer à jouer
        </button>
      </div>
    );
  }
  return (
    <div className="gm-opts">
      <button
        type="button"
        className="btn block"
        disabled={view.draw_offer !== "none"}
        onClick={() => {
          store.offerDraw();
          onClose();
        }}
      >
        {view.draw_offer === "you" ? "Nulle proposée" : "Proposer la nulle"}
      </button>
      <button type="button" className="btn block danger" onClick={() => setConfirm(true)}>
        Abandonner
      </button>
      <button type="button" className="link" onClick={onClose}>
        Reprendre la partie
      </button>
    </div>
  );
}

/** Barre d'actions du bas (téléphone) : coups, messages, retourner le plateau, options. */
export function GameNav({ onMoves, onChat, onFlip, onMore, flipped, unread, over }: { onMoves: () => void; onChat: () => void; onFlip: () => void; onMore: () => void; flipped: boolean; unread: number; over: boolean }) {
  const ico = { viewBox: "0 0 24 24", width: 22, height: 22, fill: "none", stroke: "currentColor", strokeWidth: 1.8, "aria-hidden": true } as const;
  return (
    <nav className="gm-nav" aria-label="Actions de partie">
      <button type="button" className="gm-nav-b" onClick={onMoves}>
        <svg {...ico}>
          <path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01" />
        </svg>
        Coups
      </button>
      <button type="button" className="gm-nav-b" onClick={onChat}>
        <svg {...ico}>
          <path d="M4 5h16v11H9l-5 4z" />
        </svg>
        Messages
        {unread > 0 && <span className="nav-count gm-nav-count">{unread}</span>}
      </button>
      <button type="button" className="gm-nav-b" aria-pressed={flipped} onClick={onFlip}>
        <svg {...ico} width={20} height={20}>
          <path d="M7 4v14l-3-3M17 20V6l3 3" />
        </svg>
        Retourner
      </button>
      {!over && (
        <button type="button" className="gm-nav-b" onClick={onMore}>
          <svg {...ico} fill="currentColor" stroke="none">
            <circle cx="5" cy="12" r="2" />
            <circle cx="12" cy="12" r="2" />
            <circle cx="19" cy="12" r="2" />
          </svg>
          Plus
        </button>
      )}
    </nav>
  );
}
