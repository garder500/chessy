//! Friendships and user search (the database side of the social features).

use rusqlite::{params, Connection, OptionalExtension};

use crate::moderation::is_blocked;
use crate::protocol::{PlayerId, Relation, SearchResult};
use crate::store::{Store, StoreResult};

#[derive(Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    UserNotFound,
    AlreadyFriends,
    /// A request from `me` to that user is already waiting.
    AlreadySent,
    Sent {
        target: PlayerId,
    },
    /// Stored like `Sent`, but `target` blocked the requester: they are told
    /// nothing and never see it (see `moderation`).
    SentHidden,
    /// The requester blocked `target`: they must unblock them first.
    YouBlocked,
    /// The other user had already asked us, so we are friends now.
    Accepted {
        target: PlayerId,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum RespondOutcome {
    NoRequest,
    Accepted { other: PlayerId },
    Declined { other: PlayerId },
}

#[derive(Debug, PartialEq, Eq)]
pub enum RemoveOutcome {
    NotFound,
    /// `was_friend` is false when only a pending request was withdrawn.
    Removed {
        other: PlayerId,
        was_friend: bool,
    },
}

pub struct FriendRow {
    pub id: PlayerId,
    pub username: String,
    pub elo: i32,
    pub last_seen: Option<String>,
}

pub struct Snapshot {
    pub friends: Vec<FriendRow>,
    /// Requests others sent us.
    pub incoming: Vec<(String, i32)>,
    /// Requests we sent that are still waiting.
    pub outgoing: Vec<String>,
}

/// `(user_a, user_b)` ordered as the table requires.
fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn status(conn: &Connection, a: &str, b: &str) -> StoreResult<Option<(String, String)>> {
    let (ua, ub) = pair(a, b);
    Ok(conn
        .query_row(
            "SELECT status, requester FROM friendships WHERE user_a = ?1 AND user_b = ?2",
            params![ua, ub],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn relation(conn: &Connection, me: &str, other: &str) -> StoreResult<Relation> {
    if me == other {
        return Ok(Relation::SelfUser);
    }
    // A request from somebody `me` blocked is stored but hidden from them.
    if is_blocked(conn, me, other)? {
        return Ok(Relation::None);
    }
    Ok(match status(conn, me, other)? {
        None => Relation::None,
        Some((s, _)) if s == "accepted" => Relation::Friend,
        Some((_, requester)) if requester == me => Relation::Outgoing,
        Some(_) => Relation::Incoming,
    })
}

impl Store {
    pub fn friend_request(&self, me: &str, username: &str) -> StoreResult<RequestOutcome> {
        let conn = self.db();
        let Some(target) = account_id(&conn, username)? else {
            return Ok(RequestOutcome::UserNotFound);
        };
        if target == me {
            return Ok(RequestOutcome::UserNotFound);
        }
        if is_blocked(&conn, me, &target)? {
            return Ok(RequestOutcome::YouBlocked);
        }
        let hidden = is_blocked(&conn, &target, me)?;
        let (ua, ub) = pair(me, &target);
        Ok(match status(&conn, me, &target)? {
            Some((s, _)) if s == "accepted" => RequestOutcome::AlreadyFriends,
            Some((_, requester)) if requester == me => RequestOutcome::AlreadySent,
            // A blocker's own request cannot be waiting (blocking deletes it),
            // but never turn a request into a friendship across a block.
            Some(_) if hidden => RequestOutcome::AlreadySent,
            Some(_) => {
                conn.execute(
                    "UPDATE friendships SET status = 'accepted' WHERE user_a = ?1 AND user_b = ?2",
                    params![ua, ub],
                )?;
                RequestOutcome::Accepted { target }
            }
            None => {
                conn.execute(
                    "INSERT INTO friendships (user_a, user_b, requester, status)
                     VALUES (?1, ?2, ?3, 'pending')",
                    params![ua, ub, me],
                )?;
                if hidden {
                    RequestOutcome::SentHidden
                } else {
                    RequestOutcome::Sent { target }
                }
            }
        })
    }

    /// Answers a request that `username` sent to `me`.
    pub fn friend_respond(
        &self,
        me: &str,
        username: &str,
        accept: bool,
    ) -> StoreResult<RespondOutcome> {
        let conn = self.db();
        let Some(other) = account_id(&conn, username)? else {
            return Ok(RespondOutcome::NoRequest);
        };
        if is_blocked(&conn, me, &other)? {
            return Ok(RespondOutcome::NoRequest);
        }
        match status(&conn, me, &other)? {
            Some((s, requester)) if s == "pending" && requester == other => {}
            _ => return Ok(RespondOutcome::NoRequest),
        }
        let (ua, ub) = pair(me, &other);
        if accept {
            conn.execute(
                "UPDATE friendships SET status = 'accepted' WHERE user_a = ?1 AND user_b = ?2",
                params![ua, ub],
            )?;
            Ok(RespondOutcome::Accepted { other })
        } else {
            conn.execute(
                "DELETE FROM friendships WHERE user_a = ?1 AND user_b = ?2",
                params![ua, ub],
            )?;
            Ok(RespondOutcome::Declined { other })
        }
    }

    /// Ends a friendship, or withdraws a request `me` sent.
    pub fn friend_remove(&self, me: &str, username: &str) -> StoreResult<RemoveOutcome> {
        let conn = self.db();
        let Some(other) = account_id(&conn, username)? else {
            return Ok(RemoveOutcome::NotFound);
        };
        let was_friend = match status(&conn, me, &other)? {
            Some((s, _)) if s == "accepted" => true,
            Some((_, requester)) if requester == me => false,
            _ => return Ok(RemoveOutcome::NotFound),
        };
        let (ua, ub) = pair(me, &other);
        conn.execute(
            "DELETE FROM friendships WHERE user_a = ?1 AND user_b = ?2",
            params![ua, ub],
        )?;
        Ok(RemoveOutcome::Removed { other, was_friend })
    }

    pub fn are_friends(&self, a: &str, b: &str) -> StoreResult<bool> {
        let conn = self.db();
        Ok(matches!(status(&conn, a, b)?, Some((s, _)) if s == "accepted"))
    }

    pub fn friend_ids(&self, me: &str) -> StoreResult<Vec<PlayerId>> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT CASE WHEN user_a = ?1 THEN user_b ELSE user_a END FROM friendships
             WHERE (user_a = ?1 OR user_b = ?1) AND status = 'accepted'",
        )?;
        let ids = stmt.query_map(params![me], |r| r.get(0))?;
        Ok(ids.collect::<Result<_, _>>()?)
    }

    pub fn friends_snapshot(&self, me: &str) -> StoreResult<Snapshot> {
        let conn = self.db();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.username, p.elo, p.last_seen, f.status, f.requester
             FROM friendships f
             JOIN players p ON p.id = CASE WHEN f.user_a = ?1 THEN f.user_b ELSE f.user_a END
             WHERE (f.user_a = ?1 OR f.user_b = ?1)
               AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.blocker = ?1 AND b.blocked = p.id)
             ORDER BY p.username_lower",
        )?;
        let rows = stmt.query_map(params![me], |r| {
            Ok((
                FriendRow {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    elo: r.get(2)?,
                    last_seen: r.get(3)?,
                },
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut snap = Snapshot {
            friends: Vec::new(),
            incoming: Vec::new(),
            outgoing: Vec::new(),
        };
        for row in rows {
            let (friend, status, requester) = row?;
            if status == "accepted" {
                snap.friends.push(friend);
            } else if requester == me {
                snap.outgoing.push(friend.username);
            } else {
                snap.incoming.push((friend.username, friend.elo));
            }
        }
        Ok(snap)
    }

    /// Accounts whose name starts with `prefix` (case-insensitive).
    pub fn search_users(
        &self,
        me: &str,
        prefix: &str,
        limit: u32,
    ) -> StoreResult<Vec<SearchResult>> {
        let conn = self.db();
        let prefix = prefix.to_ascii_lowercase();
        let mut stmt = conn.prepare(
            "SELECT id, username, elo FROM players
             WHERE username IS NOT NULL AND substr(username_lower, 1, ?2) = ?1
             ORDER BY username_lower LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![prefix, prefix.chars().count() as i64, limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i32>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut users = Vec::new();
        for (id, username, elo) in rows {
            users.push(SearchResult {
                relation: relation(&conn, me, &id)?,
                username,
                elo,
            });
        }
        Ok(users)
    }
}

fn account_id(conn: &Connection, username: &str) -> StoreResult<Option<PlayerId>> {
    Ok(conn
        .query_row(
            "SELECT id FROM players WHERE username_lower = ?1",
            params![username.to_ascii_lowercase()],
            |r| r.get(0),
        )
        .optional()?)
}
