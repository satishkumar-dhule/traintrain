//! AI inference behind one trait.
//!
//! [`client::AiClient`] streams from an OpenAI-compatible gateway (OpenCode
//! Zen by default; keyless free tier). Slices consume typed [`AiEvent`]
//! streams and never know which gateway answered (deep-module contract).
//! The assistant UI routes known intents client-side; this relay is the
//! free-form escape hatch only.

pub mod backend;
pub mod client;

pub use backend::{AiBackend, AiEventStream};
pub use client::{AiClient, AiEvent, AssembledToolCall, ChatMessage};

#[cfg(test)]
mod tests;
