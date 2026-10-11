// Serveur simulé pour le développement : `npm run dev` puis ouvrir `/?mock=1#/replay/g-demo-1`.
// Remplace `fetch` (REST v4) et `WebSocket` (accueil, amis, spectateur) par des réponses issues de
// `replay/fixtures.ts`. Chargé uniquement si `import.meta.env.DEV` et `?mock` dans l'URL : absent du bundle de production.
//
// Identifiants spéciaux : `g-missing` (404), `g-noreplay` (replay indisponible), `g-slow` (réponse lente).

import type { CampaignChapterView, CampaignLevelView, ForgeTableRow } from "../campaign";
import type { Action, BossForgeInfo, ClientMsg, ExploreRequest, ExploreResponse, Frame, GameRecord, Move, Piece, ServerMsg, SkillId, SkillOptions, Square } from "../protocol";
import {
  fixtureAnalysis,
  fixtureGames,
  fixtureLive,
  fixtureRecord,
  fixtureSpectatorView,
} from "../replay/fixtures";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ---- exploration : un petit générateur de coups pseudo-légaux (sans échecs) ----

const VALUE: Record<string, number> = { pawn: 100, knight: 300, bishop: 300, rook: 500, queen: 900, king: 0 };

function pseudoMoves(board: (Piece | null)[], color: "white" | "black"): Move[] {
  const out: Move[] = [];
  const at = (f: number, r: number) => (f < 0 || f > 7 || r < 0 || r > 7 ? undefined : r * 8 + f);
  const slide = (from: number, dirs: [number, number][]) => {
    const [f0, r0] = [from % 8, Math.floor(from / 8)];
    for (const [df, dr] of dirs) {
      for (let k = 1; k < 8; k++) {
        const to = at(f0 + df * k, r0 + dr * k);
        if (to === undefined) break;
        const t = board[to];
        if (t && t.color === color) break;
        out.push({ from, to });
        if (t) break;
      }
    }
  };
  board.forEach((p, from) => {
    if (!p || p.color !== color) return;
    const [f, r] = [from % 8, Math.floor(from / 8)];
    const dir = color === "white" ? 1 : -1;
    if (p.kind === "pawn") {
      const one = at(f, r + dir);
      if (one !== undefined && !board[one]) {
        const last = r + dir === 0 || r + dir === 7;
        out.push(last ? { from, to: one, promo: "queen" } : { from, to: one });
        const two = at(f, r + 2 * dir);
        if (!last && two !== undefined && r === (color === "white" ? 1 : 6) && !board[two]) out.push({ from, to: two });
      }
      for (const df of [-1, 1]) {
        const to = at(f + df, r + dir);
        if (to !== undefined && board[to] && board[to]!.color !== color) {
          out.push(r + dir === 0 || r + dir === 7 ? { from, to, promo: "queen" } : { from, to });
        }
      }
    } else if (p.kind === "knight") {
      for (const [df, dr] of [[1, 2], [2, 1], [-1, 2], [-2, 1], [1, -2], [2, -1], [-1, -2], [-2, -1]]) {
        const to = at(f + df, r + dr);
        if (to !== undefined && board[to]?.color !== color) out.push({ from, to });
      }
    } else if (p.kind === "king") {
      for (const [df, dr] of [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]]) {
        const to = at(f + df, r + dr);
        if (to !== undefined && board[to]?.color !== color) out.push({ from, to });
      }
    } else {
      const straight: [number, number][] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
      const diagonal: [number, number][] = [[1, 1], [1, -1], [-1, 1], [-1, -1]];
      slide(from, p.kind === "rook" ? straight : p.kind === "bishop" ? diagonal : [...straight, ...diagonal]);
    }
  });
  return out;
}

function material(board: (Piece | null)[]): number {
  return board.reduce((sum, p) => sum + (p ? (p.color === "white" ? 1 : -1) * VALUE[p.kind] : 0), 0);
}

const sqName = (s: Square) => `${"abcdefgh"[s % 8]}${Math.floor(s / 8) + 1}`;
const LETTER: Record<string, string> = { knight: "C", bishop: "F", rook: "T", queen: "D", king: "R", pawn: "" };

function applyMock(frame: Frame, action: Action, loadouts: GameRecord["loadouts"]): { frame: Frame; notation: string } {
  const color = frame.to_move;
  const board = frame.board.map((p) => (p ? { ...p } : null));
  const events: Frame["events"] = [];
  const used = { white: [...frame.used.white], black: [...frame.used.black] };
  let notation = "?";
  if (action.type === "move") {
    const piece = board[action.from]!;
    const taken = board[action.to];
    notation = `${LETTER[piece.kind]}${taken ? (piece.kind === "pawn" ? "abcdefgh"[action.from % 8] + "x" : "x") : ""}${sqName(action.to)}`;
    if (taken) events.push({ type: "captured", square: action.to, piece: { ...taken } });
    events.push({ type: "moved", from: action.from, to: action.to, piece: piece.id });
    board[action.to] = action.promo ? { ...piece, kind: action.promo } : piece;
    board[action.from] = null;
    if (action.promo) events.push({ type: "promoted", square: action.to, to: action.promo });
  } else {
    used[color].push(action.skill);
    events.push({ type: "skill_used", color, skill: action.skill, target: action.target });
    const t = action.target;
    if (t.kind === "piece_to") {
      board[t.to] = board[t.from];
      board[t.from] = null;
      events.push({ type: "teleported", from: t.from, to: t.to });
      notation = `Teleportation ${sqName(t.from)}→${sqName(t.to)}`;
    } else if (t.kind === "piece") {
      notation = `${action.skill === "freeze" ? "Freeze" : "Imune"} sur ${sqName(t.square)}`;
    } else notation = "Tornado";
  }
  void loadouts;
  const next: Frame = {
    ...frame,
    ply: frame.ply + 1,
    to_move: color === "white" ? "black" : "white",
    in_check: false,
    board,
    events,
    used,
  };
  return { frame: next, notation };
}

function skillOptionsFor(frame: Frame, loadouts: GameRecord["loadouts"]): SkillOptions[] {
  const color = frame.to_move;
  const options: SkillOptions[] = [];
  const empty = frame.board.map((p, i) => (p ? -1 : i)).filter((i) => i >= 0);
  for (const skill of loadouts[color]) {
    if (frame.used[color].includes(skill)) continue;
    if (skill === "tornado") options.push({ skill: "tornado", targets: [{ kind: "none" }] });
    if (skill === "imune") {
      const own = frame.board.flatMap((p, i) => (p && p.color === color && p.kind !== "king" ? [i] : []));
      options.push({ skill: "imune", targets: own.map((square) => ({ kind: "piece" as const, square })) });
    }
    if (skill === "freeze") {
      const foes = frame.board.flatMap((p, i) => (p && p.color !== color && p.kind !== "king" ? [i] : []));
      options.push({ skill: "freeze", targets: foes.map((square) => ({ kind: "piece" as const, square })) });
    }
    if (skill === "teleportation") {
      const pawns = frame.board.flatMap((p, i) => (p && p.color === color && p.kind === "pawn" ? [i] : []));
      const targets = pawns.flatMap((from) => empty.filter((to) => Math.abs(to - from) % 8 === 3 || to === from + 16).slice(0, 2).map((to) => ({ kind: "piece_to" as const, from, to })));
      if (targets.length) options.push({ skill: "teleportation", targets });
    }
  }
  return options;
}

function mockExplore(id: string, body: ExploreRequest): ExploreResponse {
  const record = { ...fixtureRecord(), game_id: id };
  if (!Number.isInteger(body.ply) || body.ply < 0 || body.ply > record.plies) {
    return { valid: false, error: "bad_ply", at: 0, frame: null, moves: [], skill_options: [], eval_cp: 0, best: null, notation: [] };
  }
  let frame = record.frames[body.ply];
  const notation: string[] = [];
  for (const action of body.line) {
    const legal = action.type === "move" ? pseudoMoves(frame.board, frame.to_move).some((m) => m.from === action.from && m.to === action.to) : skillOptionsFor(frame, record.loadouts).some((o) => o.skill === action.skill);
    if (!legal) {
      return { valid: false, error: "illegal_action", at: notation.length, frame: null, moves: [], skill_options: [], eval_cp: 0, best: null, notation };
    }
    const step = applyMock(frame, action, record.loadouts);
    frame = step.frame;
    notation.push(step.notation);
  }
  const moves = pseudoMoves(frame.board, frame.to_move);
  const sign = frame.to_move === "white" ? 1 : -1;
  const scored = moves
    .map((m) => ({ m, cp: material(applyMock(frame, { type: "move", from: m.from, to: m.to, promo: m.promo }, record.loadouts).frame.board) }))
    .sort((a, b) => sign * (b.cp - a.cp));
  const top = scored[0];
  const best = top
    ? { action: { type: "move" as const, from: top.m.from, to: top.m.to }, notation: applyMock(frame, { type: "move", from: top.m.from, to: top.m.to }, record.loadouts).notation, eval_cp: Math.max(-2000, Math.min(2000, top.cp)) }
    : null;
  return {
    valid: true,
    at: body.line.length,
    frame,
    moves,
    skill_options: skillOptionsFor(frame, record.loadouts),
    eval_cp: Math.max(-2000, Math.min(2000, material(frame.board))),
    best,
    notation,
  };
}

// ---- REST ----

const CAMPAIGN_FAMILIES = [
  ["attack", "Attaque", "Fer de Lance", "Le Stratège"],
  ["defense", "Défense", "Briseur de Muraille", "Le Gardien"],
  ["mobility", "Mobilité", "Marcheur du Vide", "Le Passeur"],
  ["control", "Contrôle", "Maître du Tempo", "Le Métronome"],
  ["create", "Création", "Grand Architecte", "L'Architecte"],
] as const;

const DECK_CHOICE_FROM_CHAPTER = 2;
const ELO_BASE = 400;
const ELO_PER_CHAPTER = 400;
const ELO_PER_LEVEL = 50;
const ELO_BOSS_BONUS = 100;
const LAST_NORMAL_LEVEL = 5;

const BOSS_HAND: SkillId[] = ["terminator", "trap", "freeze"];
const LENT_SKILLS: SkillId[] = ["freeze", "clone", "trap"];

const FORGE_TABLES: ForgeTableRow[][] = [
  [{ rarity: "uncommon", percent: 57 }, { rarity: "rare", percent: 29 }, { rarity: "epic", percent: 14 }],
  [{ rarity: "rare", percent: 68 }, { rarity: "epic", percent: 32 }],
  [{ rarity: "rare", percent: 68 }, { rarity: "epic", percent: 32 }],
  [{ rarity: "epic", percent: 100 }],
  [{ rarity: "epic", percent: 86 }, { rarity: "legendary", percent: 14 }],
];

/** Un chapitre en cours de forge, un à révéler, le dernier sans Légendaire restante. */
const BOSS_FORGES: (BossForgeInfo | null)[] = [
  { chapter: 0, state: "forging", skill: null, deck_full: false, legendary_unavailable: false },
  { chapter: 1, state: "pending", skill: "freeze", deck_full: false, legendary_unavailable: false },
  null,
  null,
  { chapter: 4, state: "forging", skill: null, deck_full: true, legendary_unavailable: true },
];

/** Cinq chapitres complets ; le premier a un titre gagné, les suivants laissent choisir le deck, le boss démarre d'une position imposée. */
function fixtureCampaign(): CampaignChapterView[] {
  return CAMPAIGN_FAMILIES.map(([family, name, title, bossName], chapter) => {
    const choice = chapter >= DECK_CHOICE_FROM_CHAPTER;
    const levels: CampaignLevelView[] = Array.from({ length: 7 }, (_, i) => ({
      level: i,
      name: i === 6 ? bossName : `Niveau ${i + 1}`,
      elo:
        ELO_BASE + ELO_PER_CHAPTER * chapter + ELO_PER_LEVEL * Math.min(i, LAST_NORMAL_LEVEL) + (i === 6 ? ELO_BOSS_BONUS : 0),
      boss: i === 6,
      player_deck: choice ? [] : ["trap", "terminator"],
      bot_deck: i === 6 ? BOSS_HAND : i === 0 ? [] : ["trap"],
      deck_choice: choice,
      lent: choice ? LENT_SKILLS : [],
      move_limit: i === 6 ? null : 40,
      ...(chapter === 0 && i === 1 ? { hint: "Avancez vos pions avant d'utiliser Terminator." } : {}),
      start_fen: i === 6 && chapter > 0 ? "4k3/8/8/8/8/8/4P3/4K2R w K - 0 1" : null,
      human_color: i === 6 && chapter > 0 ? (chapter === CAMPAIGN_FAMILIES.length - 1 ? "black" : "white") : null,
      objective: i === 6 ? null : choice ? "Utiliser une compétence" : "Utiliser Terminator",
      challenge: i === 6 ? null : "Gagner en 40 coups",
      best: chapter === 0 && i < 3 ? [true, i < 2, i < 1] : [false, false, false],
      rewarded: false,
    }));
    return {
      chapter,
      family,
      name,
      title,
      title_earned: chapter === 0,
      available: true,
      stars: chapter === 0 ? 6 : 0,
      boss_stars_required: 12,
      boss_unlocked: false,
      forge_table: FORGE_TABLES[chapter],
      boss_forge: BOSS_FORGES[chapter],
      levels,
    };
  });
}

function installFetch() {
  const real = window.fetch.bind(window);
  window.fetch = async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url, location.href);
    const path = url.pathname;
    if (!path.startsWith("/api/")) return real(input, init);

    if (path === "/api/campaign") {
      await wait(250);
      return json({ chapters: fixtureCampaign(), total_stars: 6, max_stars: 105 });
    }
    if (path === "/api/live") {
      await wait(250);
      return json({ games: fixtureLive() });
    }
    if (path === "/api/me/games") {
      await wait(300);
      const limit = Number(url.searchParams.get("limit") ?? 20);
      const offset = Number(url.searchParams.get("offset") ?? 0);
      const base = fixtureGames();
      const all = Array.from({ length: 26 }, (_, i) => ({ ...base[i % base.length], game_id: i < base.length ? base[i].game_id : `g-demo-${i + 1}` }));
      return json({ total: all.length, games: all.slice(offset, offset + limit) });
    }
    if (path === "/api/leaderboard") {
      await wait(200);
      const limit = Number(url.searchParams.get("limit") ?? 50);
      const offset = Number(url.searchParams.get("offset") ?? 0);
      const all = Array.from({ length: 120 }, (_, i) => ({
        rank: i + 1,
        username: i === 2 ? "jeremy" : `joueur${i + 1}`,
        elo: 1900 - i * 9,
        games: 40 + i,
        wins: 25 + (i % 9),
        draws: i % 5,
        losses: 10 + (i % 7),
      }));
      return json({ total: all.length, entries: all.slice(offset, offset + limit) });
    }
    if (/^\/api\/players\/[^/]+$/.test(path)) {
      await wait(200);
      const username = decodeURIComponent(path.slice("/api/players/".length));
      const base = fixtureGames();
      return json({
        username,
        elo: 1284,
        placed: true,
        peak_elo: 1340,
        rank: 3,
        games: 26,
        wins: 14,
        draws: 3,
        losses: 9,
        streak: 2,
        created_at: "2026-01-12T10:00:00Z",
        history: Array.from({ length: 30 }, (_, i) => ({ elo: 1100 + i * 6 + (i % 4) * 9, at: new Date(Date.UTC(2026, 8, 1 + i)).toISOString() })),
        recent: Array.from({ length: 10 }, (_, i) => ({
          game_id: base[i % base.length].game_id,
          opponent: `adversaire${i + 1}`,
          result: (["win", "loss", "draw"] as const)[i % 3],
          color: i % 2 ? "black" : "white",
          rated: true,
          elo_delta: 8 - i,
          reason: "checkmate",
          at: new Date(Date.UTC(2026, 8, 20 - i)).toISOString(),
        })),
      });
    }
    if (path === "/api/me/skills") {
      await wait(200);
      const ids = ["teleportation", "imune", "tornado", "teleportation", "imune", "tornado"] as const;
      return json({
        deck: ["teleportation", "imune", "tornado"],
        entries: Array.from({ length: 40 }, (_, i) => ({ id: i + 1, skill: ids[i % ids.length], change: i % 4 === 3 ? "lost" : "gained", source: "won", at: new Date(Date.UTC(2026, 8, 28 - i)).toISOString() })),
      });
    }
    const m = path.match(/^\/api\/games\/([^/]+)(?:\/(analysis|explore))?$/);
    if (m) {
      const id = decodeURIComponent(m[1]);
      if (id === "g-missing") return json({ error: "not_found" }, 404);
      if (id === "g-noreplay") return json({ error: "no_replay" }, 409);
      await wait(id === "g-slow" ? 3000 : 350);
      const record = { ...fixtureRecord(), game_id: id };
      if (!m[2]) return json(record);
      if (m[2] === "analysis") {
        await wait(1100);
        return json(fixtureAnalysis(Number(url.searchParams.get("depth") ?? 3), record));
      }
      const body = JSON.parse(String(init?.body ?? "{}")) as ExploreRequest;
      await wait(250);
      return json(mockExplore(id, body));
    }
    return real(input, init);
  };
}

// ---- WebSocket ----

type Handler<T> = ((e: T) => void) | null;

/** Joueurs bloqués du compte simulé (modération, voir `blocks`). */
let blocked: string[] = [];

class FakeSocket {
  static readonly OPEN = 1;
  readonly OPEN = 1;
  readyState = 0;
  onopen: Handler<Event> = null;
  onmessage: Handler<MessageEvent> = null;
  onclose: Handler<CloseEvent> = null;
  private timers: ReturnType<typeof setTimeout>[] = [];
  private watching: string | null = null;

  constructor() {
    this.later(() => {
      this.readyState = 1;
      this.onopen?.(new Event("open"));
    }, 50);
  }

  private later(fn: () => void, ms: number) {
    this.timers.push(setTimeout(fn, ms));
  }

  private reply(msg: ServerMsg) {
    this.onmessage?.(new MessageEvent("message", { data: JSON.stringify(msg) }));
  }

  send(raw: string) {
    const msg = JSON.parse(raw) as ClientMsg;
    if (msg.type === "hello") {
      this.later(() => {
        this.reply({
          type: "welcome",
          player_id: "p-mock",
          token: "mock-token",
          deck: ["teleportation", "imune", "tornado"],
          pending_reward: null,
          account: { player_id: "p-mock", username: "jeremy", guest: false, elo: 1284, rank: 3, games: 6, wins: 3, draws: 1, losses: 2, chat_muted: false },
        });
        this.reply({
          type: "friends",
          friends: [
            { username: "ada", elo: 1311, presence: "in_game", last_seen: null, game_id: "g-live-1" },
            { username: "lea", elo: 1190, presence: "online", last_seen: null, game_id: null },
          ],
          incoming: [],
          outgoing: [],
        });
      }, 80);
    } else if (msg.type === "blocks_list") {
      this.later(() => this.reply({ type: "blocks", blocked: blocked.map((username) => ({ username })) }), 50);
    } else if (msg.type === "block_user" || msg.type === "unblock_user") {
      const name = msg.username;
      blocked = msg.type === "block_user" ? [...blocked.filter((b) => b !== name), name].sort() : blocked.filter((b) => b !== name);
      this.later(() => this.reply({ type: "blocks", blocked: blocked.map((username) => ({ username })) }), 50);
    } else if (msg.type === "set_chat_muted") {
      this.later(() => this.reply({ type: "chat_settings", chat_muted: msg.muted }), 50);
    } else if (msg.type === "report_user") {
      this.later(() => this.reply({ type: "report_ack", username: msg.username }), 50);
    } else if (msg.type === "spectate") {
      this.stopWatching();
      if (msg.game_id === "g-missing") {
        this.later(() => this.reply({ type: "error", code: "no_such_game", message: "Partie introuvable" }), 200);
        return;
      }
      this.watching = msg.game_id;
      const record = { ...fixtureRecord(), game_id: msg.game_id };
      let ply = 6;
      const push = () => {
        if (this.watching !== msg.game_id) return;
        const view = fixtureSpectatorView(ply, record);
        if (ply >= record.plies) this.reply({ type: "spectate_over", view });
        else {
          this.reply({ type: "spectate_state", view });
          ply++;
          this.later(push, 3000);
        }
      };
      this.later(push, 300);
    } else if (msg.type === "unspectate") {
      this.stopWatching();
    }
  }

  private stopWatching() {
    this.watching = null;
  }

  close() {
    this.timers.forEach(clearTimeout);
    this.readyState = 3;
  }
}

export function installMock() {
  installFetch();
  (window as unknown as { WebSocket: unknown }).WebSocket = FakeSocket;
  console.info("[chessy] mode simulé (?mock=1) : REST v4 et WebSocket remplacés par des fixtures.");
}
