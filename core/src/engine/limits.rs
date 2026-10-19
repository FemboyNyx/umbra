//! Abuse limits for unauthenticated / pre-contact traffic.
//!
//! All limits are plain constants with the defaults below; `Limits` lets
//! tests (or a future config surface) override them.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// `Hello`s accepted per peer before throttling (burst) ...
pub const HELLO_BURST: u32 = 5;
/// ... then one more per this interval.
pub const HELLO_REFILL: Duration = Duration::from_secs(10);
/// `JoinHello` per peer: its own bucket, so joins can't starve Hellos.
pub const JOIN_HELLO_BURST: u32 = 3;
pub const JOIN_HELLO_REFILL: Duration = Duration::from_secs(20);
/// At most one `ContactRequest` event per unknown identity per window.
pub const CONTACT_REQUEST_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Global `ContactRequest` event budget (burst) ...
pub const CONTACT_REQUEST_BURST: u32 = 10;
/// ... refilled by one per this interval.
pub const CONTACT_REQUEST_REFILL: Duration = Duration::from_secs(10);
/// Max distinct unknown identities with a pending (unanswered) request.
pub const MAX_PENDING_CONTACT_REQUESTS: usize = 64;
/// A pending request is forgotten after this long.
pub const PENDING_CONTACT_REQUEST_TTL: Duration = Duration::from_secs(24 * 3600);
/// One-time-key claims per contact (burst) ...
pub const OTK_CLAIM_BURST: u32 = 3;
/// ... then one more per this interval.
pub const OTK_CLAIM_REFILL: Duration = Duration::from_secs(60);
/// Fresh OTKs a single contact may hold unconsumed; beyond that it gets the
/// fallback key (sessions still work, the pool isn't drained).
pub const MAX_OUTSTANDING_OTKS_PER_PEER: usize = 2;
/// Fresh OTKs outstanding across all contacts before everyone gets the fallback key.
pub const MAX_OUTSTANDING_OTKS_TOTAL: usize = 50;
/// An unconsumed OTK stops counting against its requester after this long.
pub const OUTSTANDING_OTK_TTL: Duration = Duration::from_secs(3600);
/// Replenish the published OTK pool when fewer than this remain ...
pub const OTK_POOL_LOW_WATER: usize = 5;
/// ... by generating this many.
pub const OTK_POOL_BATCH: usize = 20;
/// Bound on per-peer limiter state.
pub const MAX_TRACKED_PEERS: usize = 1024;

#[derive(Debug, Clone)]
pub struct Limits {
    pub hello_burst: u32,
    pub hello_refill: Duration,
    pub join_hello_burst: u32,
    pub join_hello_refill: Duration,
    pub contact_request_window: Duration,
    pub contact_request_burst: u32,
    pub contact_request_refill: Duration,
    pub max_pending_contact_requests: usize,
    pub pending_contact_request_ttl: Duration,
    pub otk_claim_burst: u32,
    pub otk_claim_refill: Duration,
    pub max_outstanding_otks_per_peer: usize,
    pub max_outstanding_otks_total: usize,
    pub outstanding_otk_ttl: Duration,
    pub otk_pool_low_water: usize,
    pub otk_pool_batch: usize,
    pub max_tracked_peers: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            hello_burst: HELLO_BURST,
            hello_refill: HELLO_REFILL,
            join_hello_burst: JOIN_HELLO_BURST,
            join_hello_refill: JOIN_HELLO_REFILL,
            contact_request_window: CONTACT_REQUEST_WINDOW,
            contact_request_burst: CONTACT_REQUEST_BURST,
            contact_request_refill: CONTACT_REQUEST_REFILL,
            max_pending_contact_requests: MAX_PENDING_CONTACT_REQUESTS,
            pending_contact_request_ttl: PENDING_CONTACT_REQUEST_TTL,
            otk_claim_burst: OTK_CLAIM_BURST,
            otk_claim_refill: OTK_CLAIM_REFILL,
            max_outstanding_otks_per_peer: MAX_OUTSTANDING_OTKS_PER_PEER,
            max_outstanding_otks_total: MAX_OUTSTANDING_OTKS_TOTAL,
            outstanding_otk_ttl: OUTSTANDING_OTK_TTL,
            otk_pool_low_water: OTK_POOL_LOW_WATER,
            otk_pool_batch: OTK_POOL_BATCH,
            max_tracked_peers: MAX_TRACKED_PEERS,
        }
    }
}

/// Classic token bucket: `burst` tokens, one token added per `refill`.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    tokens: f64,
    burst: f64,
    per_sec: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(burst: u32, refill: Duration) -> Self {
        Self {
            tokens: burst as f64,
            burst: burst as f64,
            per_sec: 1.0 / refill.as_secs_f64().max(1e-9),
            last: Instant::now(),
        }
    }

    pub fn try_take(&mut self, now: Instant) -> bool {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + dt * self.per_sec).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn idle_full(&self, now: Instant) -> bool {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens + dt * self.per_sec >= self.burst
    }
}

/// Per-key token buckets with bounded memory.
#[derive(Debug)]
pub struct KeyedBuckets<K> {
    map: HashMap<K, TokenBucket>,
    burst: u32,
    refill: Duration,
    max: usize,
}

impl<K: Eq + Hash + Clone> KeyedBuckets<K> {
    pub fn new(burst: u32, refill: Duration, max: usize) -> Self {
        Self {
            map: HashMap::new(),
            burst,
            refill,
            max,
        }
    }

    pub fn try_take(&mut self, key: &K, now: Instant) -> bool {
        if !self.map.contains_key(key) && self.map.len() >= self.max {
            // Buckets that refilled completely carry no state worth keeping.
            self.map.retain(|_, b| !b.idle_full(now));
            // Still full (sybil flood): evict the least recently used bucket
            // rather than locking out every newcomer, contacts included.
            // Other limits (global budget, pending cap) still bound the flood.
            if self.map.len() >= self.max {
                if let Some(old) = self
                    .map
                    .iter()
                    .min_by_key(|(_, b)| b.last)
                    .map(|(k, _)| k.clone())
                {
                    self.map.remove(&old);
                }
            }
        }
        let (burst, refill) = (self.burst, self.refill);
        self.map
            .entry(key.clone())
            .or_insert_with(|| TokenBucket::new(burst, refill))
            .try_take(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_bursts_then_refills() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new(3, Duration::from_secs(10));
        assert!((0..3).all(|_| b.try_take(t0)));
        assert!(!b.try_take(t0));
        assert!(!b.try_take(t0 + Duration::from_secs(5)));
        assert!(b.try_take(t0 + Duration::from_secs(11)));
        assert!(!b.try_take(t0 + Duration::from_secs(11)));
    }

    #[test]
    fn keyed_buckets_are_bounded() {
        let t0 = Instant::now();
        let mut k = KeyedBuckets::new(1, Duration::from_secs(3600), 2);
        assert!(k.try_take(&1, t0));
        assert!(k.try_take(&2, t0));
        assert!(
            !k.try_take(&1, t0 + Duration::from_secs(1)),
            "own bucket empty"
        );
        // Full: the least recently used bucket (2) makes room for 3.
        assert!(k.try_take(&3, t0 + Duration::from_secs(2)));
        assert_eq!(k.map.len(), 2);
        assert!(!k.map.contains_key(&2));
        assert!(
            !k.try_take(&1, t0 + Duration::from_secs(3)),
            "1 still tracked"
        );
    }
}
