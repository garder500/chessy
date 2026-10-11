import { useSyncExternalStore } from "react";
import type {
  BossForgeInfo,
  CampaignResult,
  ClientMsg,
  DeckSelectInfo,
  EloChange,
  FriendsSnapshot,
  LobbyStatus,
  Me,
  NoticeCode,
  Outcome,
  PlacementView,
  ReportReason,
  RewardOffer,
  RewardOutcomeKind,
  ServerMsg,
  SkillId,
  SoloColor,
  StateView,
  UserResult,
} from "./protocol";
import { t } from "./i18n";
import { forgedVersion, isForgedId, loadForged, noticeForged, onForgedChange } from "./forged";
import { skillRevealed } from "./screens/campaign/bossForge";
import { skillName } from "./skills";
import { sfx } from "./sound";
import { isLive, reduceSpectator, SPECTATE_ERRORS, startSpectating, type SpectatingState } from "./replay/spectator";
import { clampElo, readSolo, SOLO_DEFAULT, writeSolo, type SoloSetting } from "./solo";

export interface Toast {
  id: number;
  text: string;
}

export interface ChatLine {
  mine: boolean;
  text: string;
}

export interface GameOver {
  outcome: Outcome;
  reward: RewardOffer | null;
  rated: boolean;
  elo: EloChange | null;
  reason: string;
  /** Partie d'évaluation : avancement, et Elo estimé après la dernière. */
  placement: PlacementView | null;
  /** Résultat d'une partie de campagne ; `null` ailleurs. */
  campaign: CampaignResult | null;
}

export interface AppState {
  /** Change quand une définition de compétence forgée arrive : relance le rendu des fiches. */
  forged: number;
  connection: "connecting" | "open" | "closed" | "replaced";
  playerId: string | null;
  /** Compte connecté (un invité a `guest: true`), `null` tant que le `welcome` n'est pas reçu. */
  account: Me | null;
  deck: SkillId[];
  lobby: LobbyStatus;
  deckSelect: DeckSelectInfo | null;
  game: StateView | null;
  /** Set once the server reports the game as finished. */
  over: GameOver | null;
  /** A reward from a past game that was never claimed. */
  pendingReward: RewardOffer | null;
  /** L'écran « Votre récompense » est ouvert (il suit l'écran de victoire). */
  rewardOpen: boolean;
  friends: FriendsSnapshot;
  /** Pseudos bloqués par le compte (demandés par `loadBlocks`, voir `blocks_list`). */
  blocked: string[];
  userResults: { query: string; users: UserResult[] } | null;
  incomingChallenge: { username: string; elo: number } | null;
  outgoingChallenge: string | null;
  toasts: Toast[];
  /** Compétence qui vient d'être forgée : affichée en plein écran jusqu'à ce que le joueur la range. */
  reveal: SkillId | null;
  /** Messages de la partie en cours (remis à zéro à chaque nouvelle partie). */
  chat: ChatLine[];
  rematch: "none" | "offered" | "received";
  /** Dernier réglage du mode Solo (mémorisé dans le navigateur). */
  solo: SoloSetting;
  /** `solo_start` envoyé, partie pas encore créée (le bot répond presque instantanément). */
  soloPending: boolean;
  /** Partie en cours lancée depuis la campagne (sert aux raccourcis de dev). */
  campaignGame: boolean;
  /** Dernier état connu de la forge d'un boss de campagne. */
  bossForge: BossForgeInfo | null;
  /** Nombre d'erreurs serveur reçues : permet à un écran de savoir que sa requête a été refusée. */
  errorCount: number;
  /** Ce que le gagnant d'une classée a fait de sa récompense, appris par le perdant. */
  rewardOutcome: { by: string; kind: RewardOutcomeKind; skill: SkillId | null; refilled: SkillId | null } | null;
  /** Partie regardée en tant que spectateur (v4), `null` si on ne regarde rien. */
  spectating: SpectatingState | null;
}

/** Le compte après une fin de partie : nouvel Elo (classée ou estimation) et avancement de l'évaluation. */
export function accountAfterGame(account: Me, msg: Extract<ServerMsg, { type: "game_over" }>): Me {
  const next = { ...account };
  if (msg.elo) next.elo = msg.elo.you_after;
  const p = msg.placement;
  if (p) {
    next.placement = { placed: p.elo != null, done: p.done, total: p.total };
    if (p.elo != null) next.elo = p.elo;
  }
  return next;
}

/** Valeurs par défaut des champs que d'anciens serveurs n'envoient pas (`clock_enabled`, `opponent.bot`). */
export function normalizeState(view: StateView): StateView {
  return {
    ...view,
    clock_enabled: view.clock_enabled ?? true,
    traps: view.traps ?? [],
    benched: view.benched ?? [],
    terrain: view.terrain ?? [],
    effects: view.effects ?? [],
    opponent: { ...view.opponent, bot: view.opponent?.bot ?? false },
  };
}

export function normalizeDeckSelect(info: DeckSelectInfo): DeckSelectInfo {
  return { ...info, opponent: { ...info.opponent, bot: info.opponent?.bot ?? false } };
}

const TOKEN_KEY = "chessy.token";

export function readToken(): string | undefined {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? undefined;
  } catch {
    return undefined;
  }
}

function writeToken(token: string | null) {
  try {
    if (token === null) localStorage.removeItem(TOKEN_KEY);
    else localStorage.setItem(TOKEN_KEY, token);
  } catch {
    // Private mode: the player simply gets a fresh identity next visit.
  }
}

export const EMPTY_FRIENDS: FriendsSnapshot = { friends: [], incoming: [], outgoing: [] };

const initial: AppState = {
  forged: 0,
  connection: "connecting",
  playerId: null,
  account: null,
  deck: [],
  lobby: { type: "idle" },
  deckSelect: null,
  game: null,
  over: null,
  pendingReward: null,
  rewardOpen: false,
  friends: EMPTY_FRIENDS,
  blocked: [],
  userResults: null,
  incomingChallenge: null,
  outgoingChallenge: null,
  toasts: [],
  reveal: null,
  chat: [],
  rematch: "none",
  solo: SOLO_DEFAULT,
  soloPending: false,
  campaignGame: false,
  bossForge: null,
  errorCount: 0,
  rewardOutcome: null,
  spectating: null,
};

/** Erreurs serveur ayant un texte traduit (`errors.<code>`) ; les autres affichent le message brut du serveur. */
const KNOWN_ERRORS = new Set([
  "not_your_turn", "illegal_action", "no_such_room", "own_room", "already_in_game", "invalid_deck", "replaced",
  "session_revoked", "flooded", "queue_full", "rooms_full", "account_required", "spectate_full", "no_such_game",
  "invalid_target", "blocked", "block_list_full",
  "unknown_level", "boss_locked", "bad_deck",
]);
const errorText = (code: string, fallback: string) => (KNOWN_ERRORS.has(code) ? t(`errors.${code}`) : fallback);

/** Texte d'une notice serveur, dans la langue courante. */
export function noticeText(code: NoticeCode, username?: string): string {
  const name = username ?? t("notice.someone");
  switch (code) {
    case "friend_request_received":
    case "friend_accepted":
    case "friend_removed":
    case "challenge_declined":
    case "challenge_expired":
    case "challenge_cancelled":
    case "user_not_found":
    case "already_friends":
    case "friend_offline":
    case "rated_pair_capped":
    case "friend_busy":
      return t(`notice.${code}`, { name });
    default:
      return t("notice.default");
  }
}

/** Parties dont l'arrivée a déjà été annoncée par un son (une reconnexion renvoie `deck_select`). */
const announcedMatches = new Set<string>();

const TOAST_MS = 4500;
const CHALLENGE_MS = 60_000;
/** Au-delà, on suppose que le serveur n'a pas répondu à `solo_start` et on rend la main. */
const SOLO_PENDING_MS = 10_000;

export class Store {
  private state: AppState = { ...initial, solo: readSolo() };
  private listeners = new Set<() => void>();
  private socket: WebSocket | null = null;
  private retry = 0;
  private toastId = 0;
  private stopped = false;
  private challengeTimer: ReturnType<typeof setTimeout> | null = null;
  private soloTimer: ReturnType<typeof setTimeout> | null = null;
  private blocksAsked = false;

  constructor() {
    onForgedChange(() => this.set({ forged: forgedVersion() }));
  }

  getState = () => this.state;

  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  };

  private set(patch: Partial<AppState>) {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((fn) => fn());
  }

  private notify(text: string) {
    const id = ++this.toastId;
    this.set({ toasts: [...this.state.toasts.slice(-3), { id, text }] });
    setTimeout(() => this.dismissToast(id), TOAST_MS);
  }

  dismissReveal() {
    this.set({ reveal: null });
  }

  dismissToast(id?: number) {
    this.set({ toasts: id === undefined ? [] : this.state.toasts.filter((t) => t.id !== id) });
  }

  connect() {
    this.stopped = false;
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const socket = new WebSocket(`${scheme}://${location.host}/ws`);
    this.socket = socket;
    // Only the current socket may act: React StrictMode (and reconnects) can
    // leave a stale one around whose late events must be ignored.
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.retry = 0;
      socket.send(JSON.stringify({ type: "hello", token: readToken() } satisfies ClientMsg));
    };
    socket.onmessage = (e) => {
      if (this.socket !== socket) return;
      // Les compétences forgées citées dans le message sont décrites à part.
      noticeForged(e.data as string);
      this.receive(JSON.parse(e.data as string) as ServerMsg);
    };
    socket.onclose = () => {
      if (this.socket !== socket || this.stopped || this.state.connection === "replaced") return;
      this.set({ connection: "closed" });
      const delay = Math.min(1000 * 2 ** this.retry++, 10000);
      setTimeout(() => !this.stopped && this.socket === socket && this.connect(), delay);
    };
  }

  disconnect() {
    this.stopped = true;
    const socket = this.socket;
    this.socket = null;
    socket?.close();
  }

  /** Ferme la socket courante et en ouvre une nouvelle (le `hello` reprend le jeton stocké). */
  private reconnect() {
    const old = this.socket;
    this.socket = null;
    old?.close();
    this.set({
      connection: "connecting",
      account: null,
      friends: EMPTY_FRIENDS,
      blocked: [],
      userResults: null,
      incomingChallenge: null,
      outgoingChallenge: null,
      lobby: { type: "idle" },
      deckSelect: null,
      game: null,
      over: null,
      chat: [],
      rematch: "none",
      soloPending: false,
      rewardOutcome: null,
    });
    this.connect();
  }

  send(msg: ClientMsg) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(msg));
  }

  /** Après une connexion ou une inscription : mémorise le jeton puis reconnecte la WebSocket. */
  applyAuth(token: string) {
    writeToken(token);
    this.reconnect();
  }

  /**
   * Déconnexion : invalide la session côté serveur, efface le jeton et repart en invité.
   * Avec `everywhere`, toutes les sessions du compte sont terminées (autres appareils compris).
   */
  async logout(everywhere = false) {
    const token = readToken();
    writeToken(null);
    // Détache la socket tout de suite : le serveur la ferme à la déconnexion
    // (`session_revoked`), ce message-là ne doit pas déclencher une seconde reconnexion.
    const old = this.socket;
    this.socket = null;
    old?.close();
    if (token) {
      try {
        await fetch(everywhere ? "/api/auth/logout-all" : "/api/auth/logout", { method: "POST", headers: { Authorization: `Bearer ${token}` } });
      } catch {
        // Hors ligne : la session expirera côté serveur, le client repart en invité quand même.
      }
    }
    this.reconnect();
  }

  // ---- modération (voir docs/spec-v2.md §4) -----------------------------------

  /** Demande la liste des joueurs bloqués, une fois par connexion (le serveur ne la pousse pas). */
  loadBlocks() {
    if (this.blocksAsked) return;
    this.blocksAsked = true;
    this.send({ type: "blocks_list" });
  }

  blockUser(username: string) {
    this.send({ type: "block_user", username });
    this.notify(t("store.blocked", { name: username }));
  }

  unblockUser(username: string) {
    this.send({ type: "unblock_user", username });
  }

  setChatMuted(muted: boolean) {
    this.send({ type: "set_chat_muted", muted });
  }

  reportUser(username: string, reason: ReportReason, gameId?: string, context?: string) {
    this.send({ type: "report_user", username, reason, game_id: gameId, context: context?.slice(0, 900) });
  }

  closeReward() {
    this.set({ rewardOpen: false });
  }

  /** Passe de l'écran de victoire à l'écran de récompense. */
  openReward() {
    this.set({ rewardOpen: true });
  }

  /** Leaves a finished game and returns to the lobby. */
  leaveGame() {
    this.set({ game: null, over: null, deckSelect: null, rematch: "none", campaignGame: false, rewardOutcome: null });
  }

  /** Lance la prochaine partie d'évaluation (adversaire dont l'Elo reste caché). */
  startPlacement(color: SoloColor = "random") {
    this.send({ type: "placement_start", color });
    this.awaitSoloGame();
  }

  /** Lance une partie contre l'IA et mémorise le réglage. */
  startSolo(elo: number, color: SoloColor) {
    const solo: SoloSetting = { elo: clampElo(elo), color };
    writeSolo(solo);
    this.send({ type: "solo_start", elo: solo.elo, color: solo.color });
    this.set({ solo, campaignGame: false, rewardOutcome: null });
    this.awaitSoloGame();
  }

  /** Lance un niveau de la campagne (decks imposés par le niveau, ou `deck` choisi si le niveau le demande). */
  startCampaign(chapter: number, level: number, deck?: SkillId[]) {
    this.send({ type: "campaign_start", chapter, level, deck });
    this.set({ campaignGame: true, rewardOutcome: null });
    this.awaitSoloGame();
  }

  private awaitSoloGame() {
    this.set({ soloPending: true });
    if (this.soloTimer) clearTimeout(this.soloTimer);
    this.soloTimer = setTimeout(() => this.clearSoloPending(), SOLO_PENDING_MS);
  }

  private clearSoloPending() {
    if (this.soloTimer) clearTimeout(this.soloTimer);
    this.soloTimer = null;
    if (this.state.soloPending) this.set({ soloPending: false });
  }

  /** Regarde une partie en cours ; le serveur répond par `spectate_state` (ou une erreur d'entrée). */
  spectate(gameId: string) {
    this.set({ spectating: startSpectating(gameId) });
    this.send({ type: "spectate", game_id: gameId });
  }

  /** Quitte le mode spectateur. Les `spectate_state` encore en vol sont ignorés ensuite. */
  unspectate() {
    const current = this.state.spectating;
    if (!current) return;
    this.set({ spectating: null });
    if (isLive(current)) this.send({ type: "unspectate" });
  }

  // ---- raccourcis d'actions -------------------------------------------------

  offerDraw() {
    this.send({ type: "offer_draw" });
    if (this.state.game) this.set({ game: { ...this.state.game, draw_offer: "you" } });
  }

  respondDraw(accept: boolean) {
    this.send({ type: "respond_draw", accept });
    if (this.state.game) this.set({ game: { ...this.state.game, draw_offer: "none" } });
  }

  requestRematch() {
    this.send({ type: "rematch_request" });
    this.set({ rematch: "offered" });
  }

  respondRematch(accept: boolean) {
    this.send({ type: "rematch_respond", accept });
    this.set({ rematch: accept ? this.state.rematch : "none" });
  }

  respondChallenge(accept: boolean) {
    const challenge = this.state.incomingChallenge;
    if (!challenge) return;
    this.send({ type: "challenge_respond", username: challenge.username, accept });
    this.clearIncoming();
  }

  cancelChallenge() {
    this.send({ type: "challenge_cancel" });
    this.set({ outgoingChallenge: null });
  }

  private clearIncoming() {
    if (this.challengeTimer) clearTimeout(this.challengeTimer);
    this.challengeTimer = null;
    this.set({ incomingChallenge: null });
  }

  receive(msg: ServerMsg) {
    switch (msg.type) {
      case "welcome":
        // Un jeton invalide est remplacé par un nouveau : on le garde pour la prochaine visite.
        writeToken(msg.token);
        // Nouvelle connexion : la liste des joueurs bloqués sera redemandée (voir `loadBlocks`).
        this.blocksAsked = false;
        this.set({
          connection: "open",
          playerId: msg.player_id,
          account: msg.account ?? null,
          deck: msg.deck,
          pendingReward: msg.pending_reward,
        });
        // Après une reconnexion, le serveur a oublié le spectateur : on se réinscrit.
        if (isLive(this.state.spectating)) this.send({ type: "spectate", game_id: this.state.spectating!.gameId });
        break;
      case "friends": {
        const { type: _type, ...friends } = msg;
        this.set({ friends });
        break;
      }
      case "user_results":
        this.set({ userResults: { query: msg.query, users: msg.users } });
        break;
      case "blocks":
        this.set({ blocked: msg.blocked.map((b) => b.username) });
        break;
      case "chat_settings":
        if (this.state.account) this.set({ account: { ...this.state.account, chat_muted: msg.chat_muted } });
        break;
      case "report_ack":
        this.notify(t("store.reported"));
        break;
      case "notice":
        if (msg.code === "challenge_declined" || msg.code === "challenge_expired") this.set({ outgoingChallenge: null });
        if (msg.code === "challenge_cancelled" || msg.code === "challenge_expired") this.clearIncoming();
        sfx.play(msg.code === "friend_request_received" ? "friend_request" : "notice");
        this.notify(noticeText(msg.code, msg.username));
        break;
      case "challenge_received":
        if (this.challengeTimer) clearTimeout(this.challengeTimer);
        this.challengeTimer = setTimeout(() => this.clearIncoming(), CHALLENGE_MS);
        this.set({ incomingChallenge: msg.from });
        sfx.play("challenge");
        break;
      case "challenge_sent":
        this.set({ outgoingChallenge: msg.username });
        break;
      case "draw_offered":
        if (this.state.game) this.set({ game: { ...this.state.game, draw_offer: "them" } });
        break;
      case "draw_declined":
        if (this.state.game) this.set({ game: { ...this.state.game, draw_offer: "none" } });
        this.notify(t("store.drawDeclined"));
        break;
      case "chat":
        if (!msg.mine) sfx.play("chat");
        this.set({ chat: [...this.state.chat, { mine: msg.mine, text: msg.text }].slice(-60) });
        break;
      case "rematch_offered":
        this.set({ rematch: "received" });
        break;
      case "rematch_declined":
        this.set({ rematch: "none" });
        this.notify(t("store.rematchDeclined"));
        break;
      case "lobby":
        this.set({ lobby: msg.status });
        break;
      case "deck_select": {
        const { type: _type, ...info } = msg;
        if (!announcedMatches.has(info.game_id)) {
          announcedMatches.add(info.game_id);
          if (!info.opponent?.bot) sfx.play("match_found");
        }
        this.clearIncoming();
        this.clearSoloPending();
        this.set({
          deckSelect: normalizeDeckSelect(info),
          game: null,
          over: null,
          pendingReward: null,
          outgoingChallenge: null,
          chat: [],
          rematch: "none",
          spectating: null,
        });
        break;
      }
      case "state": {
        const { type: _type, ...view } = msg;
        this.clearSoloPending();
        this.set({ game: normalizeState(view), deckSelect: null, spectating: null });
        break;
      }
      case "spectate_state":
      case "spectate_over":
      case "spectate_ended": {
        const next = reduceSpectator(this.state.spectating, msg);
        if (next !== this.state.spectating) this.set({ spectating: next });
        break;
      }
      case "opponent_status":
        if (this.state.game) {
          this.set({ game: { ...this.state.game, opponent_connected: msg.connected } });
        }
        break;
      case "game_over":
        this.set({
          over: {
            outcome: msg.outcome,
            reward: msg.reward,
            rated: msg.rated,
            elo: msg.elo,
            reason: msg.reason,
            placement: msg.placement ?? null,
            campaign: msg.campaign ?? null,
          },
          rewardOpen: false,
          // L'Elo affiché dans la barre de navigation suit la partie classée,
          // ou l'estimation qui clôt les parties d'évaluation.
          account: this.state.account ? accountAfterGame(this.state.account, msg) : this.state.account,
        });
        break;
      case "deck_update":
        // BossForge a déjà révélé la forgée d'un boss : pas de seconde révélation.
        if (msg.gained && !skillRevealed(msg.gained)) {
          // Le nom d'une compétence forgée n'est connu qu'une fois sa définition reçue.
          const gained = msg.gained;
          void loadForged([gained]).then(() => {
            if (isForgedId(gained)) this.set({ reveal: gained });
            else this.notify(t("store.newSkill", { name: skillName(gained) }));
          });
        }
        this.set({
          deck: msg.deck,
          pendingReward: null,
          rewardOpen: false,
          over: this.state.over ? { ...this.state.over, reward: null } : null,
        });
        break;
      case "reward_outcome":
        this.set({ rewardOutcome: { by: msg.by, kind: msg.kind, skill: msg.skill, refilled: msg.refilled } });
        // Sans résultat affiché (reconnexion du perdant), l'annonce n'aurait sinon aucune trace à l'écran.
        if (!this.state.over) this.notify(`Récompense classée : ${msg.by} a une issue pour vous.`);
        break;
      case "boss_forge":
        this.set({ bossForge: msg.info });
        break;
      case "game_cancelled":
        this.clearSoloPending();
        this.set({ game: null, deckSelect: null, over: null, rematch: "none", campaignGame: false });
        sfx.play("notice");
        if (msg.reason === "opponent_left_requeued") {
          this.notify(t("store.opponentLeft"));
        } else if (msg.reason !== "you_left") {
          this.notify(t("store.gameCancelled"));
        }
        break;
      case "error":
        // Les erreurs d'entrée en mode spectateur s'affichent sur l'écran du spectateur, sans toast.
        if (this.state.spectating?.status === "joining" && SPECTATE_ERRORS.includes(msg.code)) {
          this.set({ spectating: reduceSpectator(this.state.spectating, msg) });
          break;
        }
        this.clearSoloPending();
        this.set({ errorCount: this.state.errorCount + 1 });
        if (msg.code === "session_revoked") {
          // Session terminée ailleurs (déconnexion depuis un autre onglet ou appareil) :
          // on oublie le jeton et on repart en invité, une seule fois (un invité n'a rien à révoquer).
          this.notify(t("errors.session_revoked"));
          if (readToken()) {
            writeToken(null);
            this.reconnect();
          }
          break;
        }
        if (msg.code === "replaced") this.set({ connection: "replaced" });
        if (msg.code === "illegal_action" || msg.code === "not_your_turn") sfx.play("illegal");
        this.notify(errorText(msg.code, msg.message));
        break;
    }
  }
}

export const store = new Store();
// Lets the dev console and browser checks drive the store (never in production builds).
if (import.meta.env.DEV && typeof window !== "undefined") (window as unknown as { __chessyStore: Store }).__chessyStore = store;

export function useAppState(): AppState {
  return useSyncExternalStore(store.subscribe, store.getState);
}
