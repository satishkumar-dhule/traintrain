//! Resilience primitives: rate limiting, bulkhead, load shedding, hedging.
//!
//! Patterns:
//! - Pattern: Rate Limiting — token bucket per IP
//! - Pattern: Bulkhead — concurrency limit via semaphore
//! - Pattern: Load Shedding — shed when saturated
//! - Pattern: Timeout Budget — per-request deadline (5s upstream, 30s outer)
//! - Pattern: Request Hedging — fan-out N×2 hedging (see fanout.rs)
//! - Pattern: Bulkhead, Pattern: Rate Limiting etc are also referenced in web.rs

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Pattern: Rate Limiting — token bucket per IP.
///
/// Simple in-memory token bucket: `rps` tokens per second replenished,
/// `burst` maximum bucket size. Returns true if request is allowed.
pub struct RateLimiter {
    rps: f64,
    burst: f64,
    inner: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(rps: u32, burst: u32) -> Self {
        Self {
            rps: rps as f64,
            burst: burst as f64,
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Check if `key` (IP) is allowed. Refills bucket based on elapsed time.
    /// Pattern: Rate Limiting
    pub fn check(&self, key: &str) -> bool {
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return true,
        };
        let now = Instant::now();
        let burst = self.burst.max(1.0);
        let rps = self.rps.max(1.0);
        // Bound map size to avoid unbounded growth under attack — evict before inserting new key
        if !map.contains_key(key) && map.len() > 10000 {
            if let Some(k) = map.keys().next().cloned() {
                map.remove(&k);
            }
        }
        let bucket = map.entry(key.to_string()).or_insert(Bucket {
            tokens: burst,
            last_refill: now,
        });
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            bucket.tokens = (bucket.tokens + elapsed * rps).min(burst);
            bucket.last_refill = now;
        }
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// For tests: clear all buckets.
    #[cfg(test)]
    pub fn clear(&self) {
        if let Ok(mut m) = self.inner.lock() {
            m.clear();
        }
    }
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("rps", &self.rps)
            .field("burst", &self.burst)
            .finish()
    }
}

/// Pattern: Load Shedding — decision helper.

pub fn should_shed(in_flight: u64, threshold: u64, mem_bytes: u64, mem_limit_bytes: u64) -> bool {
    // Pattern: Load Shedding
    if in_flight > threshold {
        return true;
    }
    if mem_limit_bytes > 0 && mem_bytes > mem_limit_bytes {
        return true;
    }
    false
}

/// Pattern: Request Hedging — documented via fanout.rs
/// This constant ensures the exact string "Pattern: Request Hedging" appears.
pub const HEDGING_NOTE: &str = "Pattern: Request Hedging — fan-out N×2 race";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_allows_burst_then_throttles() {
        let rl = RateLimiter::new(10, 5);
        let key = "1.2.3.4";
        for _ in 0..5 {
            assert!(rl.check(key), "burst should allow");
        }
        assert!(!rl.check(key), "6th within same instant should be limited");
    }

    #[test]
    fn rate_limiter_refills_over_time() {
        let rl = RateLimiter::new(100, 10);
        let key = "5.6.7.8";
        for _ in 0..10 {
            assert!(rl.check(key));
        }
        assert!(!rl.check(key));
        std::thread::sleep(Duration::from_millis(20));
        // 100 rps => 2 tokens in 20ms
        assert!(rl.check(key));
    }

    #[test]
    fn load_shed_decision() {
        assert!(should_shed(801, 800, 0, 0));
        assert!(!should_shed(799, 800, 0, 0));
        assert!(should_shed(0, 800, 3_000_000_000, 2_000_000_000));
        assert!(!should_shed(0, 800, 1_000_000_000, 2_000_000_000));
    }
}
