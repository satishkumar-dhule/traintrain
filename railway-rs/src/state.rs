use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::cache::Cache;
use crate::core::http::HttpClient;
use crate::core::irctc::IrctcClient;
use crate::core::metrics::{Metrics, SharedMetrics};
use crate::core::ntes::{NtesClient, NtesWebClient};
use crate::core::obs::Telemetry;
use crate::data::Datasets;

/// Shared application state handed to every handler via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http: HttpClient,
    pub cache: Arc<Cache>,
    pub metrics: SharedMetrics,
    pub telemetry: Arc<Telemetry>,
    pub ntes: NtesClient,
    pub ntes_web: NtesWebClient,
    pub irctc: IrctcClient,
    pub datasets: Arc<Datasets>,
    pub started_at: Instant,
}

impl AppState {
    /// Build state from an environment-driven config, loading the real
    /// station/train datasets from `config.data_dir`.
    pub fn from_config(config: Config) -> Result<Self, crate::core::error::AppError> {
        let http = HttpClient::new(&config.user_agent, config.http_timeout)?;
        let ntes = NtesClient::new(&http, &config.ntes_base);
        let ntes_web = NtesWebClient::new(&http, &config.ntes_base);
        let irctc = IrctcClient::new(&http, &config.irctc_base);
        let datasets = Arc::new(Datasets::load(&config.data_dir)?);
        let metrics = Arc::new(Metrics::new());
        Ok(Self {
            cache: Arc::new(Cache::with_metrics(config.cache_ttl, Some(metrics.clone()))),
            metrics,
            telemetry: Arc::new(Telemetry::new().expect("prometheus registry builds")),
            config: Arc::new(config),
            http,
            ntes,
            ntes_web,
            irctc,
            datasets,
            started_at: Instant::now(),
        })
    }

    /// Build state for tests, pointing source bases at local mocks.
    pub fn for_test(config: Config) -> Self {
        Self::from_config(config).expect("test state builds")
    }

    pub fn live_enabled(&self) -> bool {
        true
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
