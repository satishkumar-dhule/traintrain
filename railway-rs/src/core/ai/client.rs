use std::collections::VecDeque;
use std::time::Duration;

use futures::Stream;
use serde::Serialize;

use crate::core::error::AppError;

/// One turn of a conversation. `role` is `system`, `user` or `assistant`.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

/// A decoded piece of a streamed completion.
#[derive(Debug, Clone, PartialEq)]
pub enum AiEvent {
    /// Chain-of-thought fragment (some models stream `reasoning_content`
    /// before/alongside the answer). UI may show this dimmed or hide it.
    Reasoning(String),
    /// Answer text fragment.
    Delta(String),
    /// Terminal event with token usage when the upstream reported it.
    Done {
        prompt_tokens: u64,
        completion_tokens: u64,
    },
}

/// Client for one OpenAI-compatible inference gateway. Owns its dedicated
/// `reqwest::Client` because LLM completions need a long total timeout and
/// byte-stream reads — both deliberately different from the shared
/// [`crate::core::http::HttpClient`] used for short scraping calls.
#[derive(Clone)]
pub struct AiClient {
    http: reqwest::Client,
    base: String,
    model: String,
    api_key: Option<String>,
}

impl AiClient {
    pub fn new(
        base: &str,
        model: &str,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(8))
            .build()
            .map_err(|e| AppError::internal(format!("failed to build ai client: {e}")))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base)
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.base)
    }

    /// List model ids advertised by the gateway (`GET /models`). Used by the
    /// status surface so rate-limited users can pick an alternative.
    pub async fn models(&self) -> Result<Vec<String>, AppError> {
        let mut req = self.http.get(self.models_url());
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let res = req.send().await.map_err(|e| {
            AppError::source_unavailable("zen", format!("models request failed: {e}"))
        })?;
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("zen", format!("models body read failed: {e}"))
        })?;
        let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::source_unavailable("zen", format!("invalid models JSON: {e}"))
        })?;
        let ids = v
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(ids)
    }

    /// POST a streaming chat completion and return the decoded event stream.
    /// The response headers are validated here so callers fail fast on 4xx /
    /// 5xx before any events are consumed.
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<impl Stream<Item = Result<AiEvent, AppError>>, AppError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });
        let mut req = self.http.post(self.chat_url()).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let res = req.send().await.map_err(|e| {
            AppError::source_unavailable("zen", format!("chat request failed: {e}"))
        })?;
        let status = res.status();
        if !status.is_success() {
            // Zen reports errors either as OpenAI-style bodies or as
            // {"type":"error","error":{"type","message"}}; surface the real
            // message either way instead of a bare status code.
            let text = res.text().await.unwrap_or_default();
            let detail = decode_error_body(&text).unwrap_or_else(|| format!("HTTP {status}"));
            return Err(AppError::source_unavailable("zen", detail));
        }
        Ok(decode_stream(res.bytes_stream()))
    }

    /// Convenience for single-shot use (insights): collect the full answer,
    /// discarding reasoning fragments and returning `(text, prompt_tokens,
    /// completion_tokens)`.
    pub async fn chat_complete(
        &self,
        messages: &[ChatMessage],
    ) -> Result<(String, u64, u64), AppError> {
        use futures::StreamExt;
        let mut stream = Box::pin(self.chat_stream(messages).await?);
        let mut text = String::new();
        let mut usage = (0u64, 0u64);
        while let Some(ev) = stream.next().await {
            match ev? {
                AiEvent::Delta(t) => text.push_str(&t),
                AiEvent::Reasoning(_) => {}
                AiEvent::Done {
                    prompt_tokens,
                    completion_tokens,
                } => usage = (prompt_tokens, completion_tokens),
            }
        }
        Ok((text, usage.0, usage.1))
    }
}

/// Map a non-2xx response body to a human-readable reason, understanding both
/// observed Zen error shapes.
pub(crate) fn decode_error_body(text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    // Zen gateway style: {"type":"error","error":{"type":"FreeUsageLimitError",
    // "message":"Rate limit exceeded."}}
    if v.get("type").and_then(|t| t.as_str()) == Some("error") {
        if let Some(err) = v.get("error").filter(|e| e.is_object()) {
            let ty = err.get("type").and_then(|t| t.as_str()).unwrap_or("error");
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Some(format!("{ty}: {msg}"));
        }
    }
    // OpenAI style: {"error":{"message":"..."}}
    if let Some(obj) = v.as_object() {
        if let Some(err) = obj.get("error").and_then(|e| e.as_object()) {
            if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
                return Some(msg.to_string());
            }
        }
    }
    None
}

/// Incremental SSE frame decoder. Feed raw network bytes, pull complete
/// `data:` frames; tolerates CRLF, multi-line data fields, comment lines and
/// chunk boundaries split mid-frame.
#[derive(Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum Frame {
    Data(String),
    Done,
}

impl SseDecoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Pop the next complete frame, if any. `Ok(None)` = need more bytes.
    pub(crate) fn pop_frame(&mut self) -> Result<Option<Frame>, AppError> {
        loop {
            let Some(end) = find_frame_end(&self.buffer) else {
                return Ok(None);
            };
            let block: Vec<u8> = self.buffer.drain(..end + 1).collect();
            let text = String::from_utf8_lossy(&block);
            let mut data_lines: VecDeque<&str> = VecDeque::new();
            for line in text.split('\n') {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("data:") {
                    data_lines.push_back(rest.strip_prefix(' ').unwrap_or(rest));
                }
                // Non-data SSE fields (event:, id:, retry:) are ignored — the
                // OpenAI-compatible protocol carries everything in `data`.
            }
            if data_lines.is_empty() {
                continue;
            }
            let joined = data_lines.into_iter().collect::<Vec<_>>().join("\n");
            return Ok(Some(if joined.trim() == "[DONE]" {
                Frame::Done
            } else {
                Frame::Data(joined)
            }));
        }
    }

    /// Call once after the byte stream ends to flush a final unterminated
    /// frame (defensive; well-behaved servers always terminate).
    pub(crate) fn flush_tail(&mut self) -> Result<Option<Frame>, AppError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        let rest = String::from_utf8_lossy(&self.buffer).into_owned();
        self.buffer.clear();
        let trimmed = rest.trim_matches(['\r', '\n']);
        if trimmed.is_empty() || trimmed == "[DONE]" {
            return Ok(if trimmed == "[DONE]" {
                Some(Frame::Done)
            } else {
                None
            });
        }
        let data = trimmed
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(|l| l.strip_prefix(' ').unwrap_or(l))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(if data.is_empty() {
            None
        } else {
            Some(Frame::Data(data))
        })
    }
}

/// Index just past the `\n\n` / `\r\n\r\n` that terminates one SSE block.
fn find_frame_end(buf: &[u8]) -> Option<usize> {
    buf.windows(2)
        .position(|w| w == b"\n\n" || w == b"\r\r")
        .or_else(|| buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 2))
}

/// Turn one `data:` JSON frame into zero or more events. Lenient by design:
/// unknown shapes are skipped rather than failing the whole stream, except
/// explicit upstream error objects which become hard errors.
pub(crate) enum Parsed {
    Empty,
    Error(AppError),
    Events(Vec<AiEvent>),
}

pub(crate) fn parse_data_frame(frame: &str) -> Parsed {
    let v: serde_json::Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(frame = %truncate(frame, 200), %e, "skipping unparsable ai stream frame");
            return Parsed::Empty;
        }
    };

    if v.get("type").and_then(|t| t.as_str()) == Some("error") {
        if let Some(err) = v.get("error") {
            let ty = err.get("type").and_then(|t| t.as_str()).unwrap_or("error");
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            return Parsed::Error(AppError::source_unavailable("zen", format!("{ty}: {msg}")));
        }
    }

    let mut events = Vec::new();
    if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
        for choice in choices {
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(r) = delta.get("reasoning_content").and_then(|t| t.as_str()) {
                if !r.is_empty() {
                    events.push(AiEvent::Reasoning(r.to_string()));
                }
            }
            if let Some(c) = delta.get("content").and_then(|t| t.as_str()) {
                if !c.is_empty() {
                    events.push(AiEvent::Delta(c.to_string()));
                }
            }
        }
    }
    if let Some(usage) = v.get("usage").filter(|u| !u.is_null()) {
        let pt = usage.get("prompt_tokens").and_then(|t| t.as_u64());
        let ct = usage.get("completion_tokens").and_then(|t| t.as_u64());
        if pt.is_some() || ct.is_some() {
            events.push(AiEvent::Done {
                prompt_tokens: pt.unwrap_or(0),
                completion_tokens: ct.unwrap_or(0),
            });
        }
    }
    if events.is_empty() {
        Parsed::Empty
    } else {
        Parsed::Events(events)
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

type ByteStream =
    std::pin::Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>;

/// Per-stream decoding state driven through `stream::unfold`.
struct StreamState {
    bytes: ByteStream,
    dec: SseDecoder,
    pending: VecDeque<AiEvent>,
    /// Usage event observed before `[DONE]`; emitted once at termination.
    terminal: Option<AiEvent>,
    finished: bool,
}

impl StreamState {
    fn note_events(&mut self, evs: Vec<AiEvent>) {
        for ev in evs {
            match ev {
                AiEvent::Done { .. } => self.terminal = Some(ev),
                other => self.pending.push_back(other),
            }
        }
    }

    /// Mark terminated and produce the termination event: reported usage when
    /// present, zeroed otherwise.
    fn finish(&mut self) -> AiEvent {
        self.finished = true;
        self.terminal.take().unwrap_or(AiEvent::Done {
            prompt_tokens: 0,
            completion_tokens: 0,
        })
    }
}

fn decode_stream<S>(stream: S) -> impl Stream<Item = Result<AiEvent, AppError>>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    let init = StreamState {
        bytes: Box::pin(stream),
        dec: SseDecoder::new(),
        pending: VecDeque::new(),
        terminal: None,
        finished: false,
    };
    futures::stream::unfold(init, |mut st| async move {
        use futures::StreamExt as _;
        while !st.finished {
            if let Some(ev) = st.pending.pop_front() {
                return Some((Ok(ev), st));
            }
            match st.dec.pop_frame() {
                Err(e) => {
                    st.finish();
                    return Some((Err(e), st));
                }
                Ok(Some(Frame::Done)) => {
                    let ev = st.finish();
                    return Some((Ok(ev), st));
                }
                Ok(Some(Frame::Data(frame))) => match parse_data_frame(&frame) {
                    Parsed::Empty => continue,
                    Parsed::Error(e) => {
                        st.finish();
                        return Some((Err(e), st));
                    }
                    Parsed::Events(evs) => {
                        st.note_events(evs);
                        continue;
                    }
                },
                Ok(None) => {}
            }
            match st.bytes.next().await {
                Some(Ok(chunk)) => st.dec.feed(&chunk),
                Some(Err(e)) => {
                    st.finish();
                    return Some((
                        Err(AppError::source_unavailable(
                            "zen",
                            format!("stream read failed: {e}"),
                        )),
                        st,
                    ));
                }
                None => {
                    // Upstream closed the connection. Flush any trailing
                    // unterminated frame, then terminate gracefully.
                    if let Some(Err(_)) = dec_tail(&mut st) {
                        // Tail decode failures are non-fatal at EOF; fall
                        // through to the graceful termination event.
                    }
                    let ev = st.finish();
                    return Some((Ok(ev), st));
                }
            }
        }
        None
    })
}

fn dec_tail(st: &mut StreamState) -> Option<Result<(), AppError>> {
    match st.dec.flush_tail() {
        Err(e) => Some(Err(e)),
        Ok(None) => None,
        Ok(Some(Frame::Done)) => None,
        Ok(Some(Frame::Data(frame))) => match parse_data_frame(&frame) {
            Parsed::Empty | Parsed::Error(_) => None,
            Parsed::Events(evs) => {
                st.note_events(evs);
                None
            }
        },
    }
}
