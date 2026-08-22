---
name: avg-delay-vs-status-debug
description: Debug "average delay shows the same as live status" and similar two-endpoint-conflated reports. Use for "avg delay wrong", "delay same as status", "endpoints show identical data", "is the call not passed through", stale/wrong data on train pages. Documents the proven separation of both chains and a layer-by-layer triage order.
---

# Average delay vs live status: separation + triage

Recurring report: "I checked several trains, average delay and current running
status look the same — the call isn't being passed to average delay." The
backend chains **never intersect**. Verified end-to-end (do not re-derive;
extend if code changes):

| | average_delay | live_status |
|---|---|---|
| route | `/rail-api/ntes/average-delay` (`slices/average_delay/mod.rs:34`) | `/rail-api/live-status` (`slices/live_status/mod.rs:55`) |
| cache key | `average_delay:{train}` (`service.rs:21`) | `live_status:{train}:{date}` (`service.rs:31`) |
| NTES form | `post_form("q","AverageDelay","show")` (`core/ntes/web.rs:445`) | `post_form("tr","TrainRunning","FindRunningInstancePop")` (`web.rs:548`) |
| parser | `web.rs:1261` regex rows → string cells `"On Time"`/`"00:14"` → `arrival_delay`/`departure_delay` | numeric position/`delay_minutes`; Railyatri fallback allowed |
| sources | NTES-only, honest source-unavailable otherwise | NTES → Railyatri fallback |

Benign look-alikes (not bugs): shared validation arm
`"live_status" | "average_delay" => {` in `ai_insight/mod.rs:60` (dispatches on
exact kind downstream); chat tools with identical `{"train"}` schemas in
`ai_chat/tools.rs` (descriptions disambiguate).

## Triage order

1. **Hit both endpoints raw** for the same train; compare `data_source` +
   body shape. Different shapes ⇒ backend fine.
2. **Cache**: distinct prefixes above mean no collision is possible in
   current code; an old binary/process predating the keys can still serve
   stale sameness — restart per `rust-workflow`.
3. **UI wiring** (most common culprit): grep the Svelte page for which field
   feeds which card/label — symptom is usually a label bound to the other
   DTO's field, or one fetch reused for both cards. Fix wiring, rebuild
   frontend bundle.
4. **Tests that pin the split**: `tests/average_delay.rs`,
   `tests/live_status.rs`, and
   `blocked_endpoints_are_honest_source_unavailable` (`web.rs`) asserting the
   two calls fail independently when NTES blocks.

## Generalize

For ANY "endpoint A shows endpoint B's data": verify separation at every layer
before assuming a backend merge — route → service fn → cache-key prefix →
upstream form/URL → parser → DTO fields. The first layer where the two chains
differ tells you where the conflation actually is; everything upstream of it
is innocent.

Related: `ntes-client-method` (adding/fixing parsers), `ntes-live-verification`
(live probes), `rust-workflow` (restart + gates).
