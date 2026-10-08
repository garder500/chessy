//! Chat moderation (docs/spec-v2.md §4, "Modération"): block lists, the
//! "mute all chat" setting and reports. Delivery itself is filtered in
//! `social::chat`; friend requests and challenges in `social` and the store.

use super::Hub;
use crate::moderation::{BlockOutcome, NewReport, ReportOutcome};
use crate::protocol::*;

/// Longest chat excerpt kept in a report, in bytes.
pub const MAX_REPORT_CONTEXT: usize = 1024;
/// Longest game id kept in a report, in characters.
const MAX_REPORT_GAME_ID: usize = 64;

/// The excerpt with control characters turned into spaces (or dropped),
/// trimmed and cut to `MAX_REPORT_CONTEXT` bytes on a character boundary.
fn clean_context(text: &str) -> Option<String> {
    let mut clean = super::social::clean_chat(text);
    if clean.len() > MAX_REPORT_CONTEXT {
        let mut end = MAX_REPORT_CONTEXT;
        while !clean.is_char_boundary(end) {
            end -= 1;
        }
        clean.truncate(end);
    }
    (!clean.is_empty()).then_some(clean)
}

/// A plausible game id (or nothing): the id is only ever stored, never trusted.
fn clean_game_id(id: &str) -> Option<String> {
    let id = id.trim();
    let ok = !id.is_empty()
        && id.chars().count() <= MAX_REPORT_GAME_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| id.to_string())
}

impl Hub {
    fn blocks_message(&self, player: &str) -> Option<ServerMsg> {
        match self.store.blocked_names(player) {
            Ok(names) => Some(ServerMsg::Blocks {
                blocked: names
                    .into_iter()
                    .map(|username| NameRef { username })
                    .collect(),
            }),
            Err(e) => {
                self.internal_error(player, e);
                None
            }
        }
    }

    fn push_blocks(&self, player: &str) {
        if let Some(msg) = self.blocks_message(player) {
            self.send(player, msg);
        }
    }

    pub fn blocks_list(&self, player: &str) {
        if self.account(player).is_some() {
            self.push_blocks(player);
        }
    }

    /// Blocks `username`: their chat no longer reaches `player`, their friend
    /// requests and challenges go nowhere, and an existing friendship ends.
    /// The blocked side is told only what ending a friendship tells them.
    pub fn block_user(&mut self, player: &str, username: &str) {
        let Some(me) = self.account(player) else {
            return;
        };
        let my_name = me.username.unwrap_or_default();
        match self.store.block_user(player, username) {
            Err(e) => self.internal_error(player, e),
            Ok(BlockOutcome::UserNotFound) => self.notice(player, "user_not_found", Some(username)),
            Ok(BlockOutcome::SelfBlock) => {
                self.fail(player, "invalid_target", "you cannot block yourself")
            }
            Ok(BlockOutcome::Full) => {
                self.fail(player, "block_list_full", "your block list is full")
            }
            Ok(BlockOutcome::Blocked { other, removed }) => {
                self.drop_challenges_between(player, &other);
                if removed.is_some() {
                    // Same as `friend_remove`: their list changes, and a
                    // friend hears `friend_removed`.
                    self.push_friends(&other);
                    if removed == Some(true) {
                        self.notice(&other, "friend_removed", Some(&my_name));
                    }
                    self.push_friends(player);
                }
                self.push_blocks(player);
            }
        }
    }

    pub fn unblock_user(&mut self, player: &str, username: &str) {
        if self.account(player).is_none() {
            return;
        }
        match self.store.unblock_user(player, username) {
            Err(e) => self.internal_error(player, e),
            Ok(None) => self.notice(player, "user_not_found", Some(username)),
            Ok(Some(())) => {
                // A request the blocker could not see may be waiting now.
                self.push_friends(player);
                self.push_blocks(player);
            }
        }
    }

    pub fn set_chat_muted(&mut self, player: &str, muted: bool) {
        if self.account(player).is_none() {
            return;
        }
        match self.store.set_chat_muted(player, muted) {
            Err(e) => self.internal_error(player, e),
            Ok(()) => self.send(player, ServerMsg::ChatSettings { chat_muted: muted }),
        }
    }

    /// Files a report against `username`. The reporter gets `report_ack`
    /// whether or not it was a duplicate; the target is never told.
    pub fn report_user(
        &self,
        player: &str,
        username: &str,
        reason: ReportReason,
        game_id: Option<String>,
        context: Option<String>,
    ) {
        if self.account(player).is_none() {
            return;
        }
        let target = match self.store.account_by_name(username) {
            Ok(Some(t)) if t.id != player => t,
            Ok(Some(_)) => {
                return self.fail(player, "invalid_target", "you cannot report yourself")
            }
            Ok(None) => return self.notice(player, "user_not_found", Some(username)),
            Err(e) => return self.internal_error(player, e),
        };
        let target_name = target.username.clone().unwrap_or_default();
        let game_id = game_id.as_deref().and_then(clean_game_id);
        let context = context.as_deref().and_then(clean_context);
        let report = NewReport {
            reporter: player,
            target: &target.id,
            reason: reason.as_str(),
            game_id: game_id.as_deref(),
            context: context.as_deref(),
        };
        let window = self.config.report_window.as_secs();
        match self
            .store
            .file_report(&report, window, self.config.report_max)
        {
            Err(e) => self.internal_error(player, e),
            Ok(ReportOutcome::Limited) => {
                self.fail(player, "rate_limited", "you filed too many reports")
            }
            Ok(outcome) => {
                if outcome == ReportOutcome::Filed {
                    // The excerpt is user text: only its size goes to the log.
                    tracing::warn!(
                        reporter = %player,
                        target = %target.id,
                        reason = reason.as_str(),
                        game_id = ?game_id,
                        context_bytes = context.as_ref().map_or(0, |c| c.len()),
                        "player report filed"
                    );
                }
                self.send(
                    player,
                    ServerMsg::ReportAck {
                        username: target_name,
                    },
                );
            }
        }
    }
}
