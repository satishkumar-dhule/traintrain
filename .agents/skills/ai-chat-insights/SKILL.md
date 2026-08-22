---
name: ai-chat-insights
description: Work on the AI chat / insights features (Train Bro assistant). Use for "chatbot", "AI chat", "persona", "assistant formatting", "code ticks in answers", "zen", "SSE relay", "add a chat tool", "insights endpoint". Covers the server-owned persona, the tool-calling loop, markdown rendering in the Svelte frontend, and where formatting complaints get fixed.
---

# AI chat + insights (Train Bro)

## Persona is server-owned

`PERSONA` const in `railway-rs/src/slices/ai_chat/service.rs` — prepended
exactly once per request in `mod.rs:83`, so clients cannot override it.
Current text: factual, live-data-only, brief, "clean Markdown: short
paragraphs, bullet lists for options, tables for schedules, **bold** key
facts; no headings larger than ###".

**Formatting complaints ("answers full of code ticks/symbols")**: the persona
does NOT forbid code fences. If the model emits fenced blocks anyway they
render as `<pre class="md-pre">`. Fix at the source — extend PERSONA with an
explicit rule (e.g. "never wrap prose in code fences; backticks only for real
identifiers"). Do not hack the renderer to strip fences globally.

## Request path

`POST /rail-api/ai/chat` (`slices/ai_chat/mod.rs:74`) → validation caps:
`MAX_MESSAGES=40`, `MAX_CONTENT_CHARS=32_000` → `AiClient::chat_stream_with_tools`
(`core/ai/client.rs`) POSTs `{base}/chat/completions` with `stream:true` →
`SseDecoder` yields `AiEvent::{Reasoning,Delta,ToolCalls,Done}` → local tool
loop via `tools::call_tool`, **max 4 rounds** → frames re-encoded by
`service::encode_event` as SSE events (`delta|reasoning|tools|done|error`).
Gateway failures are typed `AppError::source_unavailable("zen", …)` — never
fabricate a reply when zen is down.

## Chat tools

Registry in `slices/ai_chat/tools.rs`: JSON defs (~:55), dispatch (~:193),
projections that shape slice DTOs for the model (e.g. `project_average_delay`,
live-status projection that drops history but keeps signal). To add a tool:
1. def + description (descriptions are the only disambiguator between tools
   with identical schemas — make them sharp)
2. dispatch arm calling the owning slice service
3. projection fn (compact, worst-first sorted where ranking helps)
4. mock-queue test proving the loop calls it.

`ai_insight` shares the `"live_status" | "average_delay"` validation arm in
`mod.rs` but must dispatch on the exact kind string downstream.

## Frontend rendering

Svelte app: `frontend/src/lib/pages/Assistant.svelte` renders assistant turns
via `{@html renderMarkdown(t.content)}`; user turns are plain
`whitespace-pre-wrap`. Renderer `frontend/src/lib/markdown.js` is
dependency-free (headings, lists, tables, fences, blockquotes, bold/italic,
inline code, safe links; raw HTML escaped everywhere). Unit tests:
`tests/js/markdown.test.mjs`.

**After any Svelte edit**: rebuild so the served bundle updates —
`(cd railway-rs/frontend && npm run build)` produces `static/assets/index-*`.
The old `static/tabs/*.js` vanilla registry is legacy; don't add chat UI there.
