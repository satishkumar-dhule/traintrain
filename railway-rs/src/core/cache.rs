use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// A minimal TTL cache for upstream responses (KISS: a `HashMap` guarded by a
/// `Mutex`, entries expire lazily on read and are swept on write).
///
/// Keys are human-readable strings like `"pnr:8456789012"` or
/// `"schedule:12951"`.
#[derive(Debug)]
pub struct Cache {
    ttl: Duration,
    inner: Mutex<HashMap<String, Entry>>,
}

#[derive(Debug)]
struct Entry {
    value: Value,
    expires_at: Instant,
}

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let mut map = self.inner.lock().ok()?;
        match map.get(key) {
            Some(e) if e.expires_at > Instant::now() => Some(e.value.clone()),
            Some(_) => {
                map.remove(key);
                None
            }
            None => None,
        }
    }

    pub fn set(&self, key: &str, value: Value) {
        let mut map = self.inner.lock().ok();
        if let Some(map) = map.as_mut() {
            map.retain(|_, e| e.expires_at > Instant::now());
            map.insert(
                key.to_string(),
                Entry {
                    value,
                    expires_at: Instant::now() + self.ttl,
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
