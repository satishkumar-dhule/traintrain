# AskDISHA (`askdisha.irctc.co.in`) — Complete Data-Call Reverse-Engineering Document

> **Scope of this document.** This is a full technical teardown of how IRCTC's AI
> chatbot **AskDISHA** makes data calls: every discovered API endpoint, the exact
> request headers/payload shapes, the response contract, authentication internals,
> voice/payment/embedding pipelines, third-party beacons, infrastructure fingerprints,
> and a **programmatic emulation guide** (with working request recipes) for research
> and interoperability purposes.
>
> Everything here was extracted from the **publicly served JavaScript bundle**
> (`main.0384a92c.js`, 2,684,435 bytes), live HTTP probing, Wayback Machine history,
> vendor documentation, and 30 parallel deep-research subagents. No private data was
> accessed; all credentials shown are client-shipped public values.
>
> **Ethics/legal note:** `api.disha.corover.ai` is an undocumented, token-gated
> government-adjacent service. Emulation guidance below is limited to *benign guest
> read-only interactions* and explicitly excludes OTP automation, payment abuse,
> captcha circumvention, Aadhaar flows, and load generation. Respect IRCTC/CERT-In
> rules; there is no bug-bounty program covering this surface.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [System Architecture](#2-system-architecture)
3. [Frontend Stack & Bundle Map](#3-frontend-stack--bundle-map)
4. [Infrastructure & TLS Fingerprint](#4-infrastructure--tls-fingerprint)
5. [Content-Security-Policy Decoded](#5-content-security-policy-decoded)
6. [Authentication Model — All Four Layers](#6-authentication-model)
7. [`dSession` Anti-Replay Generator — Full Algorithm](#7-dsession-generator)
8. [Master Endpoint Catalog (~50 endpoints)](#8-master-endpoint-catalog)
9. [Core Chat Loop — Request/Response Contract](#9-core-chat-loop)
10. [Server→Client Response Schema (renderTemplate variants)](#10-response-schema)
11. [Auth / OTP Login Flow](#11-auth--otp-login-flow)
12. [Voice Pipeline (STT/TTS)](#12-voice-pipeline)
13. [Payment Pipeline (Paytm Native)](#13-payment-pipeline)
14. [Booking Flow End-to-End Sequence](#14-booking-flow)
15. [Station / Location / Enquiry Calls](#15-station-location-calls)
16. [CDN Config & Feature Flags](#16-cdn-config--feature-flags)
17. [Embedding Protocol (iframe + postMessage)](#17-embedding-protocol)
18. [Telemetry & Monetization Beacons](#18-telemetry--monetization)
19. [Resilience / Error-Handling Behavior](#19-resilience-behavior)
20. [Client State Surfaces (storage keys)](#20-client-state)
21. [Version History (2018 → 2026)](#21-version-history)
22. [Vendor Ecosystem Context (CoRover pattern)](#22-vendor-ecosystem)
23. [Security Observations](#23-security-observations)
24. **[Programmatic Emulation Guide](#24-programmatic-emulation-guide)** ← recipes
25. [Emulation Scope Matrix — allowed vs forbidden](#25-emulation-scope-matrix)
26. [Open Questions / Unverified Items](#26-open-questions)

---

## 1. Executive Summary

| Property | Value |
|---|---|
| Product | AskDISHA ("Digital Interaction to Seek Help Anytime"), IRCTC's AI virtual assistant |
| Vendor | **CoRover Pvt Ltd** (Bengaluru) — white-label deployment of CoRover Conversational AI platform |
| Launched | Oct 2018 (v1 Q&A); Feb 2020 (Hindi+voice); Sep 2022 (**2.0**: end-to-end booking/cancel/refund in-chat, OTP passwordless) |
| Web app | React SPA (Create-React-App) at `https://askdisha.irctc.co.in/` |
| App hosting | Cloudflare Pages (CNAME → `irctc-booking-ui-prod.pages.dev`, Mumbai POP) |
| **API origin** | **`https://api.disha.corover.ai/dishaAPI/*`** (~50 REST endpoints) |
| Transport | Plain HTTPS REST via axios. **Zero WebSockets anywhere** (CSP `wss://*.corover.ai` allowance is dead config). Real-time = fixed-interval polling + CancelTokens. Newer CoRover stacks use SSE-over-POST; AskDISHA's current bundle does not. |
| Auth | Hardcoded tenant UUID pair (`app-id`/`auth-Key`) + OTP-minted JWT (`cxtoken`) + client-generated uuid (`x-user-token`) + per-request RSA-sealed `dSession` |
| Payments | Paytm native stack proxied through CoRover (MID `IRCTCC84510399265141`) |
| Languages | `en`, `hi`, `gu` (path segment on sendQuery; STT locales hi-IN/en-IN/gu-IN) |
| Embedding | `<script>` loader from `sdk.irctc.corover.ai` → iframe `#Disha-Bot` into `www.irctc.co.in/nget/train-search` and `www.indianrail.gov.in` |

**One-sentence architecture:** a thin CRA React shell ships hardcoded tenant
credentials to every visitor's browser and exchanges pure JSON over HTTPS with one
vendor-owned API host, which server-side fans out to IRCTC booking/PNR/refund
backends, Paytm settlement, Aadhaar eKYC (Railtel KUA), and cloud TTS/object storage
— the browser never talks to IRCTC's own transaction systems directly.

---

## 2. System Architecture

### 2.1 Bird's-eye view

```mermaid
flowchart TB
    subgraph CLIENT["Browser"]
        A["www.irctc.co.in<br/>(Angular portal)"] -- "script tag (cache-busted)" --> B["sdk.irctc.corover.ai<br/>chatbotlib.min.js"]
        B --> C["irctc_disha.widget.js<br/>(DOM shell, zero data calls)"]
        C -- creates lazy iframe --> D["iframe #Disha-Bot<br/>src=askdisha.irctc.co.in<br/>(React CRA SPA)"]
        E["www.indianrail.gov.in"] -- "chatbotlib-ir.min.js" --> C
    end

    subgraph VENDOR["CoRover platform (GCP)"]
        F["api.disha.corover.ai<br/>Google HTTPS LB → nginx 1.22.1<br/>~50 REST endpoints /dishaAPI/*"]
        G["cdn.corover.ai (Cloudflare)<br/>config JSONs, TTS audio, launcher GIFs"]
        H["cognitive.service.corover.ai<br/>legacy TTS mp3 storage"]
        I["uiresource.blob.core.windows.net<br/>Azure Blob media"]
        J["storage.corover.ai<br/>chatbot-audio-bucket-aws"]
    end

    subgraph SERVERSIDE["Server-side fan-out (invisible to browser)"]
        K["IRCTC booking / PNR /<br/>refund backends"]
        L["Paytm settlement<br/>MID IRCTCC84510399265141"]
        M["Aadhaar eKYC (Railtel KUA)"]
        N["SMS/Email OTP gateways"]
    end

    subgraph THIRD["Direct browser↔third-party"]
        O["secure.paytmpayments.com<br/>CheckoutJS only"]
        P["GTM/GA4 · Clarity · CF Insights<br/>AdSense · Unibots · VDO.AI"]
    end

    D -- "axios JSON (app-id/auth-Key/cxtoken/x-user-token/dSession)" --> F
    D -- "GET *.json config + TTS audio" --> G
    D -.-> H
    D -.-> I
    D -.-> J
    F --- K
    F --- L
    F --- M
    F --- N
    O -.-> O
    D --> P
```

### 2.2 Key structural facts

- The **SPA owns all logic**; only 3 JS chunks exist and both lazy chunks are
  polyfill/DOMPurify with **zero API calls**. Everything lives in `main.<hash>.js`.
- The widget loader chain makes **no data calls** except Google Analytics gtag.
- There is exactly **one anomalous relative-path call** (`POST /dishaAPI/bot/addUserName`
  against the page origin itself) — residue of the pre-split era when API and app
  were co-hosted on the same origin.
- CORS on the API is a **fixed constant** origin (`https://askdisha.irctc.co.in`),
  never reflected — so emulation must either run from that origin or ignore CORS
  entirely (server-side scripts don't care).

---

## 3. Frontend Stack & Bundle Map

| Artifact | Size | Contents | Data calls |
|---|---|---|---|
| `/index.html` (3,982 B) | — | Shell: GTM, Clarity, AdSense, unibots, `<audio id="myaudio">`, autoplay-unlock click handler posting `"CLICKED"` to parent | injects third parties |
| `/static/js/main.0384a92c.js` | 2,684,435 B | App code + axios + JSEncrypt + Fuse.js + react-speech-recognition + html2canvas/jsPDF + QRCodeSVG + DOMPurify consumer + jwt-decode | **ALL (~43 unique paths)** |
| `/static/js/239.9d3da7c0.chunk.js` | 111,317 B | performance.now/hrtime polyfill | none |
| `/static/js/760.948452a0.chunk.js` | 22,218 B | DOMPurify 2.5.9 | none |
| `/manifest.json` | — | "AskDisha 2.0", `id:"/eticket/"`, standalone, theme `#4458a9` | — |
| `/asset-manifest.json` | — | chunk map (confirms webpack runtime) | — |

Notable libraries observable in the bundle:

- **axios 1.x** — stock instance, `axios.create` count = **0**; no baseURL; every call
  absolute URL except the one relative anomaly. Axios UA string seen upstream: `axios/1.19.0`.
- **JSEncrypt** — RSA PKCS#1 v1.5 encryption of `dSession`.
- **jwt-decode** (custom base64url decoder) — client-side freshness check on cxtoken.
- **Fuse.js** — local fuzzy station search keyed on `code,name,utterances,district,state`.
- **react-speech-recognition** wrapper around `webkitSpeechRecognition`.
- **QRCodeSVG** (size 180, level H) for UPI QR rendering.
- **html2canvas + jsPDF + pdfobject (cdnjs)** — ticket/receipt download.
- **No service worker registered anywhere** (`serviceWorker` = 0 hits). Firebase config
  present but **inert dead code** (see §16).

---

## 4. Infrastructure & TLS Fingerprint

Live-probed host inventory:

| Host | IP | Edge/Origin | Notes |
|---|---|---|---|
| `askdisha.irctc.co.in` | Cloudflare | Pages, Mumbai POP (`cf-ray …-BOM`) | CNAME → `irctc-booking-ui-prod.pages.dev`; HSTS preload; full CSP; `Permissions-Policy: geolocation/microphone=(self …irctc…indianrail), camera=()` |
| `api.disha.corover.ai` | 34.54.202.50 | **Google Cloud HTTPS LB** (`Via: 1.1 google`, `alt-svc h3`) → nginx/1.22.1 | Root serves stock nginx welcome page (Last-Modified 2022-10-19); app routes under `/dishaAPI/`; helmet-style security headers on app responses; multiline permissions-policy breaks curl's HTTP/2 parser → probe with `--http1.1` |
| `cdn.corover.ai` | Cloudflare | ACAO `*`, speculation-rules header | static config/audio buckets |
| `cognitive.service.corover.ai` | 35.244.15.119 | nginx/1.14.1 | Angular "CoRover utility" SPA + legacy speech mp3s |
| `corover.ai` | 34.93.39.132 | nginx/1.28.0 (Ubuntu) → Next.js | marketing site, Sanity CMS, CSP-Report-Only w/ report-uri `/api/csp-report` |
| `api.corover.ai` | 172.105.52.132 (Linode) | firewalled / not listening | TCP :443 hangs |
| `sdk.irctc.corover.ai`, `sdk.ir.corover.ai` | Cloudflare Pages | widget distribution | cache-busted per pageload |
| `assistant.corover.mobi` | legacy | DNS unresolved now | v1 era host, ad iframes |
| `dishav3.ap-south-1.linodeobjects.com`, `d3upbvvdvllr10.cloudfront.net`, `storage.googleapis.com/azure-coroverbackendstorage-blob` | object storages | assets | vendor-attribution markers |
| `559p2ll052.execute-api.ap-south-1.amazonaws.com` | AWS | **NXDOMAIN (decommissioned)** | formerly mFilterIt fingerprint collector (§18) |

API response headers worth noting (POST responses):

```
access-control-allow-origin: https://askdisha.irctc.co.in   (FIXED, never reflected)
access-control-allow-credentials: true
access-control-expose-headers: Content-Length, Content-Type
strict-transport-security, x-frame-options SAMEORIGIN, nosniff,
cross-origin-opener/corp policy same-origin, referrer-policy no-referrer
```

Preflight (204) additionally whitelists methods `GET, POST, OPTIONS` and headers:

```
Content-Type, Accept, Referer, User-Agent, sec-ch-ua, sec-ch-ua-platform,
sec-ch-ua-mobile, Authorization, cxtoken, auth-Key, app-id, askdishaId,
access_key, x-user-token, x-user-mobile, credentials, withCredentials
```

> The last two entries (`credentials`, `withCredentials`) are axios **option names**,
> not real headers — someone pasted the axios config key list into the CORS allowlist.
> This whitelist is effectively a map of the entire auth surface.

---

## 5. Content-Security-Policy Decoded

Served on every page load. Each directive maps to a concrete call surface:

| Directive | Entries → meaning |
|---|---|
| `default-src` | `'self' *.corover.ai askdisha-pwa.firebaseapp.com uiresource.blob.core.windows.net` |
| `script-src` | corover, gstatic, cloudflareinsights, googlesyndication, doubleclick, trendmicro, **vdo.ai**, unibotscdn, googletagmanager, clarity.ms, **paytm/paytmpayments** |
| `connect-src` | `'self' *.corover.ai` **`wss://*.corover.ai` (unused)** googleapis, firebaseio, google-analytics, analytics.google, clarity.ms, googlesyndication, unibotscdn, paytm, paytmpayments, cdn.corover.ai, cloudflareinsights, uiresource blob |
| `frame-src` | self, firebaseapp, google.com, doubleclick, googlesyndication, **openx.net**, **criteo.com**, adtrafficquality.google, safeframe, paytm |
| `frame-ancestors` | `'self' https://*.irctc.co.in https://*.indianrail.gov.in` ← who may embed |
| `media-src` | paytm, uiresource blob, **cognitive.service.corover.ai**, cdn.corover.ai ← TTS hosts |
| `img-src` | `self *` data: blob/cloudfront |
| `font-src/style-src` | fonts.googleapis/gstatic, flaticon uicons, uiresource blob |

Verdict: `wss://*.corover.ai` is **headroom for VideoBot/VoiceBot products CoRover
sells elsewhere** — nothing in any deployed AskDISHA surface exercises it.

---

## 6. Authentication Model

Every request to `/dishaAPI/*` carries a header set built by the bundle's `Ao()`
function (verbatim de-minified):

```js
Ao = () => {
  const t = localStorage.getItem("dishav2-data"),
        n = t?.(JSON.parse(t))?.cxtoken;
  const i = redux.cxtoken || n || null;         // session JWT wins
  const { askdishaId } = redux.user;
  const { userToken } = redux.app;
  return {
    "Content-Type": "application/json",
    "app-id":      "29fd4f94-f793-4227-9588-056b5ffb1318",
    "auth-Key":    "2b5fb5d4-0753-4302-b661-f8580e9effb0",
    cxtoken:       sanitize(i),
    askdishaId:    sanitize(askdishaId),
    "x-user-token": sanitize(userToken),
  };
};
// sanitize(x): null unless x !== "" && x !== "null" && x !== "undefined"
```

### Layer table

| # | Header | Issuer | Format | Persistence | Purpose |
|---|---|---|---|---|---|
| 1 | `app-id` | build-time literal | UUIDv4 | baked in bundle | tenant identity (stable across all eras since ≥Dec 2023) |
| 2 | `auth-Key` | build-time literal | UUIDv4 | baked in bundle | tenant secret (shipped to every visitor!) |
| 3 | `cxtoken` | server (`verifyLogin` response) | JWT (3× base64url parts), custom claim `signedDate` | `dishav2-data.cxtoken`; mirrored to parent-page cookie (1 y) via postMessage | authenticated chat session |
| 4 | `x-user-token` | **client** (`Es()` uuidv4, minted lazily) | UUIDv4 | `dishav2-data.usertoken`, survives reloads | anonymous device/user id |
| 5 | `askdishaId` | server (user record) | id string | redux + dishav2-data | per-user profile pointer |
| 6 | `x-user-mobile` | client | 10-digit | transient | **only on `bot/login/en`** |
| 7 | `dSession` (body field) | client per-request | RSA blob (§7) | never stored | anti-replay nonce |

Token lifecycle rules observed in code:

- **401 handling:** helper `co(response)` dispatches `SET_UNAUTH` then returns a
  promise resolved after `setTimeout(t, 2147483647)` ms (~24.8 days) — i.e., the
  awaited flow **deadlocks on purpose**; UI shows blocking modal
  *"Session Time Out! … Click OK to start a new session"* whose only recovery is
  `window.location.reload()`. No refresh-token logic exists anywhere.
- **cxtoken freshness:** after verifyLogin, client decodes the JWT payload and
  rejects it if `Date.now() - signedDate > 10_000` ms — a replay guard, not expiry.
- **Guest mode fully supported:** all enquiry/chat calls work with
  `cxtoken:null, x-user-token:<random uuid>`.
- Quirk: in embedded mode with body height ≤ 310 px, **every sendQuery replaces
  userToken with a fresh uuid** (`u || (d.userToken = Zf())`).

---

## 7. dSession Generator

Per-request anti-replay blob. De-minified algorithm (`zi()`):

```js
function genDSession() {
  // 1. uuid v4, then insert 5 random lowercase letters at index 5
  let a = uuidv4();
  const letters = randomLowercase(5);
  a = a.slice(0,5) + letters + a.slice(5);

  // 2. plaintext = base64(marked_uuid) + "," + marked_uuid
  const plain = btoa(a) + "," + a;

  // 3. RSA-2048 PKCS#1 v1.5 encrypt with embedded public key
  const enc = new JSEncrypt();
  enc.setPublicKey(atob(REACT_APP_P865243658));   // PEM "-----BEGIN PUBLIC KEY-----"
  return enc.encrypt(plain);                       // base64 ciphertext string
}
```

Python equivalent (verified shape):

```python
import os, uuid, base64
from cryptography.hazmat.primitives import serialization, hashes
from cryptography.hazmat.primitives.asymmetric import padding

PUBKEY_PEM = b"""-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...   # extract from bundle var REACT_APP_P865243658 (base64 of PEM)
-----END PUBLIC KEY-----"""

def gen_dsession() -> str:
    marked = str(uuid.uuid4())
    marked = marked[:5] + ''.join(os.urandom(5).hex()[:5]) + marked[5:]  # 5 lowercase alnum chars
    plain = base64.b64encode(marked.encode()).decode() + "," + marked
    pub = serialization.load_pem_public_key(PUBKEY_PEM)
    ct = pub.encrypt(plain.encode(), padding.PKCS1v15())
    return base64.b64encode(ct).decode()
```

> Exact letter-generation detail: the bundle inserts **random lowercase letters**
> (`[a-z]`) — replicate with `random.choice(string.ascii_lowercase)` ×5 for fidelity.
> Server tolerance for deviations is unknown; safest is byte-faithful reproduction.

---

## 8. Master Endpoint Catalog

Base: `https://api.disha.corover.ai/dishaAPI`
Default headers: §6 `Ao()` unless noted. All bodies JSON.

### 8.1 Chat & NLU

| Method | Path | Purpose / notes |
|---|---|---|
| POST | `/bot/sendQuery/{lang}` | **core chat**; lang ∈ en\|hi\|gu; cancellable (singleton `eg`); full body §9 |
| POST | `/bot/transcribe` | audio→text; body `{query:<dataURL>, source, inputType, userToken, channel, dSession, deviceId, sessionId, isAudio:true, status:1, language}` → `{transcription}` |
| GET | `/bot/getStatus` | polled by `setInterval` for TXN_SUCCESS/TXN_FAILURE (long-poll style loop, not websocket) |

### 8.2 Session & Identity

| Method | Path | Body / notes |
|---|---|---|
| POST | `/bot/login/en` | `{source,mobileNumber,userToken,useOTP:true,audioUrl:null,inputType,dSession,sessionId,deviceId,status:1}` + header `x-user-mobile`. Resp `renderTemplate.data.Details`=**otpuuid**; `existingUser` flag; 500→reload; 429→lockout msg |
| POST | `/bot/verifyLogin/en` | `{source,otp,otpuuid,userToken,audioUrl:null,inputType,dSession,sessionId,deviceId,status,verifyPhoneNumber}` → `{cxtoken(JWT), existingData}` |
| POST | `/bot/logout` | `{source,inputType,userToken,channel,dSession,deviceId,sessionId,status:1,mobileNumber?}` |
| GET | `/bot/getUserData` | otp-send template / profile bootstrap; also surfaces `paytmCustId` |
| POST | `/bot/updateUser/{id}` | name/email/mobile/askdishaId/refreshUserData |
| POST | `/bot/getUserId/{userId}` | `{sessionId,dSession,userToken}`; status string `"1#1#1#1"` (invalid/unverified/disabled/suspended flags) |
| POST | `/bot/getUserId/getSMSeMailOTP` | `{sessionId,dSession,cxToken:"",userId:""}` |
| POST | `/bot/getUserId/verifySMSeMailOTP` | `{sessionId,dSession,cxToken,emailOTP,mobileOTP,userId}`; resp `errorIndex==="1"` ⇒ error |
| POST | `/bot/aadharAuthenticate/generateAuthentication` | Railtel KUA consent text flow |
| POST | `/bot/aadharAuthenticate/verifyAuthentication` | `userToken: t.userToken || fresh-uuid` |
| POST | `/bot/verifyPhoneNumber/{phone}` | err → `-1` |
| POST | `/bot/addUserName` | ⚠️ **RELATIVE path** → hits page origin `askdisha.irctc.co.in`, not api.disha |
| GET | `/client/getUserBookingCount/{userId}` | → email/mobile/userAadharKycStatus/monthlyBookingCount |
| POST | `/bot/cookiesData` | `withCredentials:true` |
| POST | `/bot/clearTempFlow` | flow state reset |
| POST | `/bot/sendSms` | misc |

### 8.3 Booking / Ticketing

| Method | Path | Notes |
|---|---|---|
| GET | `/addservices/eticket/{pnr}` | booking lookup; second variant suffix `.qr` (QR ticket image) |
| GET | `/bot/generateCaptcha/booking/{userToken}` | captcha image |
| GET | `/bot/verifyCaptcha/booking/{userToken}/{captcha}` | bool |
| POST | `/bot/savePassenger` | passenger form submit |
| POST | `/bot/editTrains/{id}` | body incl `deviceId,dSession,status:1,isDirect` |
| POST | `/bot/alternativeSplitJourney` | split-journey suggestion |
| GET | `/bot/pin/{pincode}[?city=]` | GST form: `{state, cityList[]}` auto-fill; fires when pin reaches 6 digits (cancelToken `zy`) |
| GET | `/bot/getRefundStatusByTxn/{txn}` | refund status |
| POST | `/bot/helpDetails/{lang}` | refund/report body `{refundDetails,pnr,transactionId,subCategory}` |
| POST | `/bot/ticketFeedback` | two shapes: `{rating,feedback,txn,pnr}` and `{answerId,feedback,comment,channel}` |
| POST | `/bot/reportAnIssue` | issue report |
| POST | `/bot/uiError` | client error reporting |
| PUT | `/bot/clicks/{flowUUID},{step}` | fire-and-forget telemetry `.catch(()=>{})` |

### 8.4 Payments (Paytm proxy cluster)

| Method | Path | Body |
|---|---|---|
| POST | `/bot/paytm/fetchCardDetails` | `{head:{token,tokenType:"TXN_TOKEN"},body:{orderId,bin}}` → BIN lookup (`channelCode`,`paymentMode`,`nativeOtpEligible`) |
| POST | `/bot/paytm/fetchCard` | `{custId}` (from getUserData `paytmCustId`) → saved cards w/ `savedCardId` |
| POST | `/bot/paytm/transactions` | `{head:{txnToken},body:{requestType:"NATIVE",orderId,paymentMode,cardInfo,authMode:"otp",coftConsent?}}` — cardInfo `"${savedCardId}|||"` or `"\|${PAN}\|${CVV}\|${MM/20YY}"` |
| POST | `/bot/paytm/vpaValidate` | `{head:{token,tokenType:"TXN_TOKEN"},body:{vpa,orderId}}` |
| POST | `/bot/directBankOTPRequest` | `{orderId,body:{txnToken,requestType:"submit",otp}}` max 2 tries unless bankRetry |
| GET | `/bot/upiStatus/{orderId}/{txnToken}?voice={bool}` | poll 7 s ×100 (~11.7 min); mobile variant 15 s; abort on TXN_FAILURE; ≥3 non-null responses required for late success |

### 8.5 Enquiry / Utility

| Method | Path | Notes |
|---|---|---|
| GET | `/bot/searchStation/{q}` | per keystroke, cancelToken `dn`, cap 50 results, catch→`-1`; resp rows `{code,name,name_hi?,name_gu?,district,state,utterances?,distance?}` |
| GET | `/bot/stationsByLocation/{lat}/{lng}` | raw GPS coords in path; slice top 200; `distance` field (km) |
| GET | `/bot/trnscheduleEnq/{trainNo}?journeyDate=&startingStationCode=` | headers only `{"Content-Type":"application/json"},data:{}` |
| GET | `/bot/getSecureUrl?fileName={name}` | signed-URL exchange; accept keys `secureUrl|secureURL|signedUrl|signedURL|url` |
| GET | `https://cdn.corover.ai/askdisha-bucket/{en|hi|gu}.json` | locale FAQ arrays |
| GET | `https://cdn.corover.ai/askdisha-bucket/getSettings.json` | feature flags (§16) |
| GET | `https://cdn.corover.ai/askdisha-bucket/{popular,countries,stationupdated}.json` | boot datasets |

---

## 9. Core Chat Loop

### 9.1 Request (verbatim field order from bundle)

```json
POST https://api.disha.corover.ai/dishaAPI/bot/sendQuery/en
Headers: Ao()  (see §6)

{
  "query":        "<user text>",
  "source":       "<navigator.userAgent>",
  "inputType":    "TEXT" | "VOICE",
  "next_context": "" | "<prevFlowUUID>,<stepNumber>",
  "cxpayload":    null | {...},
  "userToken":    "<uuid v4>",
  "suggestion":   false,
  "isFallback":   null,
  "isRefund":     null,
  "channel":      "<document.referrer if iframed else location.href>",
  "prevCode":     null,
  "audioUrl":     null | "<data:audio/webm;base64,...>",
  "isAudio":      false,
  "dSession":     "<RSA blob>",
  "deviceId":     null,
  "sessionId":    "<uuid v4 per page-load>",
  "status":       1
}
```

### 9.2 Sequence diagram

```mermaid
sequenceDiagram
    participant U as User
    participant S as SPA (askdisha.irctc.co.in)
    participant A as api.disha.corover.ai
    participant I as IRCTC backends (server-side)

    U->>S: types message
    S->>A: POST /dishaAPI/bot/sendQuery/en<br/>{query, next_context, dSession, ...}
    A->>I: intent routing / backend fetch (invisible)
    A-->>S: {answer, answerId, intent, renderTemplate,<br/>options, helpline, miniChatBotFaq, cxpayload}
    alt renderTemplate = payment/data template
        S->>U: interactive form/card rendered
    else plain answer
        S->>U: bubble (+ optional TTS <audio>)
    end
    opt VOICE input mode
        S-->>U: play response.audio via getSecureUrl signed URL
    end
```

### 9.3 Flow-context machine

Flows advance by round-tripping `next_context = "{flowUUID},{step}"` taken from the
previous response's context. Omitting it exits all flows.

| Flow | UUID (constant in bundle) | Bundle refs |
|---|---|---|
| BOOKING | `ade4a7db-d819-417d-832a-259307fd94c7` | 22 |
| REFUND | `06d8305e-0a87-47eb-834d-a86734de5892` | 4 |
| PNR | `1918abb8-6b17-426e-a79e-990c3b194c9a` | 3 |
| BOARDING | `68bce99a-844d-11ec-a8a3-0242ac120002` | 2 |
| TDR | `a154264e-3335-4796-9e1d-37b91d303040` | 2 |
| CANCEL | `e02d4aa2-678c-4c83-a199-58e25adb8dc8` | 2 |
| BOOKING_HISTORY | `2b35a9d4-…` | — |
| (unmapped sub-flows) | `f51e43dc-…, e499269a-…, d50bebd9-…, b5a92323-…, 8ce7ecdf-…, 84713750-…, 8111c891-…, 4bcb7544-…` | clicks telemetry |

---

## 10. Response Schema

Fields consumed by the client after any `sendQuery`/`getStatus`:

```jsonc
{
  "answer":          "string",            // main bubble text
  "audio":           "url-or-filename",   // TTS (resolve via getSecureUrl)
  "answerId":        "string",
  "intent":          "string",            // e.g. "FAQ"
  "renderTemplate": {                     // INTERCEPTS normal rendering
     "templateName": "passengerDetails" | "IRCTC-LOGINID-VERIFY" | ...,
     "showTrain":    bool,
     "data": { "txnToken","clientTransactionId","paymentAmount","userId",
               "qrCode"(upi://pay?...), "Details"(otpuuid), "existingUser",
               "Status":"Failure"|..., "newBoardingPoint","newBoardingDate","pnr" }
  },
  "options":         [...],               // quick-reply chips (presence matters, content ignored)
  "helpline":        "...",               // side drawer content (+3 s delay)
  "miniChatBotFaq":  ["string", ...],     // suggestion chips under input
  "apiReference":    "otp-send" | ...,    // "otp-send" ⇒ force login drawer
  "cxpayload":       {},                  // echo; inspected for redirect rewrite
  "render":          "trains" | "irctc-otp-verify",
  "errorMessage":    "string",            // fallback text
  "user_input":      "ASR transcript echo (data:audio queries)",
  "cxStructure": { "entity": [ {"type":"src"|"des"|"journeyDate"|"quota","value":"..."} ] }
}
```

**Dispatch precedence (exact):**

1. error status (`apiReference`/HTTP) → error UI
2. `renderTemplate` intercepts (`passengerDetails`+Error, `IRCTC-LOGINID-VERIFY`)
3. `options` → quick replies
4. `data.newBoardingPoint` → boarding card
5. `renderTemplate.showTrain` → train list
6. `templateName` route via `_v`/`f` dispatchers
7. plain `answer` bubble (+`helpline`/`alsoTry` side effects)

**Contract invariants (for reimplementation):**

- Unknown `templateName` degrades gracefully to text — never breaks.
- Every flow turn MUST return the next `next_context`.
- `isFallback`/`prevCode` are client-held state round-tripped in the request.
- Component mount delayed ~1500 ms after text bubble; helpline drawer +3 s.
- No confidence scores ever exposed client-side (sole `confidence` hit =
  Web Speech interim transcripts).

---

## 11. Auth / OTP Login Flow

```mermaid
sequenceDiagram
    participant U as User
    participant S as SPA
    participant A as api.disha.corover.ai

    Note over S: page load → sessionId=uuid4;<br/>restore dishav2-data or ask parent via postMessage
    S->>A: GET bot/getUserData (if cxtoken present)
    A-->>S: ok | apiReference="otp-send"

    U->>S: enters mobile number
    S->>A: POST bot/login/en  hdr x-user-mobile<br/>{mobileNumber,userToken,useOTP:true,dSession,...}
    A-->>S: renderTemplate.data.Details = otpuuid
    Note over S: 500 → reload(); 429 → "Rate limit exceeded,<br/>retry in 10 minutes"; "already logged" → show Logout-all

    U->>S: enters OTP
    S->>A: POST bot/verifyLogin/en {otp, otpuuid, ...}
    A-->>S: {cxtoken: JWT, existingData}
    Note over S: decode JWT.signedDate;<br/>now-signedDate > 10 s ⇒ reject as wrong OTP
    S->>S: SET_USER(existingData) + save cxtoken → dishav2-data
```

Separate IRCTC-ID verification path: `bot/getUserId/{id}` (status `"1#1#1#1"` flags)
→ `getSMSeMailOTP` → `verifySMSeMailOTP` (dual SMS+email OTP, `errorIndex==="1"`
means failure; no 429 branch on this trio).

Logout: single `POST bot/logout` then blank `dishav2-data`, delete cookie
`udata` (`document.cookie="udata=; expires=epoch; path=/; samesite=lax; secure"`).

---

## 12. Voice Pipeline

Two mutually exclusive capture paths chosen by `forceSafariMode` OR mobile-UA regex:

```mermaid
flowchart TD
    M[mic button] --> UA{Desktop Chromium?}
    UA -- yes --> WS[Web Speech API<br/>webkitSpeechRecognition<br/>continuous, interimResults<br/>locale: hi-IN / en-IN / en-US / en-GB / gu-IN]
    WS --> T[final transcript TEXT ONLY<br/>nothing uploaded]
    UA -- no/Safari --> MR[getUserMedia audio<br/>MediaRecorder → Blob type audio/webm<br/>auto-stop after 7000 ms]
    MR --> FR[FileReader.readAsDataURL<br/>→ data:audio/webm;base64,...]
    FR --> TR[POST /bot/transcribe<br/>{query:dataURL, isAudio:true, language, dSession,...}]
    TR --> X[data.transcription]
    T --> SQ
    X --> N{ENTER_PNR state?}
    N -- yes --> NM[word→digit normalize:<br/>zero..nine, Hindi शून्य..नौ, o/oh→0]
    N -- no --> SQ
    NM --> SQ[POST /bot/sendQuery/lang<br/>inputType:'VOICE', audioUrl=dataURL, isAudio:true]
    SQ --> R{response.audio?}
    R -- yes --> SU[GET /bot/getSecureUrl?fileName=<br/>keys: secureUrl/signedUrl/url · 10-min TTL cache]
    SU --> PB[global <audio id='myaudio'><br/>.src=.load()=.play()]
```

Facts:

- STT languages param: `behaviour.queryLang ∈ {hi,en,us,gb,gu}` mapped to
  recognition locales; `language` body field ∈ {en,hi,gu}.
- TTS asset hosts: `cdn.corover.ai/askdisha-bucket/tts/speech_<ms>_<uuid>.mp3|.wav`
  (current), `cognitive.service.corover.ai/speech_*.mp3` (legacy hardcoded prompts),
  `storage.corover.ai/chatbot-audio-bucket-aws/*_en.mp3`,
  `uiresource.blob.core.windows.net/*.wav`, plus Bhashini govt-TTS wavs under
  `disha-bhashini/`.
- URLs already matching `\.(mp3|wav|m4a|ogg|webm)(\?|$)` or `storage.googleapis.com`
  bypass the signed-URL exchange.
- No MediaSource/SourceBuffer anywhere → whole-file playback, no streaming.
- Chimes: `azure-coroverbackendstorage-blob/corover-audio-bucket/start.mp3` /
  `caught.mp3`.

---

## 13. Payment Pipeline

```mermaid
sequenceDiagram
    participant S as SPA
    participant A as api.disha.corover.ai
    participant P as secure.paytmpayments.com
    participant B as Bank ACS / NPCI

    Note over A: txnToken minted SERVER-SIDE inside sendQuery flow
    S->>S: renderTemplate.data = {txnToken, clientTransactionId(orderId), paymentAmount, userId, qrCode}
    Note over S: 10-min countdown timer starts
    S->>P: <script src=.../checkoutjs/merchants/IRCTCC84510399265141.js>
    S->>P: window.checkout && init({root:'#PaytmDiv', flow:'DEFAULT', hideHeader:true,<br/>merchant:{name:'DISHA E-Ticketing', redirect:false},<br/>data:{orderId, token, tokenType:'TXN_TOKEN', amount},<br/>handler:{transactionStatus}})
    alt native card path
        S->>A: POST bot/paytm/fetchCardDetails {bin} (lockout: 5 tries / 5 min)
        S->>A: POST bot/paytm/transactions<br/>{cardInfo:'|PAN|CVV|MM/20YY', authMode:'otp', coftConsent?{createdAt,userConsent:'1'}}
        A->>P: relays (browser never touches Paytm API directly)
        P-->>A: directForms[].content.otp needed?
        S->>A: POST bot/directBankOTPRequest {otp} (max 2 tries)
    else redirect form path
        A-->>S: redirectForm {actionUrl, method, content}
        S->>B: popup auto-submitted hidden form STRAIGHT to bank ACS
    else UPI intent path
        S->>B: scheme-swap window.location = tez://|phonepe://|paytmmp://|mobikwik:// + qrCode params<br/>(fallback to upi:// after 2500 ms if !document.hidden)
        loop every 7000 ms (cap 100 polls ≈ 11.7 min; abort on TXN_FAILURE)
            S->>A: GET bot/upiStatus/{orderId}/{txnToken}?voice=
        end
    end
    Note over S: TXN_SUCCESS → handlePaymentDone()
    S->>A: sendQuery('payment success', next_context='ade4a7db...,14', cxpayload:{resend:'N',otp})
    A-->>S: PNR + ticket (ticket issuance confirmed HERE, no direct IRCTC call)
```

Key facts:

- Merchant ID literal: `IRCTCC84510399265141` (settlement lands on IRCTC).
- Card entry happens inside Paytm's hosted iframe (CheckoutJS path — Corover sees
  nothing) **or** through the Corover proxy (native path — PAN/CVV transits CoRover).
- Saved cards require `paytmCustId` from `bot/getUserData` → `bot/paytm/fetchCard`.
- COFT consent timestamp format: `"Fri Aug 22 14:03:57 IST 2026"` (client-generated).
- VPA pre-validation regexes before network call: mobile `/^[6-9]\d{9}$/`, VPA
  `/^[a-zA-Z0-9.\-_]{2,}@[a-zA-Z]{2,}$/`.

---

## 14. Booking Flow

High-level funnel (each step emits `PUT bot/clicks/{BOOKING_UUID},{n}`):

```mermaid
stateDiagram-v2
    [*] --> SearchFlow: FROM/TO/DATE/QUOTA prefilled<br/>(deep link ?FROM=&TO=&DATE=&QUOTA=)
    SearchFlow --> TrainList: sendQuery(render 'trains')
    TrainList --> PassengerForm: select train/class
    PassengerForm --> Captcha: savePassenger draft
    Captcha --> LoginGate: generateCaptcha/verifyCaptcha
    LoginGate --> Payment: OTP login (or apiReference otp-send mid-flow)
    Payment --> TicketIssued: 'payment success' sendQuery round-trip
    TicketIssued --> [*]: addservices/eticket/{pnr} (+ .qr)
    Payment --> FailedPayment: retry within 15 min window
```

Supporting calls during flow: `searchStation` typeahead, `stationsByLocation`,
`pin/{gstpin}`, `alternativeSplitJourney`, `editTrains/{id}`,
`getRefundStatusByTxn`, `clearTempFlow`.

---

## 15. Station / Location Calls

| Concern | Behavior (verbatim findings) |
|---|---|
| Geolocation trigger | ONE site: "Find Stations Near Me" button → `navigator.geolocation.getCurrentPosition` with **no options**; raw lat/lng interpolated into path; denial handler is literally `()=>m(!1)` (silent spinner stop); cached nearby list reused, GPS never re-prompted |
| `searchStation/{q}` | Fires on **every keystroke** (`""!==e`), no debounce/throttle; dedupe purely via module-singleton CancelToken `dn` cancelling predecessor; results `.slice(0,50)`; catch → sentinel `-1`; **zero caching** (React state only) |
| Offline layer | Boot loads `stationupdated.json` (8,491 stations, ~2.9 MB) + `popular.json` (100) into Redux; Fuse fuzzy search locally over keys `[code,name,utterances,district,state]`; NEARBY mode filters the 200-item cached slice locally |
| `pin/{code}` | GST invoice block only; fires when gstpincode hits 6 digits; response `{state, cityList[]}`; >1 city opens dropdown; voice normalization "triple zero"→"00" |

---

## 16. CDN Config & Feature Flags

| File | Size | Structure |
|---|---|---|
| `getSettings.json` | 43 B | `{"id":1,"isDisabled":false,"booking":true}` |
| `popular.json` | 34 KB | 100 stations `{name,code,utterances[],name_hi,latitude,longitude,district,state,trainCount,address}` |
| `countries.json` | 13 KB | 239 × `{country,countryCode}` |
| `stationupdated.json` | 2,933,008 B | 8,491 stations (+`name_gu`) |
| `en.json` / `hi.json` / `gu.json` | 44/105/110 KB | 772/750/750 FAQ strings |

Cache profile: `cache-control: public, max-age=0, must-revalidate`,
`cf-cache-status: DYNAMIC` (never edge-cached), etag revalidation (304 verified).
No versioned querystrings anywhere.

**Feature-flag gate** (fail-open defaults `{isDisabled:false, booking:true}` on
fetch error):

```mermaid
flowchart LR
    S[getSettings.json] --> D{isDisabled?}
    D -- true & ctx != BOOKING_FLOW --> MAINT[disha-sleeping.gif<br/>'Server Under Maintainance']
    D -- false --> B{booking flag?}
    B -- true & ctx == BOOKING_FLOW --> BLOCK[block booking flow<br/>deflect via IB component]
    B -- false --> OK[all flows live]
```

Current live values mean general Q&A/PNR/cancel/TDR are ON but **in-chat ticket
booking is toggled OFF** at CDN level.

Dead code: full Firebase project config (`askdisha-pwa`, apiKey `AIzaSyBQo_-dUiOjk3pDywglBZ4IJiAvmdI3uO4`,
sender `630662269520`, VAPID key, measurement `G-53XKGQEHNH`) appears twice in the
bundle and is **never consumed** — `initializeApp`=0 hits, no SW registered, no FCM
receive path client-side. Also embedded: empty `REACT_APP_DISHA_PROD_KEY`,
localhost key `bd151501-…-local`, and the mFilterIt script URL (now dead).

---

## 17. Embedding Protocol

Loader chain (both irctc.co.in and indianrail.gov.in):

```mermaid
sequenceDiagram
    participant P as Parent page (irctc.co.in / indianrail.gov.in)
    participant L1 as chatbotlib(.min|-ir).js
    participant L2 as *_disha.widget.js
    participant F as iframe #Disha-Bot (askdisha.irctc.co.in)

    P->>L1: <script src=sdk.irctc.corover.ai/askdisha-bucket/chatbotlib.min.js?ts><br/>(indianrail: sdk.ir.corover.ai chatbotlib-ir.min.js)
    L1->>P: gtag(UA-122267849-1); gate on includedPaths<br/>['/nget/train-search','/eticket/train-search'] (irctc only)
    L1->>L2: inject sdk/*/sdk/{irctc|ir}_disha.widget.js?ts (cache-busted EVERY load)
    L2->>P: aside + launcher GIF (FLauncher.gif) + div#corover-askDisha
    L2->>F: create iframe src=about:blank allow='geolocation;microphone;camera;otp-credentials;midi;accelerometer;gyroscope;payment'
    Note over F: hydrated lazily on first open via<br/>contentWindow.location.replace('https://askdisha.irctc.co.in')
    P->>F: postMessage('botOpen' | 'hideCross' | 'showCross', '*')
    F->>P: 'LOADED' | 'CLICKED' (keep-alive) | 'getToken' | 'getState' | 'getRecent'
    F->>P: {action:'SET_DISHA_DATA', data} → localStorage['disha-data']
    P->>F: {action:'GET_DISHA_DATA', data:localStorage['disha-data']}
    P->>P: persists cookies: cxtoken (1y), recents (1y), state (2 min)
    P->>F: {type:'REDIRECT_CONDITION_MET', data:[{type:'src'|'des'|'journeyDate'|'quota',value}]}<br/>⇒ opens askdisha.irctc.co.in?FROM=&TO=&DATE=&QUOTA=
    P->>F: {type:'LANGUAGE_UPDATE', data:lang} | 'openBot' | 'helpline'
```

Deep-link grammar: `?FROM=<src>&TO=<dst>&DATE=<yyyyMMdd>&QUOTA=<quota>`;
variants `#webIR` hash and `?helpline=true`. Receive-side origin allowlist:
`askdisha.irctc.co.in || www.irctc.co.in` (+ stray `127.0.0.1:5500` dev leftover).
All postMessage targets are wildcard `"*"`.

---

## 18. Telemetry & Monetization

### First-party (to CoRover)

| Call | Trigger | Payload |
|---|---|---|
| `PUT /bot/clicks/{flowUUID},{step}` | every funnel step | fire-and-forget, `.catch(()=>{})` |
| `POST /bot/uiError` | client errors | error details |
| `POST /bot/ticketFeedback` | rating widgets | `{rating,feedback,txn,pnr}` or `{answerId,feedback,comment,channel}` |
| `POST /bot/sendQuery` piggyback | flow steps | `next_context="{uuid},{step}"` carries position |

Identity headers (`app-id`/`auth-Key`/`askdishaId`/`x-user-token`) tag every event.

### Third-party loaded by index.html

| Script | ID | Endpoints | Sends |
|---|---|---|---|
| GTM | `GTM-TFG2G9PG` → GA4 `G-JSTMKS9Y3J` | `google-analytics.com/g/collect` (+ regional) | page URL/title, events, `_ga` cid (~2 y), sid, UTMs |
| MS Clarity | `s4wshk96li` | `scripts.clarity.ms/0.8.69/clarity.js`, upload `y.clarity.ms/collect`, heartbeat `c.clarity.ms/c.gif` | **full session replay**: DOM mutations, click/scroll/mouse coords, viewport; cookies `_clck` 365 d. Bypasses GTM governance |
| Cloudflare Insights | beacon token `33664005ea4746feaf6a47d1953339f5` | `cloudflareinsights.com` | RUM page views, CWV timings |
| AdSense | `ca-pub-8692878304946020` | pagead2/safeframe/doubleclick | ad profiling |
| Unibots player | — | `socket.unibots.in/website/playerConfig`, `api.unibots.in/block`, `socket.unibots.in/ga`, `api.ipify.org`, `pro.ip-api.com`, doubleclick VAST | hostname, **full page path**, viewability events, **public IP→city geo**; own freq-cap localStorage `gabywa_ubp`; can inject its OWN Clarity/GA codes |
| VDO.AI / OpenX / Criteo | CSP whitelisted | video/display ad frames | programmatic demand |
| mFilterIt v3.3.6 (dead) | `window.mfilterit` | `POST 559p2ll052.execute-api.ap-south-1.amazonaws.com/stage/web/exapi/post_fp` (NXDOMAIN since ~2024) | `btoa({mid, FP1})` where FP1 = canvas/WebGL/audio/font/screen/deviceMemory fingerprint + `bot_status` verdict + frame ancestry — **anti-ticket-bot gating**; returns `{deviceID, mfTxnId, bot_status}` |

Observation: a government chatbot page carrying **two independent video-ad stacks**
plus AdSense/OpenX/Criteo and third-party device fingerprinting is unusual for a
public-service surface.

---

## 19. Resilience Behavior

| Concern | Finding |
|---|---|
| Interceptors | **NONE** — 401 handled per-call-site via shared `co()` helper (deadlock-until-reload pattern) |
| Timeouts | none (`timeout:0` everywhere); hung connections stall until navigation |
| Retry | none automatic; user-initiated "Try again" modal only |
| Cancellation | CancelToken singleton per domain: `eg`=chat, `dn`=stations, `fb`=train status, `oS`=Paytm cluster, `zy`=PIN, `Ds`=phone, `ho`=booking-history — "last request wins" |
| Debounce | none on typeahead; only react-speech-recognition 250 ms leading debounce |
| Polling | fixed-interval, zero backoff: UPI 7 s ×100 (variant 15 s; broken ~4-poll parse heuristic in one variant); getStatus loops; OTP-resend cooldown 90 s @1 s ticks |
| Client rate limits | BIN lookup lockout: 5 attempts / 300 000 ms; login 429 → hard disable until reload |
| Offline | `navigator.onLine` hook + online/offline listeners → banner; per-call classifier `isNetworkError = !resp && ('Network Error'===msg || !onLine)`; graceful degradation ([] FAQs, null cookiesData) |

---

## 20. Client State

| Surface | Keys |
|---|---|
| `localStorage` | `dishav2-data` = JSON `{cxtoken, journeys[], usertoken, uuid, mobile}`; `journey_source/destination/date/quota/class`; `typedMessages`; `selectedLanguageVariant` (`en-ind|en-usa|en-gb`, timezone-autodetected: america→usa, london→gb); `boardingChangeData` |
| Cookies (parent domain, set by widget) | `cxtoken` (1 y), `recents` (1 y), `state` (2 min), `udata` (deleted on logout, `samesite=lax; secure`) |
| Redux | `app.{cxtoken, userToken, disabledSettings:{other,booking}, 401}`, `user.askdishaId`, behaviour `{input_mode, queryLang, lang}` |

---

## 21. Version History

```mermaid
timeline
    title AskDISHA data-call evolution (Wayback-reconstructed)
    2018 : assistant.corover.mobi/botAPI/getQuestions/irctc/ — raw JSON FAQ
    2020 : Angular SPA /indianrailways/ — same-origin /nlpAPI/getAnswer?languageCode=
    2022 : React CRA /eticket/ — prefix renamed /dishaAPI/ ; Azure→Linode assets
    Dec 2023 : custom domain askdisha.irctc.co.in — same-origin relative /dishaAPI/*
    H2 2025 : backend split → absolute https://api.disha.corover.ai/dishaAPI/* ;
              new: transcribe/upiStatus/aadharAuthenticate/getSecureUrl
    2026 : current main.0384a92c.js ; app-id unchanged since Dec 2023
```

Prefix lineage: **botAPI → nlpAPI → dishaAPI → dedicated api.disha subdomain**.
The credential pair survived all four migrations.

Official milestones: launch Oct 2018 (via NICSI-selected CoRover); Hindi+voice
Feb 2020 (PIB PRID 1603955: claims 10 billion cumulative interactions, >150 M
passengers benefited); live refund status Aug 2020; AskDISHA 2.0 Sep 2022
(transactional, OTP-passwordless, ₹20 crore beta transactions); BharatGPT GenAI
positioning Aug 2023–Jan 2024; FY22 audited volume: 53,16,235 queries (BSE annual
report); 2025 re-tender `2025/IRCTC/CO/SER/Chatbot ASKDISHA` for a successor TSP.

---

## 22. Vendor Ecosystem

Constant across ALL CoRover deployments (AskDISHA, LIC MITRA, NPCI Ask PAi,
DMRC, Income-tax etc.):

- script-tag loader → iframe'd SPA → REST against `api.<tenant>.corover.ai/<tenant>API/*`
  (or same-origin prefix like `/npcinew/`)
- static UUID credential pair in custom headers (`app-id`+`auth-Key`; LIC uses
  lowercase singular `appid`; NPCI adds `partner-key`; newer DMRC moves keys
  server-side into a reverse proxy)
- wildcard postMessage handshake (`LOADED/getToken/getState/getRecent/CLICKED`)
- media via presigned-URL endpoint + object-storage buckets
- streaming (where present, newer stacks) = **SSE-over-POST** (`fetch` +
  ReadableStream, `data:` lines, `[DONE]` sentinel) — never WebSockets
- mobile distribution = WebView loading `https://<tenant>.corover.mobi?channel=android`
  — no native SDK exists (npm/PyPI/Maven registries: zero artifacts)

AskDISHA differs only in owning a vanity app domain instead of `*.corover.ai`.

---

## 23. Security Observations

1. **Secrets-in-client**: `app-id`/`auth-Key` ship to every visitor. Publicly
   unresearched (zero writeups found as of Aug 2026).
2. **CORS posture is sound**: fixed ACAO, never reflects arbitrary origins despite
   `allow-credentials:true`; preflight always 204 regardless of Origin (gating is
   browser-layer + app-layer token check, which returns identical generic
   `{"status":401,"message":"Not Allowed!"}` for any origin — good, no enumeration).
3. **One unverified disclosure** (LinkedIn, Oct 2024): JWT-in-URL + signup OTP bypass
   claims against CoRover; no PoC, unacknowledged.
4. No CVEs / CERT-In advisories name CoRover/AskDISHA. Confirmed IRCTC-ecosystem
   breaches were all IDORs on third-party vendor portals (insurance 2018/2022/2024/2025,
   corporate booking 2025) — the same flaw class one would test the chatbot's
   `{pnr}`/`{txn}`-in-path endpoints for. **Do not test against production.**
5. `postMessage(..., "*")` wildcards both directions; dev-origin `127.0.0.1:5500`
   left in widget allowlist.
6. Aadhaar-in-bot (`aadharAuthenticate/*`) has received no public scrutiny; Kerala HC
   (Jul 2026) upheld Aadhaar-OTP Tatkal auth on IRCTC proper, with govt stating
   Aadhaar numbers are not stored.
7. Privacy-notable third parties: Unibots IP→city resolution outside GTM governance;
   Clarity full session replay; historical mFilterIt fingerprinting.

---

## 24. Programmatic Emulation Guide

Everything below reproduces **guest-mode, read-only** behavior. Python 3.10+
assumed. Install: `pip install requests cryptography uuid` (stdlib uuid fine).

### 24.1 Minimal viable client

```python
"""askdisha_client.py — guest-mode AskDISHA emulator (research use)."""
import base64, json, random, string, uuid
import requests
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import padding

BASE      = "https://api.disha.corover.ai/dishaAPI"
APP_ID    = "29fd4f94-f793-4227-9588-056b5ffb1318"
AUTH_KEY  = "2b5fb5d4-0753-4302-b661-f8580e9effb0"
ORIGIN    = "https://askdisha.irctc.co.in"
UA        = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
             "(KHTML, like Gecko) Chrome/124 Safari/537.36")

class AskDisha:
    def __init__(self):
        self.http = requests.Session()
        self.http.headers.update({"Origin": ORIGIN, "Referer": f"{ORIGIN}/",
                                  "User-Agent": UA})
        self.session_id = str(uuid.uuid4())
        self.user_token = str(uuid.uuid4())          # client-minted
        self.cxtoken    = None                        # None => guest
        self.next_ctx   = ""
        self._pub = None

    # ---- header factory (mirror of bundle's Ao()) -----------------------
    def headers(self, extra=None):
        h = {"Content-Type": "application/json",
             "app-id": APP_ID, "auth-Key": AUTH_KEY,
             "cxtoken": self.cxtoken or "",
             "x-user-token": self.user_token}
        h.update(extra or {})
        return h

    # ---- dSession generator (§7) ----------------------------------------
    def _pubkey(self):
        if self._pub is None:
            # Extract once from the bundle: grep REACT_APP_P865243658 main.*.js,
            # base64-decode -> PEM. Cache it in a file.
            pem = open("disha_pubkey.pem", "rb").read()
            self._pub = serialization.load_pem_public_key(pem)
        return self._pub

    def dsession(self):
        a = str(uuid.uuid4())
        mark = "".join(random.choice(string.ascii_lowercase) for _ in range(5))
        a = a[:5] + mark + a[5:]
        plain = base64.b64encode(a.encode()).decode() + "," + a
        ct = self._pubkey().encrypt(plain.encode(), padding.PKCS1v15())
        return base64.b64encode(ct).decode()

    def _body(self, **kw):
        b = {"source": UA, "userToken": self.user_token,
             "dSession": self.dsession(), "sessionId": self.session_id,
             "deviceId": None, "status": 1, "channel": ORIGIN,
             "next_context": self.next_ctx, "cxpayload": None,
             "audioUrl": None, "isAudio": False}
        b.update(kw)
        return b

    # ---- core chat ------------------------------------------------------
    def query(self, text: str, lang: str = "en") -> dict:
        r = self.http.post(f"{BASE}/bot/sendQuery/{lang}",
                           headers=self.headers(),
                           json=self._body(query=text, inputType="TEXT"),
                           timeout=30)
        if r.status_code == 401:
            raise PermissionError("401 Not Allowed — session/token rejected")
        r.raise_for_status()
        data = r.json()
        # track flow position exactly like the SPA
        self.next_ctx = data.get("context") or data.get("next_context") or self.next_ctx
        return data

    # ---- read-only utilities --------------------------------------------
    def search_station(self, q: str) -> list:
        r = self.http.get(f"{BASE}/bot/searchStation/{q}", timeout=15)
        return [] if r.status_code != 200 else r.json()

    def schedule(self, train_no: str, yyyymmdd: str, src_code: str) -> dict:
        r = self.http.get(f"{BASE}/bot/trnscheduleEnq/{train_no}",
                          params={"journeyDate": yyyymmdd,
                                  "startingStationCode": src_code},
                          headers={"Content-Type": "application/json"},
                          timeout=20)
        return r.json()

    def faqs(self, lang="en"):
        return requests.get(
            f"https://cdn.corover.ai/askdisha-bucket/{lang}.json", timeout=30).json()
```

### 24.2 curl equivalents

```bash
# guest chat turn (replace $DSESSION with output of the python generator;
# server may tolerate empty dSession for guest enquiries — TEST FIRST, do not hammer)
curl -sS --http1.1 'https://api.disha.corover.ai/dishaAPI/bot/sendQuery/en' \
  -H 'Content-Type: application/json' \
  -H 'Origin: https://askdisha.irctc.co.in' \
  -H 'Referer: https://askdisha.irctc.co.in/' \
  -H 'app-id: 29fd4f94-f793-4227-9588-056b5ffb1318' \
  -H 'auth-Key: 2b5fb5d4-0753-4302-b661-f8580e9effb0' \
  -d '{"query":"pnr status","source":"Mozilla/5.0","inputType":"TEXT",
       "next_context":"","cxpayload":null,"userToken":"<uuid4>",
       "suggestion":false,"isFallback":null,"isRefund":null,
       "channel":"https://askdisha.irctc.co.in/","prevCode":null,
       "audioUrl":null,"isAudio":false,"dSession":"<rsa-blob>",
       "deviceId":null,"sessionId":"<uuid4>","status":1}'

# station typeahead (no auth headers needed in practice — cancelToken path)
curl -sS 'https://api.disha.corover.ai/dishaAPI/bot/searchStation/new'

# boot-time config (plain CDN)
curl -sS https://cdn.corover.ai/askdisha-bucket/getSettings.json
```

### 24.3 Practical notes for emulators

1. **Use `--http1.1`** (or configure your HTTP client accordingly): the API's
   multiline `permissions-policy` header breaks curl's HTTP/2 parser.
2. **Expect 401** `{"status":401,"message":"Not Allowed!"}` for anything beyond
   guest enquiry breadth — tokens are mandatory; static analysis cannot recover
   the intent taxonomy.
3. **JWT check**: if you obtain a cxtoken through legitimate manual login, decode
   payload and honor `signedDate` semantics (freshness guard) rather than
   standard exp/iat.
4. **Polling etiquette**: mirror the client's caps (≤100 polls, 7 s interval,
   abort on failure) — do not tighten intervals.
5. **Bot detection existed** (mFilterIt). Its gateway is dead, but assume
   behavioral/rate heuristics server-side: keep volumes human-like, keep a real
   UA, don't parallelize aggressively.
6. **The 2.9 MB `stationupdated.json`** gives you the full offline station table
   (codes, names hi/gu, lat/lng, district/state) without touching the API at all —
   prefer it for dataset needs.
7. **TTS corpus**: filenames from `response.audio` resolve via `getSecureUrl`;
   treat signed URLs as short-lived and cache ≤10 min like the client does.
8. If you need the RSA public key: `grep -oE 'REACT_APP_P865243658:"[^"]+"'
   main.*.js | cut -d'"' -f2 | base64 -d > disha_pubkey.pem`

---

## 25. Emulation Scope Matrix

| Capability | Feasibility | Verdict |
|---|---|---|
| Guest Q&A chat turns | ✅ straightforward | OK for research/interop, low volume |
| Station search / schedules / CDN datasets | ✅ trivial | Prefer CDN JSONs for bulk data |
| Locale FAQ harvest | ✅ trivial (static files) | OK |
| TTS playback/download | ⚠️ signed URLs | personal use only; respect caching windows |
| OTP login automation | ❌ | **Never.** Triggers real SMS/email; 429 lockouts; abusive |
| Aadhaar eKYC endpoints | 🚫 | **Absolutely out of scope** — legal landmine |
| Payment/UPI flows | 🚫 | Never emulate; PAN/CVV transit CoRover proxy — any replay attempt is fraud territory |
| Captcha solve attempts | 🚫 | Circumvention prohibited |
| Booking/cancel writes | 🚫 | Government transaction system; ToS violation + potential CFAA-analog exposure |
| Load testing / fuzzing | 🚫 | No bug bounty covers this; CERT-In rules apply |

---

## 26. Open Questions

| Item | Status |
|---|---|
| `wss://*.corover.ai` usage | Never exercised in deployed AskDISHA; headroom for CoRover VideoBot/VoiceBot SKUs |
| SSE-over-POST | Present in newer CoRover stacks (IRA/LIC: `agenticai.bharatgpt.ai/api/chat/stream`, `lic_v2/chat/query/stream`) — AskDISHA bundle still plain REST; future migration likely |
| Which NLU engine | Server-side only; `cxpayload` naming suggests Dialogflow-CX lineage but zero engine strings in bundle; CoRover markets proprietary "Classic NLP L1" + GenAI layers (BharatGPT 3B/0.5B gated models on HuggingFace) |
| Relative `/dishaAPI/bot/addUserName` | Residual same-origin proxy route; whether it still resolves server-side is unknown |
| Successor contract | 2025 IRCTC tender may replace CoRover; watch for new TSP bundle/hosts |
| Exact dSession server tolerance | Unknown how strictly the marked-uuid format is validated |

---

*Document generated from 30-agent parallel deep research (Aug 2026) against live
bundle `main.0384a92c.js`, live probes, Wayback snapshots 2018-2026, vendor
materials, and official IRCTC/PIB sources. Artifacts originally under
`/tmp/opencode/` (ephemeral).*
