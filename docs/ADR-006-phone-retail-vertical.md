# ADR-006 — Phone retail vertical (and where this product draws its line)

**Status:** proposed — needs 2 decisions (§9)
**Date:** 2026-10-20
**Relates to:** ADR-005 (batches/barcodes), STRATEGY-001 (positioning), DESIGN-001 (UI)

---

## 1. Recording policy (resolved)

**Decision (owner):** record **everything, in full detail**, and **never require** a field a shop doesn't have — no mandatory TVA, no mandatory supplier invoice, no mandatory tax ID, no mandatory e-invoice.

This is a **product configuration** decision, implemented as compliance profiles in `docs/ADR-007-recording-modes.md` (default = *Simple*: no tax fields, full capture underneath). The `.exe` is a complete record of the business, not a tax form with a cart attached.

Kept out of scope, unchanged: anything whose purpose is to **delete, hide, or falsify** a recorded sale. Total capture makes that moot — nothing is optional to record.

---

## 2. Serving unregistered / parallel-import shops: practical notes

No moral framing — these are the operational facts that shape the product:

| Fact | Design response |
|---|---|
| Goods often arrive **without a supplier invoice** (parallel import, China import, cash purchases) | Supplier + reference are **optional**; a delivery can be recorded with just cost and quantity. Internal `delivery_ref` is generated if none is typed |
| **Cash-first, no bankable paper** | Cash session and drawer variance matter more than invoices; the Z-report is the daily proof |
| **No trust in cloud** — "they'll see my numbers" | Aggregates-only upload (ADR-003), no customer names/phones leaving the machine, visible sync log, one-click local export |
| **Prices move with FX and grey-market cost** | Cost is entered **per delivery lot** (ADR-005) and frozen at sale → margin stays true even when the buying price jumps week to week |
| **High unit value, small volumes** (10–60 units/day) | IMEI-level records, warranty slips, instalments and trade-ins matter far more than throughput |
| Shops formalise over time | Compliance profiles (ADR-007) flip on **without re-entering history** — you keep the customer through that transition |
| Mixed legal status across the customer base | Product stays **neutral and configurable**; the shop decides which fields it uses |

**Positioning that follows:** *« Il enregistre tout, chaque dinar. Il n'exige rien. »* — records everything, demands nothing. That is a genuinely strong pitch to this market, and it needs no concealment features to deliver it.

---

## 3. The real insight: phones change the data model, not the ethics

You've actually given me a much more valuable signal than the legal one: **your vertical is phones and accessories.** That changes the inventory model fundamentally, and it's the reason most generic POS software fails these shops.

| Generic POS assumption | Phone shop reality |
|---|---|
| Product = SKU with a quantity | Product = **iPhone 15 Pro 256GB Black** — one *unit*, one IMEI, one cost, one supplier, one warranty start date |
| Stock = `qty: 12` | Stock = **12 individual units**, each with its own IMEI, colour, storage, battery health, box/papers, purchase price |
| Batch/expiry | Not expiry — **serial number, warranty clock, and activation state** |
| Price per product | Price varies by **storage × colour × condition (new/sealed/open-box/used)** and by supplier FX |
| Return = restock | Return = a **unit** whose IMEI status must be checked (blacklisted? reported stolen? already resold?) |
| Sale ends the relationship | Sale **starts** warranty service, SAV tickets, and often a **trade-in** chain |

**Phones are serialized inventory. Accessories are batch inventory.** Your system needs both models side by side — which is exactly why the ADR-005 "every product has ≥1 lot" design should be extended to "**every unit is a lot**" for serialized items.

---

## 4. Design: serialized units

### 4.1 Schema extension (adds to ADR-005, no rewrite)
```sql
-- one row per physical device
units(
  id TEXT PRIMARY KEY,
  product_id TEXT NOT NULL REFERENCES products(id),   -- the model/variant
  serial TEXT NULL,                -- IMEI1 / serial, normalized digits only
  imei2 TEXT NULL,
  condition TEXT NOT NULL,         -- new|sealed|open_box|refurb|used
  batch_id TEXT NOT NULL REFERENCES batches(id),  -- keeps cost/expiry machinery uniform
  supplier_id TEXT NULL,
  purchase_cost_millimes INTEGER NOT NULL,
  state TEXT NOT NULL,             -- in_stock|reserved|sold|returned|in_repair|scrapped
  received_day DATE NOT NULL,
  warranty_months INTEGER NOT NULL DEFAULT 12,
  warranty_start_day DATE NULL,    -- set at sale
  notes TEXT,
  UNIQUE(serial)
);
unit_events(id, unit_id, kind, at, user_id, sale_id NULL, note)  -- append-only history
```
**Key trick, consistent with ADR-005:** a *serialized* product still goes through `batches` (one lot per delivery, cost per lot). The `units` table hangs off the batch. So stock valuation, FEFO-free consumption, returns-to-lot, and per-lot margin all keep working **unchanged** — the unit layer is additive, not a parallel system. Accessories keep using plain batch quantities with `tracks_serial = 0`.

`sale_lines.unit_id` (nullable) + `unit_id` on returns → the sale, the receipt, the warranty and the return all point at the same physical device.

### 4.2 Variants (the matrix problem)
`iPhone 15 Pro` × `128/256/512/1TB` × `Black/Blue/Titanium` = 16 SKUs. Do **not** create 16 products by hand.

- `products` = model (barcode-free, not sellable)
- `product_variants` = the sellable SKU (storage, colour, condition) with its own barcode and price
- Grid UI for selecting variant at sale time (big buttons: capacity × colour), so a cashier taps twice instead of searching
- Import via a CSV template from a supplier price list; a shop can load 300 SKUs in one paste

### 4.3 IMEI / serial capture
- **Scan or type at receipt**; normalized to digits only, check digit validated (Luhn) with an override.
- Duplicate IMEI on receive → hard block with "already in stock / already sold on <date>".
- **IMEI check at sale**: warn if the device is flagged (returned, in repair, previously sold). Local blacklist table, plus an optional manual "check" against a public IMEI service when online — **never required**, never blocking.
- **Do not market this as a "stolen phone checker"** unless you integrate a real registry; describe it as an internal duplicate/consistency check. Overclaiming here is a legal risk of its own.
- Phone-camera scanning on the shop-WiFi phone page (v2) — IMEI barcodes are dense; a camera is often better than a cheap 1D scanner.

### 4.4 Warranty & SAV — the retention engine of this vertical
This is the feature that keeps phone shops on your software, because warranty disputes are their daily pain:
- Warranty start = sale date; duration per product *or* per brand policy (`Apple 12m`, `Samsung 24m`, accessories `3m`).
- A **warranty lookup by IMEI** (or by phone number) at the counter: "sold 14/03/2026, warranty until 14/03/2027, supplier: X".
- **SAV tickets**: intake → diagnosis → sent to supplier / repaired / replaced / refused, with the unit's state machine (`in_repair`), and the customer's promise date.
- **Supplier warranty claims**: batch a month's faulty units into one claim with proof-of-purchase, and track the supplier's refund/replacement against the original lot cost. This is real money in this trade and no generic POS does it.
- Print a warranty slip (A5) with IMEI, date, and terms in Arabic/French.

### 4.5 Trade-in / occasion (huge in this market, legally sensitive)
Two-way: the shop buys the customer's old phone and sells the new one.
- A **purchase from an individual** document: ID-holder name, ID number, IMEI, condition, price, signature. In many jurisdictions reselling second-hand phones requires exactly this kind of record — **that's a feature, not a restriction**, and it protects the shop (`vente d'occasion` traceability).
- The traded unit enters stock as `condition = used`, own cost, own IMEI, own warranty (usually none or 3 months shop warranty).
- Net settlement: new sale − trade-in = amount due; the receipt shows both.
- Optional photo of the device + ID document, stored **locally only** (never uploaded — consistent with ADR-003).

### 4.6 Accessories: keep it simple
Batches + quantity + barcode, as in ADR-005. Add: **compatibility tags** (`iPhone 15`, `USB-C`) so search works, and **bundle/kit** support ("screen protector + case + installation"). Low effort, high perceived value.

### 4.7 Payments specific to this trade
- **Instalments / credit**: high-ticket items are often paid over weeks → the credit book (STRATEGY-001 ①) becomes central, with per-instalment schedule and a WhatsApp reminder. Credit limit per customer matters more than for groceries.
- **Mixed payment** (cash + card + credit) on one sale.
- **Deposit / reservation**: customer reserves a unit with a deposit → unit state `reserved`, not sellable, with an expiry on the hold.

---

## 5. What this changes in earlier documents

| Document | Change |
|---|---|
| `PLAN.md` §5 | `+ units`, `+ product_variants`, `+ unit_events`, `+ sav_tickets`, `+ dealer_purchases` (trade-in), `+ instalment_plans`; `sale_lines.unit_id` |
| ADR-005 §3 | batches become the cost/carrying layer; units hang off them. Barcode section unchanged; add IMEI/Luhn + duplicate logic |
| ADR-005 §9 (not included) | **Serial/IMEI tracking moves from "out of scope" to core** |
| STRATEGY-001 §3 ① | Credit book becomes the flagship for this vertical, not just retail groceries |
| STRATEGY-001 §5 | Segment: **phone & accessories retail**, 1–4 staff, 10–60 sales/day, high unit value |
| DESIGN-001 §5 | `+ VariantGrid`, `+ UnitPicker` (list of units of the chosen variant with IMEI tail, condition, cost hidden by role), `+ WarrantyLookup`, `+ SavTicket` board, `+ TradeInForm` |

---

## 6. Competitive angle for this vertical

What phone shops use today: a paper notebook, WhatsApp, and a spreadsheet — or an accounting package that has no concept of an IMEI. What they need, and nobody sells them well:

1. **IMEI-level stock that survives the everyday mess** (open-box units, trade-ins, SAV returns).
2. **Warranty + SAV tracking with supplier claims** (their actual monthly loss).
3. **Instalment credit book with WhatsApp statements** (their actual cash flow problem).
4. **Trade-in with a proper vendor record** (their legal protection and their supply of used stock).
5. **A till that works when the shop's 3G doesn't** (their actual daily reality).

Add margins per lot (ADR-005 §3.3) and you can tell a shop *"this delivery of 12 units made you 6.2% — you could have made 14%"*. That's the sentence that sells an annual licence.

---

## 7. Effort

| Item | Days |
|---|---|
| `units` + `product_variants` + schema/migrations/`--doctor` invariants | 3.0 |
| Variant matrix UI (grid, import template) | 2.0 |
| IMEI capture, normalization, duplicate detection, unit picker at sale | 2.5 |
| Warranty lookup + warranty slip | 1.5 |
| SAV tickets + state machine + supplier claims | 4.0 |
| Trade-in / dealer purchase flow + local ID record | 2.5 |
| Instalments + deposits/reservations | 2.5 |
| Accessory compatibility tags + bundles | 1.0 |
| **Total** | **19 d** |

That's a v1.1-sized investment. **Suggested split:** serialized units + IMEI + variants + warranty lookup in **v1** (8 d) because entering a phone shop's stock without it is impossible; SAV/trade-in/instalments in **v1.1** (11 d) because they only pay off once shops are live.

Revised v1: **≈ 67 dev-days** (was 59). It's getting large — §9 asks you to choose what to cut.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| Scope is now genuinely big (67 d) | Cut from §9; ship a phone-shop-only v1 before adding back-office extras |
| IMEI data entry is slow and error-prone | Scanner + camera + Luhn validation + bulk paste from supplier list; make it a 1-field flow |
| Reputation drifts toward "grey-market software" | Positioning is *records everything, requires nothing*; compliance module present so shops can formalise; no concealment features |
| Second-hand obligations vary by jurisdiction | Keep the trade-in record generic (ID holder, ID no., IMEI, signature) and configurable; let the shop's accountant confirm fields |
| Warranty claims are paperwork-heavy | Ship only the claim-tracking part in v1.1, not supplier portal integration |
| IMEI "checking" overclaims | Label it internal consistency; never promise a stolen-phone registry |

---

## 9. Decisions needed

1. **Cut list for v1** — with phones added, v1 is ~67 days. Pick what defers: (a) SAV + warranty claims, (b) trade-in, (c) instalments, (d) Arabic receipts, (e) multi-till?
   *My cut: a+b+c → v1.1, keep units/IMEI/variants/warranty-lookup + credit book in v1 → ≈ 55 days.*
2. **Used / open-box share** — if used is >30% of your shops' volume, trade-in moves up into v1 (it also supplies their used stock).
3. **Trade-in record fields** — ID-document capture (recommended: protects the shop on used sales), or name + IMEI + signature only?
4. **Receipts in v1** — profiles A + B only (recommended), with C/D arriving alongside the TEIF module? (ADR-007 §7)
