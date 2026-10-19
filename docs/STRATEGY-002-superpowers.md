# STRATEGY-002 — What I would add to make this genuinely overpowered

**Status:** my recommendations (not requests from the owner)
**Date:** 2026-10-20
**Context:** 13 docs, ~94 dev-days planned, 0 shops installed. These ideas are chosen for *leverage per day*, not for feature parity.

---

## 0. The thesis

The system you've specified is already **feature-rich**. What it isn't yet is:

1. **Impossible to lose data in** — the only fear that beats price in a phone shop
2. **Cheap for you to support** — a solo dev cannot hand-hold 50 shops
3. **Faster to sell** — your install must beat everyone's demo
4. **Compounding** — every sale should make the product smarter

So my suggestions are not "more modules". They are **four superpowers**, each small enough to actually ship.

---

## 1. Superpower #1 — Zero-dinar loss (the fear killer)

Every competitor says "your data is safe". Almost none can prove it. You can, cheaply.

### 1a. Crash-proof sale journal (~2 d) ⭐ my top pick
A second, **tiny append-only file** — one line per committed sale, `fsync`'d *before* the SQLite commit:

```
2026-10-20T14:22:31Z V-1042 1500000 cash L2 iPhone13 14:22 ok
```
- Total DB corruption → `--repair` replays the journal → **not one sale is lost**.
- Human-readable, 40 bytes/sale → 300 sales/day = 4 MB/year of compressed safety.
- Claim you can put on a poster: **« Aucune vente perdue. Jamais. »** — and it's true, and it's testable (fuzz the DB, replay, compare).

### 1b. Encrypted off-site backup, zero-knowledge (~3 d)
Local backups don't survive a stolen/burned PC — currently the shop's only defence is a USB stick they'll forget.
- Nightly: encrypt the backup **client-side** with the shop's key (SEC-001 hierarchy), upload to Cloudflare R2 / Backblaze (~0.5 ¢/GB/month → pennies per shop).
- **You cannot read it.** The key never leaves the shop. That's the privacy promise intact *and* the disaster-recovery story solved.
- Restore from the dashboard: "your shop died → new PC → recovery code → everything back, including photos and debts."
- This is a **paid-tier feature** people happily pay for, and it costs you ~0.

### 1c. Signed daily hash (~1 d)
Each closing produces `day_hash = Ed25519(previous_hash || day totals)`, printed on the Z-report and shown in the app.
- In a control or a dispute: **provable** that the day's figures were not altered after closing.
- Pairs perfectly with ADR-011 (editable + attributed): the record is editable, and *the fact of every edit is provably timestamped*.

**Total: 6 days for a reliability story no local competitor has.**

---

## 2. Superpower #2 — Support 50 shops alone (solo-dev economics)

Your real constraint isn't features, it's **your calendar**. Every support call is a day lost.

### 2a. Health bundle (~2 d)
Once a day, an aggregate-only, PII-free bundle: app version, error ring buffer, slow-query list, disk space, DB size, sync status, backup status, crash count. Consented at setup, visible to the shop ("voici ce qui est envoyé").
- You see a failing disk **before** the shop calls. You ship a fix before they notice.
- Turns support from reactive panic into a Monday-morning review.
- Feeds marketing later: *« 0 vente perdue sur 12 magasins en 18 mois »* — a claim backed by telemetry, not hope.

### 2b. One-click "Réparer" + printer/scanner self-test (~1 d)
`--doctor` in the UI: integrity check, chain check, backup verify, disk space, port conflict, printer reachable, scanner keystroke test. Fix buttons for the 8 known problems.
- Most support tickets in this market are: printer not printing, scanner double-typing, disk full, wrong date.

---

## 3. Superpower #3 — Sell in 60 seconds (distribution)

The demo *is* the sales process. Target: **exe → 3 products → sale → receipt → profit, in under 60 seconds, no account, no internet.**

### 3a. WhatsApp digital receipt (~1 d) ⭐ highest marketing ROI
Instead of (or in addition to) a thermal ticket: build the receipt text → `wa.me` deep link → the *customer* receives a branded, dated ticket on their phone.
- Shop saves printer money and paper.
- **Every customer sees your product's name** — free distribution in a market that runs on WhatsApp.
- Works offline until send. No API, no cost.

### 3b. Carnet page photo → import (~3 d)
The single biggest onboarding blocker: 30 pages of handwritten debt notebook.
- Photograph the page → local OCR (tesseract, offline — **no upload**, names stay local) → a review table where the shop fixes the 10% OCR got wrong → import.
- Even at 80% accuracy this converts a 3-hour chore into 20 minutes, and it's the *demo* that makes an owner say "take my money".

### 3c. Supplier price-list import (~1.5 d)
Every distributor sends an Excel/PDF price list. Paste it → products, barcodes, costs, prices created in bulk. A phone shop's catalog is largely "whatever my supplier sells this month".

**Total: 5.5 days, and it's the difference between a 3-week onboarding and a 20-minute one.**

---

## 4. Superpower #4 — Compound: turn the data into decisions

These use **only local data, no AI, no PII**, and they change behaviour daily — which is what makes software indispensable.

### 4a. Restock + cash-flow advisor (~2.5 d) ⭐ the "wow" that isn't hype
Deterministic, from their own history:
```
iPhone 13 128GB : vend 12/semaine → commander 24 avant mardi (délai fournisseur 5j)
Trésorerie : 8 unités possibles maintenant · 12 rendraient le solde négatif le 28/10
```
- Combines stock velocity + supplier lead time + **cash position including open debts** (ADR-010).
- Nobody in this market answers "can I afford this shipment?" — it's the question every phone shop owner has *every week*.

### 4b. Basket affinity (~1.5 d)
From tickets only, no customers: *« 70% des acheteurs d'iPhone 13 prennent une coque · marge coque 42% »* → suggest a bundle at the point of sale.
- Pure product-level math; no privacy cost; increases basket size, which the owner feels immediately.

### 4c. Dead-cash detector (~1 d)
Not just dead *stock*: capital frozen by units that haven't moved in 60/90 days, at their **current** replacement cost (ADR-008). *« 4 200 TND immobilisés dans 11 articles sans vente depuis 90j — dont 2 300 dans 3 iPhones 11. »*
- Turns the stock report from a list into an argument for a discount campaign, with the suggested discount.

---

## 5. Superpower #5 — The mobile second till (biggest structural differentiator)

Your exe already runs a web server. Bind it to the **shop LAN** (opt-in, TLS + pairing PIN, SEC-001 §5):
- **Phone camera = barcode/IMEI scanner** → stock count on the shelves in 20 minutes instead of 4 hours.
- **Tablet = second till** during rush hours, no extra licence debate, no extra install.
- Cloud POS vendors cannot offer "works with no internet, on your old tablet, from the same box that runs the till".

Effort ~3 d after the auth layer exists. This is the feature that makes a shop say *"so I don't need to buy another PC?"*

---

## 6. What I would **not** build (and I'd say so in the README)

| Temptation | Why it kills you |
|---|---|
| **Building all 94 days before shop #1** | You'll build for imagined workflows. Cut a **pilot slice (~35 d)**: M0–M2 + debts, sell to 3 shops at a pilot price, then let their actual complaints order the backlog |
| **The AI assistant before ~100 real questions** | You have no eval data. Log every question an owner asks during the pilot (even by voice/paper), then build the intent set from *their* words. The 200-question suite should be *harvested*, not imagined |
| **Cloud dashboard before the local app is loved** | If the till isn't loved, the dashboard sells nothing |
| **Multi-till, e-commerce, loyalty, full accounting** | Already ruled out — stay disciplined |
| **Onboarding tours, feature checklists, badges** | DESIGN-002 already forbids; a shopkeeper's patience is the scarcest resource |
| **Chasing Odoo/competitor parity** | You'd lose. Win on: never down, never lost a dinar, real debts, real costs, derja |

---

## 7. If I had to pick five things to add

| # | Suggestion | Effort | Why it's a superpower |
|---|---|---|---|
| 1 | **Crash-proof sale journal** | 2 d | "No sale lost, ever" — provable, unique |
| 2 | **Zero-knowledge off-site backup** | 3 d | Kills the #1 fear; paid-tier feature, ~0 cost |
| 3 | **WhatsApp digital receipt** | 1 d | Free distribution with every sale |
| 4 | **Restock + cash-flow advisor** | 2.5 d | Answers the weekly question nobody answers |
| 5 | **Carnet photo → import** | 3 d | Turns the worst onboarding into a 20-min win |
| | **Total** | **11.5 d** | ~12 % more build, disproportionate product |

Everything else in the 13 existing docs stands. These five add no new surfaces to maintain — they extend mechanics that already exist (journal ← sale path, backup ← SEC-001 keys, WhatsApp ← receipt renderer, advisor ← stock/debt/report code, OCR import ← debt import).

---

## 8. The one measurement rule

Before anything else: **instrument the app from day 1** — per-screen latency, error ring buffer, slow queries, backup status. Two reasons: you can only promise "fast and reliable" if you can see it, and in 18 months that data is your entire marketing story. Cost: ~1 day. It is the highest-ROI day in this document.
