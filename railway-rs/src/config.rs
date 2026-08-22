use std::path::PathBuf;
use std::time::Duration;

/// Runtime configuration, read from environment variables with sane defaults.
///
/// Environment variables:
/// - `RAILWAY_PORT`          (default `3000`)
/// - `RAILWAY_DATA_DIR`      (default `./data`)
/// - `RAILWAY_STATIC_DIR`    (default `./static`)
/// - `RAILWAY_HTTP_TIMEOUT`  seconds (default `15`)
/// - `RAILWAY_CACHE_TTL`     seconds (default `120`)
/// - `RAILWAY_USER_AGENT`    (default realistic browser UA)
/// - `RAILWAY_SOURCE_RAILYATRI_BASE` (default `https://www.railyatri.in`)
/// - `RAILWAY_SOURCE_ETRAIN_BASE`    (default `https://etrain.info`)
/// - `RAILWAY_SOURCE_NTES_BASE`      (default `https://enquiry.indianrail.gov.in`)
/// - `RAILWAY_SOURCE_IR_BASE`        (default `https://www.indianrail.gov.in`)
/// - `RAILWAY_SOURCE_IRCTC_BASE`     (default `https://www.irctc.co.in`)
/// - `RAILWAY_SOURCE_PAYTM_BASE`     (default `https://travel.paytm.com`)
/// - `RAILWAY_AI_ENABLED`    (default `true`) — master switch for AI endpoints
/// - `RAILWAY_AI_BASE`       (default `https://opencode.ai/zen/v1`) — OpenAI-compatible
///   inference gateway; override to point at any compatible server
/// - `RAILWAY_AI_MODEL`      (default `x-preview-f-free` — keyless free Zen model)
/// - `RAILWAY_AI_API_KEY`    (optional) — sent as `Authorization: Bearer` when set;
///   the free tier works without any key (no login required)
/// - `RAILWAY_AI_TIMEOUT_SECS` (default `120`) — total timeout for LLM completions
///
/// Every source URL is prefixed by these base URLs so tests can point them at
/// a local mock upstream. Real deployments keep the defaults.
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub http_timeout: Duration,
    pub cache_ttl: Duration,
    pub user_agent: String,
    pub railyatri_base: String,
    pub etrain_base: String,
    pub ntes_base: String,
    pub ir_base: String,
    pub irctc_base: String,
    pub paytm_base: String,
    pub ai_enabled: bool,
    pub ai_base: String,
    pub ai_model: String,
    pub ai_api_key: Option<String>,
    pub ai_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3000,
            data_dir: PathBuf::from("data"),
            static_dir: PathBuf::from("static"),
            http_timeout: Duration::from_secs(15),
            cache_ttl: Duration::from_secs(120),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36".to_string(),
            railyatri_base: "https://www.railyatri.in".to_string(),
            etrain_base: "https://etrain.info".to_string(),
            ntes_base: "https://enquiry.indianrail.gov.in".to_string(),
            ir_base: "https://www.indianrail.gov.in".to_string(),
            irctc_base: "https://www.irctc.co.in".to_string(),
            paytm_base: "https://travel.paytm.com".to_string(),
            ai_enabled: true,
            ai_base: "https://opencode.ai/zen/v1".to_string(),
            ai_model: "x-preview-f-free".to_string(),
            ai_api_key: None,
            ai_timeout: Duration::from_secs(120),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let d = Self::default();
        Self {
            port: env_u16("RAILWAY_PORT", d.port),
            data_dir: PathBuf::from(
                std::env::var("RAILWAY_DATA_DIR").unwrap_or_else(|_| "data".into()),
            ),
            static_dir: PathBuf::from(
                std::env::var("RAILWAY_STATIC_DIR").unwrap_or_else(|_| "static".into()),
            ),
            http_timeout: Duration::from_secs(env_u64(
                "RAILWAY_HTTP_TIMEOUT",
                d.http_timeout.as_secs(),
            )),
            cache_ttl: Duration::from_secs(env_u64("RAILWAY_CACHE_TTL", d.cache_ttl.as_secs())),
            user_agent: std::env::var("RAILWAY_USER_AGENT").unwrap_or(d.user_agent),
            railyatri_base: std::env::var("RAILWAY_SOURCE_RAILYATRI_BASE")
                .unwrap_or(d.railyatri_base),
            etrain_base: std::env::var("RAILWAY_SOURCE_ETRAIN_BASE").unwrap_or(d.etrain_base),
            ntes_base: std::env::var("RAILWAY_SOURCE_NTES_BASE").unwrap_or(d.ntes_base),
            ir_base: std::env::var("RAILWAY_SOURCE_IR_BASE").unwrap_or(d.ir_base),
            irctc_base: std::env::var("RAILWAY_SOURCE_IRCTC_BASE").unwrap_or(d.irctc_base),
            paytm_base: std::env::var("RAILWAY_SOURCE_PAYTM_BASE").unwrap_or(d.paytm_base),
            ai_enabled: std::env::var("RAILWAY_AI_ENABLED")
                .map(|v| {
                    !matches!(
                        v.trim().to_ascii_lowercase().as_str(),
                        "0" | "false" | "off" | "no"
                    )
                })
                .unwrap_or(d.ai_enabled),
            ai_base: std::env::var("RAILWAY_AI_BASE").unwrap_or(d.ai_base),
            ai_model: std::env::var("RAILWAY_AI_MODEL").unwrap_or(d.ai_model),
            ai_api_key: std::env::var("RAILWAY_AI_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            ai_timeout: Duration::from_secs(env_u64(
                "RAILWAY_AI_TIMEOUT_SECS",
                d.ai_timeout.as_secs(),
            )),
        }
    }

    /// Join a source base URL with a path/query segment.
    pub fn source_url(&self, base: &str, path: &str) -> String {
        format!("{}{}", base.trim_end_matches('/'), path)
    }
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
