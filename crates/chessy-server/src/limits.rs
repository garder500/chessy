//! A token bucket for per-connection message quotas, and the caps on how
//! many WebSocket connections may be open at once.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What to do with the message just presented to a [`RateLimiter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Over quota: ignore the message. `first` is set for the first drop of a
    /// streak, the one time the sender is told about it.
    Drop {
        first: bool,
    },
    /// Over quota for `flood_after` messages in a row: hang up.
    Disconnect,
}

/// Refills at `rate` tokens per second up to `burst`; a message costs tokens.
#[derive(Debug)]
pub struct RateLimiter {
    rate: f64,
    burst: f64,
    tokens: f64,
    last: Instant,
    /// Messages refused since the last accepted one.
    streak: u32,
}

impl RateLimiter {
    pub fn new(rate: f64, burst: u32, now: Instant) -> Self {
        RateLimiter {
            rate,
            burst: f64::from(burst),
            tokens: f64::from(burst),
            last: now,
            streak: 0,
        }
    }

    pub fn take(&mut self, cost: u32, now: Instant, flood_after: u32) -> Verdict {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.burst);
        let cost = f64::from(cost);
        if self.tokens >= cost {
            self.tokens -= cost;
            self.streak = 0;
            return Verdict::Allow;
        }
        self.streak = self.streak.saturating_add(1);
        if self.streak >= flood_after {
            Verdict::Disconnect
        } else {
            Verdict::Drop {
                first: self.streak == 1,
            }
        }
    }
}

/// Why a connection was refused by [`ConnectionLimiter::try_acquire`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `max_connections` sockets are already open.
    ServerFull,
    /// This address already holds `max_connections_per_ip` sockets.
    TooManyFromIp,
}

#[derive(Debug, Default)]
struct Slots {
    total: usize,
    per_ip: HashMap<IpAddr, usize>,
}

/// Caps the open WebSocket connections, overall and per client address.
///
/// A limit of `0` means "unlimited". A connection whose address is unknown
/// (`None`) only counts against the global cap.
#[derive(Debug)]
pub struct ConnectionLimiter {
    max_total: usize,
    max_per_ip: usize,
    slots: Mutex<Slots>,
}

impl ConnectionLimiter {
    pub fn new(max_total: usize, max_per_ip: usize) -> Arc<Self> {
        Arc::new(ConnectionLimiter {
            max_total,
            max_per_ip,
            slots: Mutex::new(Slots::default()),
        })
    }

    /// Takes a slot, or says why not. Both counts are checked and bumped under
    /// one lock, so a refusal never leaves a count raised. The slot is given
    /// back when the returned guard is dropped.
    pub fn try_acquire(self: &Arc<Self>, ip: Option<IpAddr>) -> Result<ConnSlot, Refusal> {
        let ip = ip.map(bucket);
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if self.max_total > 0 && slots.total >= self.max_total {
            return Err(Refusal::ServerFull);
        }
        if let Some(ip) = ip {
            let held = slots.per_ip.get(&ip).copied().unwrap_or(0);
            if self.max_per_ip > 0 && held >= self.max_per_ip {
                return Err(Refusal::TooManyFromIp);
            }
            slots.per_ip.insert(ip, held + 1);
        }
        slots.total += 1;
        Ok(ConnSlot {
            limiter: Arc::clone(self),
            ip,
        })
    }

    /// Connections open right now.
    pub fn open(&self) -> usize {
        self.slots.lock().unwrap_or_else(|e| e.into_inner()).total
    }

    /// Distinct addresses currently holding a slot.
    pub fn tracked_ips(&self) -> usize {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .per_ip
            .len()
    }
}

/// The address a per-IP count is kept for: IPv4-mapped IPv6 addresses count as
/// the IPv4 address, and an IPv6 address counts as its /64 (one subscriber
/// usually owns a whole /64, so counting single addresses would be no cap).
fn bucket(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let mut octets = v6.octets();
            octets[8..].fill(0);
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        v4 => v4,
    }
}

/// One open connection's place under the caps. Dropping it gives the place
/// back, whichever way the connection (or its task) ends.
#[derive(Debug)]
#[must_use = "the slot is released as soon as the guard is dropped"]
pub struct ConnSlot {
    limiter: Arc<ConnectionLimiter>,
    ip: Option<IpAddr>,
}

impl Drop for ConnSlot {
    fn drop(&mut self) {
        let mut slots = self
            .limiter
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        slots.total = slots.total.saturating_sub(1);
        if let Some(ip) = self.ip {
            if let Some(held) = slots.per_ip.get_mut(&ip) {
                *held -= 1;
                if *held == 0 {
                    slots.per_ip.remove(&ip);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn bursts_then_refills() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(10.0, 3, t0);
        for _ in 0..3 {
            assert_eq!(l.take(1, t0, 5), Verdict::Allow);
        }
        assert_eq!(l.take(1, t0, 5), Verdict::Drop { first: true });
        assert_eq!(l.take(1, t0, 5), Verdict::Drop { first: false });
        let later = t0 + Duration::from_millis(250);
        assert_eq!(l.take(1, later, 5), Verdict::Allow, "2.5 tokens came back");
        assert_eq!(l.take(2, later, 5), Verdict::Drop { first: true });
    }

    #[test]
    fn a_long_streak_disconnects() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(1.0, 1, t0);
        assert_eq!(l.take(1, t0, 3), Verdict::Allow);
        assert!(matches!(l.take(1, t0, 3), Verdict::Drop { .. }));
        assert!(matches!(l.take(1, t0, 3), Verdict::Drop { .. }));
        assert_eq!(l.take(1, t0, 3), Verdict::Disconnect);
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn the_global_cap_refuses_then_frees() {
        let l = ConnectionLimiter::new(2, 0);
        let a = l.try_acquire(None).unwrap();
        let _b = l.try_acquire(None).unwrap();
        assert_eq!(l.try_acquire(None).unwrap_err(), Refusal::ServerFull);
        drop(a);
        let _c = l.try_acquire(None).unwrap();
        assert_eq!(l.open(), 2);
    }

    #[test]
    fn the_per_ip_cap_is_per_address_and_cleans_up() {
        let l = ConnectionLimiter::new(0, 1);
        let a = l.try_acquire(ip("10.0.0.1")).unwrap();
        assert_eq!(
            l.try_acquire(ip("10.0.0.1")).unwrap_err(),
            Refusal::TooManyFromIp
        );
        let b = l.try_acquire(ip("10.0.0.2")).unwrap();
        // A refusal leaves nothing behind.
        assert_eq!((l.open(), l.tracked_ips()), (2, 2));
        drop(a);
        drop(b);
        assert_eq!((l.open(), l.tracked_ips()), (0, 0));
    }

    #[test]
    fn unknown_addresses_only_count_globally_and_zero_is_unlimited() {
        let l = ConnectionLimiter::new(0, 1);
        let slots: Vec<_> = (0..50).map(|_| l.try_acquire(None).unwrap()).collect();
        assert_eq!((l.open(), l.tracked_ips()), (50, 0));
        drop(slots);
        assert_eq!(l.open(), 0);
    }

    #[test]
    fn ipv6_counts_by_prefix_and_mapped_addresses_as_ipv4() {
        let l = ConnectionLimiter::new(0, 1);
        let _a = l.try_acquire(ip("2001:db8::1")).unwrap();
        assert!(l.try_acquire(ip("2001:db8::ffff")).is_err(), "same /64");
        assert!(l.try_acquire(ip("2001:db8:0:1::1")).is_ok(), "other /64");
        let _b = l.try_acquire(ip("192.0.2.7")).unwrap();
        assert!(l.try_acquire(ip("::ffff:192.0.2.7")).is_err(), "mapped IPv4");
    }

    #[tokio::test]
    async fn an_aborted_task_gives_its_slot_back() {
        let l = ConnectionLimiter::new(1, 1);
        let slot = l.try_acquire(ip("10.0.0.1")).unwrap();
        let task = tokio::spawn(async move {
            let _slot = slot;
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;
        assert_eq!(l.open(), 1);
        task.abort();
        let _ = task.await;
        assert_eq!((l.open(), l.tracked_ips()), (0, 0));
    }
}
