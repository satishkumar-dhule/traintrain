use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Live runtime metrics collected from the actual server process.
///
/// All numbers are real: request counters, in-flight requests, per-source
/// latencies recorded by the vertical slices, uptime. Nothing is fabricated.
#[derive(Debug)]
pub struct Metrics {
    requests_total: AtomicU64,
    in_flight: AtomicU64,
    bytes_out: AtomicU64,
    requests_by_path: Mutex<HashMap<String, u64>>,
    /// source name -> (total latency ms, sample count)
    source_latency: Mutex<HashMap<String, (f64, u64)>>,
    /// EMA of request processing latency in ms
    request_latency: Mutex<f64>,
    started_at: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub in_flight: u64,
    pub bytes_out: u64,
    pub requests_by_path: Vec<PathCount>,
    pub source_latency: Vec<SourceLatency>,
    pub uptime_secs: u64,
    pub req_per_sec: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathCount {
    pub path: String,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLatency {
    pub source: String,
    pub avg_latency_ms: f64,
    pub samples: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            in_flight: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            requests_by_path: Mutex::new(HashMap::new()),
            source_latency: Mutex::new(HashMap::new()),
            request_latency: Mutex::new(0.0),
            started_at: Instant::now(),
        }
    }

    pub fn begin_request(&self) -> RequestGuard<'_> {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        RequestGuard { metrics: self }
    }

    pub fn record_path(&self, path: &str) {
        if let Ok(mut m) = self.requests_by_path.lock() {
            if m.len() >= MAX_PATHS && !m.contains_key(path) {
                return;
            }
            *m.entry(path.to_string()).or_insert(0) += 1;
        }
    }

    pub fn add_bytes(&self, n: u64) {
        self.bytes_out.fetch_add(n, Ordering::Relaxed);
    }

    /// Record the latency of a successful upstream fetch from `source`.
    pub fn record_source_latency(&self, source: &str, latency: Duration) {
        if let Ok(mut m) = self.source_latency.lock() {
            let entry = m.entry(source.to_string()).or_insert((0.0, 0));
            entry.0 += latency.as_secs_f64() * 1000.0;
            entry.1 += 1;
        }
    }

    /// Exponential moving average of request processing time (ms).
    pub fn record_request_latency(&self, latency: Duration) {
        let ms = latency.as_secs_f64() * 1000.0;
        if let Ok(mut m) = self.request_latency.lock() {
            let alpha = 0.1;
            *m = (*m) * (1.0 - alpha) + ms * alpha;
        }
    }

    pub fn request_latency_ms(&self) -> f64 {
        self.request_latency.lock().map(|m| *m).unwrap_or(0.0)
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let uptime = self.started_at.elapsed().as_secs();
        let total = self.requests_total.load(Ordering::Relaxed);
        let req_per_sec = if uptime > 0 {
            total as f64 / uptime as f64
        } else {
            0.0
        };
        let mut paths = self
            .requests_by_path
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(p, c)| PathCount {
                        path: p.clone(),
                        count: *c,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        paths.sort_by(|a, b| b.count.cmp(&a.count));
        let mut latency = self
            .source_latency
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(s, (tot, n))| SourceLatency {
                        source: s.clone(),
                        avg_latency_ms: if *n > 0 { tot / *n as f64 } else { 0.0 },
                        samples: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        latency.sort_by(|a, b| a.source.cmp(&b.source));
        MetricsSnapshot {
            requests_total: total,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            requests_by_path: paths,
            source_latency: latency,
            uptime_secs: uptime,
            req_per_sec,
        }
    }
}

/// RAII guard decrementing the in-flight counter when the request completes.
pub struct RequestGuard<'a> {
    metrics: &'a Metrics,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.metrics.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

pub type SharedMetrics = Arc<Metrics>;

/// Upper bound on distinct paths tracked, so an attacker cannot grow the
/// metrics map without limit by requesting unique URLs.
const MAX_PATHS: usize = 1024;
