//! IRCTC (www.irctc.co.in) no-login protocol support: Akamai session
//! bootstrap, request signing and the mobile booking-API clients.
//!
//! IRCTC does not require a login for availability and prepared-chart data,
//! but it is Akamai-protected: a first `GET /` harvests the `TS018d84e5`
//! cookie and every subsequent call must carry `Greq` (epoch ms), the page
//! `Referer` and `Origin`. It is also IP-geofenced - datacenter / non-Indian
//! IPs get an HTTP 403 - so this module is exercised hermetically in tests
//! against a mock upstream; live failures surface honestly as
//! `AppError::SourceUnavailable`.

pub mod client;
pub mod normalize;

pub use client::IrctcClient;
