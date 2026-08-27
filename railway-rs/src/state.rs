use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::ai::{AiBackend, AiClient};
use crate::core::cache::Cache;
use crate::core::confirmtkt::ConfirmTktClient;
use crate::core::corover::CoroverClient;
use crate::core::erail::ErailClient;
use crate::core::etrain::EtrainClient;
use crate::core::failover::Failover;
use crate::core::http::HttpClient;
use crate::core::indiarailinfo::IndiaRailInfoClient;
use crate::core::irctc::IrctcClient;
use crate::core::ixigo::IxigoClient;
use crate::core::metrics::{Metrics, SharedMetrics};
use crate::core::ntes::{NtesClient, NtesWebClient};
use crate::core::obs::Telemetry;
use crate::core::paytm::PaytmClient;
use crate::core::resilience::RateLimiter;
use crate::core::retrieval::RetrievalIndex;
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
    pub paytm: PaytmClient,
    pub confirmtkt: ConfirmTktClient,
    pub ixigo: IxigoClient,
    pub erail: ErailClient,
    pub indiarailinfo: IndiaRailInfoClient,
    pub etrain: EtrainClient,
    /// Primary AI backend (OpenAI-compatible zen gateway).
    pub ai: Arc<dyn AiBackend>,
    pub datasets: Arc<Datasets>,
    /// BM25 retrieval index over stations + trains (AI RAG layer).
    pub retrieval: Arc<RetrievalIndex>,
    /// AskDISHA guest client, `Some` iff the module is enabled
    /// (`ASKDISHA_ENABLED`). When `None` the askdisha slice router is not
    /// merged and its endpoints answer 404 (zero network footprint).
    pub askdisha: Option<Arc<CoroverClient>>,
    /// Per-source circuit breaker for flip-flop failover.
    pub failover: Arc<Failover>,
    /// Pattern: Rate Limiting — per-IP token bucket
    pub rate_limiter: Arc<RateLimiter>,
    /// Pattern: Bulkhead — concurrency semaphore
    pub bulkhead: Arc<tokio::sync::Semaphore>,
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
        let paytm = PaytmClient::new(&http, &config.paytm_base);
        let confirmtkt = ConfirmTktClient::new(&http, &config.confirmtkt_base);
        let ixigo = IxigoClient::new(&http, &config.ixigo_base);
        let erail = ErailClient::new(&http, &config.erail_base);
        let indiarailinfo = IndiaRailInfoClient::new(&http, &config.indiarailinfo_base);
        let etrain = EtrainClient::new(&http, &config.etrain_base);
        let ai = Arc::new(AiClient::new(
            &config.ai_base,
            &config.ai_model,
            config.ai_api_key.clone(),
            config.ai_timeout,
        )?) as Arc<dyn AiBackend>;
        let datasets = Arc::new(Datasets::load(&config.data_dir)?);
        let retrieval = Arc::new(RetrievalIndex::build(datasets.retrieval_entries()));
        let metrics = Arc::new(Metrics::new());
        let askdisha = config.askdisha_enabled.then(|| {
            Arc::new(CoroverClient::new(
                config.corover_base.clone(),
                config.corover_cdn_base.clone(),
                config.http_timeout,
            ))
        });
        let failover = Arc::new(Failover::new(
            config.failover_threshold,
            config.failover_cooldown,
        ));
        let rate_limiter = Arc::new(RateLimiter::new(
            config.rate_limit_rps,
            config.rate_limit_burst,
        ));
        let bulkhead = Arc::new(tokio::sync::Semaphore::new(
            config.concurrency_limit.max(1),
        ));
        Ok(Self {
            cache: Arc::new(Cache::with_metrics(config.cache_ttl, Some(metrics.clone()))),
            metrics,
            telemetry: Arc::new(Telemetry::new().expect("prometheus registry builds")),
            config: Arc::new(config),
            http,
            ntes,
            ntes_web,
            irctc,
            paytm,
            confirmtkt,
            ixigo,
            erail,
            indiarailinfo,
            etrain,
            ai,
            datasets,
            retrieval,
            askdisha,
            failover,
            rate_limiter,
            bulkhead,
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
