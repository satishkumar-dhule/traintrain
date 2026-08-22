//! Paytm Travel (travel.paytm.com) train-search support.
//!
//! Paytm's public web search API (`GET /api/trains/v5/search`) lists direct
//! trains between two stations for a departure date with per-class
//! availability status, fares and IRCTC waitlist details - without login and
//! without the IP-geofencing that blocks IRCTC from datacenter IPs.
//!
//! The wire protocol (reverse-engineered from the travel.paytm.com web app):
//!
//! - `GET /api/trains/v5/search?departureDate=YYYYMMDD&source=<CODE>&
//!   destination=<CODE>&quota=GN&client=web&...` returns
//!   `{ body: { trains: [...] } }`; upstream failures come back as HTTP 400
//!   with `{ status: { result: "failure" } }`.
//! - Per-train `availability[]` carries the class-wise booking status
//!   (`GNWL82/WL59`, `AVAILABLE 0022`, ...), fare and Paytm's PNR prediction.
//!
//! Responses are normalized into the same intermediate shape the IRCTC
//! availability normalizer emits (`{ "trains": [...] }`), plus a per-class
//! `availability` list. Extraction is defensive but a missing/empty train
//! list is an honest `AppError::SourceUnavailable`, never fabricated data.

pub mod client;
pub mod normalize;

pub use client::PaytmClient;
