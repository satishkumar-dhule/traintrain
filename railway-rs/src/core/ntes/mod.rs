//! NTES (enquiry.indianrail.gov.in) protocol support: payload crypto, mobile
//! client and public web-form client.
pub mod client;
pub mod crypto;
pub mod web;

pub use client::NtesClient;
pub use crypto::NtesCrypto;
pub use web::NtesWebClient;
