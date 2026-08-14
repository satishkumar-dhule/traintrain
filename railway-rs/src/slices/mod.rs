//! Vertical slices. Each slice owns its HTTP surface, business logic and data
//! sources in one directory (`mod.rs` = axum router + handlers, `service.rs` =
//! normalisation to `crate::models` DTOs, `sources/` = DataSource impls).
//!
//! Contract (fixed - do not change):
//! - every slice exposes `pub fn router() -> Router<AppState>`
//! - handlers take `State<AppState>`
//! - live data only; failures surface as honest `AppError`s.

pub mod exceptional;
pub mod live_station;
pub mod live_status;
pub mod observability;
pub mod pnr;
pub mod schedule;
pub mod search;
pub mod stations;
pub mod trains_between;
