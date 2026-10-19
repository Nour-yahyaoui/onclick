# ADR-002 — Trial, licensing & offline enforcement

**Status:** proposed — needs your decision
**Date:** 2026-10-20
**Relates to:** `PLAN.md` §12 (security & operations), §8 (sync)

---

## 1. First, the uncomfortable truth

> "The app runs fully offline, and it must stop working after 15 days."

Those two requirements fight each other. A machine with no network and full control over its own disk can always lie to you. Any purely local check is **defeatable by a determined person**, usually in under 30 minutes.

So the real question isn't *"how do I make it uncrackable"* (impossible) but:

> **"How do I make the honest-customer path frictionless, the casual-abuse path annoying enough to not be worth it, and the determined-pirate path not my problem?"**

That's an achievable target, and it's cheap. Perfect enforcement is not achievable, and chasing it will eat weeks that should go into the POS.

**Also important:** this is a *sales* mechanism, not a security boundary. Do not let it damage reliability, data safety, or the antivirus relationship.

---

## 2. What "15 days" should actually count

Three candidate definitions, with very different abuse resistance:

| Definition | Fairness | Abuse | Recommendation |
|---|---|---|---|
| Calendar days since first launch | Poor — a shop can install, wait, still use 15 days later for free | Easy (delay start) | ✗ |
| **Distinct days the app is actually used** | Good | Moderate | ✓ **use this** |
| Wall-clock elapsed with clock-tamper detection | Fair | Moderate | ✓ use as a *second* signal |
| Sale-count limit (e.g. 1,000 sales) | Very fair to small shops | Decent | ✓ **use as a parallel cap** |

**Recommended rule:** trial ends at **whichever comes first** — 15 *active* days **or** 1,000 sales. Both counters live in the same hardened store. Why two: a busy shop hits the cap in 3 days (and is clearly a real buyer), a slow shop gets a genuine 15 days of evaluation.

---

## 3. What offline enforcement is realistically worth

Effort vs. resistance, honestly scored:

| Technique | Effort | Blocks | Defeated by |
|---|---|---|---|
| Timestamp in `config.toml` | 10 min | Nobody | Deleting the file |
| Timestamp in SQLite only | 30 min | Nobody | Deleting `data/` |
| + 3 redundant HMAC'd locations (SQLite, registry, ProgramData) | 4 h | Most casual users | Anyone who finds a reset tool / knows the paths |
| + clock-rollback detection (`last_seen` watermark) | +2 h | "Set clock to 2020" | Editing the blob |
| + machine fingerprint binding | +3 h | Copy-the-folder-to-another-PC | Spoofing the fingerprint |
| Binary obfuscation / custom VM | 1–3 weeks | Slightly more people | Someone with Ghidra |
| Hardware dongle | $$$ | Most | Lost dongle = furious customer |
| **Online license check** | 2 d | **Real** | Needs internet (kills your offline goal) |
| **Signed license file (Ed25519), offline-verifiable** | 1.5 d | **Real**, no internet needed | Key extraction from binary → but you reissue keys, that costs *them* a purchase |

**Note the last two rows.** Online checks and signed licenses are the only mechanisms that actually enforce. The local-trial hardening is *speed bump* territory — worth ~1 day, not 2 weeks.

---

## 4. Three models that actually work

### Model A — Hardened local trial + signed offline license ⭐ **recommended**
```
Day 0        : trial starts. Counters written to 3 HMAC'd locations.
Day 1–15     : banner "Trial — 12 days left", never a surprise block.
Buyer pays   : you send a license file (or a 40-char key).
Activation   : works completely OFFLINE — the exe verifies an Ed25519
               signature with a public key compiled into the binary.
After trial  : no valid license → read-only mode (see §5), never data loss.
```
- Zero phone-home. Your privacy story stays clean ("your data never leaves your shop").
- You can issue: 15-day trial *extension* tokens, per-machine licenses, or unlimited ones — all signed, all offline.
- Theft is possible only by patching the binary or extracting the private key (it never ships — only the public key does).
- **Cost: ~2.5 days** including the little offline key-generator tool you run on your own laptop.

### Model B — Subscription lease piggybacked on the Neon sync
Because this app *already* syncs daily metrics to Neon, you get a license channel for free:
- Each successful sync pulls back a `licence` row: `plan`, `paid_until`, `grace_days`.
- Online shops: enforced tightly, no local hacks matter.
- Offline shops: **30-day offline grace**, then read-only. A shop that goes a month without internet has bigger problems than your invoice.
- Best economics (recurring revenue), but **only works for shops with some internet**. A truly offline shop can't use it.
- **Cost: ~0.5–1 day** on top of Model A (it reuses the sync task).

### Model C — No enforcement in v1
- Ship it, sell it to shops you personally know, invoice manually.
- Rationale: you have 0 customers today. Spending 3 days on DRM before your first sale is optimising the wrong thing — those 3 days are better spent on the stock ledger or receipts.
- Add Model A when customer #3 arrives, or when you start selling to strangers.

**My actual advice:** build Model A's *core* (it's small), but **do it during M7 packaging — not up front**. And if time is tight before your first install, ship Model C and retrofit. The trial-state code is ~600 lines; it is not architectural.

---

## 5. "Block everything" is the wrong failure mode

Never brick the app or hold data hostage. A shop that loses access to its own sales history on day 16 will:
- hate the product, demand a refund, leave a bad reputation locally, and
- possibly have a legal argument about destroying their business records.

**Recommended degradation ladder:**

| State | POS / sales | Stock, reports, dashboard | Data export & backup |
|---|---|---|---|
| Trial, >3 days left | ✓ | ✓ | ✓ |
| Trial, ≤3 days left | ✓ + prominent amber banner | ✓ | ✓ |
| **Trial expired** | ✗ "Enter license key" | ✓ **read-only** | ✓ **always allowed** |
| License expired (subscription) | ✗ | ✓ read-only, full history | ✓ |
| Clock tampered | ✗ | ✓ read-only | ✓ |
| License valid | ✓ | ✓ | ✓ |

Rules: **you can always read, report and export your own data.** You just can't record new sales. That's defensible, respectful, and still forces the purchase — because the shop's actual work stops.

Also: warn from **day 1**, not day 15. A counter in the header ("Trial: 11 days left · Buy"). Surprise blocks generate support calls; countdowns generate sales.

---

## 6. Concrete implementation sketch

### 6.1 What's stored (and where)
```
Blob (fixed binary layout, HMAC-SHA256 keyed with a compile-time key):
  { version, install_id(uuidv7), machine_fp(u64), first_seen,
    active_days_hash, sales_count, last_seen_utc, flags }
Locations (3 independent copies, written on each launch):
  1. SQLite table  licence_state
  2. %APPDATA%\store\.lic  (hidden file)
  3. HKCU\Software\<vendor>\store  (binary value)
```
On startup: read all 3, take the **most restrictive** valid blob (union of active days, max of last_seen). Any blob failing HMAC → treated as corrupted, not as "reset" (an attacker who deletes one copy gains nothing).

### 6.2 Clock-tamper detection
- Persist `last_seen_utc`. If `now < last_seen - 10 min` → `TAMPERED` flag, sticky.
- If the flag trips, the trial **freezes** (no new active day accrues) rather than expiring instantly — protects an honest user with a dead CMOS battery, which happens more often than you'd think.
- Recovery: an offline "clock-ok" token you sign, or just a license.

### 6.3 Machine fingerprint
`first 8 bytes of SHA-256(volume serial ‖ motherboard UUID ‖ CPU id)`, tolerating one component change. **Do not bind by MAC** (USB dongles & Wi-Fi shuffles break it). Mismatch + no license → fresh trial on a new machine; that's an accepted leak (§3).
- Policy: **2 machine activations** per license before reissue. Warn before the second one.

### 6.4 License file (the part that actually enforces)
```
license.json : { shop_name, license_id, machine_fp|null,
                 issued_at, paid_until|null, max_tills, features[] }
license.sig  : Ed25519 signature over the canonical JSON
```
- Public key embedded in the exe; private key on your laptop, offline.
- `machine_fp: null` = portable license (for shops that will move hardware) — your call per customer.
- You also need a **tiny signing CLI** (~150 LOC) to emit keys/licenses. Build it in M7; never ship it.

### 6.5 Seed data trick
Consider making the trial **generous in features but visibly time-limited**: no feature-gating, no nagware, no crippled reports. Full product, hard clock. Trials that cripple functionality convert worse and generate "is it broken?" support.

---

## 7. Alternatives you may not have considered

| Alternative | Why it might beat a trial |
|---|---|
| **Paid pilots** — charge a small setup fee from day 1 (e.g. 150 TND + monthly) | No DRM complexity at all; paying customers are qualified; you fund development |
| **Free tier forever, paid = cloud dashboard + multi-till + backups** | The *local POS* is free and unkillable (great for trust & word-of-mouth in a small market); you charge for what's genuinely cloud-shaped. Fits your architecture perfectly — the Neon dashboard is already the paid half |
| **Per-shop perpetual license, no expiry** | Simpler than subscriptions, no lease logic, no renewal support burden. Common for local software in Tunisia where recurring card payments are painful |
| **Sell install + support service, not software** | Higher revenue per shop, zero piracy concern, but doesn't scale past your calendar |
| **Trial = 30 days** | 15 days may be too short for a shop to judge month-end reports and stock value — both only become meaningful after a full month. **Consider 30.** |

**My favourite for your case: free local POS + paid cloud dashboard.** You already built the split. The POS being free removes all DRM risk from the part that must never fail (money handling, offline), and the dashboard is the natural thing to charge for — it's the "check my profits from anywhere" value. If you'd rather charge for the POS itself, do Model A with **30 days**.

---

## 8. Cost summary

| Work | Days | When |
|---|---|---|
| Hardened trial state (3 copies, HMAC, counters, tamper flag) | 1.5 | M7 |
| Read-only degradation mode + banner UI | 0.5 | M7 |
| License file verify + activation UI | 1.0 | M7 |
| Signing CLI (yours, not shipped) | 0.5 | M7 |
| *(optional)* subscription lease via Neon sync | 0.5–1.0 | M7+ |
| **Total** | **3.5–4.5 d** | — |

Insulate it: put licensing behind a `LicenceService` trait with a `DevUnlimited` implementation, so development never fights the trial and tests can freeze/reset state deterministically.

---

## 9. Decisions needed from you

1. **Which model?** A (local trial + offline license) / B (subscription lease) / A+B / C (no enforcement, v1)?
2. **Charge for the POS, or free POS + paid cloud dashboard?** (Changes the whole licensing surface.)
3. **15 or 30 days?** I'd argue 30 — month-end reports are the real selling point.
4. **One-time perpetual or annual?** Affects whether we need the lease/grace machinery at all.
5. **Do your target shops reliably have internet?** If yes, Model B becomes attractive and much stronger. If no, offline license only.

Once decided, I'll fold it into `PLAN.md` §12 + §14 (M7) and record the accepted decision here.
