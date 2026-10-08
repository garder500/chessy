//! All live state: connections, lobby, running games and pending rewards.
//!
//! The hub is synchronous and never awaits, so it lives behind one mutex
//! (see [`crate::app::App`]). Anything that must happen later is queued as a
//! [`Timer`] for the caller to schedule. Friends, challenges, chat and
//! rematches live in the `social` submodule.

mod social;
mod solo;
mod spectate;
mod view;

pub use spectate::{LiveGame, LiveSeat, SpectatorView, UsedSkills, MAX_SPECTATORS};

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chessy_engine::{Action, Color, Game, Outcome, SkillId, SkillKind, SkillTarget};
use rand::seq::{IndexedRandom, SliceRandom};
use tokio::sync::mpsc::UnboundedSender;

pub use crate::games_store::GameKind;
use crate::limits::{RateLimiter, Verdict};
use crate::protocol::*;
use crate::store::{reason_of, GameRecord, Store, StoreError};

pub const MAX_DECK: usize = 7;
pub const MAX_PICKS: usize = 3;
/// A game shorter than this many plies is never rated.
pub const MIN_RATED_PLIES: u32 = 4;

#[derive(Clone, Copy, Debug)]
pub struct HubConfig {
    /// How long a disconnected player has to come back before forfeiting.
    pub reconnect_grace: Duration,
    /// How long players have to choose their skills.
    pub deck_select_time: Duration,
    /// Time each player starts with.
    pub clock_initial: Duration,
    /// Time added after each action (move or skill).
    pub clock_increment: Duration,
    /// How often the ranked queue is re-examined while someone waits.
    pub queue_sweep_interval: Duration,
    /// Ranked matching: Elo gap allowed at once, how much it widens per
    /// second of waiting, and its ceiling.
    pub ranked_range_base: i32,
    pub ranked_range_per_second: f64,
    pub ranked_range_max: i32,
    /// How long a friend challenge stays open.
    pub challenge_ttl: Duration,
    /// Minimum time between two chat messages from one player.
    pub chat_interval: Duration,
    /// Solo: the bot waits a random time in this range before it starts to think.
    pub bot_delay_min: Duration,
    pub bot_delay_max: Duration,
    /// Solo: the longest the bot's search may run (its level asks for less at low Elo).
    pub bot_think_max: Duration,
    /// Solo: the bot accepts a draw offer only after this many actions...
    pub bot_draw_min_plies: u32,
    /// ...and when its evaluation is within this many centipawns of equal.
    pub bot_draw_window: i32,
    /// Spectators of a game between people see it this much late (anti-cheat).
    /// Solo games are always shown live.
    pub spectator_delay: Duration,
    /// Per-connection message quota: tokens refilled per second and burst size.
    /// One message costs one token, the heavier ones `expensive_cost`.
    pub msg_rate: f64,
    pub msg_burst: u32,
    pub expensive_cost: u32,
    /// A connection that stays over quota for this many messages in a row is closed.
    pub flood_disconnect_after: u32,
    /// A connection with more than this many server messages waiting to be
    /// written is a slow consumer and is closed (enforced in `ws`).
    pub outbound_queue_cap: usize,
    /// A single socket write that takes longer than this closes the connection.
    pub write_timeout: Duration,
    /// Most players waiting in the ranked queue, in the casual queue and in
    /// rooms (each); the ranked matching is quadratic in its queue.
    pub lobby_cap: usize,
    /// How long a won reward can wait to be claimed.
    pub reward_ttl: Duration,
    /// Rated games allowed between the same two accounts within
    /// `rated_pair_window`; further games between them are unrated.
    pub rated_pair_max: u32,
    pub rated_pair_window: Duration,
    /// The cap above is for a busy server. With few accounts online it widens in
    /// proportion: `rated_pair_max * rated_pair_scale / (accounts online - 1)`, never
    /// below `rated_pair_max`. Two players alone get `rated_pair_max * rated_pair_scale`
    /// games, and `1` turns the scaling off.
    pub rated_pair_scale: u32,
    /// A pair that reached `rated_pair_max` is left waiting for someone else in
    /// the ranked queue, but not for ever: with only two players around, after
    /// this wait they are paired anyway, and the game does not count.
    pub capped_pair_wait: Duration,
    /// Clock time charged for an action the player was offered but the real
    /// position refuses (a hidden piece was in the way): trying moves at random
    /// to find hidden pieces is not free. Games without a clock are not charged.
    pub blocked_attempt_cost: Duration,
    /// A session unused for this long is expired: its token no longer resolves
    /// (REST and WebSocket hello) and the periodic purge deletes it.
    pub session_ttl: Duration,
    /// `last_used` is refreshed at most this often per session (a write per
    /// request would be wasteful; the TTL is far longer than this).
    pub session_touch_interval: Duration,
    /// How often the purge task deletes expired sessions (see `spawn_session_purge`).
    pub session_purge_interval: Duration,
}

impl Default for HubConfig {
    fn default() -> Self {
        HubConfig {
            reconnect_grace: Duration::from_secs(60),
            deck_select_time: Duration::from_secs(60),
            clock_initial: Duration::from_secs(600),
            clock_increment: Duration::from_secs(3),
            queue_sweep_interval: Duration::from_secs(3),
            ranked_range_base: 100,
            ranked_range_per_second: 25.0,
            ranked_range_max: 800,
            challenge_ttl: Duration::from_secs(60),
            chat_interval: Duration::from_secs(1),
            bot_delay_min: Duration::from_millis(600),
            bot_delay_max: Duration::from_millis(1400),
            bot_think_max: Duration::from_secs(3),
            bot_draw_min_plies: crate::bot::DRAW_MIN_PLIES,
            bot_draw_window: crate::bot::DRAW_WINDOW,
            spectator_delay: Duration::from_secs(30),
            msg_rate: 20.0,
            msg_burst: 40,
            expensive_cost: 4,
            flood_disconnect_after: 100,
            outbound_queue_cap: 1000,
            write_timeout: Duration::from_secs(10),
            lobby_cap: 1000,
            reward_ttl: Duration::from_secs(6 * 3600),
            rated_pair_max: 3,
            rated_pair_window: Duration::from_secs(3600),
            rated_pair_scale: 10,
            capped_pair_wait: Duration::from_secs(20),
            blocked_attempt_cost: Duration::from_secs(10),
            session_ttl: Duration::from_secs(30 * 24 * 3600),
            session_touch_interval: Duration::from_secs(3600),
            session_purge_interval: Duration::from_secs(3600),
        }
    }
}

impl HubConfig {
    /// Largest Elo gap accepted between two ranked players when the older of
    /// them has waited `wait`.
    pub fn ranked_range(&self, wait: Duration) -> i32 {
        let widened =
            f64::from(self.ranked_range_base) + self.ranked_range_per_second * wait.as_secs_f64();
        widened.min(f64::from(self.ranked_range_max)) as i32
    }
}

#[derive(Debug)]
pub enum Timer {
    Forfeit {
        game_id: String,
        color: Color,
        epoch: u64,
    },
    DeckTimeout {
        game_id: String,
    },
    /// `color` runs out of time unless the game has moved past `ply`.
    Flag {
        game_id: String,
        color: Color,
        ply: u32,
    },
    QueueSweep,
    /// The bot plays unless the game has moved past `ply`. Handled by
    /// [`crate::app::App`], which runs the search off the hub lock.
    BotMove {
        game_id: String,
        ply: u32,
    },
    ChallengeExpire {
        challenger: PlayerId,
        target: PlayerId,
        seq: u64,
    },
    /// Sends the spectators of a game the views whose delay is over.
    SpectatorFlush {
        game_id: String,
    },
}

struct Conn {
    id: u64,
    tx: UnboundedSender<ServerMsg>,
    /// The session token this connection authenticated with.
    token: String,
    limiter: RateLimiter,
}

enum Phase {
    DeckSelect { picks: [Option<Vec<SkillId>>; 2] },
    Playing { game: Box<Game> },
}

/// Each player's remaining time; only the side to move is running.
struct Clock {
    remaining: [Duration; 2],
    since: Instant,
    running: Option<Color>,
}

impl Clock {
    fn left(&self, color: Color, now: Instant) -> Duration {
        let left = self.remaining[color.index()];
        if self.running == Some(color) {
            left.saturating_sub(now.saturating_duration_since(self.since))
        } else {
            left
        }
    }

    fn expired(&self, color: Color, now: Instant) -> bool {
        self.running == Some(color) && self.left(color, now).is_zero()
    }

    /// `mover` has acted: charge their time, add the increment, hand over.
    fn press(&mut self, mover: Color, now: Instant, increment: Duration, over: bool) {
        self.remaining[mover.index()] = self.left(mover, now) + increment;
        self.since = now;
        self.running = if over { None } else { Some(mover.opposite()) };
    }

    /// Takes `cost` off `color`'s time (they stay on the move).
    fn charge(&mut self, color: Color, cost: Duration, now: Instant) {
        self.remaining[color.index()] = self.left(color, now).saturating_sub(cost);
        self.since = now;
    }

    fn view(&self, now: Instant) -> ClockView {
        ClockView {
            white_ms: self.left(Color::White, now).as_millis() as u64,
            black_ms: self.left(Color::Black, now).as_millis() as u64,
            running: self.running,
        }
    }
}

/// What a session keeps so the finished game can be recorded and replayed.
struct Recording {
    kind: GameKind,
    /// Every action applied so far, skills that keep the turn included.
    actions: Vec<Action>,
}

struct Session {
    /// Indexed by `Color::index()`.
    players: [PlayerId; 2],
    phase: Phase,
    connected: [bool; 2],
    /// Bumped on every disconnect so stale forfeit timers can be ignored.
    epoch: [u64; 2],
    /// Came from the ranked queue between two accounts; whether Elo really
    /// moves also depends on how long the game lasts.
    rated: bool,
    /// Game length both players asked for (`None`: the server default).
    time: Option<TimeControl>,
    /// Who plays each colour, as shown to the other side.
    info: [OpponentInfo; 2],
    started_unix: i64,
    clock: Option<Clock>,
    draw_offer: Option<Color>,
    /// The ply at which each colour last offered a draw (one offer per ply).
    last_offer_ply: [Option<u32>; 2],
    /// Set for a Solo game: one seat is the bot (its `players` entry is a
    /// synthetic id that is never connected and has no account).
    solo: Option<solo::Solo>,
    recording: Recording,
    /// Every action as each colour saw it (indexed by `Color::index()`), so a
    /// player who reloads the page gets their journal back.
    history: [Vec<HistoryEntry>; 2],
}

impl Session {
    fn color_of(&self, player: &str) -> Option<Color> {
        Color::BOTH
            .into_iter()
            .find(|c| self.players[c.index()] == player)
    }
}

/// What the forge needs to make a skill off the hub lock (see
/// [`Hub::begin_forge`]).
pub struct ForgeJob {
    pub player: PlayerId,
    /// The skill to drop if the deck is full.
    pub replace: Option<SkillId>,
    /// The rarity the draw asked for.
    pub target: chessy_engine::forge::Rarity,
    pub seed: u64,
    /// Signatures already in the world: a skill that repeats one is Common.
    pub known: std::collections::HashSet<String>,
    pub store: Store,
}

struct PendingReward {
    /// A skill is being forged for the "random" choice: ignore further choices.
    forging: bool,
    loser: PlayerId,
    /// What the loser owned when the game ended: the only skills the winner
    /// may take (and only those the loser still owns when they claim).
    loser_deck: Vec<SkillId>,
    created: Instant,
}

struct QueueEntry {
    player: PlayerId,
    time: Option<TimeControl>,
    elo: i32,
    since: Instant,
}

pub struct Hub {
    store: Store,
    config: HubConfig,
    conns: HashMap<PlayerId, Conn>,
    next_conn: u64,
    next_game: u64,
    /// Oldest first.
    ranked_queue: Vec<QueueEntry>,
    casual_queue: VecDeque<(PlayerId, Option<TimeControl>)>,
    sweep_pending: bool,
    rooms: HashMap<String, (PlayerId, Option<TimeControl>)>,
    games: HashMap<String, Session>,
    player_game: HashMap<PlayerId, String>,
    rewards: HashMap<PlayerId, PendingReward>,
    /// Open challenges by challenger.
    challenges: HashMap<PlayerId, social::Challenge>,
    next_challenge: u64,
    /// Players who just finished a game and may still ask for a rematch.
    rematches: HashMap<PlayerId, social::Rematch>,
    last_chat: HashMap<PlayerId, Instant>,
    timers: Vec<(Duration, Timer)>,
    /// Spectators and delayed views per running game (see `spectate`).
    feeds: HashMap<String, spectate::Feed>,
    /// The game each spectator watches.
    watching: HashMap<PlayerId, String>,
}

fn room_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..5)
        .map(|_| *ALPHABET.choose(&mut rng).expect("alphabet is not empty") as char)
        .collect()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Hub {
    pub fn new(store: Store, config: HubConfig) -> Self {
        Hub {
            store,
            config,
            conns: HashMap::new(),
            next_conn: 0,
            next_game: 0,
            ranked_queue: Vec::new(),
            casual_queue: VecDeque::new(),
            sweep_pending: false,
            rooms: HashMap::new(),
            games: HashMap::new(),
            player_game: HashMap::new(),
            rewards: HashMap::new(),
            challenges: HashMap::new(),
            next_challenge: 0,
            rematches: HashMap::new(),
            last_chat: HashMap::new(),
            timers: Vec::new(),
            feeds: HashMap::new(),
            watching: HashMap::new(),
        }
    }

    pub fn take_timers(&mut self) -> Vec<(Duration, Timer)> {
        std::mem::take(&mut self.timers)
    }

    fn send(&self, player: &str, msg: ServerMsg) {
        if let Some(conn) = self.conns.get(player) {
            let _ = conn.tx.send(msg);
        }
    }

    fn fail(&self, player: &str, code: &str, message: &str) {
        self.send(player, ServerMsg::error(code, message));
    }

    fn internal_error(&self, player: &str, err: StoreError) {
        tracing::error!("store error: {err}");
        self.fail(player, "internal", "internal error");
    }

    fn deck_of(&self, player: &str) -> Result<Vec<SkillId>, StoreError> {
        self.store.deck(player)
    }

    /// What the opponent sees of `player`.
    fn opponent_info(&self, player: &str) -> OpponentInfo {
        match self.store.player_row(player) {
            Ok(Some(row)) if row.username.is_some() => OpponentInfo {
                username: row.username,
                elo: Some(row.elo),
                guest: false,
                bot: false,
            },
            _ => OpponentInfo {
                username: None,
                elo: None,
                guest: true,
                bot: false,
            },
        }
    }

    // ---- connections -----------------------------------------------------

    /// Registers a connection, replacing any previous one for the same player.
    /// `token` is a session token. Returns the player id and the connection
    /// id used to ignore stale sockets.
    pub fn connect(
        &mut self,
        token: Option<String>,
        tx: UnboundedSender<ServerMsg>,
    ) -> Result<(PlayerId, u64), StoreError> {
        let known = match &token {
            Some(t) => self.store.session_player(
                t,
                self.config.session_ttl,
                self.config.session_touch_interval,
            )?,
            None => None,
        };
        let (id, token) = match (known, token) {
            (Some(id), Some(token)) => (id, token),
            _ => self.store.create_player()?,
        };
        self.next_conn += 1;
        let conn_id = self.next_conn;
        let conn = Conn {
            id: conn_id,
            tx,
            token: token.clone(),
            limiter: RateLimiter::new(self.config.msg_rate, self.config.msg_burst, Instant::now()),
        };
        if let Some(old) = self.conns.insert(id.clone(), conn) {
            let _ = old.tx.send(ServerMsg::error(
                "replaced",
                "this account connected from somewhere else",
            ));
        }
        let _ = self.store.touch_last_seen(&id);
        let pending_reward = match self.live_reward(&id) {
            Some(r) => self.offer_for(&id, &r.loser, &r.loser_deck).ok(),
            None => None,
        };
        let account = self
            .store
            .me(&id)?
            .ok_or(StoreError::Invalid("player vanished"))?;
        self.send(
            &id,
            ServerMsg::Welcome {
                player_id: id.clone(),
                token,
                deck: self.deck_of(&id)?,
                pending_reward,
                account,
            },
        );
        self.push_friends(&id);
        self.resume(&id);
        self.spectate_resume(&id);
        self.notify_presence(&id);
        Ok((id, conn_id))
    }

    pub fn is_current(&self, player: &str, conn_id: u64) -> bool {
        self.conns.get(player).is_some_and(|c| c.id == conn_id)
    }

    pub fn is_connected(&self, player: &str) -> bool {
        self.conns.contains_key(player)
    }

    /// Charges one incoming message to the connection's quota. Returns false
    /// when it must be ignored: stale connection, or over quota (the sender
    /// is told once per streak; a flood closes the connection).
    pub fn admit(&mut self, player: &str, conn_id: u64, cost: u32) -> bool {
        let flood_after = self.config.flood_disconnect_after;
        let Some(conn) = self.conns.get_mut(player).filter(|c| c.id == conn_id) else {
            return false;
        };
        match conn.limiter.take(cost, Instant::now(), flood_after) {
            Verdict::Allow => true,
            Verdict::Drop { first } => {
                if first {
                    self.fail(player, "rate_limited", "you are sending messages too fast");
                }
                false
            }
            Verdict::Disconnect => {
                self.fail(player, "flooded", "too many messages: disconnected");
                self.disconnect(player, conn_id);
                false
            }
        }
    }

    /// A session was ended (logout): the connection that authenticated with
    /// it is told and dropped like any other disconnect.
    pub fn revoke_session(&mut self, token: &str) {
        let found = self
            .conns
            .iter()
            .find(|(_, c)| c.token == token)
            .map(|(player, c)| (player.clone(), c.id));
        if let Some((player, conn_id)) = found {
            self.fail(
                &player,
                "session_revoked",
                "this session was ended: sign in again",
            );
            self.disconnect(&player, conn_id);
        }
    }

    /// Brings a (re)connected player up to date with wherever they were.
    fn resume(&mut self, player: &str) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            self.send(
                player,
                ServerMsg::Lobby {
                    status: self.lobby_status(player),
                },
            );
            return;
        };
        let Some(session) = self.games.get_mut(&game_id) else {
            return;
        };
        let color = session
            .color_of(player)
            .expect("player is in their session");
        session.connected[color.index()] = true;
        let opponent = session.players[color.opposite().index()].clone();
        self.send(&opponent, ServerMsg::OpponentStatus { connected: true });
        self.send_session_to(&game_id, player);
    }

    /// Sends a player whatever picture of their game they need right now.
    fn send_session_to(&self, game_id: &str, player: &str) {
        let Some(session) = self.games.get(game_id) else {
            return;
        };
        let Some(color) = session.color_of(player) else {
            return;
        };
        match &session.phase {
            Phase::DeckSelect { picks } => {
                let Ok(deck) = self.deck_of(player) else {
                    return;
                };
                self.send(
                    player,
                    ServerMsg::DeckSelect {
                        game_id: game_id.to_string(),
                        you: color,
                        deck,
                        max_picks: MAX_PICKS,
                        seconds: self.config.deck_select_time.as_secs(),
                        submitted: picks[color.index()].is_some(),
                        opponent: session.info[color.opposite().index()].clone(),
                        rated: session.rated,
                    },
                );
            }
            Phase::Playing { game } => {
                let spectators = self.watcher_count(game_id);
                let mut view = state_view(game_id, session, game, color, Vec::new(), spectators);
                view.history = session.history[color.index()].clone();
                self.send(player, ServerMsg::State(Box::new(view)));
            }
        }
    }

    pub fn disconnect(&mut self, player: &str, conn_id: u64) {
        if !self.is_current(player, conn_id) {
            return;
        }
        self.conns.remove(player);
        self.last_chat.remove(player);
        self.spectate_leave(player);
        self.leave_lobby_silently(player);
        self.drop_challenges(player);
        self.drop_rematch(player, true);
        let _ = self.store.touch_last_seen(player);
        self.disconnect_from_game(player);
        self.notify_presence(player);
    }

    fn disconnect_from_game(&mut self, player: &str) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return;
        };
        let Some(session) = self.games.get_mut(&game_id) else {
            return;
        };
        let color = session
            .color_of(player)
            .expect("player is in their session");
        session.connected[color.index()] = false;
        session.epoch[color.index()] += 1;
        let epoch = session.epoch[color.index()];
        let opponent = session.players[color.opposite().index()].clone();
        self.send(&opponent, ServerMsg::OpponentStatus { connected: false });
        self.timers.push((
            self.config.reconnect_grace,
            Timer::Forfeit {
                game_id,
                color,
                epoch,
            },
        ));
    }

    pub fn on_timer(&mut self, timer: Timer) {
        match timer {
            Timer::Forfeit {
                game_id,
                color,
                epoch,
            } => {
                let Some(session) = self.games.get(&game_id) else {
                    return;
                };
                if session.connected[color.index()] || session.epoch[color.index()] != epoch {
                    return;
                }
                match session.phase {
                    Phase::Playing { .. } => self.end_by_resignation(&game_id, color, "disconnect"),
                    Phase::DeckSelect { .. } => self.cancel_session(&game_id, "opponent left"),
                }
            }
            Timer::DeckTimeout { game_id } => {
                let Some(Session {
                    phase: Phase::DeckSelect { picks },
                    players,
                    ..
                }) = self.games.get(&game_id)
                else {
                    return;
                };
                let missing: Vec<Color> = Color::BOTH
                    .into_iter()
                    .filter(|c| picks[c.index()].is_none())
                    .collect();
                let players = players.clone();
                for color in missing {
                    let auto = self.auto_pick(&players[color.index()]);
                    self.set_picks(&game_id, color, auto);
                }
                self.start_if_ready(&game_id);
            }
            Timer::Flag {
                game_id,
                color,
                ply,
            } => {
                let Some(Session {
                    phase: Phase::Playing { game },
                    ..
                }) = self.games.get(&game_id)
                else {
                    return;
                };
                if game.outcome().is_over() || game.pos.ply != ply || game.side_to_move() != color {
                    return;
                }
                self.end_by_timeout(&game_id, color);
            }
            // The app runs the bot's search off the lock (see `App::fire`).
            Timer::BotMove { .. } => {}
            Timer::QueueSweep => {
                self.sweep_pending = false;
                self.match_ranked();
                self.ensure_sweep();
            }
            Timer::ChallengeExpire {
                challenger,
                target,
                seq,
            } => self.expire_challenge(&challenger, &target, seq),
            Timer::SpectatorFlush { game_id } => self.spectate_flush(&game_id),
        }
    }

    // ---- lobby -----------------------------------------------------------

    fn lobby_status(&self, player: &str) -> LobbyStatus {
        if self.ranked_queue.iter().any(|e| e.player == player) {
            return LobbyStatus::Queued { ranked: true };
        }
        if self.casual_queue.iter().any(|(p, _)| p == player) {
            return LobbyStatus::Queued { ranked: false };
        }
        match self.rooms.iter().find(|(_, (p, _))| p == player) {
            Some((code, _)) => LobbyStatus::RoomWaiting { code: code.clone() },
            None => LobbyStatus::Idle,
        }
    }

    fn leave_lobby_silently(&mut self, player: &str) {
        self.ranked_queue.retain(|e| e.player != player);
        self.casual_queue.retain(|(p, _)| p != player);
        self.rooms.retain(|_, (p, _)| p != player);
    }

    /// Checks the player may start looking for a game; clears any unclaimed
    /// reward (starting a new game forfeits it), any rematch they were
    /// weighing and any previous lobby spot.
    fn enter_lobby(&mut self, player: &str) -> bool {
        if self.player_game.contains_key(player) {
            self.fail(player, "already_in_game", "finish your current game first");
            return false;
        }
        self.rewards.remove(player);
        self.drop_rematch(player, true);
        self.leave_lobby_silently(player);
        self.spectate_leave(player);
        true
    }

    pub fn queue_join(&mut self, player: &str, ranked: Option<bool>, time: Option<TimeControl>) {
        if !self.enter_lobby(player) {
            return;
        }
        let account = match self.store.player_row(player) {
            Ok(Some(row)) if row.username.is_some() => Some(row),
            Ok(_) => None,
            Err(e) => return self.internal_error(player, e),
        };
        match account {
            Some(row) if ranked.unwrap_or(true) => {
                if self.ranked_queue.len() >= self.config.lobby_cap {
                    return self.fail(player, "queue_full", "the ranked queue is full: try again");
                }
                self.ranked_queue.push(QueueEntry {
                    player: player.to_string(),
                    time,
                    elo: row.elo,
                    since: Instant::now(),
                });
                self.match_ranked();
                self.ensure_sweep();
            }
            _ => match self.casual_queue.iter().position(|(_, t)| *t == time) {
                Some(i) => {
                    let (other, _) = self.casual_queue.remove(i).expect("position is valid");
                    self.create_game(other, player.to_string(), false, GameKind::Duel, time)
                }
                None => self.casual_queue.push_back((player.to_string(), time)),
            },
        }
        if self.player_game.contains_key(player) {
            return; // matched straight away: deck_select is on its way
        }
        self.send(
            player,
            ServerMsg::Lobby {
                status: self.lobby_status(player),
            },
        );
    }

    /// Repeatedly pairs the two waiting ranked players closest in Elo among
    /// those whose gap is within the range the older one's wait has earned
    /// (ties go to whoever has waited longest). A pair that already played its
    /// share of rated games waits for someone else for `capped_pair_wait`.
    fn match_ranked(&mut self) {
        let now = Instant::now();
        // Pairs that already played their share of rated games against each
        // other: left waiting for someone else (see `pair_capped`).
        let mut refused: Vec<(PlayerId, PlayerId)> = Vec::new();
        loop {
            let mut best: Option<(usize, usize, i32)> = None;
            for (i, a) in self.ranked_queue.iter().enumerate() {
                for (j, b) in self.ranked_queue.iter().enumerate().skip(i + 1) {
                    if a.time != b.time {
                        continue;
                    }
                    if refused.iter().any(|(x, y)| {
                        (x == &a.player && y == &b.player) || (x == &b.player && y == &a.player)
                    }) {
                        continue;
                    }
                    let gap = (a.elo - b.elo).abs();
                    let wait = now.saturating_duration_since(a.since.min(b.since));
                    if gap <= self.config.ranked_range(wait) && best.is_none_or(|(_, _, g)| gap < g)
                    {
                        best = Some((i, j, gap));
                    }
                }
            }
            let Some((i, j, _)) = best else { return };
            let (pa, pb) = (&self.ranked_queue[i].player, &self.ranked_queue[j].player);
            if self.pair_capped(pa, pb) {
                // Someone else is preferred, but two people alone in the queue
                // are not left waiting for ever: they play an unrated game.
                let since = self.ranked_queue[i].since.max(self.ranked_queue[j].since);
                if now.saturating_duration_since(since) < self.config.capped_pair_wait {
                    refused.push((pa.clone(), pb.clone()));
                    continue;
                }
            }
            let second = self.ranked_queue.remove(j);
            let first = self.ranked_queue.remove(i);
            self.create_game(
                first.player,
                second.player,
                true,
                GameKind::Duel,
                first.time,
            );
        }
    }

    /// Keeps a sweep timer running while anyone waits in the ranked queue.
    fn ensure_sweep(&mut self) {
        if !self.sweep_pending && !self.ranked_queue.is_empty() {
            self.sweep_pending = true;
            self.timers
                .push((self.config.queue_sweep_interval, Timer::QueueSweep));
        }
    }

    pub fn create_room(&mut self, player: &str, time: Option<TimeControl>) {
        if !self.enter_lobby(player) {
            return;
        }
        if self.rooms.len() >= self.config.lobby_cap {
            return self.fail(player, "rooms_full", "too many open rooms: try again");
        }
        let mut code = room_code();
        while self.rooms.contains_key(&code) {
            code = room_code();
        }
        self.rooms.insert(code.clone(), (player.to_string(), time));
        self.send(
            player,
            ServerMsg::Lobby {
                status: LobbyStatus::RoomWaiting { code },
            },
        );
    }

    pub fn join_room(&mut self, player: &str, code: &str) {
        if self.player_game.contains_key(player) {
            self.fail(player, "already_in_game", "finish your current game first");
            return;
        }
        let code = code.trim().to_ascii_uppercase();
        match self.rooms.get(&code) {
            None => self.fail(player, "no_such_room", "that room does not exist"),
            Some((host, _)) if host == player => {
                self.fail(player, "own_room", "you cannot join your own room")
            }
            Some(_) => {
                let (host, time) = self.rooms.remove(&code).expect("room checked above");
                self.leave_lobby_silently(player);
                self.create_game(host, player.to_string(), false, GameKind::Room, time);
            }
        }
    }

    pub fn leave_lobby(&mut self, player: &str) {
        self.leave_lobby_silently(player);
        self.send(
            player,
            ServerMsg::Lobby {
                status: LobbyStatus::Idle,
            },
        );
    }

    // ---- game setup ------------------------------------------------------

    /// Starts a game between two players with random colours.
    fn create_game(
        &mut self,
        a: PlayerId,
        b: PlayerId,
        rated: bool,
        kind: GameKind,
        time: Option<TimeControl>,
    ) {
        let (white, black) = if rand::random_bool(0.5) {
            (a, b)
        } else {
            (b, a)
        };
        self.start_session(white, black, rated, kind, time);
    }

    /// Opens deck selection for two players, clearing whatever else they
    /// were doing: lobby spots, challenges, rematches and unclaimed rewards.
    fn start_session(
        &mut self,
        white: PlayerId,
        black: PlayerId,
        rated: bool,
        kind: GameKind,
        time: Option<TimeControl>,
    ) {
        self.open_session(white, black, rated, kind, None, time);
    }

    /// [`Self::start_session`], optionally against the bot: for a Solo game
    /// the bot's seat in `white`/`black` is left empty and filled with a
    /// synthetic id here. The bot has already chosen its skills.
    fn open_session(
        &mut self,
        mut white: PlayerId,
        mut black: PlayerId,
        rated: bool,
        kind: GameKind,
        solo: Option<solo::Solo>,
        time: Option<TimeControl>,
    ) {
        self.next_game += 1;
        let game_id = format!(
            "g{}-{:06x}",
            self.next_game,
            rand::random::<u32>() & 0xff_ffff
        );
        let mut seat_solo = None;
        let mut picks = [None, None];
        if let Some(seat) = solo {
            let id = crate::bot::bot_id(&game_id);
            match seat.bot {
                Color::White => white = id,
                Color::Black => black = id,
            }
            picks[seat.bot.index()] = Some(crate::bot::pick_deck());
            seat_solo = Some(seat);
        }
        let pair = [white.clone(), black.clone()];
        let humans: Vec<&PlayerId> = pair.iter().filter(|p| !crate::bot::is_bot_id(p)).collect();
        // Rematches and queue pairings between the same two accounts stop
        // counting for Elo (and rewards) once they pile up.
        let capped = rated && humans.len() == 2 && self.pair_capped(&white, &black);
        let rated = rated && !capped;
        if capped {
            for player in &humans {
                self.send(player, ServerMsg::notice("rated_pair_capped", None));
            }
        }
        for player in &humans {
            self.rewards.remove(*player);
            self.leave_lobby_silently(player);
            self.spectate_leave(player);
            self.drop_challenges(player);
            if let Some(r) = self.rematches.get(*player) {
                // Rematching each other is not a departure.
                let silent = pair.contains(&r.opponent);
                self.drop_rematch(player, !silent);
            }
        }
        let info = |hub: &Hub, color: Color| match &seat_solo {
            Some(s) if s.bot == color => crate::bot::info(s.elo),
            _ => hub.opponent_info(&pair[color.index()]),
        };
        let connected = [
            seat_solo.as_ref().is_some_and(|s| s.bot == Color::White)
                || self.conns.contains_key(&white),
            seat_solo.as_ref().is_some_and(|s| s.bot == Color::Black)
                || self.conns.contains_key(&black),
        ];
        let session = Session {
            info: [info(self, Color::White), info(self, Color::Black)],
            players: [white.clone(), black.clone()],
            phase: Phase::DeckSelect { picks },
            connected,
            epoch: [0, 0],
            rated,
            time,
            started_unix: now_unix(),
            clock: None,
            draw_offer: None,
            last_offer_ply: [None, None],
            recording: Recording {
                kind: if seat_solo.is_some() {
                    GameKind::Solo
                } else {
                    kind
                },
                actions: Vec::new(),
            },
            solo: seat_solo,
            history: [Vec::new(), Vec::new()],
        };
        for player in &humans {
            self.player_game.insert((*player).clone(), game_id.clone());
        }
        self.games.insert(game_id.clone(), session);
        self.timers.push((
            self.config.deck_select_time,
            Timer::DeckTimeout {
                game_id: game_id.clone(),
            },
        ));
        for player in &humans {
            self.send_session_to(&game_id, player);
        }
        for player in &humans {
            self.notify_presence(player);
        }
    }

    pub fn select_deck(&mut self, player: &str, skills: Vec<SkillId>) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return self.fail(player, "not_in_game", "you are not in a game");
        };
        let Some(session) = self.games.get(&game_id) else {
            return;
        };
        let Phase::DeckSelect { picks } = &session.phase else {
            return self.fail(player, "wrong_phase", "the game has already started");
        };
        let color = session
            .color_of(player)
            .expect("player is in their session");
        if picks[color.index()].is_some() {
            return self.fail(player, "already_selected", "you already chose your skills");
        }
        let deck = match self.deck_of(player) {
            Ok(d) => d,
            Err(e) => return self.internal_error(player, e),
        };
        let mut seen = Vec::new();
        let valid = skills.len() <= MAX_PICKS
            && skills.iter().all(|s| {
                let fresh = !seen.contains(s);
                seen.push(*s);
                fresh && deck.contains(s) && s.kind() == SkillKind::Classic
            });
        if !valid {
            return self.fail(
                player,
                "invalid_deck",
                "pick up to three distinct classic skills from your deck",
            );
        }
        let mut loadout = skills;
        loadout.extend(deck.iter().filter(|s| s.kind() == SkillKind::Unique));
        self.set_picks(&game_id, color, loadout);
        self.start_if_ready(&game_id);
    }

    fn set_picks(&mut self, game_id: &str, color: Color, loadout: Vec<SkillId>) {
        if let Some(Session {
            phase: Phase::DeckSelect { picks },
            ..
        }) = self.games.get_mut(game_id)
        {
            picks[color.index()] = Some(loadout);
        }
    }

    /// Up to three random classic skills plus all unique ones.
    fn auto_pick(&self, player: &str) -> Vec<SkillId> {
        let deck = self.deck_of(player).unwrap_or_default();
        let mut classic: Vec<SkillId> = deck
            .iter()
            .copied()
            .filter(|s| s.kind() == SkillKind::Classic)
            .collect();
        classic.shuffle(&mut rand::rng());
        classic.truncate(MAX_PICKS);
        classic.extend(deck.iter().filter(|s| s.kind() == SkillKind::Unique));
        classic
    }

    fn start_if_ready(&mut self, game_id: &str) {
        let Some(session) = self.games.get_mut(game_id) else {
            return;
        };
        let Phase::DeckSelect { picks } = &session.phase else {
            return;
        };
        let (Some(white), Some(black)) = (&picks[0], &picks[1]) else {
            // Tell the player who picked that we are waiting for the other.
            let players = session.players.clone();
            for player in players {
                self.send_session_to(game_id, &player);
            }
            return;
        };
        let game = Game::new(white, black);
        session.phase = Phase::Playing {
            game: Box::new(game),
        };
        // A Solo game has no clock.
        if session.solo.is_none() {
            let initial = session
                .time
                .map_or(self.config.clock_initial, TimeControl::initial);
            session.clock = Some(Clock {
                remaining: [initial; 2],
                since: Instant::now(),
                running: Some(Color::White),
            });
            self.timers.push((
                initial,
                Timer::Flag {
                    game_id: game_id.to_string(),
                    color: Color::White,
                    ply: 0,
                },
            ));
        }
        self.spectate_open(game_id);
        self.broadcast_state(game_id, Vec::new());
        self.schedule_bot(game_id);
    }

    /// A player backs out of deck selection. The game is dropped for them; a
    /// matchmaking opponent is put back in the queue they came from, anyone
    /// else (room, challenge, rematch) just sees the game cancelled.
    pub fn leave_deck_select(&mut self, player: &str) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return;
        };
        let Some(session) = self.games.get(&game_id) else {
            return;
        };
        if !matches!(session.phase, Phase::DeckSelect { .. }) {
            return self.fail(player, "not_in_deck_select", "the game has already started");
        }
        let requeue = session.solo.is_none() && session.recording.kind == GameKind::Duel;
        let rated = session.rated;
        let time = session.time;
        let other = session
            .players
            .iter()
            .find(|p| p.as_str() != player)
            .cloned();
        self.games.remove(&game_id);
        self.player_game.remove(player);
        self.send(
            player,
            ServerMsg::GameCancelled {
                reason: "you_left".to_string(),
            },
        );
        self.send(
            player,
            ServerMsg::Lobby {
                status: LobbyStatus::Idle,
            },
        );
        if let Some(other) = other.filter(|o| !crate::bot::is_bot_id(o)) {
            self.player_game.remove(&other);
            self.send(
                &other,
                ServerMsg::GameCancelled {
                    reason: if requeue {
                        "opponent_left_requeued"
                    } else {
                        "opponent_left"
                    }
                    .to_string(),
                },
            );
            if requeue {
                self.requeue(&other, rated, time);
            } else {
                self.send(
                    &other,
                    ServerMsg::Lobby {
                        status: LobbyStatus::Idle,
                    },
                );
            }
            self.notify_presence(&other);
        }
        self.notify_presence(player);
    }

    /// Puts a player back in the queue they were matched from (at the front
    /// of the casual one, they have already waited).
    fn requeue(&mut self, player: &str, rated: bool, time: Option<TimeControl>) {
        let elo = match self.store.player_row(player) {
            Ok(Some(row)) if rated && row.username.is_some() => Some(row.elo),
            _ => None,
        };
        match elo {
            Some(elo) => {
                self.ranked_queue.push(QueueEntry {
                    player: player.to_string(),
                    time,
                    elo,
                    since: Instant::now(),
                });
                self.match_ranked();
                self.ensure_sweep();
            }
            None => match self.casual_queue.iter().position(|(_, t)| *t == time) {
                Some(i) => {
                    let (other, _) = self.casual_queue.remove(i).expect("position is valid");
                    self.create_game(other, player.to_string(), false, GameKind::Duel, time)
                }
                None => self.casual_queue.push_front((player.to_string(), time)),
            },
        }
        if !self.player_game.contains_key(player) {
            self.send(
                player,
                ServerMsg::Lobby {
                    status: self.lobby_status(player),
                },
            );
        }
    }

    fn cancel_session(&mut self, game_id: &str, reason: &str) {
        let Some(session) = self.games.remove(game_id) else {
            return;
        };
        self.spectate_cancel(game_id, reason);
        for player in &session.players {
            self.player_game.remove(player);
            self.send(
                player,
                ServerMsg::GameCancelled {
                    reason: reason.to_string(),
                },
            );
            self.send(
                player,
                ServerMsg::Lobby {
                    status: LobbyStatus::Idle,
                },
            );
        }
        for player in &session.players {
            self.notify_presence(player);
        }
    }

    // ---- playing ---------------------------------------------------------

    pub fn action(&mut self, player: &str, action: Action) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return self.fail(player, "not_in_game", "you are not in a game");
        };
        let Some(session) = self.games.get_mut(&game_id) else {
            return;
        };
        let color = session
            .color_of(player)
            .expect("player is in their session");
        let Phase::Playing { game } = &mut session.phase else {
            return self.fail(player, "wrong_phase", "the game has not started yet");
        };
        if game.side_to_move() != color {
            return self.fail(player, "not_your_turn", "it is not your turn");
        }
        let now = Instant::now();
        // The flag timer may be a moment behind: the clock has the last word.
        if session
            .clock
            .as_ref()
            .is_some_and(|c| c.expired(color, now))
        {
            return self.end_by_timeout(&game_id, color);
        }
        // What a player may do is decided on the board they see; the real one
        // then judges it. Both refusals read the same.
        let view_pos = {
            let hidden = view::hidden_ids(&game.pos, color);
            view::view_position(&game.pos, color, &hidden)
        };
        if !game.is_legal_on(&view_pos, action) {
            return self.fail(player, "illegal_action", "that action is not allowed");
        }
        let mut events = match game.apply(action) {
            Ok(events) => events,
            Err(_) => return self.blocked_attempt(player, &game_id, color),
        };
        view::mask_best_move(game, color, &mut events);
        session.recording.actions.push(action);
        let outcome = game.outcome();
        if game.side_to_move() == color && !outcome.is_over() {
            // Mind Reading / Mind Control keep the turn: same player, same
            // clock, same flag timer; only the new state goes out.
            return self.broadcast_state(&game_id, events);
        }
        let ply = game.pos.ply;
        session.draw_offer = None;
        let mut next_flag = None;
        if let Some(clock) = &mut session.clock {
            clock.press(color, now, self.config.clock_increment, outcome.is_over());
            next_flag = Some(clock.remaining[color.opposite().index()]);
        }
        if let (Some(delay), false) = (next_flag, outcome.is_over()) {
            self.timers.push((
                delay,
                Timer::Flag {
                    game_id: game_id.clone(),
                    color: color.opposite(),
                    ply,
                },
            ));
        }
        self.broadcast_state(&game_id, events);
        if outcome.is_over() {
            self.finish_game(&game_id, outcome, reason_of(&outcome));
        } else {
            self.schedule_bot(&game_id);
        }
    }

    pub fn resign(&mut self, player: &str) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return self.fail(player, "not_in_game", "you are not in a game");
        };
        let Some(color) = self.games.get(&game_id).and_then(|s| s.color_of(player)) else {
            return;
        };
        match self.games.get(&game_id).map(|s| &s.phase) {
            Some(Phase::Playing { .. }) => self.end_by_resignation(&game_id, color, "resignation"),
            _ => self.cancel_session(&game_id, "a player left before the game started"),
        }
    }

    fn end_by_resignation(&mut self, game_id: &str, color: Color, reason: &str) {
        let Some(Session {
            phase: Phase::Playing { game },
            ..
        }) = self.games.get_mut(game_id)
        else {
            return;
        };
        game.resign(color);
        let outcome = game.outcome();
        self.broadcast_state(game_id, Vec::new());
        self.finish_game(game_id, outcome, reason);
    }

    /// `color` ran out of time.
    fn end_by_timeout(&mut self, game_id: &str, color: Color) {
        let Some(Session {
            phase: Phase::Playing { game },
            clock,
            ..
        }) = self.games.get_mut(game_id)
        else {
            return;
        };
        game.flag(color);
        let outcome = game.outcome();
        if let Some(clock) = clock {
            clock.remaining[color.index()] = Duration::ZERO;
            clock.running = None;
        }
        self.broadcast_state(game_id, Vec::new());
        self.finish_game(game_id, outcome, "timeout");
    }

    pub fn offer_draw(&mut self, player: &str) {
        let Some((game_id, color)) = self.playing_as(player) else {
            return;
        };
        let Some(session) = self.games.get_mut(&game_id) else {
            return;
        };
        let Phase::Playing { game } = &session.phase else {
            return;
        };
        let ply = game.pos.ply;
        match session.draw_offer {
            // They already offered: offering back is accepting.
            Some(by) if by != color => return self.respond_draw(player, true),
            Some(_) => return self.fail(player, "draw_pending", "your offer is already open"),
            None => {}
        }
        if session.last_offer_ply[color.index()] == Some(ply) {
            return self.fail(
                player,
                "draw_already_offered",
                "wait for a move before offering again",
            );
        }
        session.draw_offer = Some(color);
        session.last_offer_ply[color.index()] = Some(ply);
        if session.solo.is_some() {
            return self.solo_answer_draw(&game_id, color);
        }
        let opponent = session.players[color.opposite().index()].clone();
        self.send(&opponent, ServerMsg::DrawOffered {});
    }

    pub fn respond_draw(&mut self, player: &str, accept: bool) {
        let Some((game_id, color)) = self.playing_as(player) else {
            return;
        };
        let Some(session) = self.games.get_mut(&game_id) else {
            return;
        };
        if session.draw_offer != Some(color.opposite()) {
            return self.fail(player, "no_draw_offer", "there is no draw offer to answer");
        }
        session.draw_offer = None;
        if !accept {
            let offerer = session.players[color.opposite().index()].clone();
            return self.send(&offerer, ServerMsg::DrawDeclined {});
        }
        let Phase::Playing { game } = &mut session.phase else {
            return;
        };
        game.agree_draw();
        let outcome = game.outcome();
        if let Some(clock) = &mut session.clock {
            clock.running = None;
        }
        self.broadcast_state(&game_id, Vec::new());
        self.finish_game(&game_id, outcome, "agreed_draw");
    }

    /// The player's running game and colour, or an error to them.
    fn playing_as(&self, player: &str) -> Option<(String, Color)> {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            self.fail(player, "not_in_game", "you are not in a game");
            return None;
        };
        let session = self.games.get(&game_id)?;
        if !matches!(session.phase, Phase::Playing { .. }) {
            self.fail(player, "wrong_phase", "the game has not started yet");
            return None;
        }
        let color = session.color_of(player)?;
        Some((game_id, color))
    }

    fn broadcast_state(&mut self, game_id: &str, events: Vec<chessy_engine::Event>) {
        self.remember_action(game_id, &events);
        self.send_player_states(game_id, &events);
        self.spectate_publish(game_id, events);
    }

    /// Files what `events` did under each colour's history, filtered as the
    /// live state is. Nothing to file when nothing happened (a clock penalty,
    /// a draw offer, the start of the game).
    fn remember_action(&mut self, game_id: &str, events: &[chessy_engine::Event]) {
        if events.is_empty() {
            return;
        }
        let Some(session) = self.games.get_mut(game_id) else {
            return;
        };
        let Phase::Playing { game } = &session.phase else {
            return;
        };
        for color in Color::BOTH {
            let hidden = view::hidden_ids(&game.pos, color);
            let seen = view::events(events.to_vec(), color, game, &hidden);
            let landed = seen
                .iter()
                .filter_map(|e| match e {
                    chessy_engine::Event::Moved { to, .. } => {
                        game.pos.board[*to as usize].map(|p| Landed {
                            square: *to,
                            kind: p.kind,
                        })
                    }
                    _ => None,
                })
                .collect();
            session.history[color.index()].push(HistoryEntry {
                ply: game.pos.ply,
                to_move: game.side_to_move(),
                events: seen,
                landed,
            });
        }
    }

    fn send_player_states(&self, game_id: &str, events: &[chessy_engine::Event]) {
        let Some(session) = self.games.get(game_id) else {
            return;
        };
        let Phase::Playing { game } = &session.phase else {
            return;
        };
        let spectators = self.watcher_count(game_id);
        for color in Color::BOTH {
            let view = state_view(game_id, session, game, color, events.to_vec(), spectators);
            self.send(
                &session.players[color.index()],
                ServerMsg::State(Box::new(view)),
            );
        }
    }

    /// `color` played something the board they see allows but the real one
    /// refuses: a hidden piece is in the way. They are told what they are told
    /// for any refused action, and pay for the attempt with clock time (the
    /// new clock goes out with a fresh state), so the refusal cannot be used to
    /// scan the board for hidden pieces.
    fn blocked_attempt(&mut self, player: &str, game_id: &str, color: Color) {
        self.fail(player, "illegal_action", "that action is not allowed");
        let cost = self.config.blocked_attempt_cost;
        let now = Instant::now();
        let Some(session) = self.games.get_mut(game_id) else {
            return;
        };
        let Phase::Playing { game } = &session.phase else {
            return;
        };
        let ply = game.pos.ply;
        let Some(clock) = &mut session.clock else {
            return;
        };
        clock.charge(color, cost, now);
        let left = clock.remaining[color.index()];
        if left.is_zero() {
            return self.end_by_timeout(game_id, color);
        }
        self.timers.push((
            left,
            Timer::Flag {
                game_id: game_id.to_string(),
                color,
                ply,
            },
        ));
        self.send_player_states(game_id, &[]);
    }

    // ---- game end and rewards -------------------------------------------

    fn finish_game(&mut self, game_id: &str, outcome: Outcome, reason: &str) {
        let Some(session) = self.games.remove(game_id) else {
            return;
        };
        for player in &session.players {
            self.player_game.remove(player);
        }
        // `pos.ply` decides whether the game is long enough to be rated; the
        // number of actions (what a replay has) is what gets stored.
        let (ply, loadouts) = match &session.phase {
            Phase::Playing { game } => (
                game.pos.ply,
                Some([
                    game.loadouts[0].slots.iter().map(|s| s.skill).collect(),
                    game.loadouts[1].slots.iter().map(|s| s.skill).collect(),
                ]),
            ),
            Phase::DeckSelect { .. } => (0, None),
        };
        let [white, black] = &session.players;
        let solo = session.solo.is_some();
        let change = match &loadouts {
            // Every game that was played is recorded, Solo ones included (they
            // stay out of the profile and the ranking, see `store`).
            Some(loadouts) => {
                let record = GameRecord {
                    id: game_id,
                    white,
                    black,
                    outcome: &outcome,
                    reason,
                    plies: session.recording.actions.len() as u32,
                    rated: session.rated && ply >= MIN_RATED_PLIES,
                    started_unix: session.started_unix,
                    kind: session.recording.kind,
                    loadouts,
                    actions: &session.recording.actions,
                    solo_elo: session.solo.map(|s| s.elo),
                };
                match self.store.record_game(&record) {
                    Ok(change) => change,
                    Err(e) => {
                        tracing::error!("could not record game {game_id}: {e}");
                        None
                    }
                }
            }
            None => None,
        };
        let winner = outcome.winner();
        for color in Color::BOTH {
            let player = &session.players[color.index()];
            // Only a rated game (ranked, between accounts, long enough) pays a skill:
            // friendly games and Solo would otherwise be farmed.
            let reward = if !solo && change.is_some() && Some(color) == winner {
                let loser = &session.players[color.opposite().index()];
                let loser_deck = self.deck_of(loser).unwrap_or_default();
                let offer = self.offer_for(player, loser, &loser_deck).ok();
                self.rewards.insert(
                    player.clone(),
                    PendingReward {
                        forging: false,
                        loser: loser.clone(),
                        loser_deck,
                        created: Instant::now(),
                    },
                );
                offer
            } else {
                None
            };
            let elo = change.map(|c| match color {
                Color::White => EloView {
                    you_before: c.white_before,
                    you_after: c.white_after,
                    opp_before: c.black_before,
                    opp_after: c.black_after,
                },
                Color::Black => EloView {
                    you_before: c.black_before,
                    you_after: c.black_after,
                    opp_before: c.white_before,
                    opp_after: c.white_after,
                },
            });
            self.send(
                player,
                ServerMsg::GameOver {
                    outcome,
                    reward,
                    rated: change.is_some(),
                    elo,
                    reason: reason.to_string(),
                },
            );
            self.send(
                player,
                ServerMsg::Lobby {
                    status: LobbyStatus::Idle,
                },
            );
        }
        if solo {
            self.offer_solo_rematch(&session);
        } else {
            self.offer_rematch(
                &session.players,
                session.rated,
                session.recording.kind,
                session.time,
            );
        }
        for player in &session.players {
            self.notify_presence(player);
            // Ratings changed: refresh what friends see.
            self.push_friends(player);
        }
    }

    /// Accounts (not guests) connected right now: the people a ranked player can really meet.
    fn accounts_online(&self) -> u32 {
        let n = self
            .conns
            .keys()
            .filter(|p| matches!(self.store.player_row(p), Ok(Some(row)) if row.username.is_some()))
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// Rated games two accounts may play against each other in the window: the
    /// fewer other people there are to play, the more they may play each other.
    fn pair_cap(&self) -> u32 {
        let base = self.config.rated_pair_max;
        let others = self.accounts_online().saturating_sub(1).max(1);
        let scaled = u64::from(base) * u64::from(self.config.rated_pair_scale) / u64::from(others);
        u32::try_from(scaled).unwrap_or(u32::MAX).max(base)
    }

    /// Whether `a` and `b` have already played `rated_pair_max` rated games
    /// against each other within `rated_pair_window` (anti Elo boosting).
    /// A store error counts as not capped: it must not block play.
    fn pair_capped(&self, a: &str, b: &str) -> bool {
        let window = self.config.rated_pair_window.as_secs();
        match self.store.rated_games_between(a, b, window) {
            Ok(n) => n >= self.pair_cap(),
            Err(e) => {
                tracing::error!("pair cap lookup failed: {e}");
                false
            }
        }
    }

    /// The player's unclaimed reward, if it has not expired.
    fn live_reward(&self, player: &str) -> Option<&PendingReward> {
        self.rewards
            .get(player)
            .filter(|r| r.created.elapsed() < self.config.reward_ttl)
    }

    /// What the winner may take: skills the loser owned when the game ended
    /// *and* still owns, that the winner does not have.
    fn offer_for(
        &self,
        winner: &str,
        loser: &str,
        snapshot: &[SkillId],
    ) -> Result<RewardOffer, StoreError> {
        let deck = self.deck_of(winner)?;
        let loser_now = self.deck_of(loser)?;
        let steal_options = snapshot
            .iter()
            .copied()
            .filter(|s| loser_now.contains(s) && !deck.contains(s))
            .collect();
        Ok(RewardOffer {
            deck_full: deck.len() >= MAX_DECK,
            deck,
            steal_options,
        })
    }

    pub fn reward_choice(&mut self, player: &str, choice: RewardChoice) {
        if matches!(choice, RewardChoice::Random { .. }) {
            // A random skill is forged: see `begin_forge`, which the app calls.
            return self.fail(
                player,
                "invalid_reward",
                "a random skill is forged, not drawn",
            );
        }
        let Some(pending) = self.rewards.remove(player) else {
            return self.fail(player, "no_reward", "you have no reward to claim");
        };
        if pending.created.elapsed() >= self.config.reward_ttl {
            return self.fail(player, "no_reward", "that reward has expired");
        }
        if pending.forging {
            self.rewards.insert(player.to_string(), pending);
            return self.fail(player, "forging", "a skill is already being forged for you");
        }
        match self.resolve_reward(player, &pending.loser, &pending.loser_deck, choice, None) {
            Ok(()) => {}
            Err(msg) => {
                // Let the player try again with a corrected choice.
                self.rewards.insert(player.to_string(), pending);
                self.fail(player, "invalid_reward", msg);
            }
        }
    }

    /// The winner chose a random skill: checks the reward and returns what
    /// the forge needs. The forging itself takes a while (it measures the
    /// candidates on real positions), so the app does it off the lock and then
    /// calls [`Hub::finish_forge`].
    pub fn begin_forge(&mut self, player: &str, replace: Option<SkillId>) -> Option<ForgeJob> {
        let Some(pending) = self.rewards.get_mut(player) else {
            self.fail(player, "no_reward", "you have no reward to claim");
            return None;
        };
        if pending.created.elapsed() >= self.config.reward_ttl {
            self.rewards.remove(player);
            self.fail(player, "no_reward", "that reward has expired");
            return None;
        }
        if pending.forging {
            self.fail(player, "forging", "a skill is already being forged for you");
            return None;
        }
        let known = match self.store.forged_signatures() {
            Ok(known) => known,
            Err(_) => {
                self.fail(player, "internal", "internal error");
                return None;
            }
        };
        let seed: u64 = rand::random();
        let target = chessy_engine::forge::generate::roll_rarity(&mut chessy_engine::ai::Rng::new(
            seed ^ 0xA5A5,
        ));
        if let Some(pending) = self.rewards.get_mut(player) {
            pending.forging = true;
        }
        Some(ForgeJob {
            player: player.to_string(),
            replace,
            target,
            seed,
            known,
            store: self.store.clone(),
        })
    }

    /// The forge is done: `skill` is what it made, `None` if it failed.
    pub fn finish_forge(&mut self, player: &str, replace: Option<SkillId>, skill: Option<SkillId>) {
        let Some(mut pending) = self.rewards.remove(player) else {
            return;
        };
        pending.forging = false;
        let result = match skill {
            None => Err("the forge could not make a skill, try again"),
            Some(_) => self.resolve_reward(
                player,
                &pending.loser,
                &pending.loser_deck,
                RewardChoice::Random { replace },
                skill,
            ),
        };
        if let Err(msg) = result {
            self.rewards.insert(player.to_string(), pending);
            self.fail(player, "invalid_reward", msg);
        }
    }

    fn resolve_reward(
        &mut self,
        winner: &str,
        loser: &str,
        snapshot: &[SkillId],
        choice: RewardChoice,
        forged: Option<SkillId>,
    ) -> Result<(), &'static str> {
        let db = |_: StoreError| "internal error";
        let winner_deck = self.deck_of(winner).map_err(db)?;
        // Only what the loser had at the end of the game and still has now
        // (they may have won skills elsewhere since, or lost some).
        let loser_deck: Vec<SkillId> = self
            .deck_of(loser)
            .map_err(db)?
            .into_iter()
            .filter(|s| snapshot.contains(s))
            .collect();

        let (gain, loser_loses, replace) = match choice {
            RewardChoice::Skip => {
                self.send(
                    winner,
                    ServerMsg::DeckUpdate {
                        deck: winner_deck,
                        gained: None,
                        lost: None,
                    },
                );
                return Ok(());
            }
            RewardChoice::Steal { skill, replace } => {
                if !loser_deck.contains(&skill) {
                    return Err("that skill is not available to take any more");
                }
                if winner_deck.contains(&skill) {
                    return Err("you already have that skill");
                }
                (Some(skill), Some(skill), replace)
            }
            RewardChoice::Random { replace } => {
                // The forge made the skill; the loser loses one of theirs at random.
                let skill = forged.ok_or("no skill was forged")?;
                if winner_deck.contains(&skill) {
                    return Err("you already have that skill");
                }
                let mut rng = rand::rng();
                (Some(skill), loser_deck.choose(&mut rng).copied(), replace)
            }
        };

        let winner_drops = if gain.is_some() && winner_deck.len() >= MAX_DECK {
            match replace {
                Some(r) if winner_deck.contains(&r) && Some(r) != gain => Some(r),
                _ => return Err("your deck is full: choose a skill to replace"),
            }
        } else {
            None
        };

        self.store
            .apply_reward(winner, loser, gain, loser_loses, winner_drops)
            .map_err(|_| "could not apply that reward")?;
        let _ = self.store.refill_if_empty(loser);

        let winner_after = self.deck_of(winner).map_err(db)?;
        self.send(
            winner,
            ServerMsg::DeckUpdate {
                deck: winner_after,
                gained: gain,
                lost: winner_drops,
            },
        );
        if let Ok(loser_after) = self.deck_of(loser) {
            self.send(
                loser,
                ServerMsg::DeckUpdate {
                    deck: loser_after,
                    gained: None,
                    lost: loser_loses,
                },
            );
        }
        Ok(())
    }
}

fn state_view(
    game_id: &str,
    session: &Session,
    game: &Game,
    you: Color,
    events: Vec<chessy_engine::Event>,
    spectators: usize,
) -> StateView {
    let hidden = view::hidden_ids(&game.pos, you);
    let mut moves = Vec::new();
    let mut skill_options: Vec<SkillOptions> = Vec::new();
    if game.side_to_move() == you {
        // Listed on the board the player sees, never on the real one: hidden
        // pieces and traps would show in what is allowed (see `view`).
        let seen = view::view_position(&game.pos, you, &hidden);
        for action in game.legal_actions_on(&seen) {
            match action {
                Action::Move { from, to, promo } => {
                    moves.push(chessy_engine::Move { from, to, promo })
                }
                Action::Skill { skill, target } => push_target(&mut skill_options, skill, target),
            }
        }
    }
    let theirs = game.loadout(you.opposite());
    StateView {
        game_id: game_id.to_string(),
        you,
        ply: game.pos.ply,
        to_move: game.side_to_move(),
        in_check: game.pos.in_check(game.side_to_move()),
        board: view::board(&game.pos, &hidden),
        moves,
        skill_options,
        my_skills: view::skills(game.loadout(you)),
        opponent_skills: OpponentSkills {
            total: theirs.slots.len(),
            used: theirs
                .slots
                .iter()
                .filter(|s| s.used)
                .map(|s| s.skill)
                .collect(),
        },
        effects: view::effects(&game.pos, &hidden),
        traps: view::own_traps(&game.pos, you),
        benched: view::own_benched(&game.pos, you),
        terrain: view::terrain(&game.pos),
        outcome: game.outcome(),
        events: view::events(events, you, game, &hidden),
        opponent_connected: session.connected[you.opposite().index()],
        clock: session
            .clock
            .as_ref()
            .map(|c| c.view(Instant::now()))
            .unwrap_or(ClockView {
                white_ms: 0,
                black_ms: 0,
                running: None,
            }),
        clock_enabled: session.clock.is_some(),
        rated: session.rated,
        opponent: session.info[you.opposite().index()].clone(),
        draw_offer: match session.draw_offer {
            None => DrawOffer::None,
            Some(by) if by == you => DrawOffer::You,
            Some(_) => DrawOffer::Them,
        },
        ply_count: game.pos.ply,
        spectators,
        history: Vec::new(),
    }
}

fn push_target(options: &mut Vec<SkillOptions>, skill: SkillId, target: SkillTarget) {
    match options.iter_mut().find(|o| o.skill == skill) {
        Some(o) => o.targets.push(target),
        None => options.push(SkillOptions {
            skill,
            targets: vec![target],
        }),
    }
}
