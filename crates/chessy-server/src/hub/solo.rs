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
//! Solo games are recorded for replay (`kind: solo`, the bot's seat is NULL)
//! but stay out of the public history and the ranking; no Elo, no reward.

use std::time::Duration;

use chessy_engine::ai::Strength;
use chessy_engine::{Action, Color, SkillId};

use super::social::Rematch;
use super::{Hub, Phase, Session, Timer};
use crate::bot::{self, BotJob};
use crate::campaign::{self, Level, LevelRef};
use crate::games_store::GameKind;
use crate::protocol::*;
use crate::store::reason_of;

/// Skill of the bot in a campaign boss fight: it always plays its skills.
const BOSS_SKILL_PERMILLE: u32 = 1000;

/// The bot's seat in a Solo session.
#[derive(Clone, Debug)]
pub(super) struct Solo {
    pub bot: Color,
    /// The level the player chose (400..=2800).
    pub elo: i32,
    /// Set when the game is a campaign level: its decks are imposed.
    pub campaign: Option<LevelRef>,
    /// The human's skills in a campaign level (imposed, or chosen).
    pub deck: Vec<SkillId>,
    /// Set by the debug-only `dev_finish`: the win records every star of the level.
    pub dev_all_stars: bool,
}

impl Solo {
    pub(super) fn level(&self) -> Option<&'static Level> {
        campaign::level(self.campaign?)
    }

    /// The FEN a campaign level starts from, when it is not the standard position.
    pub(super) fn start_fen(&self) -> Option<&'static str> {
        Some(self.level()?.start.as_ref()?.fen)
    }
}

/// What a rematch against the bot needs to remember.
#[derive(Clone, Debug)]
pub(super) struct SoloSetup {
    pub elo: i32,
    /// The colour the human had in the game that just ended.
    pub human_color: Color,
    pub campaign: Option<LevelRef>,
    pub deck: Vec<SkillId>,
}

impl Hub {
    /// Refuses (and says so) a player who cannot start a game against the bot.
    pub(super) fn ensure_idle(&mut self, player: &str) -> bool {
        let idle = !self.player_game.contains_key(player)
            && matches!(self.lobby_status(player), LobbyStatus::Idle);
        if !idle {
            self.fail(player, "already_in_game", "finish your current game first");
        }
        idle
    }

    pub(super) fn random_color() -> Color {
        if rand::random_bool(0.5) {
            Color::White
        } else {
            Color::Black
        }
    }

    pub fn solo_start(&mut self, player: &str, elo: i64, color: SoloColor) {
        if !self.ensure_idle(player) {
            return;
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
            SoloColor::Random => Self::random_color(),
        };
        self.start_solo(player, elo as i32, human, None, Vec::new());
    }

    /// Opens deck selection against a bot of level `elo` (a campaign level
    /// skips it, playing `deck`); `human` is the player's colour.
    pub(super) fn start_solo(
        &mut self,
        player: &str,
        elo: i32,
        human: Color,
        campaign: Option<LevelRef>,
        deck: Vec<SkillId>,
    ) {
        let (white, black) = match human {
            Color::White => (player.to_string(), String::new()),
            Color::Black => (String::new(), player.to_string()),
        };
        let seat = Solo {
            bot: human.opposite(),
            elo,
            campaign,
            deck,
            dev_all_stars: false,
        };
        self.open_session(white, black, false, GameKind::Solo, Some(seat), None);
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
        let min = self.config.bot_delay_min;
        let span = self.config.bot_delay_max.saturating_sub(min).as_millis() as u64;
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
        let mut strength = Strength::from_elo(solo.elo);
        if solo.campaign.is_some_and(LevelRef::is_boss) {
            strength.skill_permille = BOSS_SKILL_PERMILLE;
        }
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
            ..
        }) = self.games.get_mut(game_id)
        else {
            return;
        };
        if game.outcome().is_over() || game.pos.ply != ply || game.side_to_move() != solo.bot {
            return;
        }
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
        *draw_offer = None;
        let outcome = game.outcome();
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
        // A campaign level is won by mate only: no draw, whatever the bot thinks.
        if solo.campaign.is_some() {
            *draw_offer = None;
            return self.send(&human, ServerMsg::DrawDeclined {});
        }
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
        let human = session.players[human_color.index()].clone();
        let bot_id = session.players[solo.bot.index()].clone();
        self.rematches.insert(
            human,
            Rematch::against_bot(
                bot_id,
                SoloSetup {
                    elo: solo.elo,
                    human_color,
                    campaign: solo.campaign,
                    deck: solo.deck.clone(),
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
        match setup.campaign {
            Some(at) => {
                let mut deck = setup.deck;
                if campaign::level(at).is_some_and(|level| level.deck_choice) {
                    let Some(checked) = self.checked_deck(player, deck) else {
                        return;
                    };
                    deck = checked;
                }
                self.start_solo(player, at.elo(), setup.human_color, Some(at), deck)
            }
            None => self.start_solo(
                player,
                setup.elo,
                setup.human_color.opposite(),
                None,
                setup.deck,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::BOSS_LEVEL;
    use crate::hub::{Hub, HubConfig, Phase};
    use crate::store::Store;

    /// A campaign game where the human is Black, so the bot moves first.
    fn campaign_game(at: LevelRef) -> (Hub, String) {
        let mut hub = Hub::new(Store::open(":memory:").unwrap(), HubConfig::default());
        hub.start_solo("p", 1200, Color::Black, Some(at), Vec::new());
        let game_id = hub.player_game["p"].clone();
        (hub, game_id)
    }

    fn bot_job_at_start(hub: &Hub, game_id: &str) -> BotJob {
        let Phase::Playing { game } = &hub.games[game_id].phase else {
            panic!("the game is running");
        };
        hub.bot_job(game_id, game.pos.ply).expect("the bot is to move")
    }

    #[test]
    fn a_campaign_boss_bot_always_plays_its_skills() {
        let (hub, game_id) = campaign_game(LevelRef { chapter: 0, level: BOSS_LEVEL });
        let job = bot_job_at_start(&hub, &game_id);
        assert_eq!(job.strength.skill_permille, BOSS_SKILL_PERMILLE);
    }

    #[test]
    fn a_campaign_level_that_is_not_a_boss_keeps_the_elo_skill() {
        let (hub, game_id) = campaign_game(LevelRef { chapter: 0, level: 0 });
        let job = bot_job_at_start(&hub, &game_id);
        assert_eq!(job.strength.skill_permille, Strength::from_elo(1200).skill_permille);
    }

    #[test]
    fn a_campaign_never_accepts_a_draw() {
        let (mut hub, game_id) = campaign_game(LevelRef { chapter: 0, level: 0 });
        hub.solo_answer_draw(&game_id, Color::Black);
        assert!(matches!(hub.games[&game_id].phase, Phase::Playing { .. }));
    }
}
