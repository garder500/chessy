import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { api, ApiError, gameErrorText } from "../api";
import { useT } from "../i18n";
import type { Action, Color, GameRecord } from "../protocol";
import { arrowSquares } from "../replay/arrow";
import {
  beginExplore,
  currentResponse,
  exploreErrorText,
  exploreRequest,
  extendExplore,
  undoExplore,
  type ExploreState,
} from "../replay/explore";
import { evalSeries } from "../replay/evalCurve";
import { colorOf, COLOR_FR, defaultOrientation, endSound, exploreViews, frameToView, isReplayable, kindLabel, lastIndex, opposite, seatName } from "../replay/frames";
import { analysisByPly } from "../replay/labels";
import { resultLine } from "../replay/lists";
import { initialNav, keyToNav, navReduce, shouldHandleKey, stepDelay } from "../replay/nav";
import { sfx } from "../sound";
import { readToken, useAppState } from "../store";
import { timeText } from "../time";
import { relativeTime } from "../ui/social";
import { EvalBar, Plate } from "./game/Plate";
import { PromotionPicker, SpawnPicker } from "./game/Overlays";
import { SkillList } from "./game/SidePanels";
import { AnalysisPanel, type AnalysisState } from "./replay/AnalysisPanel";
import { Sheet } from "../ui/Sheet";
import { useCompact } from "../ui/useCompact";
import { Accuracy, BottomControls, EvalStrip, MoveCard } from "./replay/Compact";
import { BoardStage } from "./replay/BoardStage";
import { Controls } from "./replay/Controls";
import { EvalChart } from "./replay/EvalChart";
import { ExplorePanel, MovePanel } from "./replay/InfoPanels";
import { MoveList } from "./replay/MoveList";
import { useBoardInteraction } from "./replay/useBoardInteraction";
import "./game.css";
import "./replay.css";

type Load = { status: "loading" } | { status: "ready"; record: GameRecord } | { status: "error"; message: string; retry: boolean };

/** Page `#/replay/<id>` : charge la partie puis affiche le lecteur. */
export function Replay({ gameId, autoAnalyse = false }: { gameId: string; autoAnalyse?: boolean }) {
  const t = useT();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const ctl = new AbortController();
    setLoad({ status: "loading" });
    api
      .game(gameId, readToken(), ctl.signal)
      .then((record) => {
        if (ctl.signal.aborted) return;
        if (!isReplayable(record)) setLoad({ status: "error", message: t("replay.unavailable"), retry: false });
        else setLoad({ status: "ready", record });
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        // Introuvable, accès refusé ou pas de replay : réessayer n'y changera rien.
        const final = e instanceof ApiError && (e.status === 404 || e.status === 403 || e.code === "no_replay");
        setLoad({ status: "error", message: gameErrorText(e), retry: !final });
      });
    return () => ctl.abort();
  }, [gameId, attempt]);

  if (load.status === "loading") {
    return (
      <main className="rp">
        <div className="rp-state card" role="status" aria-live="polite">
          <span className="rp-spinner" aria-hidden="true" />
          <p>{t("replay.loading")}</p>
        </div>
      </main>
    );
  }
  if (load.status === "error") {
    return (
      <main className="rp">
        <div className="rp-state card" role="alert">
          <h1 className="rp-state-title">{load.message}</h1>
          <p className="muted">{t("replay.error_hint")}</p>
          <div className="gm-row rp-state-actions">
            {load.retry && (
              <button type="button" className="btn pri" onClick={() => setAttempt((n) => n + 1)}>
                {t("replay.retry")}
              </button>
            )}
            <a className={`btn${load.retry ? "" : " pri"}`} href="#/games">
              {t("replay.my_games")}
            </a>
          </div>
        </div>
      </main>
    );
  }
  return <ReplayPlayer key={load.record.game_id} record={load.record} autoAnalyse={autoAnalyse} />;
}

interface ExploreRun {
  state: ExploreState;
  pending: boolean;
  error: string | null;
}

function EngineBar({ cp, orientation }: { cp: number; orientation: Color }) {
  const t = useT();
  // Part des blancs : sigmoïde douce, du point de vue de l'orientation (les blancs en bas si on les regarde d'en bas).
  const share = 1 / (1 + Math.exp(-cp / 350));
  const whiteBottom = orientation === "white";
  return (
    <div className="gm-eval rp-engine" role="img" aria-label={t(cp >= 0 ? "replay.engine_white" : "replay.engine_black")}>
      <i style={whiteBottom ? { height: `${Math.round(share * 100)}%` } : { height: `${Math.round(share * 100)}%`, top: 0, bottom: "auto" }} />
    </div>
  );
}

function ReplayPlayer({ record, autoAnalyse }: { record: GameRecord; autoAnalyse: boolean }) {
  const t = useT();
  const { account } = useAppState();
  const username = account && !account.guest ? account.username : null;
  const mine = colorOf(record, username);
  const max = lastIndex(record);

  const [orientation, setOrientation] = useState<Color>(() => defaultOrientation(record, username));
  const [nav, dispatch] = useReducer(navReduce, max, (m) => initialNav(m));
  const [showBest, setShowBest] = useState(true);
  const compact = useCompact();
  const [more, setMore] = useState(false);

  // ---- analyse ----
  const [depth, setDepth] = useState(3);
  const [analysis, setAnalysis] = useState<AnalysisState>({ status: "idle" });
  const analysisCtl = useRef<AbortController | null>(null);

  const runAnalysis = useCallback(
    (d: number) => {
      analysisCtl.current?.abort();
      const ctl = new AbortController();
      analysisCtl.current = ctl;
      setAnalysis({ status: "loading", depth: d });
      api
        .analysis(record.game_id, d, readToken(), ctl.signal)
        .then((data) => !ctl.signal.aborted && setAnalysis({ status: "ready", depth: d, data }))
        .catch((e: unknown) => !ctl.signal.aborted && setAnalysis({ status: "error", depth: d, error: gameErrorText(e) }));
    },
    [record.game_id],
  );
  const cancelAnalysis = useCallback(() => {
    analysisCtl.current?.abort();
    setAnalysis({ status: "idle" });
  }, []);

  useEffect(() => {
    if (autoAnalyse) runAnalysis(3);
    return () => analysisCtl.current?.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const analysisData = analysis.status === "ready" ? analysis.data : null;
  const byPly = useMemo(() => analysisByPly(analysisData), [analysisData]);
  const series = useMemo(() => (analysisData ? evalSeries(analysisData, record.plies) : null), [analysisData, record.plies]);

  // ---- exploration ----
  const [explore, setExplore] = useState<ExploreRun | null>(null);
  const [exploreStarting, setExploreStarting] = useState(false);
  const [exploreNote, setExploreNote] = useState<string | null>(null);
  const exploreCtl = useRef<AbortController | null>(null);
  const exploring = explore !== null;

  const exitExplore = useCallback(() => {
    exploreCtl.current?.abort();
    setExplore(null);
    setExploreStarting(false);
    setExploreNote(null);
  }, []);

  const startExplore = useCallback(() => {
    exploreCtl.current?.abort();
    const ctl = new AbortController();
    exploreCtl.current = ctl;
    dispatch({ type: "pause" });
    setExploreStarting(true);
    setExploreNote(null);
    const ply = nav.index;
    api
      .explore(record.game_id, exploreRequest(ply, []), readToken(), ctl.signal)
      .then((res) => {
        if (ctl.signal.aborted) return;
        const state = beginExplore(ply, res);
        setExploreStarting(false);
        if (state) setExplore({ state, pending: false, error: null });
        else setExploreNote(exploreErrorText(res.error ?? "bad_response"));
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        setExploreStarting(false);
        setExploreNote(gameErrorText(e));
      });
  }, [nav.index, record.game_id]);

  useEffect(() => () => exploreCtl.current?.abort(), []);

  const onExploreAction = useCallback(
    (action: Action) => {
      if (!explore || explore.pending) return;
      exploreCtl.current?.abort();
      const ctl = new AbortController();
      exploreCtl.current = ctl;
      const { state } = explore;
      setExplore({ ...explore, pending: true, error: null });
      api
        .explore(record.game_id, exploreRequest(state.ply, state.line, action), readToken(), ctl.signal)
        .then((res) => {
          if (ctl.signal.aborted) return;
          const out = extendExplore(state, action, res);
          setExplore({ state: out.state, pending: false, error: out.error ? exploreErrorText(out.error) : null });
          const frame = currentResponse(out.state).frame;
          if (!out.error && frame) sfx.playEvents(frame.events, { me: orientation, actor: opposite(frame.to_move) });
        })
        .catch((e: unknown) => {
          if (ctl.signal.aborted) return;
          setExplore({ state, pending: false, error: gameErrorText(e) });
        });
    },
    [explore, record.game_id, orientation],
  );

  const undoExploreMove = useCallback(() => {
    setExplore((cur) => (cur && !cur.pending ? { state: undoExplore(cur.state), pending: false, error: null } : cur));
  }, []);

  // ---- position affichée ----
  const response = explore ? currentResponse(explore.state) : null;
  const frame = response?.frame ?? record.frames[nav.index];
  const views = useMemo(() => {
    if (response?.frame) return exploreViews(record, response.frame, orientation, response.moves, response.skill_options);
    return { display: frameToView(record, record.frames[nav.index], orientation), play: null };
  }, [record, response, nav.index, orientation]);

  const interact = useBoardInteraction(
    views.play,
    onExploreAction,
    exploring && !explore.pending,
    `${nav.index}|${explore?.state.line.length ?? -1}`,
  );

  // ---- lecture automatique, sons, clavier ----
  useEffect(() => {
    if (!nav.playing) return;
    const timer = setTimeout(() => dispatch({ type: "tick" }), stepDelay(nav.speed));
    return () => clearTimeout(timer);
  }, [nav.playing, nav.index, nav.speed]);

  const prevIndex = useRef(nav.index);
  useEffect(() => {
    const prev = prevIndex.current;
    prevIndex.current = nav.index;
    // Un son par pas en avant ; les sauts et les retours en arrière restent silencieux.
    if (nav.index !== prev + 1) return;
    sfx.playEvents(record.frames[nav.index].events, { me: orientation, actor: record.moves[nav.index - 1]?.color });
    if (nav.index === max) {
      const end = endSound(record.frames[max].outcome, mine ?? orientation);
      if (end) sfx.play(end);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nav.index]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!shouldHandleKey({ key: e.key, ctrlKey: e.ctrlKey, metaKey: e.metaKey, altKey: e.altKey, target: e.target as HTMLElement | null })) return;
      if (explore || exploreStarting) {
        if (e.key === "Escape") exitExplore();
        return;
      }
      if (e.key === "f" || e.key === "F") {
        setOrientation((o) => opposite(o));
        return;
      }
      const action = keyToNav(e.key);
      if (!action) return;
      e.preventDefault();
      dispatch(action);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [explore, exploreStarting, exitExplore]);

  // Toute navigation dans la partie principale referme l'exploration.
  const seek = useCallback(
    (index: number) => {
      if (exploring || exploreStarting) exitExplore();
      dispatch({ type: "goto", index });
    },
    [exploring, exploreStarting, exitExplore],
  );

  // ---- dérivés d'affichage ----
  const move = !exploring && nav.index > 0 ? record.moves[nav.index - 1] : null;
  const moveAnalysis = move ? byPly.get(nav.index) : undefined;
  const bestHere = exploring ? (response?.best ?? null) : analysisData && nav.index < max ? (byPly.get(nav.index + 1)?.best ?? null) : null;
  const arrow = showBest && bestHere ? arrowSquares(bestHere.action) : null;
  const evalCp = exploring ? (response?.eval_cp ?? null) : series ? series[nav.index] : null;
  const subject = move && mine === move.color ? "you" : (move?.color ?? "white");
  const over = frame.outcome.type !== "ongoing";
  const atEnd = !exploring && nav.index === max;

  const plateProps = (color: Color) => {
    const used = frame.used[color];
    return {
      name: seatName(record[color]),
      elo: record[color].elo,
      color,
      board: frame.board,
      ownBench: frame.benched.filter((b) => b.owner === color).map((b) => b.piece),
      rivalBench: frame.benched.filter((b) => b.owner !== color).map((b) => b.piece),
      clock: { white_ms: 0, black_ms: 0, running: null },
      clockEnabled: false,
      stamp: 0,
      active: frame.to_move === color && !over,
      used,
      remaining: Math.max(0, record.loadouts[color].length - used.length),
      bot: record[color].bot,
      you: mine === color,
      clockLabel: "",
    };
  };

  let hint: string;
  let tone = "";
  if (exploreStarting) hint = t("replay.explore_loading");
  else if (exploring) {
    hint = explore.pending
      ? t("replay.thinking")
      : interact.activeSkill
        ? t("replay.pick_target")
        : t("replay.explore_to_move", { color: COLOR_FR[frame.to_move] });
    tone = "skill";
  } else if (atEnd && over) hint = resultLine(frame.outcome, record.result.reason);
  else if (frame.in_check) {
    hint = t(frame.to_move === "white" ? "replay.check_white" : "replay.check_black");
    tone = "check";
  } else hint = nav.index === 0 ? t("replay.hint_initial") : t("replay.hint_after", { index: nav.index, max, color: COLOR_FR[frame.to_move] });

  const top = opposite(orientation);
  const title = t("replay.vs_title", { white: seatName(record.white), black: seatName(record.black) });
  const skillsDisabled = !exploring || explore.pending;

  if (compact) {
    const vs = mine ? seatName(record[opposite(mine)]) : title;
    return (
      <main className="rp rp-compact">
        <header className="rc-top">
          <a className="rc-ic" href="#/games" aria-label={t("replay.back_aria")}>
            <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
              <path d="M15 5l-7 7 7 7" />
            </svg>
          </a>
          <h1 className="rc-title">{mine ? t("replay.vs_opponent", { name: vs }) : vs}</h1>
          <button type="button" className="rc-ic" aria-label={t("replay.flip")} aria-pressed={orientation === "black"} onClick={() => setOrientation((o) => opposite(o))}>
            <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
              <path d="M7 4v14l-3-3M17 20V6l3 3" />
            </svg>
          </button>
          <button type="button" className="rc-ic" aria-label={t("replay.more_options")} onClick={() => setMore(true)}>
            <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true">
              <circle cx="5" cy="12" r="2" />
              <circle cx="12" cy="12" r="2" />
              <circle cx="19" cy="12" r="2" />
            </svg>
          </button>
        </header>
        <EvalStrip cp={evalCp} />
        <BoardStage view={views.display} orientation={orientation} interactive={exploring && !explore.pending} highlights={interact.highlights} onSquare={interact.onSquare} arrow={arrow}>
          {interact.promotion && <PromotionPicker options={interact.promotion.options} onCancel={interact.cancelPromotion} onPick={interact.pickPromotion} />}
          {interact.spawn && <SpawnPicker skill={interact.spawn.skill} options={interact.spawn.options} onPick={interact.pickSpawn} onCancel={interact.cancelSpawn} />}
        </BoardStage>
        <div className="rc-body">
          {exploreNote && (
            <p className="rp-inline-error" role="alert">
              {exploreNote}
            </p>
          )}
          {exploring ? (
            <>
              <ExplorePanel state={explore.state} pending={explore.pending} error={explore.error} onUndo={undoExploreMove} onExit={exitExplore} />
              {views.play && <SkillList slots={views.play.my_skills} view={views.play} myTurn={!skillsDisabled} active={interact.activeSkill} onToggle={interact.toggleSkill} />}
            </>
          ) : (
            <>
              <MoveCard move={move} analysis={moveAnalysis} index={nav.index} max={max} analysing={analysis.status === "loading"} onAnalyse={() => runAnalysis(depth)} />
              {over && atEnd && <p className="muted rc-note">{hint}</p>}
              <Accuracy data={analysisData} state={analysis} onAnalyse={() => runAnalysis(depth)} names={{ white: seatName(record.white), black: seatName(record.black) }} />
            </>
          )}
        </div>
        <BottomControls nav={nav} disabled={exploring || exploreStarting} dispatch={dispatch} />
        <Sheet open={more} title={t("replay.sheet_title")} onClose={() => setMore(false)}>
          <div className="rc-more">
            {exploring ? (
              <button type="button" className="btn block" onClick={() => { exitExplore(); setMore(false); }}>
                {t("replay.back_to_game")}
              </button>
            ) : (
              <button type="button" className="btn block" disabled={exploreStarting} onClick={() => { startExplore(); setMore(false); }}>
                {exploreStarting ? t("replay.loading_short") : t("replay.explore_here")}
              </button>
            )}
            {bestHere && !exploring && (
              <label className="rp-check">
                <input type="checkbox" checked={showBest} onChange={(e) => setShowBest(e.target.checked)} />
                <span>
                  {t("replay.arrow_label")} <strong className="mono">{bestHere.notation}</strong>
                </span>
              </label>
            )}
            <AnalysisPanel record={record} state={analysis} depth={depth} onDepth={setDepth} onAnalyse={() => runAnalysis(depth)} onCancel={cancelAnalysis} />
            <MoveList
              moves={record.moves}
              index={exploring ? explore.state.ply : nav.index}
              analysis={analysisData}
              onSelect={(i) => {
                seek(i);
                setMore(false);
              }}
            />
            <a className="link" href="#/games">
              {t("replay.my_games")}
            </a>
          </div>
        </Sheet>
      </main>
    );
  }

  return (
    <main className="rp">
      <header className="rp-head">
        <div className="rp-head-main">
          <p className="eyebrow">{t("replay.eyebrow")}</p>
          <h1 className="rp-title">{title}</h1>
          <p className="rp-meta">
            <span className="tag">{kindLabel(record.kind, record.rated)}</span>
            <span className="muted">{t("replay.ply", { count: record.plies })}</span>
            {record.time_control && <span className="muted">{timeText(record.time_control)}</span>}
            {record.at && <span className="muted">{relativeTime(record.at)}</span>}
            <span>{resultLine(record.result.outcome, record.result.reason)}</span>
          </p>
        </div>
        <div className="rp-head-actions">
          <button type="button" className="btn sm" onClick={() => setOrientation((o) => opposite(o))} aria-pressed={orientation === "black"} aria-keyshortcuts="F" title={t("replay.flip_title")}>
            {t("replay.flip")}
          </button>
          {exploring ? (
            <button type="button" className="btn sm" onClick={exitExplore}>
              {t("replay.back_to_game")}
            </button>
          ) : (
            <button type="button" className="btn sm" onClick={startExplore} disabled={exploreStarting}>
              {exploreStarting ? t("replay.loading_short") : t("replay.explore_here")}
            </button>
          )}
          <a className="btn sm ghost" href="#/games">
            {t("replay.my_games")}
          </a>
        </div>
      </header>

      <div className="rp-grid">
        <section className="rp-center" aria-label={t("replay.board_aria")}>
          <Plate {...plateProps(top)} />
          <div className={`gm-hint ${tone}`} role="status" aria-live="polite">
            <span>{hint}</span>
            {exploring && (
              <button type="button" className="btn sm ghost" onClick={exitExplore}>
                {t("replay.back_to_game")}
              </button>
            )}
          </div>
          {exploreNote && (
            <p className="rp-inline-error" role="alert">
              {exploreNote}
            </p>
          )}

          <div className="gm-boardrow">
            {evalCp !== null ? <EngineBar cp={evalCp} orientation={orientation} /> : <EvalBar view={views.display} />}
            <BoardStage view={views.display} orientation={orientation} interactive={exploring && !explore.pending} highlights={interact.highlights} onSquare={interact.onSquare} arrow={arrow}>
              {interact.promotion && <PromotionPicker options={interact.promotion.options} onCancel={interact.cancelPromotion} onPick={interact.pickPromotion} />}
              {interact.spawn && <SpawnPicker skill={interact.spawn.skill} options={interact.spawn.options} onPick={interact.pickSpawn} onCancel={interact.cancelSpawn} />}
            </BoardStage>
          </div>

          <Plate {...plateProps(orientation)} />
          <Controls nav={nav} disabled={exploring || exploreStarting} dispatch={dispatch} />
          {analysisData && <EvalChart analysis={analysisData} plies={record.plies} index={exploring ? explore.state.ply : nav.index} onSeek={seek} />}
        </section>

        <aside className="rp-side">
          {exploring ? (
            <>
              <ExplorePanel state={explore.state} pending={explore.pending} error={explore.error} onUndo={undoExploreMove} onExit={exitExplore} />
              {views.play && (
                <SkillList slots={views.play.my_skills} view={views.play} myTurn={!skillsDisabled} active={interact.activeSkill} onToggle={interact.toggleSkill} />
              )}
            </>
          ) : (
            <MovePanel
              move={move}
              analysis={moveAnalysis}
              subject={subject}
              evalCp={evalCp}
              bestHere={bestHere}
              showBest={showBest}
              onShowBest={setShowBest}
              onSeeAlternative={() => {
                setShowBest(true);
                seek(nav.index - 1);
              }}
            />
          )}
          <AnalysisPanel
            record={record}
            state={analysis}
            depth={depth}
            onDepth={setDepth}
            onAnalyse={() => runAnalysis(depth)}
            onCancel={cancelAnalysis}
          />
          <MoveList moves={record.moves} index={exploring ? explore.state.ply : nav.index} analysis={analysisData} onSelect={seek} />
          <p className="muted rp-keys">
            {t("replay.shortcuts")}
          </p>
        </aside>
      </div>
    </main>
  );
}
