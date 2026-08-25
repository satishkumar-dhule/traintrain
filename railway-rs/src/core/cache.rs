use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::core::metrics::SharedMetrics;

/// A minimal TTL cache for upstream responses (KISS: a `HashMap` guarded by a
/// `Mutex`, entries expire lazily on read and are swept on write).
///
/// Keys are human-readable strings like `"pnr:8456789012"` or
/// `"schedule:12951"`.
#[derive(Debug)]
pub struct Cache {
    ttl: Duration,
    inner: Mutex<HashMap<String, Entry>>,
    metrics: Option<SharedMetrics>,
}

#[derive(Debug)]
struct Entry {
    value: Value,
    expires_at: Instant,
}

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        Self::with_metrics(ttl, None)
    }

    /// Build a cache that records hit/miss counters into shared metrics
    /// (used by the observability dashboards and Prometheus `/metrics`).
    pub fn with_metrics(ttl: Duration, metrics: Option<SharedMetrics>) -> Self {
        Self {
            ttl,
            inner: Mutex::new(HashMap::new()),
            metrics,
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let mut map = self.inner.lock().ok()?;
        match map.get(key) {
            Some(e) if e.expires_at > Instant::now() => {
                if let Some(m) = &self.metrics {
                    m.record_cache_hit();
                }
                Some(e.value.clone())
            }
            Some(_) => {
                if let Some(m) = &self.metrics {
                    m.record_cache_miss();
                }
                map.remove(key);
                None
            }
            None => {
                if let Some(m) = &self.metrics {
                    m.record_cache_miss();
                }
                None
            }
        }
    }

    pub fn set(&self, key: &str, value: Value) {
        self.set_with_ttl(key, value, self.ttl);
    }

    /// Insert an entry with a per-entry TTL (overrides the cache-wide default).
    /// Used for slow-changing upstream data that deserves a longer shelf life,
    /// e.g. the 2-hour per-train NTES exception calendar.
    pub fn set_with_ttl(&self, key: &str, value: Value, ttl: Duration) {
        let mut map = self.inner.lock().ok();
        if let Some(map) = map.as_mut() {
            map.retain(|_, e| e.expires_at > Instant::now());
            map.insert(
                key.to_string(),
                Entry {
                    value,
                    expires_at: Instant::now() + ttl,
                },
            );
        }
    }

    pub fn remove(&self, key: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(key);
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| serde_json::from_value(v).ok())
    }
    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<(), serde_json::Error> {
        let v = serde_json::to_value(value)?;
        self.set(key, v);
        Ok(())
    }
    pub fn set_json_with_ttl<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), serde_json::Error> {
        let v = serde_json::to_value(value)?;
        self.set_with_ttl(key, v, ttl);
        Ok(())
    }
}
pub mod keys {
    pub fn live_status(train: &str, date: &str) -> String {
        format!("live_status:{train}:{date}")
    }
    pub fn schedule(train: &str) -> String {
        format!("schedule:{train}")
    }
    pub fn availability(src: &str, dst: &str, date: &str, source: &str) -> String {
        format!("availability:{src}:{dst}:{date}:{source}")
    }
    pub fn trains_between(src: &str, dst: &str) -> String {
        format!("trains_between:{src}:{dst}")
    }
    pub fn live_station(station: &str, hours: &str) -> String {
        format!("live_station:{station}:{hours}")
    }
    pub fn live_station_to(station: &str, hours: &str, dest: &str) -> String {
        format!("live_station:{station}:{hours}:to-{dest}")
    }
    pub fn average_delay(train: &str) -> String {
        format!("average_delay:{train}")
    }
    pub fn heritage(selection: u8) -> String {
        format!("heritage:{selection}")
    }
    pub fn parcel() -> String {
        "parcel".to_string()
    }
    pub fn exceptional(train: &str) -> String {
        format!("exceptional:{train}")
    }
    pub fn station_timetable(station: &str, date: &str) -> String {
        format!("station_timetable:{station}:{date}")
    }
    pub fn chart(train: &str, date: &str, station: &str) -> String {
        format!("irctc:chart:{train}:{date}:{station}")
    }
    pub fn train_on_map(train: &str) -> String {
        format!("train_on_map:{train}")
    }
    pub fn train_on_map_station(train: &str, station: &str) -> String {
        format!("train_on_map:{train}:{}", station.to_ascii_uppercase())
    }
    pub fn journey_basis(train: &str, station: &str) -> String {
        format!("journey_basis:{train}:{station}")
    }
    pub fn pnr(pnr: &str) -> String {
        format!("pnr:{pnr}")
    }
    pub fn search_stations(q: &str) -> String {
        format!("search:stations:{q}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let c = Cache::new(Duration::from_secs(60));
        assert!(c.get("a").is_none());
        c.set("a", json!({"x": 1}));
        assert_eq!(c.get("a"), Some(json!({"x": 1})));
    }

    #[tokio::test]
    async fn expired_entry_is_evicted() {
        let c = Cache::new(Duration::from_millis(30));
        c.set("a", json!(1));
        assert!(c.get("a").is_some());
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(c.get("a").is_none());
        assert_eq!(c.len(), 0);
    }

    #[tokio::test]
    async fn per_entry_ttl_overrides_cache_ttl() {
        let c = Cache::new(Duration::from_millis(30));
        c.set_with_ttl("long", json!(1), Duration::from_secs(60));
        c.set("short", json!(2));
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(c.get("short").is_none());
        assert!(c.get("long").is_some());
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let c = Cache::new(Duration::from_secs(60));
        c.set("pnr:1", json!("one"));
        c.set("pnr:2", json!("two"));
        assert_eq!(c.get("pnr:1"), Some(json!("one")));
        assert_eq!(c.get("pnr:2"), Some(json!("two")));
        c.remove("pnr:1");
        assert!(c.get("pnr:1").is_none());
        assert!(c.get("pnr:2").is_some());
    }
}
