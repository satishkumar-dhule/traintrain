---
name: ntes-live-verification
description: Verify railway-rs behavior against the live server and the real NTES endpoints (enquiry.indianrail.gov.in/mntes) without editing code. Use for "live test", "verify the NTES flow", "probe the running server", "check data_source / freshness", "confirm what the app sends upstream", or diagnosing "NTES blocked/unreachable" from the sandbox. Includes reading real page fixtures and the deployment caveat.
---

# Live verification (running app + real NTES)

Two distinct targets: the **running app** on `localhost:3000`, and the **real
NTES origin** at `enquiry.indianrail.gov.in`. These are different — the sandbox
deployment often cannot reach NTES while the app itself is up.

## 1. Probe the running app

```bash
curl -s localhost:3000/healthz                        # must be 200
curl -s 'localhost:3000/rail-api/observability'       # runtime metrics
curl -s 'localhost:3000/rail-api/source-status'       # per-source status
curl -s 'localhost:3000/rail-api/ntes/trains-between?src=NDLS&dst=DLI'
```

Read JSON carefully:
- `data_source` names the actual answering source. In the sandbox NTES is
  usually unreachable, so expect `"Railyatri"`/`"IRCTC"` fallback or an honest
  `source-unavailable` error — that is CORRECT behavior, not a bug.
- Check `state.cache` semantics: a second identical request should be a cache
  hit (faster, and `data_source` reflects whatever first produced it).
- App log: `/tmp/railway-rs.log`. Restart procedure: `rust-workflow` skill.

## 2. Verify the app-to-upstream flow (hermetic first)

Never assume network behavior — assert it. Hermetic tests record what the app
actually POSTs via `MockServer::calls()` (returns `(path, body)` pairs). For
NTES web-form flows, confirm: POST path (`/mntes/<Endpoint>?opt=..&subOpt=..`),
form fields (`lan=en`, the feature fields, csrf token), headers
(UA/Referer/Origin/X-Requested-With), and the session retry on empty/markerless
response.

## 3. Test the real NTES origin directly

From the sandbox, NTES returns empty/challenged responses, so live probing is
only reliable from a network that can reach it. When you CAN reach it:

- Bootstrap the session: `GET {base}/mntes/` then `GET {base}/mntes/GetCSRFToken`
  (extract `csrfToken`). Reuse cookies across the flow.
- POST the form with the same body the app builds (see
  `ntes-client-method`), with a browser UA + `Referer: {base}/` +
  `Origin: {base}` + `X-Requested-With: XMLHttpRequest`.
- A blank/empty body, or a page missing the expected `rows_marker`, means a
  challenge — reset the session and retry once (the app does exactly this).
- **Capture real pages** (`curl -o .agents/fixtures/<feature>.html`) so the
  parser can be developed/fixed against ground truth without a live connection.

## 4. Researching NTES (protocol details)

When pinning down endpoint variants (JSON vs HTML forms, cookie/CSRF needs,
`TrnMap`, `TrainsAtStation`, `AverageDelay`, `HeritageTrainsBetweenStation`,
`TrainRunning`/`splTrnDtl`, `TrnBtwStnJson`):
- Prefer official NTES pages and community open-source projects, then verify
  assumptions with a real capture before coding.
- Answer protocol questions empirically with small live probes and record the
  results in the slice doc comment — don't guess headers/fields.

## Deployment caveat

`GET /mntes` variants may answer on one deployment and not another (IP
blocking). When the app reports "NTES did not answer", first re-check
`source-status` and the raw request from that deployment; the app should keep
falling back honestly rather than failing the page.
