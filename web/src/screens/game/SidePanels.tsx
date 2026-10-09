import { useEffect, useRef, useState, type CSSProperties } from "react";
import { pieceName, type LogLine } from "../../game/logic";
import { drawPiece } from "../../game/textures";
import { t, useT } from "../../i18n";
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
  if (slot.used) return t("game.slot_used");
  const max = slot.max_uses ?? 1;
  const left = max - (slot.uses ?? 0);
  const count = max > 1 ? t("game.slot_uses", { left, max }) : t("game.slot_one_use");
  if (!myTurn) return max > 1 ? count : t("game.slot_opp_turn");
  if (targets === 0) return t("game.slot_no_target");
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
  const t = useT();
  if (pieces.length === 0) return null;
  return (
    <section className="gm-panel card gm-bench" aria-labelledby="gm-bench-h">
      <div className="gm-panel-head">
        <h2 id="gm-bench-h" className="gm-h">
          {t("game.bench_title")}
        </h2>
        <span className="mono muted">{pieces.length}</span>
      </div>
      <ul className="gm-bench-row">
        {pieces.map((p) => (
          <li key={p.id} title={t("game.bench_piece_title", { piece: pieceName(p.kind) })}>
            <PieceThumb piece={p} size={48} />
            <span className="muted">{pieceName(p.kind)}</span>
          </li>
        ))}
      </ul>
      <p className="muted gm-bench-note">{t("game.bench_note")}</p>
    </section>
  );
}

/** Les compétences du joueur : un clic lance le ciblage. */
export function SkillList({ slots, view, myTurn, active, onToggle }: SkillListProps) {
  const t = useT();
  const options = (skill: string) => view.skill_options.find((o) => o.skill === skill)?.targets.length ?? 0;
  return (
    <section className="gm-panel card" aria-labelledby="gm-skills-h">
      <div className="gm-panel-head">
        <h2 id="gm-skills-h" className="gm-h">
          {t("game.skills_title")}
        </h2>
        <span className="mono muted">{slots.filter((s) => !s.used).length}/{slots.length}</span>
      </div>
      {slots.length === 0 && <p className="muted gm-empty">{t("game.skills_none")}</p>}
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
                    {info.unique && <span className="tag">{t("game.tag_unique")}</span>}
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
  const t = useT();
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
      <PanelHead id="gm-journal-h" title={t("game.journal_title")} count={log.length} compact={compact} open={open} onToggle={() => setFold(!fold)} peek={last ? t("game.log_peek", { who: last.actor === you ? t("game.you_name") : t("game.who_opp"), text: last.text }) : undefined} />
      <ol className="gm-log" hidden={!open}>
        {log.length === 0 && <li className="muted gm-empty">{t("game.log_empty")}</li>}
        {log.map((line) => (
          <li key={line.key ?? line.ply} className={line.actor === you ? "me" : "opp"}>
            <span className="mono gm-log-n">{line.ply}</span>
            <span className="gm-log-who">{line.actor === you ? t("game.you_name") : t("game.who_opp")}</span>
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
  const t = useT();
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    setConfirm(false);
  }, [view.game_id, over]);
  if (over) return null;
  return (
    <section className="gm-panel card gm-actions" aria-label={t("game.actions_aria")}>
      {view.draw_offer === "them" && (
        <div className="gm-banner" role="alert">
          <strong>{t("game.draw_offered_title")}</strong>
          <span className="muted">{t("game.draw_offered_msg")}</span>
          <div className="gm-row">
            <button type="button" className="btn sm pri" onClick={() => store.respondDraw(true)}>
              {t("game.accept")}
            </button>
            <button type="button" className="btn sm" onClick={() => store.respondDraw(false)}>
              {t("game.decline")}
            </button>
          </div>
        </div>
      )}
      {confirm ? (
        <div className="gm-banner danger" role="alertdialog" aria-label={t("game.resign_confirm_aria")}>
          <strong>{t("game.resign_title")}</strong>
          <span className="muted">{t("game.resign_msg")}</span>
          <div className="gm-row">
            <button type="button" className="btn sm danger" onClick={() => store.send({ type: "resign" })} autoFocus>
              {t("game.resign_yes")}
            </button>
            <button type="button" className="btn sm" onClick={() => setConfirm(false)}>
              {t("game.resign_continue")}
            </button>
          </div>
        </div>
      ) : (
        <div className="gm-row">
          <button type="button" className="btn sm danger" onClick={() => setConfirm(true)}>
            {t("game.resign_btn")}
          </button>
          <button type="button" className="btn sm" disabled={view.draw_offer !== "none"} onClick={() => store.offerDraw()}>
            {view.draw_offer === "you" ? t("game.draw_offered_title") : t("game.offer_draw")}
          </button>
        </div>
      )}
    </section>
  );
}

/** Remplace le chat contre l'IA : il n'y a personne à qui écrire. */
export function TrainingNote() {
  const t = useT();
  return (
    <section className="gm-panel card gm-training" aria-labelledby="gm-training-h">
      <h2 id="gm-training-h" className="gm-h">
        {t("game.training_title")}
      </h2>
      <p className="muted gm-empty">{t("game.training_note")}</p>
    </section>
  );
}

const QUICK = ["game.quick_1", "game.quick_2", "game.quick_3", "game.quick_4", "game.quick_5"];

export function Chat({ lines, flat }: { lines: ChatLine[]; flat?: boolean }) {
  const t = useT();
  const [text, setText] = useState("");
  const list = useRef<HTMLDivElement>(null);
  const compact = useCompact() && !flat;
  const [fold, setFold] = useState(false);
  const open = !compact || fold;
  useEffect(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines.length, open]);

  const send = (msg: string) => {
    const clean = msg.trim();
    if (clean) store.send({ type: "chat", text: clean.slice(0, 140) });
  };

  // Bloquer ou signaler l'adversaire (un invité n'a pas de pseudo : rien à viser).
  const { game } = useAppState();
  const opponent = game?.opponent.username ?? null;
  const excerpt = lines
    .slice(-10)
    .map((l) => t("game.log_peek", { who: l.mine ? t("game.you_name") : t("game.who_opp"), text: l.text }))
    .join("\n");

  return (
    <section className={`gm-panel${flat ? "" : " card"} gm-chat${open ? "" : " folded"}`} aria-labelledby="gm-chat-h">
      {compact ? (
        <PanelHead id="gm-chat-h" title={t("game.chat_title")} count={lines.length} compact open={open} onToggle={() => setFold(!fold)} peek={lines.length ? t("game.log_peek", { who: lines[lines.length - 1].mine ? t("game.you_name") : t("game.who_opp"), text: lines[lines.length - 1].text }) : t("game.chat_hello")} />
      ) : (
        <h2 id="gm-chat-h" className="gm-h">
          {t("game.chat_title")}
        </h2>
      )}
      <div className="gm-chat-list" ref={list} role="log" aria-live="polite" hidden={!open}>
        {lines.length === 0 && <p className="muted gm-empty">{t("game.chat_empty")}</p>}
        {lines.map((l, i) => (
          <p key={i} className={`gm-msg ${l.mine ? "me" : "opp"}`}>
            <span className="sr-only">{l.mine ? t("game.chat_you_sr") : t("game.chat_opp_sr")}</span>
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
        {QUICK.map((key) => (
          <button key={key} type="button" className="gm-chip" onClick={() => send(t(key))}>
            {t(key)}
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
          {t("game.chat_label")}
        </label>
        <input id="gm-chat-input" className="input" value={text} maxLength={140} placeholder={t("game.chat_placeholder")} onChange={(e) => setText(e.target.value)} autoComplete="off" />
        <button type="submit" className="btn sm" disabled={!text.trim()}>
          {t("game.send")}
        </button>
      </form>
    </section>
  );
}

/** Proposition de nulle de l'adversaire : visible sans ouvrir de menu. */
export function DrawBanner({ view }: { view: StateView }) {
  const t = useT();
  if (view.draw_offer !== "them") return null;
  return (
    <div className="gm-banner" role="alert">
      <strong>{t("game.draw_offered_title")}</strong>
      <span className="muted">{t("game.draw_offered_msg")}</span>
      <div className="gm-row">
        <button type="button" className="btn sm pri" onClick={() => store.respondDraw(true)}>
          {t("game.accept")}
        </button>
        <button type="button" className="btn sm" onClick={() => store.respondDraw(false)}>
          {t("game.decline")}
        </button>
      </div>
    </div>
  );
}

/** Contenu du panneau « Plus » (téléphone) : proposer la nulle, abandonner avec confirmation. */
export function Options({ view, onClose }: { view: StateView; onClose: () => void }) {
  const t = useT();
  const [confirm, setConfirm] = useState(false);
  if (confirm) {
    return (
      <div className="gm-opts" role="alertdialog" aria-label={t("game.resign_confirm_aria")}>
        <h3 className="sheet-title">{t("game.resign_title")}</h3>
        <p className="sheet-sub">
          {t(view.rated ? "game.resign_msg_rated" : "game.resign_msg")}
        </p>
        <button type="button" className="btn danger solid block" onClick={() => store.send({ type: "resign" })} autoFocus>
          {t("game.resign_do")}
        </button>
        <button type="button" className="btn block" onClick={() => setConfirm(false)}>
          {t("game.keep_playing")}
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
        {view.draw_offer === "you" ? t("game.draw_offered_title") : t("game.offer_draw_long")}
      </button>
      <button type="button" className="btn block danger" onClick={() => setConfirm(true)}>
        {t("game.resign_do")}
      </button>
      <button type="button" className="link" onClick={onClose}>
        {t("game.back_to_game")}
      </button>
    </div>
  );
}

/** Barre d'actions du bas (téléphone) : coups, messages, retourner le plateau, options. */
export function GameNav({ onMoves, onChat, onFlip, onMore, flipped, unread, over }: { onMoves: () => void; onChat: () => void; onFlip: () => void; onMore: () => void; flipped: boolean; unread: number; over: boolean }) {
  const t = useT();
  const ico = { viewBox: "0 0 24 24", width: 22, height: 22, fill: "none", stroke: "currentColor", strokeWidth: 1.8, "aria-hidden": true } as const;
  return (
    <nav className="gm-nav" aria-label={t("game.actions_aria")}>
      <button type="button" className="gm-nav-b" onClick={onMoves}>
        <svg {...ico}>
          <path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01" />
        </svg>
        {t("game.sheet_moves")}
      </button>
      <button type="button" className="gm-nav-b" onClick={onChat}>
        <svg {...ico}>
          <path d="M4 5h16v11H9l-5 4z" />
        </svg>
        {t("game.sheet_messages")}
        {unread > 0 && <span className="nav-count gm-nav-count">{unread}</span>}
      </button>
      <button type="button" className="gm-nav-b" aria-pressed={flipped} onClick={onFlip}>
        <svg {...ico} width={20} height={20}>
          <path d="M7 4v14l-3-3M17 20V6l3 3" />
        </svg>
        {t("game.nav_flip")}
      </button>
      {!over && (
        <button type="button" className="gm-nav-b" onClick={onMore}>
          <svg {...ico} fill="currentColor" stroke="none">
            <circle cx="5" cy="12" r="2" />
            <circle cx="12" cy="12" r="2" />
            <circle cx="19" cy="12" r="2" />
          </svg>
          {t("game.nav_more")}
        </button>
      )}
    </nav>
  );
}
