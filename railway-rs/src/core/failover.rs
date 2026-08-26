use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Per-source circuit-breaker with flip-flop failover.
///
/// After `threshold` consecutive *live-source* failures the circuit *opens*:
/// the source is skipped for `cooldown` without any outbound call (flip to
/// the next healthy source). When the cooldown expires the next request
/// probes the source (half-open); one success closes the circuit again.
///
/// `NotFound` is **not** a failure — it is a valid answer ("train does not
/// exist"). Only `SourceUnavailable` / `Internal` increments the counter, so
/// a missing train never trips the breaker.
///
/// Threading: the inner map is guarded by a plain `Mutex`; every method
/// locks only long enough to read/update one entry, so there is no cross-
/// await hold and no need for `tokio::sync`.
#[derive(Debug)]
pub struct Failover {
    threshold: u32,
    cooldown: Duration,
    inner: Mutex<HashMap<String, Entry>>,
}

#[derive(Debug, Clone)]
struct Entry {
    consecutive_failures: u32,
    state: State,
    opened_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Snapshot {
    pub source: String,
    pub state: State,
    pub consecutive_failures: u32,
    pub available: bool,
    /// seconds since the circuit opened, if open
    pub open_secs: Option<u64>,
}

impl Failover {
    pub fn new(threshold: u32, cooldown: Duration) -> Self {
        let t = threshold.max(1);
        Self {
            threshold: t,
            cooldown,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(3, Duration::from_secs(30))
    }

    /// Whether `source` may be called right now.
    ///
    /// Closed / HalfOpen => true. Open => true only when cooldown has
    /// elapsed (the caller then probes half-open).
    pub fn is_available(&self, source: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let entry = map.entry(source.to_string()).or_insert(Entry {
            consecutive_failures: 0,
            state: State::Closed,
            opened_at: None,
        });
        match entry.state {
            State::Closed => true,
            State::HalfOpen => true,
            State::Open => {
                if let Some(opened) = entry.opened_at {
                    if opened.elapsed() >= self.cooldown {
                        // flip to half-open: allow one probe
                        entry.state = State::HalfOpen;
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Convenience: true when the circuit says "skip this source".
    pub fn should_skip(&self, source: &str) -> bool {
        !self.is_available(source)
    }

    /// Record a success for `source`. Resets the failure counter and closes
    /// the circuit (including half-open).
    pub fn record_success(&self, source: &str) {
        let mut map = self.inner.lock().unwrap();
        let entry = map.entry(source.to_string()).or_insert(Entry {
            consecutive_failures: 0,
            state: State::Closed,
            opened_at: None,
        });
        entry.consecutive_failures = 0;
        entry.state = State::Closed;
        entry.opened_at = None;
    }

    /// Record a live-source failure. `NotFound`-class errors must NOT call
    /// this; callers are responsible for filtering.
    pub fn record_failure(&self, source: &str) {
        let mut map = self.inner.lock().unwrap();
        let entry = map.entry(source.to_string()).or_insert(Entry {
            consecutive_failures: 0,
            state: State::Closed,
            opened_at: None,
        });
        // Half-open failure re-opens immediately.
        if entry.state == State::HalfOpen {
            entry.state = State::Open;
            entry.opened_at = Some(Instant::now());
            entry.consecutive_failures = self.threshold;
            return;
        }
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
        if entry.consecutive_failures >= self.threshold && entry.state == State::Closed {
            entry.state = State::Open;
            entry.opened_at = Some(Instant::now());
        }
    }

    /// Flip-flop ordering: stable sort of `candidates` (most-preferred first in
    /// the input) so that available sources stay first and open circuits move
    /// to the tail, preserving the caller's preference among healthy sources.
    ///
    /// When NTES is open this flips `["ntes","railyatri"]` → `["railyatri","ntes"]`
    /// without the caller rebuilding the chain, and avoids paying the NTES
    /// timeout while the breaker is hot.
    pub fn ordered<'a>(&self, candidates: &[&'a str]) -> Vec<&'a str> {
        let map = self.inner.lock().unwrap();
        let mut with_avail: Vec<(&str, bool, u64)> = candidates
            .iter()
            .map(|s| {
                let avail = map
                    .get(*s)
                    .map(|e| match e.state {
                        State::Closed | State::HalfOpen => true,
                        State::Open => e
                            .opened_at
                            .map(|o| o.elapsed() >= self.cooldown)
                            .unwrap_or(false),
                    })
                    .unwrap_or(true);
                // secondary key: lower failure count first
                let fails = map
                    .get(*s)
                    .map(|e| e.consecutive_failures as u64)
                    .unwrap_or(0);
                (*s, avail, fails)
            })
            .collect();
        // stable: available first, then fewer failures, then input order
        with_avail.sort_by(|a, b| {
            b.1.cmp(&a.1) // available true first
                .then(a.2.cmp(&b.2))
        });
        with_avail.into_iter().map(|(s, _, _)| s).collect()
    }

    /// Snapshot for observability — cheap clone of the map.
    pub fn snapshot(&self) -> Vec<Snapshot> {
        let map = self.inner.lock().unwrap();
        let mut out: Vec<Snapshot> = map
            .iter()
            .map(|(k, e)| {
                let available = match e.state {
                    State::Closed | State::HalfOpen => true,
                    State::Open => e
                        .opened_at
                        .map(|o| o.elapsed() >= self.cooldown)
                        .unwrap_or(false),
                };
                Snapshot {
                    source: k.clone(),
                    state: e.state,
                    consecutive_failures: e.consecutive_failures,
                    available,
                    open_secs: e.opened_at.map(|o| o.elapsed().as_secs()),
                }
            })
            .collect();
        out.sort_by(|a, b| a.source.cmp(&b.source));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_is_available() {
        let f = Failover::new(3, Duration::from_secs(30));
        assert!(f.is_available("ntes"));
        assert!(!f.should_skip("ntes"));
    }

    #[test]
    fn trips_after_threshold() {
        let f = Failover::new(3, Duration::from_secs(30));
        f.record_failure("ntes");
        f.record_failure("ntes");
        assert!(f.is_available("ntes"));
        f.record_failure("ntes");
        assert!(!f.is_available("ntes"));
        assert!(f.should_skip("ntes"));
        let snap = f.snapshot();
        let ntes = snap.iter().find(|s| s.source == "ntes").unwrap();
        assert_eq!(ntes.state, State::Open);
        assert_eq!(ntes.consecutive_failures, 3);
    }

    #[test]
    fn success_resets_counter() {
        let f = Failover::new(3, Duration::from_secs(30));
        f.record_failure("ntes");
        f.record_failure("ntes");
        f.record_success("ntes");
        assert!(f.is_available("ntes"));
        let snap = f.snapshot();
        let ntes = snap.iter().find(|s| s.source == "ntes").unwrap();
        assert_eq!(ntes.consecutive_failures, 0);
        assert_eq!(ntes.state, State::Closed);
    }

    #[test]
    fn half_open_probe_on_cooldown_expiry() {
        let f = Failover::new(2, Duration::from_millis(30));
        f.record_failure("ntes");
        f.record_failure("ntes");
        assert!(!f.is_available("ntes"));
        std::thread::sleep(Duration::from_millis(40));
        // cooldown elapsed => half-open probe allowed
        assert!(f.is_available("ntes"));
        let snap = f.snapshot();
        let ntes = snap.iter().find(|s| s.source == "ntes").unwrap();
        assert_eq!(ntes.state, State::HalfOpen);
        // half-open success closes
        f.record_success("ntes");
        assert!(f.is_available("ntes"));
        assert_eq!(
            f.snapshot()
                .iter()
                .find(|s| s.source == "ntes")
                .unwrap()
                .state,
            State::Closed
        );
    }

    #[test]
    fn half_open_failure_reopens_immediately() {
        let f = Failover::new(2, Duration::from_millis(20));
        f.record_failure("ntes");
        f.record_failure("ntes");
        std::thread::sleep(Duration::from_millis(30));
        assert!(f.is_available("ntes")); // half-open
        f.record_failure("ntes");
        assert!(!f.is_available("ntes"));
        assert_eq!(
            f.snapshot()
                .iter()
                .find(|s| s.source == "ntes")
                .unwrap()
                .state,
            State::Open
        );
    }

    #[test]
    fn ordered_flip_flops_when_primary_open() {
        let f = Failover::new(2, Duration::from_secs(30));
        // ntes open, railyatri closed
        f.record_failure("ntes");
        f.record_failure("ntes");
        let order = f.ordered(&["ntes", "railyatri"]);
        assert_eq!(order, vec!["railyatri", "ntes"]);
        // both healthy preserves input order
        let g = Failover::new(3, Duration::from_secs(30));
        assert_eq!(g.ordered(&["ntes", "railyatri"]), vec!["ntes", "railyatri"]);
        // all open preserves failure-count ordering (equal => input order)
        let h = Failover::new(1, Duration::from_secs(30));
        h.record_failure("ntes");
        h.record_failure("railyatri");
        assert_eq!(h.ordered(&["ntes", "railyatri"]), vec!["ntes", "railyatri"]);
    }

    #[test]
    fn ordered_prefers_less_failed() {
        let f = Failover::new(10, Duration::from_secs(30));
        f.record_failure("ntes");
        f.record_failure("ntes");
        f.record_failure("railyatri");
        let order = f.ordered(&["ntes", "railyatri", "corover-api"]);
        // corover 0, railyatri 1, ntes 2
        assert_eq!(order, vec!["corover-api", "railyatri", "ntes"]);
    }

    #[test]
    fn three_source_chain_flip_flops() {
        let f = Failover::new(2, Duration::from_secs(30));
        f.record_failure("corover-api");
        f.record_failure("corover-api");
        // corover open => ntes becomes first available
        assert_eq!(
            f.ordered(&["corover-api", "ntes", "railyatri"]),
            vec!["ntes", "railyatri", "corover-api"]
        );
        f.record_failure("ntes");
        f.record_failure("ntes");
        // corover + ntes open => railyatri first
        assert_eq!(
            f.ordered(&["corover-api", "ntes", "railyatri"]),
            vec!["railyatri", "corover-api", "ntes"]
        );
    }

    #[test]
    fn snapshot_sorted_and_complete() {
        let f = Failover::new(3, Duration::from_secs(30));
        f.record_failure("zebra");
        f.record_success("apple");
        let snap = f.snapshot();
        assert_eq!(snap[0].source, "apple");
        assert_eq!(snap[1].source, "zebra");
    }
}
