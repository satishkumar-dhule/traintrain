//! Backend-neutral contract for AI inference.
//!
//! Slices consume [`AiBackend`] objects and speak only [`ChatMessage`] in /
//! [`AiEvent`] streams out, so the upstream gateway (Zen) and any in-process
//! engine (local GGUF) are interchangeable behind one trait.

use async_trait::async_trait;
use futures::Stream;

use super::client::{AiEvent, ChatMessage};
use crate::core::error::AppError;

/// One model round's decoded event stream (boxed so every backend shares one
/// concrete type across callers' loops).
pub type AiEventStream = std::pin::Pin<Box<dyn Stream<Item = Result<AiEvent, AppError>> + Send>>;

/// A chat-completion provider. Implementations must be cloneable-shared
/// (`Arc`) and callable from multiple requests concurrently; concurrency
/// control (e.g. a local single-flight queue) is the implementation's job.
#[async_trait]
pub trait AiBackend: Send + Sync {
    /// Stable source tag for metrics, logs and status ("zen", "local").
    fn tag(&self) -> &'static str;

    /// Model identifier surfaced in status.
    fn model(&self) -> &str;

    /// Stream one chat completion, advertising local function tools. When the
    /// model requests one, the stream yields an assembled
    /// [`AiEvent::ToolCalls`] just before the terminal [`AiEvent::Done`].
    async fn chat_stream_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
    ) -> Result<AiEventStream, AppError>;

    /// Single-shot completion returning `(text, prompt_tokens,
    /// completion_tokens)`. Never returns an empty answer.
    async fn chat_complete(&self, messages: &[ChatMessage])
        -> Result<(String, u64, u64), AppError>;
}
