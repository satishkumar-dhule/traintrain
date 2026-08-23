//! In-process micro-LLM backend: a quantized GGUF model executed on CPU with
//! candle, so Train Bro answers without any upstream dependency.
//!
//! Design constraints (micro models, ≤512 MB hosts):
//! - Tool calling is turned into **classification**: each round first asks
//!   the model for exactly one JSON line — `{"tool":"NAME","args":{...}}` or
//!   `{"answer":true}` — parsed leniently (fuzzy tool names, arg coercion)
//!   with one corrective retry. Only when the decision is "answer" (or the
//!   model cannot produce valid JSON) does prose generation run. Free-form
//!   emission of tool calls is exactly what models this small fail at.
//! - ChatML formatting (`<|im_start|>`), which both SmolLM2 and Qwen2/2.5
//!   instruct models use; OpenAI-shaped `tool` turns from the caller's loop
//!   are flattened into readable text.
//! - All inference runs in `spawn_blocking`; requests queue behind the
//!   engine `Arc` instead of competing for RAM concurrently.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use candle_core::quantized::gguf_file;
use candle_core::{Device, IndexOp, Tensor, D};
use candle_transformers::models::quantized_llama;
use candle_transformers::models::quantized_qwen2;
use tokenizers::Tokenizer;
use tokio::sync::mpsc;

use super::backend::{AiBackend, AiEventStream};
use super::client::{AiEvent, AssembledToolCall, ChatMessage};
use crate::config::Config;
use crate::core::error::AppError;

/// Extra tokens reserved for generation when clamping rendered history.
const GENERATION_RESERVE: usize = 96;
/// Cap on the decision-phase reply (one short JSON line is plenty).
const DECIDE_MAX_TOKENS: usize = 48;

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Quantized weights behind a mutex: candle keeps the KV cache inside the
/// model (`forward` takes `&mut self`), so holding the lock across an entire
/// generation both satisfies borrow rules and enforces the single-flight
/// policy that keeps RAM bounded.
struct Weights {
    inner: std::sync::Mutex<Inner>,
}

enum Inner {
    Llama(quantized_llama::ModelWeights),
    Qwen2(quantized_qwen2::ModelWeights),
}

/// Loaded model + tokenizer, built lazily on first request.
pub struct Engine {
    weights: Weights,
    tokenizer: Tokenizer,
    device: Device,
    eos_ids: Vec<u32>,
    ctx: usize,
    max_tokens: usize,
}

impl Engine {
    /// Load the GGUF + tokenizer. Blocking; call via `spawn_blocking`.
    fn load(
        model_path: &Path,
        tokenizer_path: &Path,
        ctx: usize,
        max_tokens: usize,
    ) -> Result<Self, AppError> {
        let started = std::time::Instant::now();
        let mut file = std::fs::File::open(model_path).map_err(|e| {
            AppError::source_unavailable("local", format!("model open failed: {e}"))
        })?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| AppError::source_unavailable("local", format!("invalid gguf: {e}")))?;
        let arch = content
            .metadata
            .get("general.architecture")
            .and_then(|v| v.to_string().ok().cloned())
            .unwrap_or_default();
        let device = Device::Cpu;
        let inner = match arch.as_str() {
            a if a.starts_with("llama") || a.starts_with("smollm") => Inner::Llama(
                quantized_llama::ModelWeights::from_gguf(content, &mut file, &device).map_err(
                    |e| {
                        AppError::source_unavailable(
                            "local",
                            format!("gguf llama load failed: {e}"),
                        )
                    },
                )?,
            ),
            a if a.starts_with("qwen") => Inner::Qwen2(
                quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device).map_err(
                    |e| {
                        AppError::source_unavailable(
                            "local",
                            format!("gguf qwen2 load failed: {e}"),
                        )
                    },
                )?,
            ),
            other => {
                return Err(AppError::source_unavailable(
                    "local",
                    format!("unsupported gguf architecture: {other}"),
                ))
            }
        };
        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
            AppError::source_unavailable("local", format!("tokenizer load failed: {e}"))
        })?;
        let eos_ids = [
            "<|im_end|>",
            "<|endoftext|>",
            "</s>",
            "<|im_start|>",
            "<eos>",
        ]
        .iter()
        .filter_map(|t| tokenizer.token_to_id(t))
        .collect();
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            %arch,
            ctx,
            max_tokens,
            "local ai engine loaded"
        );
        Ok(Self {
            weights: Weights {
                inner: std::sync::Mutex::new(inner),
            },
            tokenizer,
            device,
            eos_ids,
            ctx,
            max_tokens,
        })
    }

    fn encode(&self, text: &str) -> Result<Vec<u32>, AppError> {
        self.tokenizer
            .encode(text, true)
            .map(|e| e.get_ids().to_vec())
            .map_err(|e| AppError::source_unavailable("local", format!("encode failed: {e}")))
    }

    fn decode(&self, ids: &[u32]) -> String {
        self.tokenizer.decode(ids, false).unwrap_or_default()
    }
}

fn tensor_err(e: candle_core::Error) -> AppError {
    AppError::source_unavailable("local", format!("inference failed: {e}"))
}

/// Greedy argmax over the [vocab] logits row candle returns (its `forward`
/// already slices away every position except the last; batch dim = 1).
fn sample(logits: &Tensor) -> Result<u32, AppError> {
    let row = logits.squeeze(0).map_err(tensor_err)?;
    row.argmax(D::Minus1)
        .map_err(tensor_err)?
        .to_scalar::<u32>()
        .map_err(tensor_err)
}

/// Prefill the whole prompt and sample the first generated token.
fn prefill(weights: &mut Inner, engine: &Engine, prompt_ids: &[u32]) -> Result<u32, AppError> {
    let input = Tensor::new(prompt_ids, &engine.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(tensor_err)?;
    let logits = match weights {
        Inner::Llama(m) => m.forward(&input, 0),
        Inner::Qwen2(m) => m.forward(&input, 0),
    }
    .map_err(tensor_err)?;
    sample(&logits)
}

/// Feed one previously sampled token and sample the next.
fn step(weights: &mut Inner, engine: &Engine, token: u32, pos: usize) -> Result<u32, AppError> {
    let input = Tensor::new(&[token], &engine.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(tensor_err)?;
    let logits = match weights {
        Inner::Llama(m) => m.forward(&input, pos),
        Inner::Qwen2(m) => m.forward(&input, pos),
    }
    .map_err(tensor_err)?;
    sample(&logits)
}

/// Lock the weights for an entire generation (single-flight) and reset the
/// internal KV cache so prior conversations cannot leak in.
fn lock_weights(engine: &Arc<Engine>) -> Result<std::sync::MutexGuard<'_, Inner>, AppError> {
    let mut guard = engine
        .weights
        .inner
        .lock()
        .map_err(|_| AppError::internal("local ai weights mutex poisoned"))?;
    match &mut *guard {
        Inner::Llama(m) => m.clear_kv_cache(),
        Inner::Qwen2(m) => m.clear_kv_cache(),
    }
    Ok(guard)
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// Backend handle: cheap to share, loads the engine once on first use.
pub struct LocalBackend {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    ctx: usize,
    max_tokens: usize,
    engine: Arc<tokio::sync::OnceCell<Arc<Engine>>>,
}

impl LocalBackend {
    /// Validate configuration and file presence without loading weights.
    pub fn from_config(config: &Config) -> Result<Self, AppError> {
        let model_path = config.ai_local_model_path.clone();
        if !model_path.is_file() {
            return Err(AppError::source_unavailable(
                "local",
                format!(
                    "model file missing at {} (set RAILWAY_LOCAL_MODEL_PATH)",
                    model_path.display()
                ),
            ));
        }
        let tokenizer_path = model_path.with_file_name("tokenizer.json");
        if !tokenizer_path.is_file() {
            return Err(AppError::source_unavailable(
                "local",
                format!(
                    "tokenizer.json missing next to {} (copy it from the model repo)",
                    model_path.display()
                ),
            ));
        }
        Ok(Self {
            model_path,
            tokenizer_path,
            ctx: config.ai_local_ctx.max(512),
            max_tokens: config.ai_local_max_tokens.clamp(32, 2048),
            engine: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    async fn ensure_engine(&self) -> Result<Arc<Engine>, AppError> {
        if let Some(e) = self.engine.get() {
            return Ok(e.clone());
        }
        let (mp, tp, ctx, mt) = (
            self.model_path.clone(),
            self.tokenizer_path.clone(),
            self.ctx,
            self.max_tokens,
        );
        let loaded = tokio::task::spawn_blocking(move || Engine::load(&mp, &tp, ctx, mt))
            .await
            .map_err(|e| AppError::internal(format!("engine load task panicked: {e}")))??;
        // Race-safe: if another caller set theirs first, keep theirs.
        let _ = self.engine.set(Arc::new(loaded));
        Ok(self.engine.get().expect("engine set above").clone())
    }
}

#[async_trait]
impl AiBackend for LocalBackend {
    fn tag(&self) -> &'static str {
        "local"
    }

    fn model(&self) -> &str {
        self.model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("local-gguf")
    }

    async fn chat_stream_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
    ) -> Result<AiEventStream, AppError> {
        let engine = self.ensure_engine().await?;
        let (tx, rx) = mpsc::unbounded_channel::<Result<AiEvent, AppError>>();
        let msgs = messages.to_vec();
        let tools = tools.to_vec();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = run_round(&engine, &msgs, &tools, tx.clone()) {
                let _ = tx.send(Err(e));
            }
        });
        Ok(Box::pin(futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        })))
    }

    async fn chat_complete(
        &self,
        messages: &[ChatMessage],
    ) -> Result<(String, u64, u64), AppError> {
        use futures::StreamExt;
        let mut stream = self.chat_stream_with_tools(messages, &[]).await?;
        let mut text = String::new();
        let mut usage = (0u64, 0u64);
        while let Some(ev) = stream.next().await {
            match ev? {
                AiEvent::Delta(t) => text.push_str(&t),
                AiEvent::Done {
                    prompt_tokens,
                    completion_tokens,
                } => usage = (prompt_tokens, completion_tokens),
                AiEvent::Reasoning(_) | AiEvent::ToolCalls(_) => {}
            }
        }
        if text.trim().is_empty() {
            return Err(AppError::source_unavailable(
                "local",
                "engine returned an empty completion",
            ));
        }
        Ok((text, usage.0, usage.1))
    }
}

// ---------------------------------------------------------------------------
// Generation pipeline
// ---------------------------------------------------------------------------

/// Outcome of the decision phase for one round.
#[derive(Debug)]
enum Decision {
    /// Execute this tool with normalized argument JSON.
    Call(String, String),
    /// Move on to free-form prose generation.
    Answer,
    /// Unparsable / unknown tool / missing args (drives the retry).
    Invalid(String),
}

/// Full single-call pipeline: optional decide phase, then streamed prose.
fn run_round(
    engine: &Arc<Engine>,
    messages: &[ChatMessage],
    tools: &[serde_json::Value],
    tx: mpsc::UnboundedSender<Result<AiEvent, AppError>>,
) -> Result<(), AppError> {
    let started = std::time::Instant::now();
    let decision = if tools.is_empty() {
        Decision::Answer
    } else {
        decide_with_retry(engine, messages, tools)?
    };

    if let Decision::Call(name, args) = decision {
        tracing::info!(tool = %name, "local ai decided tool call");
        let _ = tx.send(Ok(AiEvent::ToolCalls(vec![AssembledToolCall {
            id: "call_0".to_string(),
            name,
            arguments: args,
        }])));
        let _ = tx.send(Ok(AiEvent::Done {
            prompt_tokens: 0,
            completion_tokens: 0,
        }));
        return Ok(());
    }

    let prompt = render_chatml(engine, messages, None, false, engine.max_tokens)?;
    let prompt_ids = engine.encode(&prompt)?;
    generate_streaming(engine, &prompt_ids, started, &tx)
}

/// Decide phase with one corrective retry on invalid output.
fn decide_with_retry(
    engine: &Arc<Engine>,
    messages: &[ChatMessage],
    tools: &[serde_json::Value],
) -> Result<Decision, AppError> {
    let manifest = compact_manifest(tools);
    let attempt = |corrective: Option<String>| -> Result<String, AppError> {
        let mut msgs = messages.to_vec();
        if let Some(c) = corrective {
            msgs.push(ChatMessage::user(c));
        }
        let prompt = render_chatml(engine, &msgs, Some(&manifest), true, DECIDE_MAX_TOKENS)?;
        let ids = engine.encode(&prompt)?;
        generate_raw(engine, &ids, DECIDE_MAX_TOKENS)
    };

    let raw = attempt(None)?;
    match parse_decision(&raw, tools) {
        d @ (Decision::Call(..) | Decision::Answer) => Ok(d),
        Decision::Invalid(reason) => {
            tracing::info!(%reason, raw = %truncate_for_log(&raw), "invalid local decision; retrying");
            let corrective = format!(
                "[CHECK] Your previous reply was invalid ({reason}). Reply again with \
                 exactly one JSON line: {{\"tool\":\"NAME\",\"args\":{{...}}}} or \
                 {{\"answer\":true}} - nothing else."
            );
            let raw2 = attempt(Some(corrective))?;
            match parse_decision(&raw2, tools) {
                d @ (Decision::Call(..) | Decision::Answer) => Ok(d),
                Decision::Invalid(reason2) => {
                    tracing::info!(%reason2, raw = %truncate_for_log(&raw2), "local decision retry failed; answering directly");
                    Ok(Decision::Answer)
                }
            }
        }
    }
}

/// Non-streaming greedy generation, returns generated text only.
fn generate_raw(
    engine: &Arc<Engine>,
    prompt_ids: &[u32],
    max_new: usize,
) -> Result<String, AppError> {
    let mut w = lock_weights(engine)?;
    let mut generated: Vec<u32> = Vec::with_capacity(max_new);
    // Prefill the whole prompt, then feed one token per step.
    let mut next = prefill(&mut w, engine, prompt_ids)?;
    let mut pos = prompt_ids.len();
    for _ in 0..max_new {
        if engine.eos_ids.contains(&next) {
            break;
        }
        generated.push(next);
        next = step(&mut w, engine, next, pos)?;
        pos += 1;
    }
    Ok(engine.decode(&generated))
}

/// Streaming greedy generation emitting [`AiEvent::Delta`] fragments with a
/// hold-back buffer so partial special-token prefixes never leak.
fn generate_streaming(
    engine: &Arc<Engine>,
    prompt_ids: &[u32],
    started: std::time::Instant,
    tx: &mpsc::UnboundedSender<Result<AiEvent, AppError>>,
) -> Result<(), AppError> {
    const MARKERS: [&str; 2] = ["<|im_end|>", "<|endoftext|>"];
    let mut w = lock_weights(engine)?;
    let mut generated: Vec<u32> = Vec::with_capacity(engine.max_tokens);
    let mut sent_chars = 0usize; // chars of the decoded generation already sent
    let mut stopped_early = false;
    let mut next = prefill(&mut w, engine, prompt_ids)?;
    let mut pos = prompt_ids.len();

    for _ in 0..engine.max_tokens {
        if engine.eos_ids.contains(&next) {
            break;
        }
        generated.push(next);

        let gen_text = engine.decode(&generated);
        if gen_text.contains("<|im_start|>") || MARKERS.iter().any(|m| gen_text.contains(m)) {
            stopped_early = true;
            break;
        }
        let total = gen_text.chars().count();
        let hold = MARKERS
            .iter()
            .map(|m| marker_hold_len(&gen_text, m))
            .max()
            .unwrap_or(0);
        let safe = total.saturating_sub(hold);
        if safe > sent_chars {
            let delta: String = gen_text
                .chars()
                .skip(sent_chars)
                .take(safe - sent_chars)
                .collect();
            if !delta.is_empty() {
                let _ = tx.send(Ok(AiEvent::Delta(delta)));
            }
            sent_chars = safe;
        }

        next = step(&mut w, engine, next, pos)?;
        pos += 1;
    }

    // Flush anything held back that turned out not to be a marker.
    if !stopped_early {
        let gen_text = engine.decode(&generated);
        let total = gen_text.chars().count();
        if total > sent_chars {
            let tail: String = gen_text.chars().skip(sent_chars).collect();
            let _ = tx.send(Ok(AiEvent::Delta(tail)));
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    let tps = if elapsed > 0.0 {
        generated.len() as f64 / elapsed
    } else {
        0.0
    };
    tracing::info!(
        prompt_tokens = prompt_ids.len(),
        completion_tokens = generated.len(),
        decode_tps = format!("{tps:.1}"),
        latency_ms = started.elapsed().as_millis() as u64,
        "local ai round complete"
    );
    let _ = tx.send(Ok(AiEvent::Done {
        prompt_tokens: prompt_ids.len() as u64,
        completion_tokens: generated.len() as u64,
    }));
    Ok(())
}

/// Length of the suffix of `text` that is a strict prefix of `marker`.
fn marker_hold_len(text: &str, marker: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mchars: Vec<char> = marker.chars().collect();
    let max = chars.len().min(mchars.len().saturating_sub(1));
    for l in (1..=max).rev() {
        if chars[chars.len() - l..] == mchars[..l] {
            return l;
        }
    }
    0
}

// ---------------------------------------------------------------------------
// ChatML rendering
// ---------------------------------------------------------------------------

/// Render the conversation as ChatML, clamping history to fit the context
/// budget. `decide_prefix=true` ends inside an assistant turn pre-seeded with
/// `{"` so the model continues structurally valid JSON.
fn render_chatml(
    engine: &Arc<Engine>,
    messages: &[ChatMessage],
    tool_manifest: Option<&str>,
    decide_prefix: bool,
    reserve: usize,
) -> Result<String, AppError> {
    let system_extra = tool_manifest.map(|m| format!("\n\n{DECISION_PROTOCOL}\n{m}"));
    let build = |keep_from: usize, extra: &Option<String>| -> String {
        let mut out = String::with_capacity(4_096);
        let mut name_by_id = std::collections::HashMap::new();
        for msg in messages.iter().skip(keep_from) {
            if let Some(calls) = &msg.tool_calls {
                for c in calls {
                    if let (Some(id), Some(f)) =
                        (c.get("id").and_then(|v| v.as_str()), c.get("function"))
                    {
                        if let Some(n) = f.get("name").and_then(|v| v.as_str()) {
                            name_by_id.insert(id.to_string(), n.to_string());
                        }
                    }
                }
                let lines: Vec<String> = calls
                    .iter()
                    .filter_map(|c| {
                        let n = c.get("function")?.get("name")?.as_str()?;
                        let a = c
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        Some(format!("{{\"tool\":\"{n}\",\"args\":{a}}}"))
                    })
                    .collect();
                out.push_str(&format!(
                    "<|im_start|>assistant\n{}<|im_end|>\n",
                    lines.join("\n")
                ));
                continue;
            }
            match msg.role.as_str() {
                "system" => {
                    let suffix = extra.clone().unwrap_or_default();
                    out.push_str(&format!(
                        "<|im_start|>system\n{}{}<|im_end|>\n",
                        msg.content, suffix
                    ));
                }
                "user" => {
                    out.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", msg.content));
                }
                "tool" => {
                    let label = msg
                        .tool_call_id
                        .as_deref()
                        .and_then(|id| name_by_id.get(id))
                        .cloned()
                        .unwrap_or_else(|| "data".to_string());
                    out.push_str(&format!(
                        "<|im_start|>user\n[{label} RESULT]\n{}<|im_end|>\n",
                        msg.content
                    ));
                }
                "assistant" => {
                    out.push_str(&format!(
                        "<|im_start|>assistant\n{}<|im_end|>\n",
                        msg.content
                    ));
                }
                _ => {}
            }
        }
        out.push_str("<|im_start|>assistant\n");
        if decide_prefix {
            out.push_str("{\"");
        }
        out
    };

    // Clamp: drop oldest non-system turns until the prompt fits the budget.
    let budget = engine
        .ctx
        .saturating_sub(reserve + GENERATION_RESERVE)
        .max(256);
    let mut keep_from = 0usize;
    let mut rendered = build(keep_from, &system_extra);
    loop {
        let n_tokens = engine.encode(&rendered)?.len();
        if n_tokens <= budget || keep_from + 1 >= messages.len() {
            break;
        }
        keep_from += 1;
        if messages[keep_from].role == "system" && keep_from + 1 < messages.len() {
            keep_from += 1;
        }
        rendered = build(keep_from, &system_extra);
    }
    Ok(rendered)
}

const DECISION_PROTOCOL: &str = "\
TOOL PROTOCOL: You may call ONE tool per turn. To call one, reply with exactly \
one JSON line: {\"tool\":\"NAME\",\"args\":{\"param\":\"value\"}}. When you have \
enough information, reply exactly {\"answer\":true}. After [TOOL RESULT] blocks \
appear, prefer answering. Use station codes when known.";

/// Compact one-line-per-tool manifest from OpenAI envelopes.
fn compact_manifest(tools: &[serde_json::Value]) -> String {
    let mut lines = vec!["AVAILABLE TOOLS:".to_string()];
    for t in tools {
        let f = t.get("function");
        let name = f
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let desc = f
            .and_then(|f| f.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let params = f.and_then(|f| f.get("parameters"));
        let props = params
            .and_then(|p| p.get("properties"))
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let required: Vec<String> = params
            .and_then(|p| p.get("required"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let plist: Vec<String> = props
            .keys()
            .map(|k| {
                if required.iter().any(|r| r == k) {
                    k.clone()
                } else {
                    format!("[{k}]")
                }
            })
            .collect();
        let desc_short: String = desc.split('.').next().unwrap_or(desc).trim().to_string();
        lines.push(format!("- {name}({}): {desc_short}", plist.join(", ")));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Lenient decision parsing
// ---------------------------------------------------------------------------

fn parse_decision(raw: &str, tools: &[serde_json::Value]) -> Decision {
    let trimmed = raw.trim();
    let Some(start) = trimmed.find('{') else {
        return Decision::Invalid("no JSON object found".into());
    };
    let Some(end) = trimmed.rfind('}') else {
        return Decision::Invalid("JSON object not closed".into());
    };
    if end < start {
        return Decision::Invalid("malformed JSON".into());
    }
    let v: serde_json::Value = match serde_json::from_str(&trimmed[start..=end]) {
        Ok(v) => v,
        Err(e) => return Decision::Invalid(format!("unparsable JSON: {e}")),
    };
    if v.get("answer").is_some() {
        return Decision::Answer;
    }
    let Some(name) = v.get("tool").and_then(|t| t.as_str()) else {
        return Decision::Invalid("missing 'tool' key".into());
    };
    if matches!(name, "none" | "answer" | "") {
        return Decision::Answer;
    }
    let Some(schema) = find_tool(tools, name) else {
        return Decision::Invalid(format!("unknown tool '{name}'"));
    };
    let matched = schema["function"]["name"].as_str().expect("real tool name");
    let args_in = v.get("args").cloned().unwrap_or(serde_json::json!({}));
    let Some(args_json) = normalize_args(&args_in, &schema["function"]["parameters"]) else {
        return Decision::Invalid(format!("bad args for tool '{matched}'"));
    };
    Decision::Call(matched.to_string(), args_json)
}

fn find_tool<'a>(tools: &'a [serde_json::Value], name: &str) -> Option<&'a serde_json::Value> {
    // Exact, then case-insensitive, then edit distance <= 2.
    for t in tools {
        if t["function"]["name"].as_str()? == name {
            return Some(t);
        }
    }
    let lower = name.to_ascii_lowercase();
    for t in tools {
        let n = t["function"]["name"].as_str()?;
        if n.eq_ignore_ascii_case(name) || n.eq_ignore_ascii_case(&lower) {
            return Some(t);
        }
    }
    let mut best: Option<(&str, usize)> = None;
    for t in tools {
        let n = t["function"]["name"].as_str()?;
        let d = levenshtein(&lower, &n.to_ascii_lowercase());
        if d <= 2 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((n, d));
        }
    }
    best.and_then(|(n, _)| {
        tools
            .iter()
            .find(|t| t["function"]["name"] == serde_json::json!(n))
    })
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Coerce and validate args against the tool's parameters object. Returns the
/// normalized compact JSON string, or None when required args are missing.
fn normalize_args(args: &serde_json::Value, parameters: &serde_json::Value) -> Option<String> {
    let empty = serde_json::Map::new();
    let props = parameters
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or(&empty);
    let required: Vec<&str> = parameters
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    let obj = args.as_object()?;
    let mut out = serde_json::Map::new();
    for (k, spec) in props {
        let Some(val) = obj.get(k) else {
            continue;
        };
        let want = spec
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let coerced =
            match want {
                "number" | "integer" => match val {
                    serde_json::Value::Number(_) => Some(val.clone()),
                    serde_json::Value::String(s) => s.trim().parse::<f64>().ok().and_then(|n| {
                        serde_json::Number::from_f64(n).map(serde_json::Value::Number)
                    }),
                    _ => None,
                },
                _ => Some(serde_json::Value::String(match val {
                    serde_json::Value::String(s) => s.trim().to_string(),
                    other => other.to_string().trim_matches('"').to_string(),
                })),
            }?;
        out.insert(k.clone(), coerced);
    }
    for r in required {
        let present = out
            .get(r)
            .map(|v| match v {
                serde_json::Value::String(s) => !s.trim().is_empty(),
                _ => !v.is_null(),
            })
            .unwrap_or(false);
        if !present {
            return None;
        }
    }
    serde_json::to_string(&serde_json::Value::Object(out)).ok()
}

fn truncate_for_log(s: &str) -> String {
    s.chars().take(160).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(name: &str, props: &[(&str, &str)], required: &[&str]) -> serde_json::Value {
        let properties: serde_json::Map<String, serde_json::Value> = props
            .iter()
            .map(|(k, ty)| (k.to_string(), serde_json::json!({ "type": ty })))
            .collect();
        serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
                "description": format!("{name} tool."),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required,
                }
            }
        })
    }

    #[test]
    fn parses_clean_tool_call() {
        let tools = vec![envelope(
            "trains_between",
            &[("src", "string"), ("dst", "string")],
            &["src", "dst"],
        )];
        let d = parse_decision(
            r#"xx {"tool":"trains_between","args":{"src":"NDLS","dst":"PNBE"}} yy"#,
            &tools,
        );
        match d {
            Decision::Call(n, a) => {
                assert_eq!(n, "trains_between");
                let v: serde_json::Value = serde_json::from_str(&a).unwrap();
                assert_eq!(v["src"], "NDLS");
                assert_eq!(v["dst"], "PNBE");
            }
            other => panic!("expected Call, got {other:?}"),
        }
    }

    #[test]
    fn fuzzy_fixes_tool_name_and_coerces_args() {
        let tools = vec![envelope(
            "station_board",
            &[("train", "string"), ("hours", "integer")],
            &["train"],
        )];
        let d = parse_decision(
            r#"{"tool":"sttion_board","args":{"train":"12951","hours":"4","junk":"x"}}"#,
            &tools,
        );
        match d {
            Decision::Call(n, a) => {
                assert_eq!(n, "station_board");
                let v: serde_json::Value = serde_json::from_str(&a).unwrap();
                assert_eq!(v["train"], "12951");
                assert_eq!(v["hours"], 4.0);
                assert!(v.get("junk").is_none(), "unknown props dropped");
            }
            other => panic!("expected Call, got {other:?}"),
        }
    }

    #[test]
    fn missing_required_arg_is_invalid() {
        let tools = vec![envelope(
            "trains_between",
            &[("src", "string"), ("dst", "string")],
            &["src", "dst"],
        )];
        let d = parse_decision(r#"{"tool":"trains_between","args":{"src":"NDLS"}}"#, &tools);
        assert!(matches!(d, Decision::Invalid(_)));
    }

    #[test]
    fn answer_shape_and_none_tool_map_to_answer() {
        assert!(matches!(
            parse_decision(r#"{"answer":true}"#, &[]),
            Decision::Answer
        ));
        assert!(matches!(
            parse_decision(r#"{"tool":"none"}"#, &[]),
            Decision::Answer
        ));
        assert!(matches!(
            parse_decision("no json at all", &[]),
            Decision::Invalid(_)
        ));
    }

    #[test]
    fn compact_manifest_lists_params_with_required_markers() {
        let tools = vec![
            envelope("live_status", &[("train", "string")], &["train"]),
            envelope(
                "search_rail",
                &[("q", "string"), ("limit", "integer")],
                &["q"],
            ),
        ];
        let m = compact_manifest(&tools);
        assert!(m.contains("- live_status(train):"));
        // Param order follows serde_json's sorted map; check markers only.
        let sr = m
            .split('\n')
            .find(|l| l.starts_with("- search_rail"))
            .unwrap();
        assert!(sr.starts_with("- search_rail("));
        assert!(sr.contains("[limit]"), "optional param marked: {sr}");
        assert!(
            !sr.contains("[q]") && sr.contains('q'),
            "required param unmarked: {sr}"
        );
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
    }
}
