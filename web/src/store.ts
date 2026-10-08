import { useSyncExternalStore } from "react";
import type {
  ClientMsg,
  DeckSelectInfo,
  EloChange,
  FriendsSnapshot,
  LobbyStatus,
  Me,
  NoticeCode,
  Outcome,
  RewardOffer,
  ServerMsg,
  SkillId,
  SoloColor,
  StateView,
  UserResult,
} from "./protocol";
import { forgedVersion, isForgedId, loadForged, noticeForged, onForgedChange } from "./forged";
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
  /** Partie regardée en tant que spectateur (v4), `null` si on ne regarde rien. */
  spectating: SpectatingState | null;
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
  userResults: null,
  incomingChallenge: null,
  outgoingChallenge: null,
  toasts: [],
  reveal: null,
  chat: [],
  rematch: "none",
  solo: SOLO_DEFAULT,
  soloPending: false,
  spectating: null,
};

const ERROR_TEXT: Record<string, string> = {
  not_your_turn: "Ce n'est pas votre tour.",
  illegal_action: "Action impossible.",
  no_such_room: "Cette salle n'existe pas.",
  own_room: "Vous ne pouvez pas rejoindre votre propre salle.",
  already_in_game: "Vous êtes déjà dans une partie.",
  invalid_deck: "Sélection de compétences invalide.",
  replaced: "Ce compte s'est connecté depuis un autre onglet.",
  session_revoked: "Votre session a pris fin : vous êtes repassé en invité.",
  flooded: "Connexion coupée : trop de messages envoyés.",
  queue_full: "La file classée est pleine, réessayez dans un instant.",
  rooms_full: "Trop de salles ouvertes, réessayez dans un instant.",
  account_required: "Un compte est nécessaire pour cette action.",
  spectate_full: "Cette partie a atteint son maximum de spectateurs.",
  no_such_game: "Cette partie n'existe pas ou est terminée.",
};

/** Texte français d'une notice serveur. */
export function noticeText(code: NoticeCode, username?: string): string {
  const who = username ?? "Ce joueur";
  switch (code) {
    case "friend_request_received":
      return `${who} vous a envoyé une demande d'ami.`;
    case "friend_accepted":
      return `${who} est maintenant votre ami.`;
    case "friend_removed":
      return `${who} a été retiré de vos amis.`;
    case "challenge_declined":
      return `${who} a refusé votre défi.`;
    case "challenge_expired":
      return "Le défi a expiré.";
    case "challenge_cancelled":
      return `${who} a annulé son défi.`;
    case "user_not_found":
      return "Joueur introuvable.";
    case "already_friends":
      return `${who} est déjà votre ami.`;
    case "friend_offline":
      return `${who} n'est pas en ligne.`;
    case "rated_pair_capped":
      return "Vous avez déjà joué 3 parties classées l'un contre l'autre cette heure : celle-ci ne compte pas pour l'Elo.";
    case "friend_busy":
      return `${who} est en pleine partie.`;
    default:
      return "Notification.";
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

  closeReward() {
    this.set({ rewardOpen: false });
  }

  /** Passe de l'écran de victoire à l'écran de récompense. */
  openReward() {
    this.set({ rewardOpen: true });
  }

  /** Leaves a finished game and returns to the lobby. */
  leaveGame() {
    this.set({ game: null, over: null, deckSelect: null, rematch: "none" });
  }

  /** Lance une partie contre l'IA et mémorise le réglage. */
  startSolo(elo: number, color: SoloColor) {
    const solo: SoloSetting = { elo: clampElo(elo), color };
    writeSolo(solo);
    this.send({ type: "solo_start", elo: solo.elo, color: solo.color });
    this.set({ solo, soloPending: true });
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
        this.notify("Votre proposition de nulle a été refusée.");
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
        this.notify("La revanche n'aura pas lieu.");
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
          over: { outcome: msg.outcome, reward: msg.reward, rated: msg.rated, elo: msg.elo, reason: msg.reason },
          rewardOpen: false,
          // L'Elo affiché dans la barre de navigation suit la partie classée.
          account:
            this.state.account && msg.elo ? { ...this.state.account, elo: msg.elo.you_after } : this.state.account,
        });
        break;
      case "deck_update":
        if (msg.gained) {
          // Le nom d'une compétence forgée n'est connu qu'une fois sa définition reçue.
          const gained = msg.gained;
          void loadForged([gained]).then(() => {
            if (isForgedId(gained)) this.set({ reveal: gained });
            else this.notify(`Nouvelle compétence : ${skillName(gained)}`);
          });
        }
        this.set({
          deck: msg.deck,
          pendingReward: null,
          rewardOpen: false,
          over: this.state.over ? { ...this.state.over, reward: null } : null,
        });
        break;
      case "game_cancelled":
        this.clearSoloPending();
        this.set({ game: null, deckSelect: null, over: null, rematch: "none" });
        sfx.play("notice");
        if (msg.reason === "opponent_left_requeued") {
          this.notify("Votre adversaire est parti : nouvelle recherche en cours…");
        } else if (msg.reason !== "you_left") {
          this.notify("La partie a été annulée.");
        }
        break;
      case "error":
        // Les erreurs d'entrée en mode spectateur s'affichent sur l'écran du spectateur, sans toast.
        if (this.state.spectating?.status === "joining" && SPECTATE_ERRORS.includes(msg.code)) {
          this.set({ spectating: reduceSpectator(this.state.spectating, msg) });
          break;
        }
        this.clearSoloPending();
        if (msg.code === "session_revoked") {
          // Session terminée ailleurs (déconnexion depuis un autre onglet ou appareil) :
          // on oublie le jeton et on repart en invité, une seule fois (un invité n'a rien à révoquer).
          this.notify(ERROR_TEXT.session_revoked);
          if (readToken()) {
            writeToken(null);
            this.reconnect();
          }
          break;
        }
        if (msg.code === "replaced") this.set({ connection: "replaced" });
        if (msg.code === "illegal_action" || msg.code === "not_your_turn") sfx.play("illegal");
        this.notify(ERROR_TEXT[msg.code] ?? msg.message);
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
