//! One task per WebSocket: parses client messages and forwards server messages.
//!
//! Each socket is guarded on its own, before the hub lock is taken: frames
//! are rate limited (a flood closes the socket), the messages waiting to be
//! written are capped, and a write that stalls closes the socket. The hub
//! applies a second, per-player quota weighted by message cost.
//!
//! The number of open sockets is capped too (overall and per client address,
//! see `HubConfig::max_connections*`). The check happens in the upgrade
//! handler, before the hub is touched; the slot it takes is a guard owned by
//! the socket task, so it comes back however that task ends.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::app::App;
use crate::limits::{ConnSlot, RateLimiter, Refusal, Verdict};
use crate::protocol::{ClientMsg, ServerMsg};

const MAX_MESSAGE_BYTES: usize = 16 * 1024;
/// Frames tolerated before `hello`, which must come first.
const MAX_PRE_HELLO_FRAMES: u32 = 5;

/// Seconds a refused client is asked to wait (`Retry-After`).
const RETRY_AFTER_SECS: &str = "10";

/// The address of the client behind a request. With `trust_proxy` it is the
/// right-most `X-Forwarded-For` entry (the one our own proxy appended, so the
/// client cannot forge it; the left-most entries are whatever the client sent).
/// This assumes exactly one trusted proxy in front of the server. Without
/// `trust_proxy`, or if the header is missing or unreadable, it is the TCP
/// peer, which is `None` when the server was not started with connect info.
fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, trust_proxy: bool) -> Option<IpAddr> {
    if trust_proxy {
        // Several header lines count as one comma-joined list: the last line
        // holds the right-most entry.
        let forwarded = headers
            .get_all("x-forwarded-for")
            .iter()
            .next_back()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .map(str::trim)
            .and_then(|v| {
                v.parse::<IpAddr>()
                    .ok()
                    .or_else(|| v.parse::<SocketAddr>().ok().map(|a| a.ip()))
            });
        if forwarded.is_some() {
            return forwarded;
        }
    }
    peer
}

/// Upgrades `/ws`, or refuses with `503 Service Unavailable` (and a
/// `Retry-After`) when the server is full or this address holds too many
/// connections. Both caps answer 503: the client cannot tell which one it hit.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app): State<Arc<App>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
) -> Response {
    let peer = peer.map(|Extension(ConnectInfo(addr))| addr.ip());
    let ip = client_ip(&headers, peer, app.config().trust_proxy);
    let slot = match app.admit_connection(ip) {
        Ok(slot) => slot,
        Err(why) => {
            tracing::debug!("refusing a websocket from {ip:?}: {why:?}");
            let text = match why {
                Refusal::ServerFull => "server full",
                Refusal::TooManyFromIp => "too many connections from this address",
            };
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, RETRY_AFTER_SECS)],
                text,
            )
                .into_response();
        }
    };
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection(socket, app, slot))
}

async fn connection(socket: WebSocket, app: Arc<App>, slot: ConnSlot) {
    // Held until the end of the task (or until it is dropped mid-way).
    let _slot = slot;
    let config = *app.config();
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let mut identity: Option<(String, u64)> = None;
    let mut frames = RateLimiter::new(config.msg_rate, config.msg_burst, Instant::now());
    let mut pre_hello = 0u32;

    loop {
        tokio::select! {
            outgoing = rx.recv() => {
                // The hub dropped our sender: this connection was replaced.
                let Some(msg) = outgoing else { break };
                let text = serde_json::to_string(&msg).expect("server messages serialize");
                let write = tokio::time::timeout(
                    config.write_timeout,
                    sink.send(Message::Text(text.into())),
                );
                if !matches!(write.await, Ok(Ok(()))) {
                    break; // the client is gone or not reading
                }
                if msg.closes_connection() {
                    let _ = sink.send(Message::Close(None)).await;
                    break;
                }
                if rx.len() > config.outbound_queue_cap {
                    tracing::warn!("closing a slow connection ({} queued)", rx.len());
                    break;
                }
            }
            incoming = stream.next() => {
                let Some(Ok(frame)) = incoming else { break };
                let text = match frame {
                    Message::Text(t) => t,
                    Message::Close(_) => break,
                    _ => continue,
                };
                match frames.take(1, Instant::now(), config.flood_disconnect_after) {
                    Verdict::Allow => {}
                    Verdict::Drop { first } => {
                        if first {
                            let _ = tx.send(ServerMsg::error(
                                "rate_limited",
                                "you are sending messages too fast",
                            ));
                        }
                        continue;
                    }
                    Verdict::Disconnect => {
                        let _ = tx.send(ServerMsg::error(
                            "flooded",
                            "too many messages: disconnected",
                        ));
                        // The error is flushed (and the socket closed) by the arm above.
                        continue;
                    }
                }
                let msg: ClientMsg = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(_) => {
                        let _ = tx.send(ServerMsg::error("bad_message", "could not parse message"));
                        continue;
                    }
                };
                match (&identity, msg) {
                    (None, ClientMsg::Hello { token }) => match app.connect(token, tx.clone()) {
                        Ok(id) => identity = Some(id),
                        Err(e) => {
                            tracing::error!("hello failed: {e}");
                            let _ = tx.send(ServerMsg::error("internal", "internal error"));
                            break;
                        }
                    },
                    (None, _) => {
                        pre_hello += 1;
                        if pre_hello > MAX_PRE_HELLO_FRAMES {
                            break;
                        }
                        let _ = tx.send(ServerMsg::error("hello_first", "send hello first"));
                    }
                    (Some((player, conn_id)), msg) => app.handle(player, *conn_id, msg),
                }
            }
        }
    }

    if let Some((player, conn_id)) = identity {
        app.disconnect(&player, conn_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(lines: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for line in lines {
            h.append("x-forwarded-for", line.parse().unwrap());
        }
        h
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn the_header_is_ignored_unless_trusted() {
        let h = headers(&["203.0.113.9"]);
        assert_eq!(client_ip(&h, ip("10.0.0.1"), false), ip("10.0.0.1"));
        assert_eq!(client_ip(&h, None, false), None);
    }

    #[test]
    fn the_right_most_entry_wins_when_trusted() {
        let h = headers(&["6.6.6.6, 203.0.113.9"]);
        assert_eq!(client_ip(&h, ip("10.0.0.1"), true), ip("203.0.113.9"));
        // Several header lines: the last one is the proxy's.
        let h = headers(&["6.6.6.6", "7.7.7.7, 203.0.113.9"]);
        assert_eq!(client_ip(&h, ip("10.0.0.1"), true), ip("203.0.113.9"));
        let h = headers(&["203.0.113.9:4711"]);
        assert_eq!(client_ip(&h, None, true), ip("203.0.113.9"));
    }

    #[test]
    fn a_missing_or_unreadable_header_falls_back_to_the_peer() {
        assert_eq!(
            client_ip(&HeaderMap::new(), ip("10.0.0.1"), true),
            ip("10.0.0.1")
        );
        let h = headers(&["not-an-address"]);
        assert_eq!(client_ip(&h, ip("10.0.0.1"), true), ip("10.0.0.1"));
    }
}
