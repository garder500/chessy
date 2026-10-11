// Mirrors crates/chessy-server/src/protocol.rs and the engine's serialized types.
// Squares are indices 0..63 with a1 = 0 and h8 = 63 (rank * 8 + file).
import type { Family } from "./catalog";

export type Square = number;
export type Color = "white" | "black";
export type PieceKind = "pawn" | "knight" | "bishop" | "rook" | "queen" | "king";

/** Les 27 compétences écrites à la main. */
export type BuiltinSkillId =
  | "teleportation"
  | "imune"
  | "freeze"
  | "rollback"
  | "clone"
  | "destiny_swapper"
  | "remover"
  | "wall"
  | "mirage"
  | "evolve"
  | "switch"
  | "mind"
  | "control"
  | "morph"
  | "canceller"
  | "tornado"
  | "invisibility"
  | "terminator"
  | "trap"
  | "bench"
  | "forcefield"
  | "transposition"
  | "queensac"
  | "temporal"
  | "geomancy"
  | "celestial"
  | "godhelp"
;

/** Une compétence forgée par le serveur : `forged_<n>` (voir `forged.ts`). */
export type ForgedSkillId = `forged_${number}`;

export type SkillId = BuiltinSkillId | ForgedSkillId;

export interface Piece {
  id: number;
  kind: PieceKind;
  color: Color;
  /** Case de départ (toujours présente côté serveur v3). */
  home?: Square;
  /** Mirage : ne capture ni n'attaque, disparaît quand elle est prise. */
  mirage?: boolean;
  /** Pion-mur (Wall). */
  wall?: boolean;
  /** Pièce temporaire (Terminator, God Help). */
  temp?: boolean;
}

/** Types de pièce proposables par Mirage et Morph. */
export type SpawnKind = Exclude<PieceKind, "king">

export interface Move {
  from: Square;
  to: Square;
  promo?: PieceKind;
}

export type SkillTarget =
  | { kind: "none" }
  | { kind: "piece"; square: Square }
  | { kind: "square"; square: Square }
  | { kind: "piece_to"; from: Square; to: Square }
  | { kind: "pair"; a: Square; b: Square }
  /** Mirage / Morph : le type de pièce s'appelle `piece` dans le JSON (`kind` est le tag). */
  | { kind: "spawn"; square: Square; piece: SpawnKind };

export type Action =
  | { type: "move"; from: Square; to: Square; promo?: PieceKind }
  | { type: "skill"; skill: SkillId; target: SkillTarget };

export type EffectKind =
  | "immune"
  | "frozen"
  | "invisible"
  | "forcefield"
  | "celestial"
  | "locked"
  | "morphed"
  | "color_loan"
  | "vanish"
  /** Effets de partie entière (compétences forgées) : `piece` vaut alors `NO_PIECE`. */
  | "truce"
  | "fog"
  | "silenced"
  | "domain";

/** `ActiveEffect.piece` d'un effet qui ne concerne aucune pièce. */
export const NO_PIECE = 65_535;

/** Les effets sans fin (Force Field, Celestial) ont `expires_at = 4294967295`. */
export const NEVER_EXPIRES = 4_294_967_295;

export interface ActiveEffect {
  kind: EffectKind;
  piece: number;
  expires_at: number;
  orig_kind?: PieceKind;
  orig_color?: Color;
  /** Silence : le joueur visé. */
  owner?: Color;
}

export interface Terrain {
  square: Square;
  owner: Color;
  expires_at: number;
}

export type GameEvent =
  | { type: "moved"; from: Square; to: Square; piece: number }
  | { type: "captured"; square: Square; piece: Piece }
  | { type: "promoted"; square: Square; to: PieceKind }
  | { type: "castled"; rook_from: Square; rook_to: Square }
  | { type: "skill_used"; color: Color; skill: SkillId; target: SkillTarget }
  | { type: "teleported"; from: Square; to: Square }
  | { type: "cloned"; from: Square; to: Square; piece: Piece }
  | { type: "swapped"; a: Square; b: Square }
  | { type: "removed"; square: Square; piece: Piece }
  | { type: "rolled_back"; from: Square; to: Square }
  | { type: "effect_added"; piece: number; effect: EffectKind; expires_at: number }
  | { type: "spawned"; square: Square; piece: Piece }
  | { type: "transformed"; square: Square; kind: PieceKind }
  | { type: "switched"; square: Square; piece: Piece }
  | { type: "rotated"; moves: { from: Square; to: Square }[] }
  | { type: "trap_set"; square: Square }
  | { type: "trap_sprung"; square: Square; piece: number }
  | { type: "benched"; square: Square; piece: Piece }
  | { type: "unbenched"; square: Square; piece: Piece }
  | { type: "pushed"; piece: number; from: Square; to: Square }
  | { type: "saved"; piece: number; from: Square; to: Square }
  /** N'est envoyé qu'à l'auteur de Mind Reading. */
  | { type: "best_move"; from: Square; to: Square; promo?: PieceKind }
  | { type: "cancelled"; skill: SkillId }
  | { type: "terrain"; squares: Square[] }
  | { type: "global_effect"; effect: EffectKind; expires_at: number; owner?: Color }
  | { type: "vanished"; square: Square; piece: Piece }
  /** Domaine : des fous frappent la pièce `piece` qui donnait échec en `square` (un `captured` suit). */
  | { type: "ambushed"; square: Square; piece: Piece }
  | { type: "loan_ended"; square: Square; piece: Piece };

export type Outcome =
  | { type: "ongoing" }
  | { type: "checkmate"; winner: Color }
  | { type: "resignation"; winner: Color }
  | { type: "timeout"; winner: Color }
  | { type: "draw_agreed" }
  | { type: "stalemate" }
  | { type: "fifty_moves" }
  | { type: "repetition" }
  | { type: "insufficient_material" };

export interface SkillSlot {
  skill: SkillId;
  used: boolean;
  /** Usages déjà consommés (absent = serveur ancien : 0 ou 1 selon `used`). */
  uses?: number;
  /** Usages permis (3 pour Mind Reading, 1 sinon). */
  max_uses?: number;
}

export interface SkillOptions {
  skill: SkillId;
  targets: SkillTarget[];
}

export interface Clock {
  white_ms: number;
  black_ms: number;
  running: Color | null;
}

export interface OpponentInfo {
  username: string | null;
  elo: number | null;
  guest: boolean;
  /** Adversaire IA (mode Solo) ; absent = faux. */
  bot?: boolean;
}

/** Une action déjà jouée, telle que ce joueur l'a vue (voir `StateView.history`). */
export interface HistoryEntry {
  /** `ply` de la position obtenue. */
  ply: number;
  /** Camp au trait après l'action. */
  to_move: Color;
  events: GameEvent[];
  /** Où se trouvent, après l'action, les pièces des événements `moved`. */
  landed: { square: Square; kind: PieceKind }[];
}

export interface StateView {
  game_id: string;
  clock: Clock;
  /** Faux en solo : pas d'horloge. Absent (serveur ancien) = vrai, voir `normalizeState`. */
  clock_enabled: boolean;
  rated: boolean;
  opponent: OpponentInfo;
  draw_offer: "none" | "you" | "them";
  ply_count: number;
  you: Color;
  ply: number;
  to_move: Color;
  in_check: boolean;
  board: (Piece | null)[];
  moves: Move[];
  skill_options: SkillOptions[];
  my_skills: SkillSlot[];
  opponent_skills: { total: number; used: SkillId[] };
  effects: ActiveEffect[];
  /** Cases piégées par le joueur (les siennes uniquement). */
  traps: Square[];
  /** Ses pièces actuellement sur le banc. */
  benched: Piece[];
  terrain: Terrain[];
  outcome: Outcome;
  events: GameEvent[];
  opponent_connected: boolean;
  /** Nombre de spectateurs courant (v4) ; absent = serveur ancien. */
  spectators?: number;
  /**
   * Toutes les actions déjà jouées, de la plus ancienne à la plus récente. Envoyé seulement quand le
   * joueur rejoint sa partie (rechargement de la page, reconnexion) ; absent des états en direct.
   */
  history?: HistoryEntry[];
  /** Niveau de campagne de la partie ; `null` ou absent hors campagne. */
  campaign?: CampaignContext | null;
}

/** Niveau de campagne d'une partie et ce qu'il faut en savoir pendant qu'on la joue. */
export interface CampaignContext {
  chapter: number;
  level: number;
  move_limit: number | null;
  objective: string | null;
  challenge: string | null;
}

export type BossForgeState = "forging" | "pending" | "placed";

/** Où en est la compétence forgée pour le boss d'un chapitre. */
export interface BossForgeInfo {
  chapter: number;
  state: BossForgeState;
  skill: SkillId | null;
  deck_full: boolean;
  legendary_unavailable: boolean;
}

export type RewardOutcomeKind = "stolen" | "forged" | "spared";

/** Étoiles d'un niveau de campagne : [victoire, objectif, défi]. */
export type CampaignStars = [boolean, boolean, boolean];

export interface CampaignResult {
  chapter: number;
  level: number;
  stars: CampaignStars;
  best: CampaignStars;
  chapter_stars: number;
  boss_unlocked: boolean;
  boss_stars_required: number;
  boss_just_unlocked: boolean;
  /** Titre gagné par cette partie (victoire contre le boss). */
  title: string | null;
  total_stars: number;
  hint_available: boolean;
  boss_forge: BossForgeInfo | null;
}

/** Un niveau de `GET /api/campaign` (le boss a `level` 6 et `boss: true`). */
export interface CampaignLevel {
  level: number;
  name: string;
  elo: number;
  boss: boolean;
  /** Vide quand `deck_choice` : le joueur compose son deck parmi le sien. */
  player_deck: SkillId[];
  bot_deck: SkillId[];
  deck_choice: boolean;
  start_fen: string | null;
  human_color: "white" | "black" | null;
  objective: string | null;
  challenge: string | null;
  best: CampaignStars;
  rewarded: boolean;
}

export interface CampaignChapter {
  chapter: number;
  family: Family;
  name: string;
  title: string;
  title_earned: boolean;
  available: boolean;
  stars: number;
  boss_stars_required: number;
  boss_unlocked: boolean;
  levels: CampaignLevel[];
}

export interface RewardOffer {
  deck: SkillId[];
  steal_options: SkillId[];
  deck_full: boolean;
}

export type SoloColor = Color | "random";

export type LobbyStatus =
  | { type: "idle" }
  | { type: "queued"; ranked: boolean }
  | { type: "room_waiting"; code: string };

export interface DeckSelectInfo {
  game_id: string;
  opponent: OpponentInfo;
  rated: boolean;
  you: Color;
  deck: SkillId[];
  max_picks: number;
  seconds: number;
  submitted: boolean;
}

// ---- comptes, classement, amis (voir docs/spec-v2.md) ----

export interface Me {
  player_id: string;
  username: string | null;
  guest: boolean;
  elo: number;
  rank: number | null;
  games: number;
  wins: number;
  draws: number;
  losses: number;
  /** Le compte ne reçoit aucun message de chat (`set_chat_muted`) ; absent = serveur ancien. */
  chat_muted?: boolean;
  /** Parties d'évaluation (v5) ; absent = serveur ancien (on suppose le joueur déjà évalué). */
  placement?: PlacementProgress;
}

/** Avancement des parties d'évaluation : `placed` quand l'Elo est une estimation. */
export interface PlacementProgress {
  placed: boolean;
  done: number;
  total: number;
}

/** Fin d'une partie d'évaluation ; `elo` (et `before`) une fois les `total` parties jouées. */
export interface PlacementView {
  done: number;
  total: number;
  elo?: number | null;
  before?: number | null;
}

export interface LeaderboardEntry {
  rank: number;
  username: string;
  elo: number;
  games: number;
  wins: number;
  draws: number;
  losses: number;
}

export interface Leaderboard {
  total: number;
  entries: LeaderboardEntry[];
}

export interface RecentGame {
  game_id: string;
  opponent: string | null;
  result: "win" | "loss" | "draw";
  color: Color;
  rated: boolean;
  elo_delta: number | null;
  reason: string;
  at: string;
}

export interface PublicProfile {
  username: string;
  title: string | null;
  elo: number;
  /** Faux tant que l'Elo est celui de départ (parties d'évaluation non jouées) ; absent = serveur ancien. */
  placed?: boolean;
  peak_elo: number;
  rank: number | null;
  games: number;
  wins: number;
  draws: number;
  losses: number;
  streak: number;
  created_at: string;
  history: { elo: number; at: string }[];
  recent: RecentGame[];
}

export type Presence = "online" | "in_game" | "offline";

export interface FriendInfo {
  username: string;
  elo: number;
  presence: Presence;
  last_seen: string | null;
  /** Partie en cours de l'ami (v4), pour le bouton « Regarder » ; absent = serveur ancien. */
  game_id?: string | null;
}

export interface FriendsSnapshot {
  friends: FriendInfo[];
  incoming: { username: string; elo: number }[];
  outgoing: { username: string }[];
}

/** Motifs de signalement (docs/spec-v2.md §4, « Modération »). */
export type ReportReason = "spam" | "harassment" | "cheating" | "inappropriate_name" | "other";

export type Relation = "none" | "friend" | "incoming" | "outgoing" | "self";

export interface UserResult {
  username: string;
  elo: number;
  relation: Relation;
}

export type NoticeCode =
  | "friend_request_received"
  | "friend_accepted"
  | "friend_removed"
  | "challenge_declined"
  | "challenge_expired"
  | "challenge_cancelled"
  | "user_not_found"
  | "already_friends"
  | "friend_offline"
  | "friend_busy"
  | "rated_pair_capped";

export interface EloChange {
  you_before: number;
  you_after: number;
  opp_before: number;
  opp_after: number;
}

export type ServerMsg =
  | { type: "welcome"; player_id: string; token: string; deck: SkillId[]; pending_reward: RewardOffer | null; account: Me }
  | ({ type: "friends" } & FriendsSnapshot)
  | { type: "user_results"; query: string; users: UserResult[] }
  | { type: "blocks"; blocked: { username: string }[] }
  | { type: "chat_settings"; chat_muted: boolean }
  | { type: "report_ack"; username: string }
  | { type: "notice"; code: NoticeCode; username?: string }
  | { type: "challenge_received"; from: { username: string; elo: number } }
  | { type: "challenge_sent"; username: string }
  | { type: "draw_offered" }
  | { type: "draw_declined" }
  | { type: "chat"; text: string; mine: boolean }
  | { type: "rematch_offered" }
  | { type: "rematch_declined" }
  | { type: "lobby"; status: LobbyStatus }
  | ({ type: "deck_select" } & DeckSelectInfo)
  | ({ type: "state" } & StateView)
  | { type: "opponent_status"; connected: boolean }
  | { type: "game_over"; outcome: Outcome; reward: RewardOffer | null; rated: boolean; elo: EloChange | null; reason: string; campaign?: CampaignResult | null; placement?: PlacementView }
  | { type: "deck_update"; deck: SkillId[]; gained: SkillId | null; lost: SkillId | null }
  | { type: "reward_outcome"; by: string; kind: RewardOutcomeKind; skill: SkillId | null; refilled: SkillId | null }
  | { type: "boss_forge"; info: BossForgeInfo }
  | { type: "game_cancelled"; reason: string }
  | { type: "spectate_state"; view: SpectatorView }
  | { type: "spectate_over"; view: SpectatorView }
  | { type: "spectate_ended"; reason: string }
  | { type: "error"; code: string; message: string };

export type RewardChoice =
  | { kind: "steal"; skill: SkillId; replace?: SkillId }
  | { kind: "random"; replace?: SkillId }
  | { kind: "skip" };

/** Durée de partie demandée (absente : valeur par défaut du serveur). */
export type TimeControl = "short" | "medium" | "long";

export type ClientMsg =
  | { type: "hello"; token?: string }
  | { type: "queue_join"; ranked?: boolean; time?: TimeControl }
  | { type: "solo_start"; elo: number; color: SoloColor }
  | { type: "campaign_start"; chapter: number; level: number; deck?: SkillId[] }
  | { type: "boss_forge_claim"; chapter: number }
  | { type: "boss_forge_place"; chapter: number; replace?: SkillId | null }
  | { type: "placement_start"; color?: SoloColor }
  | { type: "create_room"; time?: TimeControl }
  | { type: "join_room"; code: string }
  | { type: "leave_lobby" }
  | { type: "leave_deck_select" }
  | { type: "select_deck"; skills: SkillId[] }
  | { type: "action"; action: Action }
  | { type: "resign" }
  | { type: "dev_finish"; result: "win" | "loss" | "all_stars" }
  | { type: "reward_choice"; choice: RewardChoice }
  | { type: "friend_request"; username: string }
  | { type: "friend_respond"; username: string; accept: boolean }
  | { type: "friend_remove"; username: string }
  | { type: "friends_list" }
  | { type: "block_user"; username: string }
  | { type: "unblock_user"; username: string }
  | { type: "blocks_list" }
  | { type: "set_chat_muted"; muted: boolean }
  /** `context` : extrait de chat, 1 Kio au plus (le serveur le tronque). */
  | { type: "report_user"; username: string; reason: ReportReason; game_id?: string; context?: string }
  | { type: "user_search"; query: string }
  | { type: "challenge"; username: string; time?: TimeControl }
  | { type: "challenge_respond"; username: string; accept: boolean }
  | { type: "challenge_cancel" }
  | { type: "offer_draw" }
  | { type: "respond_draw"; accept: boolean }
  | { type: "chat"; text: string }
  | { type: "rematch_request" }
  | { type: "rematch_respond"; accept: boolean }
  | { type: "spectate"; game_id: string }
  | { type: "unspectate" };

// ---- parties enregistrées, replays, analyse, direct (voir docs/spec-v4.md §1-§3) ----

export type GameKind = "duel" | "challenge" | "room" | "solo";

/** Un joueur d'une partie : `elo` = Elo avant la partie, ou niveau du bot. */
export interface Seat {
  username: string | null;
  elo: number | null;
  bot: boolean;
}

export type GameResult = "win" | "loss" | "draw";

/** Ligne de `GET /api/me/games` ; `color` est le camp du demandeur. */
export interface GameSummary {
  game_id: string;
  kind: GameKind;
  rated: boolean;
  white: Seat;
  black: Seat;
  color: Color;
  result: GameResult;
  reason: string;
  plies: number;
  /** Durée de la partie ; `null` pour les anciennes parties. */
  time_control: TimeControl | null;
  elo_delta: number | null;
  at: string;
}

/** Comment une compétence est arrivée ou partie (voir `history_store.rs`). */
export type HistorySource = "starter" | "refill" | "forged" | "stolen" | "won" | "taken" | "replaced" | "earlier";

/** Une ligne de l'historique des compétences d'un joueur. */
export interface SkillHistoryEntry {
  id: number;
  skill: SkillId;
  change: "gained" | "lost";
  source: HistorySource;
  /** L'autre joueur (volée à, prise par), s'il a un compte. */
  other?: string;
  /** Date ISO 8601 UTC. */
  at: string;
}

/** `GET /api/me/skills` : l'historique (le plus récent d'abord) et le deck actuel. */
export interface MySkills {
  entries: SkillHistoryEntry[];
  deck: SkillId[];
}

export interface MyGames {
  total: number;
  games: GameSummary[];
}

/** Une action de la partie : `ply` = numéro (1-based) de l'action, `notation` en français courant. */
export interface MoveInfo {
  ply: number;
  color: Color;
  action: Action;
  notation: string;
}

export interface FrameTrap {
  square: Square;
  owner: Color;
}

export interface FrameBenched {
  piece: Piece;
  square: Square;
  owner: Color;
  back_at: number;
}

export interface UsedSkills {
  white: SkillId[];
  black: SkillId[];
}

/** Position après `ply` actions (0 = position initiale), avec toute l'information (partie finie). */
export interface Frame {
  ply: number;
  to_move: Color;
  in_check: boolean;
  board: (Piece | null)[];
  effects: ActiveEffect[];
  traps: FrameTrap[];
  terrain: Terrain[];
  benched: FrameBenched[];
  /** Ce qui s'est passé pour arriver à cette position. */
  events: GameEvent[];
  used: UsedSkills;
  outcome: Outcome;
}

export interface GameRecord {
  game_id: string;
  kind: GameKind;
  rated: boolean;
  white: Seat;
  black: Seat;
  result: { outcome: Outcome; reason: string };
  plies: number;
  time_control: TimeControl | null;
  at: string;
  loadouts: { white: SkillId[]; black: SkillId[]; start?: string | null };
  moves: MoveInfo[];
  /** `frames.length == plies + 1`. */
  frames: Frame[];
}

export type AnalysisLabel = "best" | "good" | "inaccuracy" | "mistake" | "blunder";

/** Meilleur coup *simple* du camp au trait ; `eval_cp` du point de vue des blancs. */
export interface BestMove {
  action: Action;
  notation: string;
  eval_cp: number;
}

export interface PlyAnalysis {
  ply: number;
  /** Point de vue des blancs, borné à ±2000 (mat = ±2000). */
  eval_cp: number;
  best: BestMove | null;
  loss_cp: number;
  label: AnalysisLabel;
}

export type LabelCounts = Record<AnalysisLabel, number>;

export interface Analysis {
  depth: number;
  plies: PlyAnalysis[];
  accuracy: { white: number; black: number };
  summary: { white: LabelCounts; black: LabelCounts };
}

export interface ExploreRequest {
  ply: number;
  line: Action[];
  depth?: number;
}

export interface ExploreResponse {
  valid: boolean;
  error?: "illegal_action" | "bad_ply";
  /** Nombre d'actions de `line` appliquées avec succès. */
  at: number;
  frame: Frame | null;
  moves: Move[];
  skill_options: SkillOptions[];
  eval_cp: number;
  best: BestMove | null;
  /** Notation de chaque action de `line`. */
  notation: string[];
}

/** Partie en cours (`GET /api/live`). */
export interface LiveGame {
  game_id: string;
  kind: GameKind;
  rated: boolean;
  white: Seat;
  black: Seat;
  ply: number;
  started_at: string;
  spectators: number;
}

export interface LiveGames {
  games: LiveGame[];
}

/** Vue d'une partie pour un spectateur : pas d'information cachée (pièges, banc, pièces invisibles). */
export interface SpectatorView {
  game_id: string;
  kind: GameKind;
  rated: boolean;
  white: Seat;
  black: Seat;
  ply: number;
  to_move: Color;
  in_check: boolean;
  board: (Piece | null)[];
  effects: ActiveEffect[];
  terrain: Terrain[];
  /** `running` : camp dont l'horloge tourne (ou un booléen selon la version du serveur). */
  clock: { white_ms: number; black_ms: number; running: Color | boolean | null };
  clock_enabled: boolean;
  events: GameEvent[];
  used: UsedSkills;
  outcome: Outcome;
  spectators: number;
  /** Retransmission différée (30 000 ms entre humains, 0 en solo). */
  delay_ms: number;
}
