//! AI inference backends behind one trait.
//!
//! Two implementations ship: [`client::AiClient`] streams from an
//! OpenAI-compatible gateway (OpenCode Zen by default; keyless free tier),
//! and [`local::LocalBackend`] runs a quantized GGUF micro model in-process
//! on CPU via candle. Slices consume typed [`AiEvent`] streams and never
//! know which one answered (deep-module contract).

pub mod backend;
pub mod client;
pub mod local;

pub use backend::{AiBackend, AiEventStream};
pub use client::{AiClient, AiEvent, AssembledToolCall, ChatMessage};
pub use local::LocalBackend;

#[cfg(test)]
mod tests;
