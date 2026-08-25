//! Wire models for every `/rail-api/*` response. Field names must match the
//! frontend contract exactly (see `static/app.js`).
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Serialize, Deserialize, Default)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduleStop {
    pub code: String,
    pub name: String,
    pub arrival: String,
    pub departure: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day: Option<u8>,
    /// Cumulative km from the route origin; only the Ask DISHA (CoRover)
    /// source carries it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_km: Option<f64>,
}

/// `GET /rail-api/live-status`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LiveStatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_location_info: Option<String>,
    /// Platform number NTES expects the train to arrive on next
    /// (next-station platform).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_start_date: Option<String>,
    /// All run dates NTES reports for this train (`vInstanceList[].startDate`),
    /// newest/relevant run first - the same "Train Instances" list the NTES
    /// Spot Train (Live Status) page shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<Vec<TrainInstance>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stations: Option<Vec<LiveStop>>,
}

/// One run instance of a train, as reported by NTES `GetTrainInstance`.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrainInstance {
    /// Run start date in NTES `DD-MMM-YYYY` spelling (e.g. `02-May-2026`).
    pub start_date: String,
    /// NTES position text for the run (e.g. `Yet to start from its source`).
    pub position: String,
    /// Platform number NTES expects this run to arrive on next.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub platform_number: String,
    /// Full station-by-station timeline for this run (same shape as
    /// `LiveStop`), so the frontend can render tabs client-side without
    /// re-fetching.  Absent when the source does not carry per-instance
    /// stops (e.g. Railyatri fallback).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stops: Option<Vec<LiveStop>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LiveStop {
    pub name: String,
    pub code: String,
    pub scheduled_arrival: String,
    pub actual_arrival: String,
    /// Platform number reported for this stop by the source (NTES only;
    /// empty when the source does not carry per-stop platforms).
    pub platform: String,
    pub delay_minutes: i64,
    pub status: String,
}

/// `GET /rail-api/ntes/live-station`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LiveStationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    /// Optional "Going to station" filter echoed back when the request carried
    /// one (absent from the JSON otherwise).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<StationTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StationTrain {
    pub number: String,
    pub name: String,
    pub sta: String,
    pub eta: String,
    pub delay_arr: bool,
    pub platform: String,
}

/// `GET /rail-api/ntes/trains-between`
#[derive(Debug, Serialize, Deserialize, Default)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct BetweenTrain {
    pub number: String,
    pub name: String,
    pub departure_time: String,
    pub arrival_time: String,
    /// 7 booleans: [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
    pub runs_on: Vec<bool>,
}

/// `GET /rail-api/ntes/station-timetable`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StationTimetableResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<StationTimetableTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StationTimetableTrain {
    pub number: String,
    pub name: String,
    pub route: String,
    pub train_type: String,
    pub classes: String,
    pub arrival: String,
    pub departure: String,
    pub days: String,
}

/// `GET /rail-api/ntes/average-delay`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AverageDelayResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_of_run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stations: Option<Vec<AverageDelayStation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AverageDelayStation {
    pub sr: String,
    pub name: String,
    pub code: String,
    pub arrival_delay: String,
    pub departure_delay: String,
}

/// `GET /rail-api/ntes/heritage`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HeritageResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<HeritageTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HeritageTrain {
    pub number: String,
    pub name: String,
    pub runs: String,
    pub train_type: String,
    pub source_time: String,
    pub source_station: String,
    pub source_code: String,
    pub duration: String,
    pub dest_time: String,
    pub dest_station: String,
    pub dest_code: String,
}

/// `GET /rail-api/ntes/parcel`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ParcelResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<ParcelTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ParcelTrain {
    pub number: String,
    pub name: String,
    pub route: String,
    pub validity_from: String,
    pub validity_to: String,
    pub days_of_run: String,
    pub source_code: String,
    pub source_time: String,
    pub dest_code: String,
    pub dest_time: String,
    pub travel_time: String,
}

/// `GET /rail-api/ntes/journey-stations`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct JourneyStationsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stations: Option<Vec<JourneyStationInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct JourneyStationInfo {
    pub code: String,
    pub name: String,
    pub seq: usize,
    pub day_change: bool,
    pub arrival_days: String,
    pub departure_days: String,
}

/// `GET /rail-api/ntes/journey-basis`
///
/// The same shape as `LiveStatusResponse` (reused verbatim via
/// `#[serde(flatten)]`) plus the `journey_station` the run was queried from.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct JourneyBasisResponse {
    #[serde(flatten)]
    pub status: LiveStatusResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey_station: Option<JourneyStationInfo>,
}

/// `GET /rail-api/ntes/train-on-map`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TrainOnMapResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Vec<RouteStation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<Vec<TrackStation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_station: Option<MapCurrentStation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey_station: Option<MapJourneyStation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RouteStation {
    pub code: String,
    pub name: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub arrival: String,
    pub departure: String,
    pub day: String,
    pub distance: String,
    pub days_of_run: String,
    pub expected_arrival: String,
    pub actual_arrival: String,
    pub expected_departure: String,
    pub actual_departure: String,
    pub arrival_delay: String,
    pub departure_delay: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TrackStation {
    pub code: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MapCurrentStation {
    pub code: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MapJourneyStation {
    pub code: String,
    pub name: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub label: String,
    pub expected_arrival: String,
    pub actual_arrival: String,
    pub delay_status: String,
    pub platform: String,
}

/// `GET /rail-api/irctc/availability`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AvailabilityResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains: Option<Vec<AvailabilityTrain>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AvailabilityTrain {
    pub number: String,
    pub name: String,
    pub from_code: String,
    pub from_name: String,
    pub to_code: String,
    pub to_name: String,
    pub departure_time: String,
    pub arrival_time: String,
    pub duration: String,
    pub distance: String,
    pub classes: Vec<String>,
    pub train_type: String,
    /// 7 booleans: [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
    pub runs_on: Vec<bool>,
    /// Per-class booking status (Paytm source only; empty for IRCTC).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub availability: Vec<AvailabilityClass>,
}

/// Class-wise availability for one train on the journey date
/// (`GNWL82/WL59`, `AVAILABLE 0022`, ... plus fare and Paytm's PNR
/// prediction percentage when the source provides them).
#[derive(Debug, Serialize, Deserialize)]
pub struct AvailabilityClass {
    /// Class code, e.g. `SL`, `3A`.
    pub class: String,
    #[serde(default)]
    pub class_name: String,
    /// Live booking status as reported by the source.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fare: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<String>,
    /// Paytm PNR-prediction confirmation chance (0-100), when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction: Option<i64>,
}

/// `GET /rail-api/irctc/chart`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ChartResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boarding_station: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coaches: Option<Vec<ChartCoach>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChartCoach {
    pub code: String,
    pub class_code: String,
    pub berths: Vec<ChartBerth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChartBerth {
    pub number: i64,
    /// `vacant`, `occupied` or `unknown` (from the upstream status verbatim).
    pub status: String,
}

/// `GET /rail-api/ntes/exceptional?train=04138[&type=cancelled|rescheduled|diverted]`
///
/// The upstream NTES `ExcpTrains` batch form is disabled server-side, so the
/// endpoint queries one train's exception calendar (`opt=TrainRunning,
/// subOpt=excpInfo`) and caches the result for 2 hours.
#[derive(Debug, Serialize, Default)]
pub struct ExceptionalResponse {
    /// Echoes the requested `type` filter when one was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train: Option<ExceptionalTrainDetail>,
    /// Exception dates for the train (filtered to `type` when requested).
    pub exceptions: Vec<ExceptionEntry>,
    /// The NTES page's own verdict when the train has no exceptional days,
    /// verbatim: `No Exceptional Details found for train 12121 !!!`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_ttl: Option<u64>,
}

/// Static train identity from the exception-calendar header.
#[derive(Debug, Serialize)]
pub struct ExceptionalTrainDetail {
    pub number: String,
    pub name: String,
    pub source: String,
    pub destination: String,
    pub days_of_run: Vec<String>,
}

/// One exceptional run date from the per-train calendar. `kind` is
/// `cancelled`, `rescheduled`, `diverted`, `new_source` or `new_destination`;
/// `note` is the human-readable label from the NTES page.
#[derive(Debug, Serialize)]
pub struct ExceptionEntry {
    pub date: String,
    pub kind: String,
    pub note: String,
}

/// `GET /rail-api/stations` (array body)
#[derive(Debug, Serialize)]
pub struct Station {
    pub code: String,
    pub name: String,
    pub city: String,
    pub zone: String,
    // AskDISHA CDN hydration extras (F1/F2): emitted only when the hydrated
    // dataset carries a value, so unhydrated rows keep the exact old shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lng: Option<f64>,
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

/// `GET /rail-api/search/suggest` (array body) - one combined IntelliSense
/// autocomplete hit; either a station (`code`) or a train (`number`).
#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub r#type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
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
    pub bytes_out: u64,
    pub top_paths: Vec<(String, u64)>,
    pub status_codes: Vec<StatusCode>,
    pub cache: CacheStats,
    pub series: SeriesData,
    pub logs: Vec<crate::core::obs::LogEntryDto>,
}

#[derive(Debug, Serialize)]
pub struct OriginStatus {
    pub name: String,
    pub latency: u64,
    pub status: String,
    /// Upstream requests served by this origin (recorded fetch count).
    pub requests: u64,
}

/// HTTP status-code distribution (`2xx/3xx/4xx/5xx` counts).
#[derive(Debug, Serialize)]
pub struct StatusCode {
    pub code: u16,
    pub count: u64,
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub entries: usize,
}

/// Column-oriented time-series for the dashboard charts. All arrays are
/// aligned by index with `times` (oldest first).
#[derive(Debug, Serialize)]
pub struct SeriesData {
    pub times: Vec<u64>,
    pub rps: Vec<f64>,
    pub latency_ms: Vec<f64>,
    pub mem_mb: Vec<f64>,
    pub cpu_frac: Vec<f64>,
    pub in_flight: Vec<u64>,
    pub sources: Vec<SourceSeries>,
}

#[derive(Debug, Serialize)]
pub struct SourceSeries {
    pub name: String,
    pub latency_ms: Vec<f64>,
}

/// `GET /rail-api/logs` query params.
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub total: usize,
    pub limit: usize,
    pub logs: Vec<crate::core::obs::LogEntryDto>,
}
