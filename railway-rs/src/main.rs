use std::net::SocketAddr;
use std::time::Duration;

use railway_rs::config::Config;
use railway_rs::core::obs::{log_ring, proc_stats, LogRingLayer};
use railway_rs::state::AppState;
use railway_rs::web;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Holds the non-blocking writer guards for the whole process lifetime.
/// Dropping these would silently stop flushing log records.
struct LogGuards {
    _stdout: tracing_appender::non_blocking::WorkerGuard,
    _file: tracing_appender::non_blocking::WorkerGuard,
}

/// Structured JSON logging (state of the art: grep-able, one JSON object per
/// line, Prometheus/Grafana/Loki friendly) written to stdout and a rolling
/// daily file under `RAILWAY_LOG_DIR` (default `logs/`), plus a mirrored
/// in-memory ring served over HTTP for the dashboard's live log stream.
///
/// Set `RAILWAY_LOG_FORMAT=pretty` for a human-readable console during local
/// development; the file output stays JSON either way.
fn init_tracing() -> LogGuards {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));

    let log_dir = std::env::var("RAILWAY_LOG_DIR").unwrap_or_else(|_| "logs".into());
    let _ = std::fs::create_dir_all(&log_dir);
    let format = std::env::var("RAILWAY_LOG_FORMAT").unwrap_or_else(|_| "json".into());

    let (file_writer, file_guard) = tracing_appender::non_blocking(
        tracing_appender::rolling::daily(&log_dir, "railway-companion.log"),
    );
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_filter(filter.clone());

    let stdout_layer = if format == "pretty" {
        tracing_subscriber::fmt::layer()
            .pretty()
            .with_writer(stdout_writer)
            .with_filter(filter.clone())
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(stdout_writer)
            .with_filter(filter.clone())
            .boxed()
    };

    let ring_layer = LogRingLayer::new(log_ring().clone()).with_filter(filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .with(ring_layer)
        .init();

    LogGuards {
        _stdout: stdout_guard,
        _file: file_guard,
    }
}

/// Background sampler: every 2s push a real time-series point (request rate,
/// latency, in-flight, RSS, CPU) and refresh the Prometheus gauges.
fn spawn_sampler(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;
            let (cpu, mem) = proc_stats();
            let snap = state.metrics.snapshot();
            state.metrics.sample_series(cpu, mem);
            state
                .telemetry
                .sample(&snap, cpu, mem, state.uptime_secs(), state.cache.len());
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guards = init_tracing();

    let config = Config::from_env();
    // Candle's CPU kernels parallelize via rayon; honor the configured local
    // thread budget before any inference can start.
    if config.ai_backend != railway_rs::config::AiBackendPolicy::Zen && config.ai_local_threads > 0
    {
        std::env::set_var("RAYON_NUM_THREADS", config.ai_local_threads.to_string());
    }
    let state = AppState::from_config(config.clone())?;

    spawn_sampler(state.clone());

    let stations = state.datasets.stations.len();
    let trains = state.datasets.trains.len();
    let app = web::router(state, config.static_dir.clone());

    let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
    tracing::info!(
        "Train Bro (railway-rs) serving on {addr} | live sources: railyatri, etrain, NTES, IRCTC | data: {stations} stations, {trains} trains | observability: /metrics, /rail-api/observability, /rail-api/logs"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
