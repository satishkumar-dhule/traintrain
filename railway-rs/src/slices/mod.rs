//! Vertical slices. Each slice owns its HTTP surface, business logic and data
//! sources in one directory (`mod.rs` = axum router + handlers, `service.rs` =
//! normalisation to `crate::models` DTOs, `sources/` = DataSource impls).
//!
//! Contract (fixed - do not change):
//! - every slice exposes `pub fn router() -> Router<AppState>`
//! - handlers take `State<AppState>`
//! - live data only; failures surface as honest `AppError`s.

pub mod ai_chat;
pub mod askdisha;
pub mod availability;
pub mod average_delay;
pub mod chart;
pub mod exceptional;
pub mod heritage;
pub mod journey_basis;
pub mod live_station;
pub mod live_status;
pub mod observability;
pub mod parcel;
pub mod pnr;
pub mod schedule;
pub mod search;
pub mod station_codes;
pub mod station_timetable;
pub mod stations;
pub mod train_on_map;
pub mod trains_between;
