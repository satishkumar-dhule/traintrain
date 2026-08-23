# Step-Reduction & Journey-Optimization Plan — railway-rs ("Train Bro")

> Deep-research synthesis · 30 parallel research streams: internal journey audit ×6, real-world
> use-case research ×7, academic/HCI literature ×6, competitive teardowns ×5, engineering
> feasibility/synthesis ×6. Aug 2026. **Plan only — no implementation.**

## 1. What this app is today

Rust/axum NTES+IRCTC mirror serving a Svelte SPA. 12 pages: Home, Train (live/schedule/avg-delay/map),
Station (board/timetable), Journeys, Availability, PNR, Exceptions, Extras (heritage/parcel), Assistant,
Insights, System, About. Data federated from NTES web forms, IRCTC, Paytm, CoRover AskDISHA, RailYatri;
120s default TTL cache; no accounts, no cookies, no third-party trackers.

Core promise: answer "is my train late / will my WL confirm / what's at my station" faster than official
channels — which lose users to lag (10–30 min NTES delay), captchas, and multi-step forms.

## 2. Real-world users & use cases (evidence-backed)

| # | Persona | Trigger | Decision | Min info needed |
|---|---------|---------|----------|-----------------|
| P1 | Morning checker | Every workday before leaving | Leave now or wait? | Delay min + trend + ETA at boarding stop |
| P2 | Station greeter | Family arriving; platform ticket ₹10/2h validity | When to leave, where to stand | Revised arrival + platform + coach position |
| P3 | Overnight sleeper | Afraid of missing stop at 3 AM | When to wake | Distance-to-station alarm (time alarms fail when train is 5h late) |
| P4 | WL-anxious booker | Tatkal rush (62–66% gone in first 10 min) or daily WL polling | Book WL or alternate? | Confirmation % band + alternates |
| P5 | Connector | Two-train trip, first +3h late | Rebook / TDR / evidence | Both trains' status at interchange |
| P6 | Cab-timer | 3 AM arrival, surge pricing | When to summon ride | Reliable revised ETA |
| P7 | Food orderer | eCatering/Zoop mid-journey meal | Order now for station X? | Will train reach X while restaurant open (documented #1 failure: food missed train) |
| P8 | Suburban commuter | Mumbai/Delhi daily ritual | Which frequent train | Countdown, delay, platform |
| P9 | Worried family | Traveler unreachable mid-route | Worry or wait | Shared live-position link |
| P10 | Low-connectivity traveler | Tower dead zones, rural | Everything above, offline | Cached timetable + last-known position |

Cross-cutting: **every flow is PNR- or train-number-gated**, and the most-documented systemic failure is
*scheduled-vs-actual time divergence* (food, pickups, alarms). Apps that collapse check→decide win.
Sources: WIMT/ixigo listings+reviews, r/indianrailways, Livemint TDR rules, parcel.indianrail.gov.in,
PIB Yatri Mitra, RailRecipe auto-cancel-on-delay docs, Lok Sabha NTES usage data (6cr enquiries/day).

## 3. Where the steps are today (audit findings)

Svelte UI is active (`frontend/src`); vanilla `static/*.js` is orphaned legacy kept alive by tests only.

| Journey | Steps today | Friction found |
|---|---|---|
| Live status | ~6 | Auto-refresh resets OFF each visit; no countdown; delay tab separate from spot view |
| Station board | ~5–7 | Hours window not deep-linked; no auto-refresh on Svelte page |
| Avg delay | 4+wait | Separate pill/tab; two delay datasets never bridged |
| Trains-between | ~6 cold | Only row action = Availability; recents shown only on idle pages, not inside inputs |
| Availability | ~7 fresh | Date defaults today even coming from Journeys; source select resets per mount |
| PNR | 3 clean / 6+ captcha loop | **Recent PNRs NOT persisted** (orphaned chip code proves intent); result drops chartStatus/freshness; jump buttons discard context |
| Insights | nav→prefill→click Explain again→wait ≤120s | Deep link doesn't auto-run; needs exact 5-digit number, no name lookup |
| Assistant | 1 turn (good local-first gate) | AI-off = hard dead end even for zero-LLM paths; seed param exists but nothing links to it; no starter chips |
| API cost | Train sweep = 6 calls/5 endpoints, avg-delay fetched twice; busy corridor = **30+ round trips** (one avg-delay call per row badge); schedule cache is **write-only** (never read); Home fires 27 letter-sweep requests per mount |

Systemic dead-end pattern: informational views (Exceptions, Parcel, Insights, System) have zero onward
entity links → users bounce back to search = extra steps.

## 4. Research-grounded principles (the "why")

1. **Network-first rendering is the enemy.** 0.1s=instant (Miller/Card/Nielsen); INP 200ms budget; 53%
   abandon >3s blank loads (Akamai); 0.1s gain ⇒ travel conversions +10.1% (Deloitte×Google, 30M
   sessions). Skeletons lose to cached content at medium waits (Viget n=136). → **Cached-paint-then-revalidate.**
2. **Known waits feel shorter** (Maister 1984); passive waits overestimated ~36% (Hornik/Larson).
   → Countdown chips ("updated 42s ago · next 18s"), explained-wait lines during slow NTES scrapes.
3. **Defaults carry d≈0.68** (Jachimowicz); Google autocomplete cuts typing 25%. → Geolocated origin,
   remembered From/To/date, recents *inside* inputs.
4. **Numeric ranges don't hurt trust; verbal hedging does** (van der Bles/Spiegelhalter PNAS n=5780);
   uncertainty improves decisions + post-error retention (Joslyn & LeClerc); transit users punish false
   precision (OneBusAway CHI'16). → "Arrival ~18:47 · likely 18:35–19:05"; WL bands = number+color+verb.
5. **Progressive disclosure: 80% of tasks complete at level 1, max 2 levels** (NN/g; Carroll studies).
   Amtrak case: surfacing delay reason cut info-finding from 4–5 screens to 1–2. → Glance row =
   time·dest·platform·status; one tap = why/where; two taps = stats.
6. **Chat beats forms only for single factual lookups & vague queries; tables beat chat for comparison**
   (Nguyen'22; NN/g chatbot tests '25–26; voice: 88–94% factual accuracy vs ~2% ever transacting; METR:
   assistance slows experts with better tools). → Route lookups to Train Bro; comparisons to tabs; end
   answers with deep links.
7. **Proactive beats pull — JITAI-style only** (Nahum-Shani): actionable triggers, capped frequency.
   Proven: ixigo SMS-parsed PNR watch, DB platform-change push, Yahoo line subscriptions.
8. **Low-bandwidth discipline = India requirement.** NN/g India study (512MB phones, data rationing);
   Flipkart Lite (63% 2G): 3× less data, +70% A2HS conversion; Twitter Lite: 600KB cold, −20% bounce;
   2G RTT ≈1.28s — each round trip costs 0.3–1.5s. → Composite endpoints, text-first, data-saver mode.
9. **Choice overload**: 24 options vs 6 → purchase 3% vs 30% (Iyengar/Lepper); infinite scroll harmful for
   search lists (Baymard). → Boards render top 15–25 upcoming + filter chips.
10. **Competitors win by input elimination**: ixigo reads booking SMS → zero-typing PNR tracking (2013);
    WIMT tracks via cell towers offline; both alarm by geography not clock. Web equivalents here: deep
    links, clipboard paste, geolocation cache, localStorage.

## 5. THE PLAN

### Phase 0 — Instrument first (~6–7 dev-days)

| # | Item | Effort |
|---|------|--------|
| 0.1 | Per-entity metrics (`train:{num}`, `station:{code}`, `pair:{S}:{D}`) in `metrics_mw`; top-N on observability; unblocks real popular-train chips (Home's are hardcoded `[12951,12309,12002]`) | 0.5–1d |
| 0.2 | Self-hosted beacon slice `POST /rail-api/beacon` (allowlisted events page_view/query_committed/result_rendered+ttfr_ms/error_shown; NDJSON; tracing sink into existing logs/ring; CSP untouched). Client SDK ~100 LOC on `router.svelte.js::navigate` + `api.js` chokepoint. No raw PNR ever sent | 2.5d |
| 0.3 | Funnels wired: PNR completion incl. 428-captcha sub-funnel; train-lookup p50/p90 ttfr; journeys→availability conversion; assistant success counters (**streaming routes bypass metrics_mw — instrument ai_chat directly**) | incl. |
| 0.4 | A/B flags without accounts: `RAILWAY_EXPERIMENTS="name=pct"` env + md5 bucketing on anonymous localStorage id (`md-5` crate already in tree); every beacon carries variant | 1.5d |

### Phase 1 — Quick wins (each XS/S; ship in days)

| # | Change | Why it works / evidence | Effort |
|---|--------|--------------------------|--------|
| 1.1 | Fix write-only schedule cache (`schedule/service.rs` never calls `cache.get`) | Repeat views go network→instant free | XS |
| 1.2 | Persist recent PNRs + chips (`rc-pnr-recent` exists!); `/pnr/{n}` already auto-submits | Saves retyping 10 digits + captcha exposure; ixigo/WIMT norm | S |
| 1.3 | Auto-refresh: persist preference per key + countdown chip; add polling to Station & PNR pages | Maister; users poll manually today | S |
| 1.4 | Auto-run Insights when URL carries kind+train (auto-run already exists for station pairs) | Pure redundant-click removal | XS |
| 1.5 | Assistant: starter chips in empty state; auto-send `/assistant/{seed}` links from Train/Station pages; keep zero-LLM paths alive when AI disabled (today hard dead end) | NN/g discoverability; gate.js fast paths exist | S |
| 1.6 | Recents inside AutoCompleteInput & PowerSearch empty states | Google autocomplete evidence | S |
| 1.7 | Swap-direction memory; Journeys→Availability carries selected run date (not always today) | Audit | XS |
| 1.8 | Kill Home 27-request letter sweep (lazy explorer); delete dead SuggestSearch.svelte | Perf audit | XS |
| 1.9 | Clamp stored historical dates to today on recent-chip replay | Off-by-one bug + steps | XS |
| 1.10 | Onward entity badges from dead ends (Exceptions rows, parcel rows, insight cards) — badge components already exist | Systemic dead-end fix | S |

### Phase 2 — Structural step reduction (the big wins, weeks)

| # | Initiative | Detail | Effort |
|---|-----------|--------|--------|
| 2.1 | **Composite endpoints** (precedent: ai_insight/ai_chat already call sibling slices in-process) | • `GET /rail-api/train/{num}/overview?parts=…` = live+schedule+avg-delay+exceptions+map (6→1 calls) • `live-status?include=delay_summary` kills per-row badge tax • `GET /rail-api/journey/{src}/{dst}/plan` = trains-between with embedded availability + delay summaries (30+→1 on busy corridors; availability page reuses same payload) • `GET /rail-api/station/{code}/board` = station meta+live board+timetable (3+N→1). Add ETag/304 on composites (none exist today) | 3+1+4+2d |
| 2.2 | **Cached-paint-then-revalidate in `api.js`** — single 40-line chokepoint: memory cache keyed by path, paint cached instantly + subtle revalidate pulse + "checked N min ago" chip; never unmount content tree. Server stamps `as_of`/`age_seconds` next to existing `data_source` | Deloitte 0.1s evidence; SWR RFC5861 pattern; INP≈0 | S–M |
| 2.3 | **Omnibox journey grammar** in PowerSearch (PNR detection `/^\d{10}$/` already exists): parse `ndls pune` / `a to b` → resolve via suggest → jump to `/journeys/S/D`. Any search becomes ≤3 steps from any page | Audit; National Rail/Yahoo patterns | M |
| 2.4 | **Paste-anywhere smart routing** — document-level paste listener: 10 digits → PNR page; 4–5 digits → train; "X to Y" → journeys. Zero UI | Web equivalent of ixigo SMS-parsing trick | S–M |
| 2.5 | **Unified Trip page** — merge Journeys+Availability into one URL-driven page with tabs (kills the availability↔journeys ping-pong); rows get Schedule links too | Audit friction #1 for planners | M |
| 2.6 | **Uncertainty display layer** — delay as point+range ("~18:47 · 18:35–19:05", plan by upper bound), WL prediction bands (>75/40–75/<40% with action verb + non-color cue); render `prediction` field availability API already returns but static UI dropped | Spiegelhalter/Joslyn/ConfirmTkt banding | M |
| 2.7 | **Geolocation default origin** — silent warm-up once, cache `{code,name,dist}` 24h, prefill everywhere ("From: NDLS · 2.1 km"); manual suggest fallback inside nearby dialog when permission denied (today: dead end) | Jachimowicz defaults; audit | M |
| 2.8 | **Name-tolerant submit** — resolve free text via suggest/BM25 before `require_station` rejects ("Pune" works without picking from list); add train-name autocomplete to Insights (currently bare numeric input while chat resolves names fine) | Chat gate proves pattern (`gate.js resolveSlot`) | S |

### Phase 3 — Proactive & ambient (the differentiators)

| # | Initiative | Detail | Effort |
|---|-----------|--------|--------|
| 3.1 | **PWA shell + offline cache** (~2–3d): fix orphaned manifest (`start_url:"/#/"` targets dead hash router; SVG-only icon), link it, service worker: cache-first assets, network-first-with-fallback for GETs so last views render offline with "as of" stamp. Flipkart Lite precedent | 2–3d |
| 3.2 | **Station alarm (client polling)** (~2–3d): bell per upcoming stop; reuse 30s interval idiom + Notification API behind user gesture; wake-lock toggle w/ visibilitychange re-acquire; degrade to scheduled-time countdown labeled "offline estimate" when signal drops. Foreground-only limits documented | 2–3d |
| 3.3 | **Server push for PNR/live-status changes** (~6–9d + risk): web-push crate + VAPID + durable store (none exists — state is all in-memory) + tokio scanner diffing watched entities. **Blocker: PNR upstream is captcha-gated (HTTP 428)** — needs a non-captcha source path first or scope push to live-status/exceptions only | 6–9d |
| 3.4 | **Proactive inline insight** — Train page auto-fires `/ai/insight` live_status summary near header (insight cache makes repeats free); kills Explain→navigate→Explain→wait chain | S |
| 3.5 | **Assistant context seeding** — inject current train/station/date into chat server-side (persona injection point exists in ai_chat/service.rs); entity carryover from sessionMemory so follow-ups ("and its route?") hit zero-LLM fast paths | S–M |
| 3.6 | **Data-saver mode** — respect `navigator.connection.saveData`; ON = fetch only on manual refresh, no map tiles/images; auto-suggest on 2g/slow-2g effective type; target <600KB cold load (Twitter Lite benchmark) | M |
| 3.7 | **QR share posters** — QR of canonical deep links (`/station/NDLS`, `/train/12951`) printable at stations/coaches; scan→zero typing | XS |
| 3.8 | **NTES CSRF keep-warm + journey-basis/train-on-map caching** — worst-case scrape cost today is 6–8 RTTs before honest failure; time-based token refresh collapses it toward 1 RTT; two NTES slices have zero cache | S |

## 6. Impact × effort matrix

```
            Low effort ──────────────────────────► High effort
 High │  1.1 cache bug   1.2 recent PNR      2.1 composite APIs ★
impact│  1.4 autorun      1.3 countdown       2.2 cached-paint
      │  1.7/1.9 dates    1.5 assistant chips 3.2 station alarm
      │  3.7 QR           2.4 paste routing    3.3 server push
      │                   1.6 recents-in-inputs
  Low │  3.8 keep-warm    2.8 name-tolerant   3.6 data-saver (defer)
impact│                   1.10 badges         2.5 trip page (do with 2.1)
```
★ = do first: composite endpoints unlock cached-paint value and cut 30-call corridors to 1.

## 7. Measurement (proves the wins)

- **North star**: p50/p90 *time-to-answer* per top task (train lookup, PNR check, board view).
- Funnels: PNR completion rate (incl. captcha sub-funnel), journeys→availability conversion,
  assistant answer success, dead-end bounce rate (should drop after 1.10).
- Guardrails: beacon volume, error rates, NTES upstream latency (don't trade steps for outages).
- A/B each Phase-1/2 change behind flags (0.4); override/opt-out monitoring per JITAI guardrails.

## 8. Risks & caveats

1. **Upstream fragility**: NTES WAF/captcha; every step-reduction that increases request fan-out must be
   cache-first and concurrency-capped (≤8).
2. **Server push blocked on PNR captcha** — scope 3.3 accordingly.
3. **Prediction display**: only render confidence if calibrated; state reference class ("last 30 days of
   this train's arrivals"); never color-without-number (CVD ~8% of men).
4. **Privacy stance**: no accounts/trackers is a feature — beacon design above preserves it (session ids,
   allowlisted events, no raw PNR).
5. **Legacy duplication**: vanilla `static/*.js` still ships in Docker; prune after Svelte parity to avoid
   double maintenance.
6. Choice-overload caveat: overload bites mainly when options are attractive + user must commit
   (Scheibehenne meta-analysis ≈0 mean effect) — top-N lists need good ranking, not just fewer rows.

---
*Research artifacts: 29 agent reports covering code audits (file:line citations throughout), use-case
evidence with sources, HCI literature (NN/g, CHI, PNAS, Baymard), competitive teardowns (WIMT, ixigo,
RailYatri, ConfirmTkt, Trainman, NTES, IRCTC/AskDisha, DB Navigator, Citymapper, National Rail, Yahoo
Transit), plus engineering feasibility studies (composite APIs, PWA/push, metrics/A-B, context
automation). All findings verified against the actual tree where applicable.*
