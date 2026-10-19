# ADR-003 — Control plane, entitlement, device binding & offline sync

**Status:** proposed — needs 4 decisions (§8)
**Date:** 2026-10-20
**Supersedes:** `PLAN.md` §8 ("local → Neon direct") and §9 (credential model)
**Relates to:** ADR-002 (licensing)

---

## 1. Your three requirements, restated

| # | Requirement | What it really constrains |
|---|---|---|
| R1 | **Never block the shop's work** because they didn't pay this month | No hard lock at expiry. Degradation must be *soft* and must not interrupt a sale |
| R2 | **Never be free forever**, and closing the wifi must not bypass it | Entitlement must be fetched over the network periodically, with a *bounded* offline window |
| R3 | **Never wait on Neon / the network** | Local SQLite is authoritative; every network call is background, timeout-bounded, and skippable |

There is a genuine contradiction in R1 + R2 that you must resolve consciously:

> **You cannot guarantee "never interrupted" and "never free" at the same time.** Something must eventually break the tie when a shop refuses to pay for a year. Your only choice is **where the floor sits**.

### The 5 states (this is the whole licensing story)

```
ACTIVE ──(lease renews online)──► ACTIVE
   │
   └─(no network / unpaid)──► GRACE ──► LAPSED ──► SUSPENDED
        still fetched          7–14 d     30–45 d     (owner-only)
```

| State | Cashier can sell? | Local reports | Cloud dashboard | Owner login | Notes |
|---|---|---|---|---|---|
| **ACTIVE** | ✓ full speed | ✓ | ✓ | ✓ | silent, no UI noise |
| **GRACE** | ✓ full speed | ✓ | ✓ | ✓ + amber banner "renew" | nothing changes operationally |
| **LAPSED** | ✓ **still works** | ✓ | ✗ off | ✓ **blocking screen** | the *owner* is blocked, not the till |
| **SUSPENDED** (LAPSED + 60 d) | ✓ but receipts watermarked + 1 interstitial per 20 sales | ✓ read-only | ✗ | ✗ | friction, never a locked till |

**Read that again:** in `LAPSED`, cashiers keep selling at full speed and no data is ever held hostage — but the cloud dashboard (the thing you charge for) is off and the **owner** sees the payment screen. In `SUSPENDED`, the shop still rings up sales; it just becomes annoying enough that a real business pays, while a dead shop costs you nothing either way.

This is the only enforcement shape compatible with R1. It is also *good* product design: the till never being down is exactly what a shopkeeper will tell other shopkeepers.

### Also: the offline window must be *generous* but *finite*
- Lease issued for **14 days** online → offline-safe for 2 weeks.
- `GRACE` = **+14 days** after the lease lapses without contact.
- So: **28 days fully offline before anything changes**, and even then the till keeps working.
- Renewal is silent and happens during any 30-second window of connectivity. A shop with internet 1 hour/day renews forever without noticing.

---

## 2. Architecture change: put a control plane in the middle

Your instinct is right, and it's an upgrade to the previous plan. **The exe must not talk to Neon directly.**

```
┌───────────── SHOP PC (offline-first) ─────────────┐
│                                                   │
│  store.exe                                        │
│  ├─ Actix/Axum on 127.0.0.1:8642                  │
│  ├─ SQLite (truth, WAL)                           │
│  ├─ Local dashboard (same SPA, zero network)      │
│  └─ Agent ──► outbox table ──► HTTPS, 5s timeout ─┼──► ┌───────────────────────────────┐
│          (never in the request path)              │    │  Next.js on Vercel            │
└───────────────────────────────────────────────────┘    │  "control plane"              │
                                                         │                               │
┌───────────── OWNER (browser, anywhere) ───────────┐    │  /api/device/pair             │
│  Web dashboard (read-only analytics)  ────────────┼───►│  /api/licence/lease           │
└───────────────────────────────────────────────────┘    │  /api/sync/ingest             │
                                                         │  /api/dashboard/*             │
                                                         │  /api/admin/*  (you only)     │
                                                         └──────────┬────────────────────┘
                                                                    │ pooled TLS
                                                            ┌───────▼────────┐
                                                            │  Neon Postgres │
                                                            │  aggregates +  │
                                                            │  entitlement   │
                                                            └────────────────┘
```

**Why this is strictly better than exe → Neon:**

| Before | After |
|---|---|
| Neon connection string on every customer PC (encrypted, but still there) | **No DB credentials ever leave your server** |
| Revoking a shop = rotating a DB password = touching every install | Revocation = one `UPDATE shops SET status='suspended'` |
| Client can write anywhere the role allows | Server validates + whitelists every field |
| No rate limiting / abuse control | Per-device quotas, payload caps, per-IP limits |
| Licensing bolted on separately | Licensing *is* the same channel as sync — one mechanism, ~0 extra cost |
| No visibility into who is offline | `sync_health` per device → you know before they call |

**The cost:** one extra hop (~80–200 ms) and Vercel function cold starts (~200–500 ms). Both irrelevant: sync is a background task, and the dashboard is SSR with revalidation. **The till never sees this hop** — R3 is structurally satisfied because no local request ever awaits the network.

---

## 3. Pairing + device binding (one device per account)

### 3.1 POS device — hard-bound, 1 active
1. Owner opens the dashboard → **Devices → Add device** → gets a **6-digit pairing code** (valid 15 min, single use).
2. Exe shows a pairing screen: `Shop: Nour Store` → enter code.
3. Exe sends: `pairing_code`, `install_id` (uuidv7), `machine_fp`, `app_version`, `exe_public_key`.
4. Server creates a `device` row, returns a **device token** (32-byte secret) **and** the first **lease token**.
5. Token stored encrypted at rest (DPAPI on Windows) — never in plaintext config.
6. Every subsequent call sends `Authorization: Bearer <device_token>` + `X-Device-Id`.

**Enforcement mechanics:**
- A second PC pairing a 1-device account is **rejected server-side** → `409 device_limit_reached`.
- The lease token is signed and bound to `device_id`; an offline copy on another machine still expires with the lease (bounded leak, see §6).
- **Device transfer** (sold PC / broken PC): owner clicks **Replace device** in the dashboard → 24h cooldown, old device `revoked`, new pairing allowed. Rate-limit to **once per 30 days** so a shop can't rotate machines to share one license.
- Heartbeat = lease renewal. `last_seen` per device; dashboard shows "offline for 9 days" so you notice a dead PC (or an attempt to dodge).

### 3.2 Dashboard — needs your call
"One device" is ambiguous for a browser, and getting it wrong hurts real usage:

| Option | Behaviour | Trade-off |
|---|---|---|
| **A. 1 concurrent session** ⭐ | New login on device B kills session on A | Owner can check from phone *and* laptop, but not simultaneously. Standard SaaS behaviour. |
| B. Hard-bind to 1 browser | First device to log in owns it forever | Breaks "check the shop from my phone in the evening" — **I'd avoid this** |
| C. 2 allowlisted devices | Explicit list, revocable | Best UX, slightly more UI |

**Recommend A (+ optional C if the accountant needs access).** Hard-binding a *dashboard* to one machine fights the whole point of a remote dashboard, and your owner will absolutely want to check numbers from a phone. The POS is where "one device" really matters, because that's where software gets copied.

---

## 4. The lease token (this is the mechanism R2 needs)

```json
{
  "v": 1,
  "shop_id": "...", "device_id": "...",
  "plan": "pro",
  "issued_at": 1761000000,
  "lease_until": 1762209600,      // +14 days
  "grace_until": 1763419200,      // +14 more
  "features": ["pos", "reports", "cloud_dashboard"],
  "max_tills": 1,
  "state": "active",
  "seq": 42
}
```
- Returned as `{ token, sig }`, **Ed25519-signed by your server**; public key compiled into the exe.
- Verified offline on every launch → **no network needed to keep working**, no phone-home per action.
- `seq` monotonic per device → prevents replaying an older, longer lease (a common crack: restore an old token file).
- Cached in the same hardened store as ADR-002 (SQLite + `%APPDATA%` + registry), most-restrictive-wins.
- Server can shorten a lease instantly by issuing `lease_until = now + 2d` on the next renewal.

**Renewal schedule** (background, never blocking):
```
on launch, then every 6h while online:
    if remaining > 10 days: skip (don't waste requests)
    else: POST /api/licence/lease { device_id, app_version, nonce }
failure → backoff 5m → 15m → 1h → 6h, cap; keep working
```

---

## 5. Offline-first sync, made boring (R3)

### 5.1 The outbox pattern
Every operation that touches the cloud writes to a local `outbox(id, kind, payload_json, idempotency_key, attempts, next_retry_at)` **inside the same SQLite transaction as the business write.** One background agent drains it. If the process dies mid-sync, the outbox is still there.

### 5.2 Hard rules that guarantee the till never waits
| Rule | Value |
|---|---|
| Network calls in a request handler | **zero** (enforced by a linter rule on the handler module) |
| Sync HTTP timeout | connect 2 s, total 5 s |
| Concurrent sync requests | 1 |
| Circuit breaker | 3 consecutive failures → open for 5 min → half-open probe |
| Payload size | ≤ 256 KB per batch, ≤ 31 days of aggregates |
| Retry | exponential 5 m → 15 m → 1 h → 6 h, jitter, **cap 6 h** |
| UI impact | one chip: `Synced 2 min ago` / `Offline — 3 days queued` |
| Backfill after long outage | 30 days per batch, newest first (dashboard shows something immediately) |

### 5.3 Idempotency & ordering
- `Idempotency-Key: {shop}:{device}:{day}:{agg_version}` — server upserts with `WHERE excluded.agg_version > existing.agg_version` → **blind retries are always safe**, at-least-once delivery becomes exactly-once effect.
- Day aggregates are the unit → no row-level conflicts, no merge logic, no CRDTs.
- Clock skew: the day is computed locally (`Africa/Tunis`) and is authoritative; `pushed_at` is server-side.

### 5.4 Ingest validation (the control plane's actual job)
`POST /api/sync/ingest` → zod schema → reject unknown fields → clamp integers to sane ranges (no negative revenue, no `i64` overflow, `sales_count ≥ 0`, `date ≤ today+1`, `ship ≤ 400`) → per-shop rate limit → insert. A compromised client can corrupt **only its own shop's** numbers.

---

## 6. Threat model — honest numbers

| Attack | Result | Detection |
|---|---|---|
| Cut the wifi forever | Works **28 days**, then LAPSED: dashboard off, owner blocked, till still selling | `sync_health` shows device silent → you call them |
| Reinstall / wipe `data/` | Re-pairing requires a **dashboard login** + 1-device limit | pairing events logged |
| Copy exe + data folder to a 2nd PC | Lease is device-bound and expires; re-pair rejected | — |
| Roll back the clock | Lease/grace/tamper flags are absolute-UTC + monotonic `seq` + `last_seen` watermark | sticky `TAMPERED` → LAPSED immediately, still no till lock |
| Replay an old lease file | `seq` monotonic, tokens expire | — |
| Patch the binary | Possible (accepted) — a determined person always can | out of scope |
| Share the dashboard login | 1 concurrent session + optional 2-device allowlist | login audit |
| Stop paying, keep using local only | **possible after 28 days of the till still working** — accepted, see below | dashboard offline = visible churn signal |

**Accepted risk, stated plainly:** with R1 ("never block the till"), a shop willing to live without the cloud dashboard can use the POS indefinitely at SUSPENDED level (watermarked receipts + interstitials). If that's unacceptable, the only alternative is to break R1 — there is no third option. My recommendation: **accept it**, and treat the watermarked-receipt POS as a *free tier* that markets you to the next shop. Shops that need multi-till, cloud backups, or a clean receipt will pay.

---

## 7. What goes in the Next.js app (control plane)

| Route | Purpose | Auth |
|---|---|---|
| `POST /api/device/pair` | pairing code → device token + first lease | pairing code |
| `POST /api/licence/lease` | renew/extend lease, returns signed token | device token |
| `POST /api/sync/ingest` | daily aggregates batch | device token |
| `GET /api/sync/health` | pending/acked days, backfill hints | device token |
| `GET /api/dashboard/*` | KPIs, trends, P&L, stock — **read-only** | dashboard session |
| `POST /api/auth/*` | owner login, session, logout | — |
| `POST /api/devices/replace` | rotate POS device | dashboard session |
| `POST /api/admin/*` | your ops: set plan, extend grace, suspend, refund | you only (separate secret) |

**No CRUD from the dashboard** — exactly as you said: analytics reads only. Business writes exist only inside the shop, on the till, on its own SQLite. That keeps the cloud DB a pure reporting mirror, which is what makes the whole design cheap and conflict-free.

**Roles in Neon:** `control_plane` (read/write on aggregates + entitlement) used only by the Next.js server; `dashboard_reader` (SELECT only) used only by dashboard queries; nothing else can connect. Pooled `-pooler` endpoint, `sslmode=require`.

**Data minimisation stays:** aggregates only — no customer names, no phones, no invoice lines, no product photos. Your privacy claim ("your shop data never leaves your till, only the totals") remains true and is a selling point.

---

## 8. Decisions I need from you

1. **Enforcement floor.** Confirm the ladder: LAPSED = *owner blocked, till alive*; SUSPENDED = *till alive but watermarked + interstitials*. Or do you want SUSPENDED to eventually stop sales (breaks R1)?
2. **Lease length.** 14-day lease + 14-day grace (= 28 days offline)? Longer if your market has unreliable internet?
3. **Dashboard device rule.** 1 concurrent session (recommended), hard-bound single browser, or 2 allowlisted devices?
4. **Billing cadence.** Monthly / annual / one-time + paid cloud plan? Affects how often LAPSED can realistically trigger.
5. *(implicit)* Confirm the **control-plane architecture** replaces direct exe → Neon. I'll rewrite `PLAN.md` §8/§9 accordingly once you confirm — it adds ~2 days to M5/M6 for the API layer, and removes client-side DB credentials entirely.

---

## 9. Effort delta vs the previous plan

| Item | Days |
|---|---|
| Control-plane API layer (pair, lease, ingest, health, validation) | +2.0 |
| Device binding + pairing UI + transfer flow | +1.5 |
| Lease token (sign/verify/cache/renew) — replaces part of ADR-002's licence work | +1.0 |
| Degradation ladder + owner block screen + banner states | +1.0 |
| Outbox + circuit breaker + backoff (was dirty-day loop) | +0.5 |
| **Net add** | **~+6 days** (≈53 total for v1) |

Everything else in `PLAN.md` stands unchanged.
