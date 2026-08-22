//! AI inference over an OpenAI-compatible gateway (OpenCode Zen by default).
//!
//! The only outbound surface is HTTPS: `POST {base}/chat/completions` with
//! `stream:true` and — when configured — an `Authorization: Bearer` key. The
//! keyless free tier needs no credentials, so the app ships with AI enabled
//! out of the box. This module is the single place that knows the wire format;
//! slices consume typed [`AiEvent`] streams (deep-module contract).

pub mod client;

pub use client::{AiClient, AiEvent, ChatMessage};

#[cfg(test)]
mod tests;
