//! Spectators (see `docs/spec-v4.md` §3): live game list, watching a game and
//! the anti-cheat delay.
//!
//! Every action of a running game produces a [`SpectatorView`] that is queued
//! on the game's [`Feed`] with a delivery time (`now + spectator_delay`).
//! A [`Timer::SpectatorFlush`] per queued view wakes the hub up at that time;
//! [`Hub::spectate_flush`] then sends what is due, in order. Nothing waits: the
//! hub only ever pushes to channels. The end of a game goes through the same
//! queue (`spectate_over`), so the feed outlives the [`Session`] until the
//! last view has been delivered.
//!
//! Spectators see both sides the way a player sees their opponent: invisible
//! pieces are removed from the board, traps and the bench never show.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use chessy_engine::{ActiveEffect, Color, Event, Game, Outcome, Piece, SkillId};
use serde::Serialize;

use super::{view, GameKind, Hub, Phase, Session, Timer};
use crate::protocol::*;

/// Spectators allowed on one game.
pub const MAX_SPECTATORS: usize = 50;

/// One side of a game as the public sees it.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LiveSeat {
    pub username: Option<String>,
    /// Elo before the game, or the bot's level.
    pub elo: Option<i32>,
    pub bot: bool,
}

impl From<&OpponentInfo> for LiveSeat {
    fn from(info: &OpponentInfo) -> Self {
        LiveSeat {
            username: info.username.clone(),
            elo: info.elo,
            bot: info.bot,
        }
    }
}

/// A running game in `GET /api/live`.
#[derive(Clone, Debug, Serialize)]
pub struct LiveGame {
    pub game_id: String,
    pub kind: GameKind,
    pub rated: bool,
    pub white: LiveSeat,
    pub black: LiveSeat,
    /// Actions played so far (the real game, not the delayed picture).
    pub ply: u32,
    pub started_at: String,
    pub spectators: usize,
}

/// Skills each side has used so far.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct UsedSkills {
    pub white: Vec<SkillId>,
    pub black: Vec<SkillId>,
}

/// What a spectator sees: everything public about a game, nothing hidden.
#[derive(Clone, Debug, Serialize)]
pub struct SpectatorView {
    pub game_id: String,
    pub kind: GameKind,
    pub rated: bool,
    pub white: LiveSeat,
    pub black: LiveSeat,
    pub ply: u32,
    pub to_move: Color,
    pub in_check: bool,
    /// Invisible pieces (of either side) are empty squares.
    pub board: Vec<Option<Piece>>,
    pub effects: Vec<ActiveEffect>,
    pub terrain: Vec<TerrainView>,
    pub clock: ClockView,
    pub clock_enabled: bool,
    /// What the last action did, filtered like an opponent's events.
    pub events: Vec<Event>,
    pub used: UsedSkills,
    pub outcome: Outcome,
    pub spectators: usize,
    pub delay_ms: u64,
}

struct Item {
    due: Instant,
    view: SpectatorView,
    over: bool,
}

struct Delivered {
    /// When it was (to be) delivered; the clock in `view` is as of then.
    at: Instant,
    view: SpectatorView,
}

/// A game's spectators and the views waiting for their delivery time.
#[derive(Default)]
pub(super) struct Feed {
    watchers: Vec<PlayerId>,
    queue: VecDeque<Item>,
    delivered: Option<Delivered>,
    /// The game is over: its final view is queued, nobody can join any more.
    closed: bool,
}

/// `view` as it looks `elapsed` later: the running clock has ticked on.
fn aged(mut view: SpectatorView, elapsed: Duration) -> SpectatorView {
    let ms = elapsed.as_millis() as u64;
    match view.clock.running {
        Some(Color::White) => view.clock.white_ms = view.clock.white_ms.saturating_sub(ms),
        Some(Color::Black) => view.clock.black_ms = view.clock.black_ms.saturating_sub(ms),
        None => {}
    }
    view
}

fn used_by(game: &Game, color: Color) -> Vec<SkillId> {
    game.loadout(color)
        .slots
        .iter()
        .filter(|s| s.used)
        .map(|s| s.skill)
        .collect()
}

fn spectator_view(
    game_id: &str,
    session: &Session,
    game: &Game,
    events: Vec<Event>,
    delay: Duration,
) -> SpectatorView {
    let hidden: HashSet<_> = view::spectator_hidden(&game.pos);
    SpectatorView {
        game_id: game_id.to_string(),
        kind: session.kind(),
        rated: session.rated,
        white: LiveSeat::from(&session.info[Color::White.index()]),
        black: LiveSeat::from(&session.info[Color::Black.index()]),
        ply: game.pos.ply,
        to_move: game.side_to_move(),
        in_check: game.pos.in_check(game.side_to_move()),
        board: view::board(&game.pos, &hidden),
        effects: view::effects(&game.pos, &hidden),
        terrain: view::terrain(&game.pos),
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
        events: view::spectator_events(events, game, &hidden),
        used: UsedSkills {
            white: used_by(game, Color::White),
            black: used_by(game, Color::Black),
        },
        outcome: game.outcome(),
        spectators: 0,
        delay_ms: delay.as_millis() as u64,
    }
}

/// `2026-10-07T12:00:00Z` for a Unix time in seconds.
fn iso(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3_600,
        secs % 3_600 / 60,
        secs % 60
    )
}

/// The counter in a game id like `g12-ab34cd`, to order games of one second.
fn game_number(game_id: &str) -> u64 {
    game_id
        .strip_prefix('g')
        .and_then(|rest| rest.split('-').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

impl Session {
    pub(super) fn kind(&self) -> GameKind {
        if self.solo.as_ref().is_some_and(super::solo::Solo::is_plain) {
            GameKind::Solo
        } else {
            self.recording.kind
        }
    }
}

impl Hub {
    // ---- live list -------------------------------------------------------

    /// Games being played (not still choosing skills), strongest first, then
    /// oldest first.
    pub fn live_games(&self, limit: usize) -> Vec<LiveGame> {
        let mut games: Vec<(i32, i64, u64, LiveGame)> = self
            .games
            .iter()
            .filter_map(|(id, s)| {
                let Phase::Playing { game } = &s.phase else {
                    return None;
                };
                if game.outcome().is_over() {
                    return None;
                }
                let elo = |c: Color| s.info[c.index()].elo.unwrap_or(1200);
                let total = elo(Color::White) + elo(Color::Black);
                Some((
                    total,
                    s.started_unix,
                    game_number(id),
                    LiveGame {
                        game_id: id.clone(),
                        kind: s.kind(),
                        rated: s.rated,
                        white: LiveSeat::from(&s.info[Color::White.index()]),
                        black: LiveSeat::from(&s.info[Color::Black.index()]),
                        ply: game.pos.ply,
                        started_at: iso(s.started_unix),
                        spectators: self.watcher_count(id),
                    },
                ))
            })
            .collect();
        games.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then_with(|| a.3.game_id.cmp(&b.3.game_id))
        });
        games.into_iter().take(limit).map(|g| g.3).collect()
    }

    /// Spectators currently on `game_id`.
    pub(super) fn watcher_count(&self, game_id: &str) -> usize {
        self.feeds.get(game_id).map_or(0, |f| f.watchers.len())
    }

    /// The game a player is in, if it is running (so a friend can watch it).
    pub(super) fn watchable_game(&self, player: &str) -> Option<String> {
        let game_id = self.player_game.get(player)?;
        let live = matches!(
            self.games.get(game_id),
            Some(Session {
                phase: Phase::Playing { .. },
                ..
            })
        );
        live.then(|| game_id.clone())
    }

    // ---- joining and leaving --------------------------------------------

    pub fn spectate(&mut self, player: &str, game_id: &str) {
        if self.player_game.contains_key(player) {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        let running = matches!(
            self.games.get(game_id),
            Some(Session {
                phase: Phase::Playing { .. },
                ..
            })
        );
        let feed = self
            .feeds
            .get(game_id)
            .filter(|f| running && !f.closed && f.delivered.is_some());
        let Some(feed) = feed else {
            return self.fail(player, "no_such_game", "that game is not being played");
        };
        let already = feed.watchers.iter().any(|w| w == player);
        if !already && feed.watchers.len() >= MAX_SPECTATORS {
            return self.fail(player, "spectate_full", "that game has enough spectators");
        }
        if self.watching.get(player).is_some_and(|g| g != game_id) {
            self.spectate_leave(player);
        }
        if !already {
            if let Some(feed) = self.feeds.get_mut(game_id) {
                feed.watchers.push(player.to_string());
            }
            self.watching
                .insert(player.to_string(), game_id.to_string());
        }
        self.send_snapshot(game_id, player);
        if !already {
            self.spectators_changed(game_id, Some(player));
        }
    }

    /// Stops watching (nothing happens if the player was not watching).
    pub fn unspectate(&mut self, player: &str) {
        self.spectate_leave(player);
    }

    /// Removes `player` from whatever they watch and updates the counters.
    pub(super) fn spectate_leave(&mut self, player: &str) {
        let Some(game_id) = self.watching.remove(player) else {
            return;
        };
        let Some(feed) = self.feeds.get_mut(&game_id) else {
            return;
        };
        feed.watchers.retain(|w| w != player);
        if !feed.closed {
            self.spectators_changed(&game_id, None);
        }
    }

    /// A reconnecting spectator (same account, new socket) gets the picture again.
    pub(super) fn spectate_resume(&self, player: &str) {
        if let Some(game_id) = self.watching.get(player) {
            self.send_snapshot(game_id, player);
        }
    }

    /// What `player` should see right now: the latest delivered view, without
    /// events, with the counter and the clock brought up to date.
    fn send_snapshot(&self, game_id: &str, player: &str) {
        let Some(feed) = self.feeds.get(game_id) else {
            return;
        };
        let Some(d) = &feed.delivered else { return };
        let mut view = aged(
            d.view.clone(),
            Instant::now().saturating_duration_since(d.at),
        );
        view.events.clear();
        view.spectators = feed.watchers.len();
        self.send(
            player,
            ServerMsg::SpectateState {
                view: Box::new(view),
            },
        );
    }

    /// The count changed: players get a fresh `state`, other spectators a
    /// `spectate_state` (without events) so their counters follow.
    fn spectators_changed(&self, game_id: &str, except: Option<&str>) {
        if let Some(session) = self.games.get(game_id) {
            for player in &session.players {
                self.send_session_to(game_id, player);
            }
        }
        if let Some(feed) = self.feeds.get(game_id) {
            for w in feed.watchers.iter().filter(|w| Some(w.as_str()) != except) {
                self.send_snapshot(game_id, w);
            }
        }
    }

    // ---- feeding ---------------------------------------------------------

    /// The game has started: opens its feed and tells friends it is watchable.
    pub(super) fn spectate_open(&mut self, game_id: &str) {
        self.feeds.insert(game_id.to_string(), Feed::default());
        let players = match self.games.get(game_id) {
            Some(s) => s.players.clone(),
            None => return,
        };
        for player in &players {
            self.notify_presence(player);
        }
    }

    /// Queues the game's current picture for its spectators; called after
    /// every action and at the end. The first picture (the starting
    /// position) is delivered at once: there is nothing to hide in it.
    pub(super) fn spectate_publish(&mut self, game_id: &str, events: Vec<Event>) {
        let Some(session) = self.games.get(game_id) else {
            return;
        };
        let Phase::Playing { game } = &session.phase else {
            return;
        };
        let Some(feed) = self.feeds.get_mut(game_id) else {
            return;
        };
        let plain_solo = session
            .solo
            .as_ref()
            .is_some_and(super::solo::Solo::is_plain);
        let delay = if plain_solo || (feed.delivered.is_none() && feed.queue.is_empty()) {
            Duration::ZERO
        } else {
            self.config.spectator_delay
        };
        let over = game.outcome().is_over();
        let configured = if plain_solo {
            Duration::ZERO
        } else {
            self.config.spectator_delay
        };
        let view = spectator_view(game_id, session, game, events, configured);
        feed.queue.push_back(Item {
            due: Instant::now() + delay,
            view,
            over,
        });
        feed.closed |= over;
        if delay.is_zero() {
            self.spectate_flush(game_id);
        } else {
            self.timers.push((
                delay,
                Timer::SpectatorFlush {
                    game_id: game_id.to_string(),
                },
            ));
        }
    }

    /// Sends every queued view whose time has come, in order. The final view
    /// goes out as `spectate_over` and frees the spectators.
    pub(super) fn spectate_flush(&mut self, game_id: &str) {
        let now = Instant::now();
        let Some(feed) = self.feeds.get_mut(game_id) else {
            return;
        };
        let mut out: Vec<ServerMsg> = Vec::new();
        let mut finished = false;
        while feed.queue.front().is_some_and(|i| i.due <= now) {
            let Some(item) = feed.queue.pop_front() else {
                break;
            };
            let mut view = item.view;
            view.spectators = feed.watchers.len();
            feed.delivered = Some(Delivered {
                at: item.due,
                view: view.clone(),
            });
            let view = Box::new(view);
            if item.over {
                out.push(ServerMsg::SpectateOver { view });
                finished = true;
                break;
            }
            out.push(ServerMsg::SpectateState { view });
        }
        if out.is_empty() {
            // Woken up a hair early: come back for the head of the queue.
            if let Some(head) = feed.queue.front() {
                self.timers.push((
                    head.due.saturating_duration_since(now),
                    Timer::SpectatorFlush {
                        game_id: game_id.to_string(),
                    },
                ));
            }
            return;
        }
        let watchers = feed.watchers.clone();
        if finished {
            self.feeds.remove(game_id);
            for w in &watchers {
                self.watching.remove(w);
            }
        }
        for w in &watchers {
            for msg in &out {
                self.send(w, msg.clone());
            }
        }
    }

    /// The game was cancelled: spectators are told and let go.
    pub(super) fn spectate_cancel(&mut self, game_id: &str, reason: &str) {
        let Some(feed) = self.feeds.remove(game_id) else {
            return;
        };
        for w in &feed.watchers {
            self.watching.remove(w);
            self.send(
                w,
                ServerMsg::SpectateEnded {
                    reason: reason.to_string(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::iso;
    use crate::hub::{Hub, HubConfig};
    use crate::protocol::ServerMsg;
    use crate::store::Store;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    fn connect(hub: &mut Hub) -> (String, UnboundedReceiver<ServerMsg>) {
        let (tx, rx) = unbounded_channel();
        let (id, _) = hub.connect(None, tx).unwrap();
        (id, rx)
    }

    #[test]
    fn a_cancelled_game_tells_its_spectators_and_frees_them() {
        let mut hub = Hub::new(Store::open(":memory:").unwrap(), HubConfig::default());
        let (a, _ra) = connect(&mut hub);
        let (b, _rb) = connect(&mut hub);
        let (s, mut rs) = connect(&mut hub);
        hub.queue_join(&a, Some(false), None);
        hub.queue_join(&b, Some(false), None);
        hub.select_deck(&a, vec![]);
        hub.select_deck(&b, vec![]);
        let game_id = hub.player_game[&a].clone();
        hub.spectate(&s, &game_id);
        assert_eq!(hub.watcher_count(&game_id), 1);
        while rs.try_recv().is_ok() {}

        hub.cancel_session(&game_id, "opponent left");
        let msgs: Vec<ServerMsg> = std::iter::from_fn(|| rs.try_recv().ok()).collect();
        assert!(matches!(
            msgs.as_slice(),
            [ServerMsg::SpectateEnded { reason }] if reason == "opponent left"
        ));
        assert_eq!(hub.watcher_count(&game_id), 0);
        assert!(hub.watching.is_empty());
        assert!(hub.live_games(10).is_empty());
        // Free to watch again (and the old game is gone).
        hub.spectate(&s, &game_id);
        assert!(
            matches!(rs.try_recv(), Ok(ServerMsg::Error { code, .. }) if code == "no_such_game")
        );
    }

    #[test]
    fn formats_unix_times() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso(1_791_374_445), "2026-10-07T12:00:45Z");
    }
}
