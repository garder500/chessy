//! Shares the [`Hub`] across connections and runs its timers.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::bot;
use crate::hub::{ForgeJob, Hub, HubConfig, Timer};
use crate::limits::{ConnSlot, ConnectionLimiter, Refusal};
use crate::protocol::{ClientMsg, PlayerId, RewardChoice, ServerMsg};
use crate::store::{Store, StoreError};

/// Failed logins allowed per username within [`LOGIN_WINDOW`] before it is locked out.
const LOGIN_MAX_FAILURES: u32 = 8;
const LOGIN_WINDOW: Duration = Duration::from_secs(300);

pub struct App {
    hub: Mutex<Hub>,
    config: HubConfig,
    /// The longest the hub lock has been held in one go, in nanoseconds.
    max_hold_ns: AtomicU64,
    store: Store,
    /// Recent failed logins per lower-cased username: (count, window start).
    login_failures: Mutex<HashMap<String, (u32, Instant)>>,
    /// Open WebSocket connections, against `max_connections[_per_ip]`.
    connections: Arc<ConnectionLimiter>,
}

impl App {
    pub fn new(store: Store, config: HubConfig) -> Arc<Self> {
        Arc::new(App {
            hub: Mutex::new(Hub::new(store.clone(), config)),
            config,
            max_hold_ns: AtomicU64::new(0),
            store,
            login_failures: Mutex::new(HashMap::new()),
            connections: ConnectionLimiter::new(
                config.max_connections,
                config.max_connections_per_ip,
            ),
        })
    }

    /// Reserves a place for a new WebSocket connection (`ip` is the client
    /// address when known). The place is given back when the guard drops.
    pub fn admit_connection(&self, ip: Option<IpAddr>) -> Result<ConnSlot, Refusal> {
        self.connections.try_acquire(ip)
    }

    /// WebSocket connections open right now (counted from the upgrade).
    pub fn open_connections(&self) -> usize {
        self.connections.open()
    }

    /// Whether `username` has failed too many logins recently.
    pub fn login_blocked(&self, username: &str) -> bool {
        let mut map = self
            .login_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match map.get(&username.to_ascii_lowercase()) {
            Some((n, since)) if since.elapsed() < LOGIN_WINDOW => *n >= LOGIN_MAX_FAILURES,
            Some(_) => {
                map.remove(&username.to_ascii_lowercase());
                false
            }
            None => false,
        }
    }

    pub fn login_failed(&self, username: &str) {
        let mut map = self
            .login_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if map.len() > 10_000 {
            map.retain(|_, (_, since)| since.elapsed() < LOGIN_WINDOW);
        }
        let entry = map
            .entry(username.to_ascii_lowercase())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed() >= LOGIN_WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }

    pub fn login_succeeded(&self, username: &str) {
        let mut map = self
            .login_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.remove(&username.to_ascii_lowercase());
    }

    pub fn config(&self) -> &HubConfig {
        &self.config
    }

    /// The longest single hold of the hub lock so far (a measure of how much
    /// one message can delay everyone else, timers included).
    pub fn max_lock_hold(&self) -> Duration {
        Duration::from_nanos(self.max_hold_ns.load(Ordering::Relaxed))
    }

    /// Resets [`Self::max_lock_hold`].
    pub fn reset_lock_hold(&self) {
        self.max_hold_ns.store(0, Ordering::Relaxed);
    }

    fn note_hold(&self, since: Instant) {
        let ns = since.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.max_hold_ns.fetch_max(ns, Ordering::Relaxed);
    }

    /// Whether `player` has a live WebSocket right now.
    pub fn is_connected(&self, player: &str) -> bool {
        let hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
        hub.is_connected(player)
    }

    /// A session ended (logout): the connection that used it is told and closed.
    pub fn session_revoked(self: &Arc<Self>, token: &str) {
        self.run(|hub| hub.revoke_session(token));
    }

    /// The database, for the REST API.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Running games for `GET /api/live`.
    pub fn live_games(&self, limit: usize) -> Vec<crate::hub::LiveGame> {
        let hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
        hub.live_games(limit)
    }

    /// A player's account changed outside the WebSocket (e.g. a guest
    /// registered); refreshes what a connected client sees.
    pub fn account_changed(self: &Arc<Self>, player: &str) {
        self.run(|hub| hub.push_friends(player));
    }

    /// Runs `f` on the hub, then schedules any timers it asked for.
    fn run<R>(self: &Arc<Self>, f: impl FnOnce(&mut Hub) -> R) -> R {
        let (result, timers) = {
            let mut hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
            let held = Instant::now();
            let result = f(&mut hub);
            let timers = hub.take_timers();
            self.note_hold(held);
            (result, timers)
        };
        self.schedule(timers);
        result
    }

    fn schedule(self: &Arc<Self>, timers: Vec<(Duration, Timer)>) {
        for (delay, timer) in timers {
            let app = Arc::clone(self);
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                app.fire(timer);
            });
        }
    }

    // Not routed through the generic `run`: spawning from a generic function
    // that the spawned task calls again would recurse at the type level.
    fn fire(self: &Arc<Self>, timer: Timer) {
        if let Timer::BotMove { game_id, ply } = timer {
            return self.fire_bot(game_id, ply);
        }
        let timers = {
            let mut hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
            let held = Instant::now();
            hub.on_timer(timer);
            let timers = hub.take_timers();
            self.note_hold(held);
            timers
        };
        self.schedule(timers);
    }

    /// The bot's turn: snapshot the game under the lock, search off it (the
    /// hub never waits for a search), then play the answer if the game is
    /// still where the snapshot left it.
    fn fire_bot(self: &Arc<Self>, game_id: String, ply: u32) {
        let job = {
            let hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
            hub.bot_job(&game_id, ply)
        };
        let Some(job) = job else { return };
        let app = Arc::clone(self);
        tokio::spawn(async move {
            let action = tokio::task::spawn_blocking(move || bot::think(&job))
                .await
                .ok()
                .flatten();
            let timers = {
                let mut hub = app.hub.lock().unwrap_or_else(|e| e.into_inner());
                hub.apply_bot_move(&game_id, ply, action);
                hub.take_timers()
            };
            app.schedule(timers);
        });
    }

    pub fn connect(
        self: &Arc<Self>,
        token: Option<String>,
        tx: UnboundedSender<ServerMsg>,
    ) -> Result<(PlayerId, u64), StoreError> {
        self.run(|hub| hub.connect(token, tx))
    }

    pub fn disconnect(self: &Arc<Self>, player: &str, conn_id: u64) {
        self.run(|hub| hub.disconnect(player, conn_id));
    }

    /// Handles one message from an established connection. Messages from a
    /// connection that has since been replaced are ignored.
    pub fn handle(self: &Arc<Self>, player: &str, conn_id: u64, msg: ClientMsg) {
        let cost = msg.cost(self.config.expensive_cost);
        let forge = self.run(|hub| {
            if !hub.admit(player, conn_id, cost) {
                return None;
            }
            match msg {
                ClientMsg::Hello { .. } => {}
                ClientMsg::QueueJoin { ranked, time } => hub.queue_join(player, ranked, time),
                ClientMsg::CreateRoom { time } => hub.create_room(player, time),
                ClientMsg::JoinRoom { code } => hub.join_room(player, &code),
                ClientMsg::LeaveLobby => hub.leave_lobby(player),
                ClientMsg::LeaveDeckSelect => hub.leave_deck_select(player),
                ClientMsg::SelectDeck { skills } => hub.select_deck(player, skills),
                ClientMsg::Action { action } => hub.action(player, action),
                ClientMsg::Resign => hub.resign(player),
                ClientMsg::RewardChoice {
                    choice: RewardChoice::Random { replace },
                } => return hub.begin_forge(player, replace),
                ClientMsg::RewardChoice { choice } => hub.reward_choice(player, choice),
                ClientMsg::OfferDraw => hub.offer_draw(player),
                ClientMsg::RespondDraw { accept } => hub.respond_draw(player, accept),
                ClientMsg::Chat { text } => hub.chat(player, &text),
                ClientMsg::RematchRequest => hub.rematch_request(player),
                ClientMsg::RematchRespond { accept } => hub.rematch_respond(player, accept),
                ClientMsg::FriendRequest { username } => hub.friend_request(player, &username),
                ClientMsg::FriendRespond { username, accept } => {
                    hub.friend_respond(player, &username, accept)
                }
                ClientMsg::FriendRemove { username } => hub.friend_remove(player, &username),
                ClientMsg::FriendsList => hub.friends_list(player),
                ClientMsg::BlockUser { username } => hub.block_user(player, &username),
                ClientMsg::UnblockUser { username } => hub.unblock_user(player, &username),
                ClientMsg::BlocksList => hub.blocks_list(player),
                ClientMsg::SetChatMuted { muted } => hub.set_chat_muted(player, muted),
                ClientMsg::ReportUser {
                    username,
                    reason,
                    game_id,
                    context,
                } => hub.report_user(player, &username, reason, game_id, context),
                ClientMsg::UserSearch { query } => hub.user_search(player, &query),
                ClientMsg::Challenge { username, time } => hub.challenge(player, &username, time),
                ClientMsg::ChallengeRespond { username, accept } => {
                    hub.challenge_respond(player, &username, accept)
                }
                ClientMsg::ChallengeCancel => hub.challenge_cancel(player),
                ClientMsg::SoloStart { elo, color } => hub.solo_start(player, elo, color),
                ClientMsg::Spectate { game_id } => hub.spectate(player, &game_id),
                ClientMsg::Unspectate => hub.unspectate(player),
            }
            None
        });
        if let Some(job) = forge {
            self.fire_forge(job);
        }
    }

    /// Forges the skill of a "random" reward off the hub lock, stores it, then
    /// hands it to the winner. (Like `fire_bot`, it takes the lock directly:
    /// going through the generic `run` from a spawned task would recurse at
    /// the type level.)
    fn fire_forge(self: &Arc<Self>, job: ForgeJob) {
        let app = Arc::clone(self);
        tokio::spawn(async move {
            let ForgeJob {
                player,
                replace,
                target,
                seed,
                known,
                store,
            } = job;
            let made = tokio::task::spawn_blocking(move || {
                let mut rng = chessy_engine::ai::Rng::new(seed);
                let budget = chessy_engine::forge::generate::Budget::live();
                let forged =
                    chessy_engine::forge::generate::forge(&mut rng, target, &known, budget);
                store
                    .insert_forged(&forged.def, &forged.graded)
                    .ok()
                    .map(chessy_engine::SkillId::Forged)
            })
            .await
            .ok()
            .flatten();
            let timers = {
                let mut hub = app.hub.lock().unwrap_or_else(|e| e.into_inner());
                hub.finish_forge(&player, replace, made);
                hub.take_timers()
            };
            app.schedule(timers);
        });
    }
}
