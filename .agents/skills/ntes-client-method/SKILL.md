---
name: ntes-client-method
description: Add or extend a client method + parser in railway-rs/src/core/ntes/web.rs (the deep NTES web-form module). Use for "add a new NTES endpoint", "scrape a new NTES form", "parse NTES HTML into JSON", "extend the NTES client", "train map / spot / heritage / parcel / average delay / station timetable / journey basis". Covers the post_form helper, session retry, HTML parsing, normalized JSON shapes, and AppError conventions.
---

# NTES web-form client method (deep module)

`src/core/ntes/web.rs` (`NtesWebClient`) mirrors the NTES web UI at
`enquiry.indianrail.gov.in/mntes`. Each feature is ONE public method that POSTs
the matching web form and returns **normalized serde_json::Value** (never raw
HTML to callers). Keep the module deep: the session/CSRF/retry machinery stays
private, callers only see clean JSON.

Read a full example first: `train_route_map` / `train_spot_map` around
`src/core/ntes/web.rs:591-655`.

## Method skeleton

```rust
/// Doc comment: which NTES form it mirrors, POST path, and the exact
/// normalized JSON shape emitted (field by field).
pub async fn my_feature(&self, param: &str) -> Result<Value, AppError> {
    let html = self
        .post_form(
            "EndpointName",            // path segment, e.g. "TrnMap" / "q"
            "opt", "sub_opt",          // ?opt=..&subOpt=..
            Some("rowsMarker"),        // HTML substring that proves a real result
                                       // page (None if ambiguous)
            &[("field", param)],       // query-string params
            &[("field", param.to_string())], // POST form fields
        )
        .await?;
    parse_my_feature(&html).ok_or_else(|| {
        AppError::source_unavailable("ntes", "no <feature> data found in the NTES response")
    })
}
```

## post_form mechanics (private, do not duplicate)

`post_form(endpoint, opt, sub_opt, rows_marker, query, fields)` does the
session dance for you:

- Bootstraps session cookies + CSRF from `/mntes/` and `/mntes/GetCSRFToken`.
- POSTs `application/x-www-form-urlencoded` to `{web}/{endpoint}?opt=..&subOpt=..`
  with `lan=en`, the caller's `fields`, and the csrf token. Headers: browser UA,
  Referer `{web}/`, Origin, `X-Requested-With: XMLHttpRequest`, cookies.
- Retries once after `reset_session()` when: transport error, empty body, or
  the response does not contain `rows_marker`. Returns
  `AppError::source_unavailable("ntes", ...)` on final failure.
- `rows_marker` is critical: pick a literal present only on a genuine result
  page (e.g. `var myStns` for the route map, `var cStn` for spot map).

## HTML parsing conventions

- Write small private `parse_<feature>(&str) -> Option<Value>` helpers.
- Extract data from the JSON-ish `var x = [...];` JavaScript blocks the NTES
  pages embed (regex on the assignment, then `serde_json` parse). Keep
  tolerant: missing keys default to `""` / `false`, never panic.
- Preserve verbatim cell values where the UI shows them (delay cells: `""`,
  `"On Time"`, or `"HH:MM"`; dates `"17-Aug-2026"`).
- Train numbers are 5-char strings WITH leading zeros (`"00111"`) — keep them
  as strings end to end.

## Normalized JSON shapes (existing examples)

- route map: `{"trainNo","trainName","source","destination","sourceCode","destCode","startDate","route":[{code,name,arrival,departure,day,distance,daysOfRun}],"track":[code,...]}`
- spot map: adds `currentStation:{code}` and `journeyStation:{code,name,label,expectedArrival,actualArrival,delayStatus,platform}`, `status:[{code,expectedArrival,expectedDeparture,actualArrival,actualDeparture,arrivalDelay,departureDelay}]`
- lists (heritage/parcel/average-delay): `{"list":[{...}]}` with `selection`/`total` captions where the page shows them.

## Errors

- Unreachable/challenged NTES → `AppError::source_unavailable("ntes", msg)`.
- Parser found nothing on a seemingly-valid page → same, with a message naming
  the feature. The slice layer will fall back to IRCTC/Railyatri and report
  `data_source` honestly.

## Fixtures + tests

- Real pages captured from `enquiry.indianrail.gov.in` belong in
  `.agents/fixtures/` at the repo root (used as test/verification inputs).
- Hermetic tests never hit the network: `tests/common/mod.rs`
  `MockServer::route_html_seq` / `ntes_web(html)` serve canned HTML; use
  `app.mock("ntes").calls()` to assert the exact form fields sent upstream.
