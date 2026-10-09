//! The social side of the hub: friends and presence, challenges, chat and rematches.

use std::time::Instant;

use super::{Hub, Timer};
use crate::games_store::GameKind;
use crate::protocol::*;
use crate::social::{RemoveOutcome, RequestOutcome, RespondOutcome};
use crate::store::{PlayerRow, StoreError};

pub const MAX_CHAT_CHARS: usize = 140;
const SEARCH_MIN: usize = 2;
const SEARCH_MAX: usize = 16;
const SEARCH_RESULTS: u32 = 10;

pub(super) struct Challenge {
    target: PlayerId,
    time: Option<TimeControl>,
    seq: u64,
}

pub(super) struct Rematch {
    pub opponent: PlayerId,
    rated: bool,
    kind: GameKind,
    time: Option<TimeControl>,
    /// Who played white last time; colours swap.
    white: PlayerId,
    requested_by: Option<PlayerId>,
    /// A rematch against the Solo bot: granted at once, see `solo`.
    pub solo: Option<super::solo::SoloSetup>,
}

impl Rematch {
    /// The rematch chance after a Solo game against the synthetic `bot` id.
    pub(super) fn against_bot(bot: PlayerId, setup: super::solo::SoloSetup) -> Self {
        Rematch {
            opponent: bot.clone(),
            rated: false,
            kind: GameKind::Solo,
            time: None,
            white: bot,
            requested_by: None,
            solo: Some(setup),
        }
    }
}

/// Control characters become spaces (newlines, tabs) or vanish; the result is trimmed.
pub fn clean_chat(text: &str) -> String {
    let spaced: String = text
        .chars()
        .filter_map(|c| match c {
            '\n' | '\r' | '\t' => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect();
    spaced.trim().to_string()
}

impl Hub {
    /// The caller's account row; guests get `account_required`.
    pub(super) fn account(&self, player: &str) -> Option<PlayerRow> {
        match self.store.player_row(player) {
            Ok(Some(row)) if row.username.is_some() => Some(row),
            Ok(_) => {
                self.fail(
                    player,
                    "account_required",
                    "create an account to use this feature",
                );
                None
            }
            Err(e) => {
                self.internal_error(player, e);
                None
            }
        }
    }

    pub(super) fn name_of(&self, player: &str) -> Option<String> {
        self.store.player_row(player).ok().flatten()?.username
    }

    pub(super) fn notice(&self, player: &str, code: &str, username: Option<&str>) {
        self.send(player, ServerMsg::notice(code, username));
    }

    // ---- friends and presence -------------------------------------------

    fn presence(&self, player: &str) -> Presence {
        if !self.conns.contains_key(player) {
            Presence::Offline
        } else if self.player_game.contains_key(player) {
            Presence::InGame
        } else {
            Presence::Online
        }
    }

    fn friends_message(&self, player: &str) -> Result<ServerMsg, StoreError> {
        let snap = self.store.friends_snapshot(player)?;
        Ok(ServerMsg::Friends {
            friends: snap
                .friends
                .into_iter()
                .map(|f| {
                    let presence = self.presence(&f.id);
                    FriendInfo {
                        username: f.username,
                        elo: f.elo,
                        presence,
                        last_seen: if presence == Presence::Offline {
                            f.last_seen
                        } else {
                            None
                        },
                        game_id: self.watchable_game(&f.id),
                    }
                })
                .collect(),
            incoming: snap
                .incoming
                .into_iter()
                .map(|(username, elo)| UserRef { username, elo })
                .collect(),
            outgoing: snap
                .outgoing
                .into_iter()
                .map(|username| NameRef { username })
                .collect(),
        })
    }

    /// Sends the friends picture to a connected account (guests have none).
    pub fn push_friends(&self, player: &str) {
        if !self.conns.contains_key(player) {
            return;
        }
        if !matches!(self.store.player_row(player), Ok(Some(r)) if r.username.is_some()) {
            return;
        }
        match self.friends_message(player) {
            Ok(msg) => self.send(player, msg),
            Err(e) => self.internal_error(player, e),
        }
    }

    /// Tells every connected friend that `player`'s presence may have changed.
    pub(super) fn notify_presence(&self, player: &str) {
        for friend in self.store.friend_ids(player).unwrap_or_default() {
            self.push_friends(&friend);
        }
    }

    pub fn friends_list(&self, player: &str) {
        if self.account(player).is_some() {
            self.push_friends(player);
        }
    }

    pub fn friend_request(&mut self, player: &str, username: &str) {
        let Some(me) = self.account(player) else {
            return;
        };
        let my_name = me.username.unwrap_or_default();
        match self.store.friend_request(player, username) {
            Err(e) => self.internal_error(player, e),
            Ok(RequestOutcome::UserNotFound) => {
                self.notice(player, "user_not_found", Some(username))
            }
            Ok(RequestOutcome::AlreadyFriends) => {
                self.notice(player, "already_friends", Some(username))
            }
            Ok(RequestOutcome::AlreadySent) => self.push_friends(player),
            // Looks like any sent request to the requester; the blocker hears nothing.
            Ok(RequestOutcome::SentHidden) => self.push_friends(player),
            Ok(RequestOutcome::YouBlocked) => self.fail(
                player,
                "blocked",
                "you blocked this player: unblock them first",
            ),
            Ok(RequestOutcome::Sent { target }) => {
                self.push_friends(player);
                self.push_friends(&target);
                self.notice(&target, "friend_request_received", Some(&my_name));
            }
            Ok(RequestOutcome::Accepted { target }) => {
                self.push_friends(player);
                self.push_friends(&target);
                self.notice(&target, "friend_accepted", Some(&my_name));
            }
        }
    }

    pub fn friend_respond(&mut self, player: &str, username: &str, accept: bool) {
        let Some(me) = self.account(player) else {
            return;
        };
        let my_name = me.username.unwrap_or_default();
        match self.store.friend_respond(player, username, accept) {
            Err(e) => self.internal_error(player, e),
            Ok(RespondOutcome::NoRequest) => {
                self.fail(player, "no_such_request", "there is no such friend request")
            }
            Ok(RespondOutcome::Accepted { other }) => {
                self.push_friends(player);
                self.push_friends(&other);
                self.notice(&other, "friend_accepted", Some(&my_name));
            }
            Ok(RespondOutcome::Declined { other }) => {
                self.push_friends(player);
                self.push_friends(&other);
            }
        }
    }

    pub fn friend_remove(&mut self, player: &str, username: &str) {
        let Some(me) = self.account(player) else {
            return;
        };
        let my_name = me.username.unwrap_or_default();
        match self.store.friend_remove(player, username) {
            Err(e) => self.internal_error(player, e),
            Ok(RemoveOutcome::NotFound) => self.notice(player, "user_not_found", Some(username)),
            Ok(RemoveOutcome::Removed { other, was_friend }) => {
                self.push_friends(player);
                self.push_friends(&other);
                if was_friend {
                    self.notice(&other, "friend_removed", Some(&my_name));
                }
            }
        }
    }

    pub fn user_search(&self, player: &str, query: &str) {
        if self.account(player).is_none() {
            return;
        }
        let query: String = query.trim().chars().take(64).collect();
        let len = query.chars().count();
        let valid = (SEARCH_MIN..=SEARCH_MAX).contains(&len)
            && query.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let users = if valid {
            match self.store.search_users(player, &query, SEARCH_RESULTS) {
                Ok(users) => users,
                Err(e) => return self.internal_error(player, e),
            }
        } else {
            Vec::new()
        };
        self.send(player, ServerMsg::UserResults { query, users });
    }

    // ---- challenges ------------------------------------------------------

    pub fn challenge(&mut self, player: &str, username: &str, time: Option<TimeControl>) {
        let Some(me) = self.account(player) else {
            return;
        };
        let my_name = me.username.clone().unwrap_or_default();
        if self.player_game.contains_key(player) {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        let target = match self.store.account_by_name(username) {
            Ok(Some(t)) if t.id != player => t,
            Ok(_) => return self.notice(player, "user_not_found", Some(username)),
            Err(e) => return self.internal_error(player, e),
        };
        let target_name = target.username.clone().unwrap_or_default();
        if !self.store.are_friends(player, &target.id).unwrap_or(false) {
            return self.fail(player, "not_friends", "you can only challenge friends");
        }
        if !self.conns.contains_key(&target.id) {
            return self.notice(player, "friend_offline", Some(&target_name));
        }
        if self.player_game.contains_key(&target.id) {
            return self.notice(player, "friend_busy", Some(&target_name));
        }
        // They challenged us already: challenging back means yes.
        if self
            .challenges
            .get(&target.id)
            .is_some_and(|c| c.target == player)
        {
            return self.challenge_respond(player, &target_name, true);
        }
        self.cancel_challenge(player);
        self.next_challenge += 1;
        let seq = self.next_challenge;
        self.challenges.insert(
            player.to_string(),
            Challenge {
                target: target.id.clone(),
                time,
                seq,
            },
        );
        self.timers.push((
            self.config.challenge_ttl,
            Timer::ChallengeExpire {
                challenger: player.to_string(),
                target: target.id.clone(),
                seq,
            },
        ));
        self.send(
            &target.id,
            ServerMsg::ChallengeReceived {
                from: UserRef {
                    username: my_name,
                    elo: me.elo,
                },
            },
        );
        self.send(
            player,
            ServerMsg::ChallengeSent {
                username: target_name,
            },
        );
    }

    pub fn challenge_respond(&mut self, player: &str, username: &str, accept: bool) {
        if self.account(player).is_none() {
            return;
        }
        let challenger = match self.store.account_by_name(username) {
            Ok(Some(c)) => c,
            Ok(None) => return self.fail(player, "no_challenge", "there is no such challenge"),
            Err(e) => return self.internal_error(player, e),
        };
        if !self
            .challenges
            .get(&challenger.id)
            .is_some_and(|c| c.target == player)
        {
            return self.fail(player, "no_challenge", "there is no such challenge");
        }
        let time = self.challenges.remove(&challenger.id).and_then(|c| c.time);
        if !accept {
            let my_name = self.name_of(player).unwrap_or_default();
            return self.notice(&challenger.id, "challenge_declined", Some(&my_name));
        }
        if self.player_game.contains_key(player) {
            return self.fail(player, "already_in_game", "finish your current game first");
        }
        if !self.conns.contains_key(&challenger.id) || self.player_game.contains_key(&challenger.id)
        {
            let name = challenger.username.unwrap_or_default();
            return self.notice(player, "challenge_expired", Some(&name));
        }
        self.create_game(
            challenger.id,
            player.to_string(),
            false,
            GameKind::Challenge,
            time,
        );
    }

    pub fn challenge_cancel(&mut self, player: &str) {
        if self.account(player).is_some() {
            self.cancel_challenge(player);
        }
    }

    /// Withdraws `player`'s open challenge, telling the target.
    fn cancel_challenge(&mut self, player: &str) {
        if let Some(c) = self.challenges.remove(player) {
            let name = self.name_of(player).unwrap_or_default();
            self.notice(&c.target, "challenge_cancelled", Some(&name));
        }
    }

    /// Removes every challenge `player` sent or received (they left or
    /// started a game), telling the other side.
    pub(super) fn drop_challenges(&mut self, player: &str) {
        self.cancel_challenge(player);
        let incoming: Vec<PlayerId> = self
            .challenges
            .iter()
            .filter(|(_, c)| c.target == player)
            .map(|(challenger, _)| challenger.clone())
            .collect();
        if incoming.is_empty() {
            return;
        }
        let name = self.name_of(player).unwrap_or_default();
        for challenger in incoming {
            self.challenges.remove(&challenger);
            self.notice(&challenger, "challenge_expired", Some(&name));
        }
    }

    /// Ends every open challenge between `player` (who just blocked `other`)
    /// and `other`. The blocked side is told what a plain refusal or
    /// withdrawal would tell them, never that a block happened.
    pub(super) fn drop_challenges_between(&mut self, player: &str, other: &str) {
        if self
            .challenges
            .get(other)
            .is_some_and(|c| c.target == player)
        {
            self.challenges.remove(other);
            let name = self.name_of(player).unwrap_or_default();
            self.notice(other, "challenge_declined", Some(&name));
        }
        if self
            .challenges
            .get(player)
            .is_some_and(|c| c.target == other)
        {
            self.cancel_challenge(player);
        }
    }

    pub(super) fn expire_challenge(&mut self, challenger: &str, target: &str, seq: u64) {
        match self.challenges.get(challenger) {
            Some(c) if c.seq == seq && c.target == target => {}
            _ => return,
        }
        self.challenges.remove(challenger);
        let challenger_name = self.name_of(challenger).unwrap_or_default();
        let target_name = self.name_of(target).unwrap_or_default();
        self.notice(challenger, "challenge_expired", Some(&target_name));
        self.notice(target, "challenge_expired", Some(&challenger_name));
    }

    // ---- chat ------------------------------------------------------------

    pub fn chat(&mut self, player: &str, text: &str) {
        let Some(game_id) = self.player_game.get(player).cloned() else {
            return self.fail(player, "not_in_game", "you are not in a game");
        };
        let Some(session) = self.games.get(&game_id) else {
            return;
        };
        let Some(color) = session.color_of(player) else {
            return;
        };
        let text = clean_chat(text);
        if text.is_empty() || text.chars().count() > MAX_CHAT_CHARS {
            return self.fail(
                player,
                "invalid_message",
                "messages are 1 to 140 characters long",
            );
        }
        let now = Instant::now();
        if self
            .last_chat
            .get(player)
            .is_some_and(|last| now.saturating_duration_since(*last) < self.config.chat_interval)
        {
            return self.fail(player, "rate_limited", "you are sending messages too fast");
        }
        self.last_chat.insert(player.to_string(), now);
        let opponent = session.players[color.opposite().index()].clone();
        // A muted or blocking recipient gets nothing; the sender sees their
        // own line as usual and cannot tell. Fails closed on a store error.
        let dropped = match self.store.drops_chat(&opponent, player) {
            Ok(dropped) => dropped,
            Err(e) => {
                tracing::error!("store error: {e}");
                true
            }
        };
        if !dropped {
            self.send(
                &opponent,
                ServerMsg::Chat {
                    text: text.clone(),
                    mine: false,
                },
            );
        }
        self.send(player, ServerMsg::Chat { text, mine: true });
    }

    // ---- rematches -------------------------------------------------------

    /// Called when a game ends: both players may now ask for a rematch.
    pub(super) fn offer_rematch(
        &mut self,
        players: &[PlayerId; 2],
        rated: bool,
        kind: GameKind,
        time: Option<TimeControl>,
    ) {
        for (i, player) in players.iter().enumerate() {
            self.rematches.insert(
                player.clone(),
                Rematch {
                    opponent: players[1 - i].clone(),
                    rated,
                    kind,
                    time,
                    white: players[0].clone(),
                    requested_by: None,
                    solo: None,
                },
            );
        }
    }

    /// Forgets `player`'s rematch chance, and the opponent's if it points
    /// back; the opponent hears `rematch_declined` when `notify` is set.
    pub(super) fn drop_rematch(&mut self, player: &str, notify: bool) {
        let Some(r) = self.rematches.remove(player) else {
            return;
        };
        if self
            .rematches
            .get(&r.opponent)
            .is_some_and(|o| o.opponent == player)
        {
            self.rematches.remove(&r.opponent);
            if notify {
                self.send(&r.opponent, ServerMsg::RematchDeclined {});
            }
        }
    }

    /// The pairing both sides agree on, with the opponent free to play.
    fn rematch_pair(&self, player: &str) -> Option<PlayerId> {
        let mine = self.rematches.get(player)?;
        let theirs = self.rematches.get(&mine.opponent)?;
        let free = self.conns.contains_key(&mine.opponent)
            && !self.player_game.contains_key(&mine.opponent)
            && !self.player_game.contains_key(player);
        (theirs.opponent == player && free).then(|| mine.opponent.clone())
    }

    pub fn rematch_request(&mut self, player: &str) {
        if let Some(setup) = self.rematches.get(player).and_then(|r| r.solo.clone()) {
            return self.start_solo_rematch(player, setup);
        }
        let Some(opponent) = self.rematch_pair(player) else {
            self.drop_rematch(player, false);
            return self.fail(player, "no_rematch", "a rematch is not possible");
        };
        match self.rematches[player].requested_by.as_deref() {
            Some(by) if by == player => return,
            // They asked first: asking back is accepting.
            Some(_) => return self.start_rematch(player),
            None => {}
        }
        for p in [player, opponent.as_str()] {
            if let Some(r) = self.rematches.get_mut(p) {
                r.requested_by = Some(player.to_string());
            }
        }
        self.send(&opponent, ServerMsg::RematchOffered {});
    }

    pub fn rematch_respond(&mut self, player: &str, accept: bool) {
        let asked = match (self.rematch_pair(player), self.rematches.get(player)) {
            (Some(opponent), Some(r)) if r.requested_by.as_deref() == Some(opponent.as_str()) => {
                opponent
            }
            _ => return self.fail(player, "no_rematch", "there is no rematch to answer"),
        };
        if accept {
            self.start_rematch(player);
        } else {
            self.drop_rematch(player, false);
            self.send(&asked, ServerMsg::RematchDeclined {});
        }
    }

    /// Both agreed: same pairing, colours swapped, same rated/friendly kind.
    fn start_rematch(&mut self, player: &str) {
        let Some(opponent) = self.rematch_pair(player) else {
            return;
        };
        let r = &self.rematches[player];
        let (rated, kind, time) = (r.rated, r.kind, r.time);
        let (white, black) = if r.white == player {
            (opponent, player.to_string())
        } else {
            (player.to_string(), opponent)
        };
        self.start_session(white, black, rated, kind, time);
    }
}
