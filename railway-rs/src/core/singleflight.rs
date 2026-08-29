use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use tokio::sync::OnceCell;

/// KISS single-flight / stampede coalescing — Pattern: Singleflight.
///
/// Concurrent callers with the same key share one in-flight future; the
/// winner's result is cloned to all waiters. No thundering herd on cache
/// miss (hot train right after TTL expiry = one fan-out, not N).
///
/// - `AppError: Clone` so the result can be fanned to waiters without re-executing.
/// - Map entry is removed as soon as the cell is filled; at most one entry per in-flight key.
///
/// DRY usage:
/// ```ignore
/// let v = state.singleflight.do_or_try("live_status:12951:2026-05-06", || fanout_n2(...)).await?;
/// ```
#[derive(Debug)]
pub struct SingleFlight<K> {
    inner: Mutex<HashMap<K, Arc<OnceCell<SharedResult>>>>,
}

type SharedResult = Result<serde_json::Value, crate::core::error::AppError>;

impl<K> Default for SingleFlight<K>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K> SingleFlight<K>
where
    K: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Execute `f` once per distinct `key` concurrently; concurrent callers
    /// with the same key wait for the winner and receive a cloned result.
    pub async fn do_or_try<F, Fut>(&self, key: K, f: F) -> SharedResult
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = SharedResult>,
    {
        let cell = {
            let mut map = self.inner.lock().unwrap();
            if let Some(c) = map.get(&key) {
                Arc::clone(c)
            } else {
                let c = Arc::new(OnceCell::new());
                map.insert(key.clone(), Arc::clone(&c));
                c
            }
        };
        // Winner runs f(), losers wait for the same cell.
        let shared = cell.get_or_init(f).await;
        let out = shared.clone();

        // Cleanup: remove if no other waiter is still holding the Arc.
        // Strong count 2 = 1 in map + 1 here → safe to remove.
        if Arc::strong_count(&cell) == 2 {
            let mut map = self.inner.lock().unwrap();
            if let Some(stored) = map.get(&key) {
                if Arc::ptr_eq(stored, &cell) {
                    map.remove(&key);
                }
            }
        }
        out
    }

    /// Convenience for DTOs: serialize to Value on success.
    pub async fn do_or_try_json<T, F, Fut>(
        &self,
        key: K,
        f: F,
    ) -> Result<T, crate::core::error::AppError>
    where
        T: serde::de::DeserializeOwned + serde::Serialize + Clone,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::core::error::AppError>>,
    {
        let val = self
            .do_or_try(key, || async {
                let t = f().await?;
                serde_json::to_value(&t)
                    .map_err(|e| crate::core::error::AppError::internal(format!("encode: {e}")))
            })
            .await?;
        serde_json::from_value(val)
            .map_err(|e| crate::core::error::AppError::internal(format!("decode: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn coalesces_concurrent_callers() {
        let sf = Arc::new(SingleFlight::<String>::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..10 {
            let sf2 = sf.clone();
            let calls2 = calls.clone();
            handles.push(tokio::spawn(async move {
                sf2.do_or_try("k".to_string(), || async {
                    calls2.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    Ok::<_, crate::core::error::AppError>(serde_json::json!({"ok":1}))
                })
                .await
            }));
        }
        let results = futures::future::join_all(handles).await;
        for r in results {
            assert_eq!(r.unwrap().unwrap()["ok"], 1);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one flight for 10 waiters");
    }

    #[tokio::test]
    async fn different_keys_run_independently() {
        let sf = SingleFlight::<String>::new();
        let a = sf
            .do_or_try("a".to_string(), || async {
                Ok::<_, crate::core::error::AppError>(serde_json::json!(1))
            })
            .await
            .unwrap();
        let b = sf
            .do_or_try("b".to_string(), || async {
                Ok::<_, crate::core::error::AppError>(serde_json::json!(2))
            })
            .await
            .unwrap();
        assert_eq!(a, serde_json::json!(1));
        assert_eq!(b, serde_json::json!(2));
    }
}
