//! Wire models for every `/rail-api/*` response. Field names must match the
//! frontend contract exactly (see `static/app.js`).
use serde::Serialize;

/// `GET /healthz`, `GET /api/healthz`
#[derive(Debug, Serialize)]
pub struct Healthz {
    pub status: &'static str,
    pub service: &'static str,
    pub runtime: &'static str,
}

/// `GET /rail-api/source-status`
#[derive(Debug, Serialize)]
pub struct SourceStatus {
    pub live_enabled: bool,
    pub mode: &'static str,
    pub cache_ttl_seconds: u64,
    pub primary_source: String,
    pub verification_links: Vec<&'static str>,
    pub notice: String,
    pub sources: Vec<SourceHealth>,
}

#[derive(Debug, Serialize)]
pub struct SourceHealth {
    pub name: &'static str,
    pub reachable: bool,
}

/// `GET /rail-api/pnr` (success)
#[derive(Debug, Serialize, Default)]
pub struct PnrResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pnr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<PnrEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<PnrEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passengers: Option<Vec<PnrPassenger>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PnrEndpoint {
    pub code: String,
    pub name: String,
    pub time: String,
    pub day: i64,
}

#[derive(Debug, Serialize)]
pub struct PnrPassenger {
    pub booking_status: String,
    pub coach: String,
    pub berth: String,
    pub current_status: String,
}

/// `GET /rail-api/schedule`
#[derive(Debug, Serialize, Default)]
pub struct ScheduleResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running_days: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stops: Option<Vec<ScheduleStop>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScheduleStop {
    pub code: String,
    pub name: String,
    pub arrival: String,
    pub departure: String,
    pub day: i64,
}

/// `GET /rail-api/live-status`
#[derive(Debug, Serialize, Default)]
pub struct LiveStatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_location_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stations: Option<Vec<LiveStop>>,
}

#[derive(Debug, Serialize)]
pub struct LiveStop {
    pub name: String,
    pub code: String,
    pub scheduled_arrival: String,
    pub actual_arrival: String,
    pub delay_minutes: i64,
    pub status: String,
}

/// `GET /rail-api/ntes/live-station`
#[derive(Debug, Serialize, Default)]
pub struct LiveStationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<StationTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StationTrain {
    pub number: String,
    pub name: String,
    pub sta: String,
    pub eta: String,
    pub delay_arr: bool,
    pub platform: String,
}

/// `GET /rail-api/ntes/trains-between`
#[derive(Debug, Serialize, Default)]
pub struct TrainsBetweenResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<BetweenTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BetweenTrain {
    pub number: String,
    pub name: String,
    pub departure_time: String,
    pub arrival_time: String,
    /// 7 booleans: [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
    pub runs_on: Vec<bool>,
}

/// `GET /rail-api/ntes/exceptional?type=cancelled|rescheduled|diverted`
#[derive(Debug, Serialize, Default)]
pub struct ExceptionalResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<ExceptionalTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExceptionalTrain {
    pub number: String,
    pub name: String,
    pub date: String,
    pub reason: String,
}

/// `GET /rail-api/stations` (array body)
#[derive(Debug, Serialize)]
pub struct Station {
    pub code: String,
    pub name: String,
    pub city: String,
    pub zone: String,
}

/// `GET /rail-api/search/trains` (array body)
#[derive(Debug, Serialize)]
pub struct TrainLite {
    pub number: String,
    pub name: String,
}

/// `GET /rail-api/search/stations` (array body)
#[derive(Debug, Serialize)]
pub struct StationLite {
    pub code: String,
    pub name: String,
}

/// `GET /rail-api/observability`
#[derive(Debug, Serialize)]
pub struct ObservabilityResponse {
    pub active_connections: u64,
    pub latency_ms: u64,
    pub req_per_sec: u64,
    pub cpu_usage: f64,
    pub mem_usage: u64,
    pub origins: Vec<OriginStatus>,
    pub uptime_secs: u64,
    pub requests_total: u64,
    pub top_paths: Vec<(String, u64)>,
}

#[derive(Debug, Serialize)]
pub struct OriginStatus {
    pub name: String,
    pub latency: u64,
    pub status: String,
}
