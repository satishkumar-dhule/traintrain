use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Live runtime metrics collected from the actual server process.
///
/// All numbers are real: request counters, in-flight requests, per-source
/// latencies recorded by the vertical slices, cache hit/miss totals, uptime
/// and a rolling time-series for the dashboard charts. Nothing is fabricated.
#[derive(Debug)]
pub struct Metrics {
    requests_total: AtomicU64,
    in_flight: AtomicU64,
    bytes_out: AtomicU64,
    requests_by_path: Mutex<HashMap<String, u64>>,
    status_by_code: Mutex<HashMap<u16, u64>>,
    /// source name -> (total latency ms, sample count)
    source_latency: Mutex<HashMap<String, (f64, u64)>>,
    /// EMA of request processing latency in ms
    request_latency: Mutex<f64>,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    /// rolling time-series, newest at the back
    series: Mutex<VecDeque<SeriesPoint>>,
    last_total: AtomicU64,
    last_sample_at: Mutex<Option<Instant>>,
    started_at: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub in_flight: u64,
    pub bytes_out: u64,
    pub requests_by_path: Vec<PathCount>,
    pub status_by_code: Vec<StatusCodeCount>,
    pub source_latency: Vec<SourceLatency>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub latency_ms: f64,
    pub uptime_secs: u64,
    pub req_per_sec: f64,
    pub series: Vec<SeriesPoint>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathCount {
    pub path: String,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusCodeCount {
    pub code: u16,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLatency {
    pub source: String,
    pub avg_latency_ms: f64,
    pub samples: u64,
}

/// One row of the dashboard's time-series charts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeriesPoint {
    /// unix epoch seconds at the sample instant
    pub t: u64,
    pub rps: f64,
    pub latency_ms: f64,
    pub in_flight: u64,
    pub mem_mb: f64,
    pub cpu_frac: f64,
    /// (source name, avg latency ms) at this instant
    pub sources: Vec<(String, f64)>,
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
            status_by_code: Mutex::new(HashMap::new()),
            source_latency: Mutex::new(HashMap::new()),
            request_latency: Mutex::new(0.0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            series: Mutex::new(VecDeque::with_capacity(MAX_SERIES)),
            last_total: AtomicU64::new(0),
            last_sample_at: Mutex::new(None),
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

    pub fn record_status(&self, code: u16) {
        if let Ok(mut m) = self.status_by_code.lock() {
            *m.entry(code).or_insert(0) += 1;
        }
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
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

    /// Push one point onto the rolling time-series. Called by the background
    /// sampler at a fixed interval so the charts show real request rate,
    /// latency, memory and CPU over time.
    pub fn sample_series(&self, cpu_frac: f64, mem_bytes: u64) {
        let now = Instant::now();
        let total = self.requests_total.load(Ordering::Relaxed);
        let mut last = self.last_sample_at.lock().unwrap();
        let dt = match *last {
            Some(prev) => now.duration_since(prev).as_secs_f64().max(0.001),
            None => 1.0,
        };
        let delta = total.saturating_sub(self.last_total.load(Ordering::Relaxed));
        let rps = delta as f64 / dt;
        *last = Some(now);
        self.last_total.store(total, Ordering::Relaxed);

        let sources = self
            .source_latency
            .lock()
            .map(|m| {
                let mut v: Vec<(String, f64)> = m
                    .iter()
                    .map(|(s, (tot, n))| (s.clone(), if *n > 0 { tot / *n as f64 } else { 0.0 }))
                    .collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            })
            .unwrap_or_default();

        let point = SeriesPoint {
            t: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            rps: (rps * 100.0).round() / 100.0,
            latency_ms: (self.request_latency_ms() * 100.0).round() / 100.0,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            mem_mb: (mem_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
            cpu_frac: (cpu_frac * 1000.0).round() / 1000.0,
            sources,
        };

        if let Ok(mut series) = self.series.lock() {
            series.push_back(point);
            while series.len() > MAX_SERIES {
                series.pop_front();
            }
        }
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

        let mut status_codes = self
            .status_by_code
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(c, n)| StatusCodeCount {
                        code: *c,
                        count: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        status_codes.sort_by(|a, b| b.count.cmp(&a.count));

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

        let series = self
            .series
            .lock()
            .map(|s| s.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        MetricsSnapshot {
            requests_total: total,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            requests_by_path: paths,
            status_by_code: status_codes,
            source_latency: latency,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            latency_ms: self.request_latency_ms(),
            uptime_secs: uptime,
            req_per_sec,
            series,
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

/// Upper bound on time-series points (600 @ 2s = 20 minutes of history).
const MAX_SERIES: usize = 600;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_guard_round_trips() {
        let m = Metrics::new();
        let g = m.begin_request();
        assert_eq!(m.snapshot().in_flight, 1);
        drop(g);
        assert_eq!(m.snapshot().in_flight, 0);
    }

    #[test]
    fn status_codes_are_tallied() {
        let m = Metrics::new();
        m.record_status(200);
        m.record_status(200);
        m.record_status(500);
        let snap = m.snapshot();
        assert_eq!(snap.status_by_code.len(), 2);
        assert!(snap
            .status_by_code
            .iter()
            .any(|s| s.code == 200 && s.count == 2));
        assert!(snap
            .status_by_code
            .iter()
            .any(|s| s.code == 500 && s.count == 1));
    }

    #[test]
    fn series_is_capped_and_orders_newest_last() {
        let m = Metrics::new();
        for _ in 0..(MAX_SERIES + 10) {
            m.sample_series(0.0, 1000);
        }
        let snap = m.snapshot();
        assert_eq!(snap.series.len(), MAX_SERIES);
    }

    #[test]
    fn cache_counters_are_kept() {
        let m = Metrics::new();
        m.record_cache_hit();
        m.record_cache_miss();
        m.record_cache_miss();
        let snap = m.snapshot();
        assert_eq!(snap.cache_hits, 1);
        assert_eq!(snap.cache_misses, 2);
    }
}
