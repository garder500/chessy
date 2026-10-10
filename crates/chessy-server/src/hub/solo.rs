//! Solo mode: a friendly game against the bot (see `docs/spec-v3.md` §5).
//!
//! The bot is one seat of an ordinary [`Session`]. It has no connection and no
//! account: its `players` entry is a synthetic id (`bot:<game id>`) that never
//! shows up in `conns`, `player_game` or the store, so every `send` to it is a
//! no-op and no database lookup finds it. What makes the game Solo is
//! `Session::solo`.
//!
//! The bot moves from a [`Timer::BotMove`] scheduled after every action that
//! hands it the turn. The timer is handled by `App::fire`, which takes a
//! snapshot with [`Hub::bot_job`], runs the search in `spawn_blocking` and
//! comes back with [`Hub::apply_bot_move`]; a result for a game that has moved
//! on (different `ply`, gone, over) is discarded.
//!
//! A second kind of bot, the matchmaking bot (see [`Disguise`]), fills a queue
//! nobody joined: it passes for a person, has an account in the ranking and
//! plays a real game with a clock and Elo, with the same machinery.
//!
//! Solo games are recorded for replay (`kind: solo`, the bot's seat is NULL)
//! but stay out of the public history and the ranking; no Elo, no reward.

use std::time::{Duration, Instant};

use chessy_engine::ai::Strength;
use chessy_engine::{Action, Color, SkillId};

use super::social::Rematch;
use super::{Hub, Phase, Session, Timer};
use crate::bot::{self, BotJob};
use crate::campaign::{self, Level, Tally};
use crate::games_store::GameKind;
use crate::protocol::*;
use crate::store::reason_of;
use crate::store::BotAccount;

/// The bot's seat in a Solo session.
#[derive(Clone, Debug)]
pub(super) struct Solo {
    pub bot: Color,
    /// The level the player chose (400..=2800), or the bot account's rating.
    pub elo: i32,
    /// Set for a matchmaking bot, which passes for a person: see [`Disguise`].
    pub disguise: Option<Disguise>,
    /// Set for a placement game: the level is the hidden one being measured
    /// (see [`Hub::placement_start`]). The bot is plain otherwise.
    pub placement: Option<i32>,
    /// Set for a campaign level: see [`CampaignRun`].
    pub campaign: Option<CampaignRun>,
}

/// A campaign level being played (docs/spec-v6.md): the level, the hand the
/// player brought, and what the game has shown so far, which decides the
/// stars when it ends.
#[derive(Clone, Debug)]
pub(super) struct CampaignRun {
    pub level: Level,
    pub hand: Vec<SkillId>,
    pub tally: Tally,
}

impl CampaignRun {
    pub fn banner(&self) -> CampaignBanner {
        CampaignBanner {
            level: self.level.id,
            objective: self.level.objective,
            challenge: self.level.challenge,
        }
    }
}

/// A matchmaking bot is a real account (its name, rating and deck are stored)
/// that plays a real game: clock, Elo, ranking, reward and public history, as
/// if it were a person. It is only the move that comes from the AI, with a
/// human latency.
#[derive(Clone, Debug)]
pub(super) struct Disguise {
    pub account: PlayerId,
    pub name: String,
    /// The queue it filled was a ranked one (Elo moves, if the game is long enough).
    pub rated: bool,
    pub kind: GameKind,
}

impl Solo {
    /// A friendly game against the visible bot (Sage), as opposed to a
    /// matchmaking bot.
    pub fn is_plain(&self) -> bool {
        self.disguise.is_none()
    }
}

/// What a rematch against the bot needs to remember.
#[derive(Clone, Debug)]
pub(super) struct SoloSetup {
    pub elo: i32,
    /// The colour the human had in the game that just ended.
    pub human_color: Color,
    pub disguise: Option<Disguise>,
}

impl Hub {
    pub fn solo_start(&mut self, player: &str, elo: i64, color: SoloColor) {
        if self.player_game.contains_key(player)
            || !matches!(self.lobby_status(player), LobbyStatus::Idle)
        {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        if !bot::is_valid_elo(elo) {
            return self.fail(
                player,
                "invalid_elo",
                "the level must be between 400 and 2800",
            );
        }
        let human = match color {
            SoloColor::White => Color::White,
            SoloColor::Black => Color::Black,
            SoloColor::Random => {
                if rand::random_bool(0.5) {
                    Color::White
                } else {
                    Color::Black
                }
            }
        };
        self.start_solo(player, elo as i32, human, None, None, None, None);
    }

    /// The next of the five placement games (docs/spec-v5.md): a plain Solo
    /// game against a bot whose level follows the player's results (a win
    /// climbs a step, anything else replays it), never shown. Only accounts can be placed; the result of the
    /// game is settled by [`Hub::settle_placement`].
    pub fn placement_start(&mut self, player: &str, color: SoloColor) {
        if self.player_game.contains_key(player)
            || !matches!(self.lobby_status(player), LobbyStatus::Idle)
        {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        let account =
            matches!(self.store.player_row(player), Ok(Some(row)) if row.username.is_some());
        if !account {
            return self.fail(
                player,
                "account_required",
                "placement games need an account",
            );
        }
        let (placed, level) = match (
            self.store.is_placed(player),
            self.store.placement_next_level(player),
        ) {
            (Ok(placed), Ok(level)) => (placed, level),
            _ => return self.fail(player, "unavailable", "try again"),
        };
        if placed {
            return self.fail(player, "already_placed", "your rating is already estimated");
        }
        let human = match color {
            SoloColor::White => Color::White,
            SoloColor::Black => Color::Black,
            SoloColor::Random => {
                if rand::random_bool(0.5) {
                    Color::White
                } else {
                    Color::Black
                }
            }
        };
        self.start_solo(player, level, human, None, None, Some(level), None);
    }

    /// Starts a campaign level (docs/spec-v6.md): a game against Sage at the
    /// level's Elo, with the hands the level fixes, white for the player. It
    /// begins at once, without deck selection.
    pub fn campaign_start(&mut self, player: &str, level_id: u8, skills: Vec<SkillId>) {
        if self.player_game.contains_key(player)
            || !matches!(self.lobby_status(player), LobbyStatus::Idle)
        {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        let Some(level) = campaign::level(level_id) else {
            return self.fail(player, "no_such_level", "there is no such level");
        };
        let (progress, deck) = match (self.store.campaign_progress(player), self.deck_of(player)) {
            (Ok(progress), Ok(deck)) => (progress, deck),
            _ => return self.fail(player, "unavailable", "try again"),
        };
        if !campaign::unlocked(&progress, level_id) {
            return self.fail(player, "level_locked", "that level is not open yet");
        }
        if !campaign::valid_hand(&level, &skills, &deck) {
            return self.fail(
                player,
                "invalid_deck",
                "pick one to three distinct classic skills from your deck",
            );
        }
        let hand = if level.choose {
            skills
        } else {
            level.hand.clone()
        };
        let elo = level.elo;
        let run = CampaignRun {
            level,
            hand,
            tally: Tally::default(),
        };
        self.start_solo(player, elo, Color::White, None, None, None, Some(run));
    }

    /// Books a finished campaign level: the stars it earned, kept if they beat
    /// the best. Returns who played it and what they are told with the game
    /// over; `None` when the game is not a campaign one or was dropped before
    /// it began.
    pub(super) fn settle_campaign(
        &mut self,
        session: &Session,
        outcome: &chessy_engine::Outcome,
        recorded: bool,
    ) -> Option<(PlayerId, CampaignResult)> {
        let solo = session.solo.as_ref()?;
        let run = solo.campaign.as_ref()?;
        let Phase::Playing { game } = &session.phase else {
            return None;
        };
        if !recorded {
            return None;
        }
        let human_color = solo.bot.opposite();
        let human = session.players[human_color.index()].clone();
        // The move the game ended on: the counter moves on after Black's turn.
        let mover = match human_color {
            Color::White => game.pos.fullmove,
            Color::Black => game.pos.fullmove.saturating_sub(1),
        };
        let earned = campaign::judge(&run.level, human_color, outcome, &run.tally, mover);
        let before = self.store.campaign_progress(&human).unwrap_or_default();
        let recorded = if earned == 0 {
            None
        } else {
            match self.store.record_campaign(&human, run.level.id, earned) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::error!("could not record campaign level {}: {e}", run.level.id);
                    return None;
                }
            }
        };
        let after = self.store.campaign_progress(&human).unwrap_or_default();
        let chapter = run.level.chapter;
        let best = campaign::stars_of(&after, run.level.id);
        let total = campaign::all_ids()
            .map(|id| campaign::star_count(campaign::stars_of(&after, id)))
            .sum();
        Some((
            human,
            CampaignResult {
                level: run.level.id,
                earned,
                best,
                gained: campaign::star_count(best)
                    - campaign::star_count(campaign::stars_of(&before, run.level.id)),
                total,
                chapter_stars: campaign::chapter_stars(&after, chapter),
                boss_opened: !run.level.boss
                    && !campaign::boss_open(&before, chapter)
                    && campaign::boss_open(&after, chapter),
                forge: recorded
                    .filter(|r| r.forge_due)
                    .and_then(|_| run.level.forge_min.map(|min| ForgeDue { chapter, min })),
            },
        ))
    }

    /// Tells `player` where they stand in the campaign.
    pub fn send_campaign(&self, player: &str) {
        let Ok(progress) = self.store.campaign_progress(player) else {
            return self.fail(player, "unavailable", "try again");
        };
        let levels = campaign::all_ids()
            .filter_map(campaign::level)
            .map(|l| {
                let record = progress.get(&l.id).copied().unwrap_or_default();
                CampaignLevelView {
                    unlocked: campaign::unlocked(&progress, l.id),
                    stars: record.stars,
                    forge_pending: record.forge_pending,
                    id: l.id,
                    chapter: l.chapter,
                    index: l.index,
                    boss: l.boss,
                    elo: l.elo,
                    hand: l.hand,
                    choose: l.choose,
                    enemy: l.enemy,
                    objective: l.objective,
                    challenge: l.challenge,
                    forge_odds: l.forge_min.map(campaign::forge_odds).unwrap_or_default(),
                    forge_min: l.forge_min,
                }
            })
            .collect();
        self.send(player, ServerMsg::Campaign { levels });
    }

    /// Books a finished placement game (`recorded`: it was played far enough
    /// to be stored; one abandoned at deck selection can be played again).
    /// Returns what the player is told with the game over.
    pub(super) fn settle_placement(
        &mut self,
        session: &Session,
        game_id: &str,
        outcome: &chessy_engine::Outcome,
        recorded: bool,
    ) -> Option<PlacementView> {
        let solo = session.solo.as_ref()?;
        let level = solo.placement?;
        if !recorded {
            return None;
        }
        let human = session.players[solo.bot.opposite().index()].clone();
        let score = match outcome.winner() {
            Some(c) if c == solo.bot => 0.0,
            Some(_) => 1.0,
            None => 0.5,
        };
        match self.store.record_placement(&human, game_id, level, score) {
            Ok(done) => Some(PlacementView {
                done: done.done,
                total: done.total,
                elo: done.elo.map(|(_, after)| after),
                before: done.elo.map(|(before, _)| before),
            }),
            Err(e) => {
                tracing::error!("could not record placement game {game_id}: {e}");
                None
            }
        }
    }

    /// A game against a matchmaking bot, for a player the queue could not pair
    /// with anyone (see `fill_with_bots`).
    pub(super) fn start_disguised(
        &mut self,
        player: &str,
        bot: BotAccount,
        human: Color,
        time: Option<TimeControl>,
        rated: bool,
        kind: GameKind,
    ) {
        let disguise = Disguise {
            account: bot.id,
            name: bot.username,
            rated,
            kind,
        };
        self.start_solo(player, bot.elo, human, Some(disguise), time, None, None);
    }

    /// Opens deck selection against a bot of level `elo`; `human` is the
    /// player's colour.
    #[allow(clippy::too_many_arguments)] // one seat setup, called from three places
    fn start_solo(
        &mut self,
        player: &str,
        elo: i32,
        human: Color,
        disguise: Option<Disguise>,
        time: Option<TimeControl>,
        placement: Option<i32>,
        campaign: Option<CampaignRun>,
    ) {
        let (white, black) = match human {
            Color::White => (player.to_string(), String::new()),
            Color::Black => (String::new(), player.to_string()),
        };
        // Elo only moves against a bot standing in for a ranked opponent, and
        // not when these two have already played their share of rated games.
        let (rated, kind) = match &disguise {
            Some(d) => (d.rated && !self.pair_capped(player, &d.account), d.kind),
            None => (false, GameKind::Solo),
        };
        let seat = Solo {
            bot: human.opposite(),
            elo,
            disguise,
            placement,
            campaign,
        };
        self.open_session(white, black, rated, kind, Some(seat), time);
    }

    // ---- the bot's turn --------------------------------------------------

    /// Plans the bot's move when it is its turn in a running Solo game.
    pub(super) fn schedule_bot(&mut self, game_id: &str) {
        let Some(Session {
            phase: Phase::Playing { game },
            solo: Some(solo),
            ..
        }) = self.games.get(game_id)
        else {
            return;
        };
        if game.outcome().is_over() || game.side_to_move() != solo.bot {
            return;
        }
        let ply = game.pos.ply;
        // A bot that passes for a person takes its time, like one.
        let (min, max) = if !solo.is_plain() {
            (
                self.config.bot_human_delay_min,
                self.config.bot_human_delay_max,
            )
        } else {
            (self.config.bot_delay_min, self.config.bot_delay_max)
        };
        let span = max.saturating_sub(min).as_millis() as u64;
        let extra = if span == 0 {
            0
        } else {
            rand::random_range(0..=span)
        };
        self.timers.push((
            min + Duration::from_millis(extra),
            Timer::BotMove {
                game_id: game_id.to_string(),
                ply,
            },
        ));
    }

    /// A snapshot for the search, if the bot is still due to move at `ply`.
    pub fn bot_job(&self, game_id: &str, ply: u32) -> Option<BotJob> {
        let Some(Session {
            phase: Phase::Playing { game },
            solo: Some(solo),
            ..
        }) = self.games.get(game_id)
        else {
            return None;
        };
        if game.outcome().is_over() || game.pos.ply != ply || game.side_to_move() != solo.bot {
            return None;
        }
        let strength = Strength::from_elo(solo.elo);
        let max_think = Duration::from_millis(strength.think_ms).min(self.config.bot_think_max);
        // The bot plays the board its side sees: what Fog or Invisibility hides
        // from it is not on the board it searches (so it cannot cheat). An
        // action that is wrong on the real board is refused by `apply_bot_move`.
        let mut snapshot = (**game).clone();
        let hidden = super::view::hidden_ids(&game.pos, solo.bot);
        snapshot.pos = super::view::view_position(&game.pos, solo.bot, &hidden);
        Some(BotJob {
            game: snapshot,
            strength,
            seed: bot::seed_for(game_id, ply),
            max_think,
        })
    }

    /// Plays the bot's answer, unless the game has moved on since the
    /// snapshot (a different `ply`, finished or gone): then it is dropped.
    pub fn apply_bot_move(&mut self, game_id: &str, ply: u32, action: Option<Action>) {
        let Some(Session {
            phase: Phase::Playing { game },
            solo: Some(solo),
            draw_offer,
            recording,
            clock,
            ..
        }) = self.games.get_mut(game_id)
        else {
            return;
        };
        if game.outcome().is_over() || game.pos.ply != ply || game.side_to_move() != solo.bot {
            return;
        }
        let bot_color = solo.bot;
        // The AI only returns legal actions; if it ever did not, play any.
        let played = action
            .and_then(|a| game.apply(a).ok().map(|events| (a, events)))
            .or_else(|| {
                game.legal_actions()
                    .into_iter()
                    .find_map(|a| game.apply(a).ok().map(|events| (a, events)))
            });
        let Some((action, events)) = played else {
            return;
        };
        recording.actions.push(action);
        if let Some(run) = solo.campaign.as_mut() {
            run.tally.observe(bot_color.opposite(), bot_color, &events);
        }
        *draw_offer = None;
        let outcome = game.outcome();
        // A bot that stands in for a person plays on the clock like one (a
        // skill that keeps the turn leaves the clock running).
        let mut next_flag = None;
        if let Some(clock) = clock {
            if game.side_to_move() != bot_color || outcome.is_over() {
                clock.press(
                    bot_color,
                    Instant::now(),
                    self.config.clock_increment,
                    outcome.is_over(),
                );
                next_flag = Some((clock.remaining[bot_color.opposite().index()], game.pos.ply));
            }
        }
        if let (Some((delay, ply)), false) = (next_flag, outcome.is_over()) {
            self.timers.push((
                delay,
                Timer::Flag {
                    game_id: game_id.to_string(),
                    color: bot_color.opposite(),
                    ply,
                },
            ));
        }
        self.broadcast_state(game_id, events);
        if outcome.is_over() {
            self.finish_game(game_id, outcome, reason_of(&outcome));
        } else {
            self.schedule_bot(game_id);
        }
    }

    // ---- draws -----------------------------------------------------------

    /// The player (`offerer`) offered a draw: the bot answers at once.
    pub(super) fn solo_answer_draw(&mut self, game_id: &str, offerer: Color) {
        let Some(Session {
            phase: Phase::Playing { game },
            solo: Some(solo),
            draw_offer,
            players,
            ..
        }) = self.games.get_mut(game_id)
        else {
            return;
        };
        let human = players[offerer.index()].clone();
        let accept = bot::accepts_draw(
            &game.pos,
            solo.bot,
            self.config.bot_draw_min_plies,
            self.config.bot_draw_window,
        );
        *draw_offer = None;
        if !accept {
            return self.send(&human, ServerMsg::DrawDeclined {});
        }
        game.agree_draw();
        let outcome = game.outcome();
        self.broadcast_state(game_id, Vec::new());
        self.finish_game(game_id, outcome, "agreed_draw");
    }

    // ---- rematch ---------------------------------------------------------

    /// After a Solo game the player may ask for a rematch; the bot always agrees.
    pub(super) fn offer_solo_rematch(&mut self, session: &Session) {
        let Some(solo) = &session.solo else { return };
        let human_color = solo.bot.opposite();
        let disguise = solo.disguise.clone();
        let human = session.players[human_color.index()].clone();
        let bot_id = session.players[solo.bot.index()].clone();
        self.rematches.insert(
            human,
            Rematch::against_bot(
                bot_id,
                SoloSetup {
                    elo: solo.elo,
                    human_color,
                    disguise,
                },
            ),
        );
    }

    /// Same level, colours swapped.
    pub(super) fn start_solo_rematch(&mut self, player: &str, setup: SoloSetup) {
        if self.player_game.contains_key(player) {
            self.rematches.remove(player);
            return self.fail(player, "no_rematch", "a rematch is not possible");
        }
        // A matchmaking bot has played since: it comes back at its level now.
        let elo = match &setup.disguise {
            Some(d) => match self.store.player_row(&d.account) {
                Ok(Some(row)) => row.elo,
                _ => setup.elo,
            },
            None => setup.elo,
        };
        self.start_solo(
            player,
            elo,
            setup.human_color.opposite(),
            setup.disguise,
            None,
            None,
            None,
        );
    }
}
