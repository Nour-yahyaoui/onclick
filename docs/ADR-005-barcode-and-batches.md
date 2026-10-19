# ADR-005 — Barcode selling & batch (lot) tracking

**Status:** ✅ **ACCEPTED** — FEFO default, warn-only out-of-stock, cashier can quick-create an unknown barcode, expiry required only when the product declares a shelf life.
**Date:** 2026-10-20
**Relates to:** `PLAN.md` §5 (schema), §6 (scope); `DESIGN-001` §5–§6 (UI surface)

---

## 1. Scope clarification (please confirm)

You said *"the desktop dashboard should support barcode selling and adding batches."*

Taken literally on the **cloud** dashboard, that would break ADR-003's core rule (cloud = read-only analytics, no CRUD) and reintroduce conflict resolution for stock. So this document assumes:

- ✅ **`store.exe` (local app: POS + local dashboard)** — barcode selling, batch receiving, batch management.
- ✅ **Cloud web dashboard** — *displays* batch/expiry analytics (expiring soon, stock value by batch) but **writes nothing**.

If you actually want to receive stock from the cloud dashboard, say so — it's possible but requires the cloud to become an owner of stock state and that costs the "conflict-free" property (≈ +4 days and a real sync-conflict design).

---

## 2. Barcode selling

### 2.1 Hardware reality
A USB barcode scanner is a **keyboard**, not a device. It types the code fast and presses Enter. No driver, no SDK, no serial port, no permissions. This is why it works offline and needs zero integration — and it's also where most POS apps break it.

### 2.2 Capture strategy (the part that must be right)

**Do not** rely on focusing an `<input>`. The cashier will be mid-dialog, or a text field will have focus, and scans will be lost.

```text
global keydown listener (window, capture phase)
  ignore keystrokes when target is a text field, EXCEPT the ScanField
  buffer chars while interval < 35 ms  → scanner signature
  on Enter (or 200 ms idle): dispatch the buffer as a scan
  if the buffer came from a human (slow, irregular) → treat as search text
```

Details that prevent real-world failures:

| Risk | Handling |
|---|---|
| Arabic keyboard layout active → digits arrive as different glyphs | read `event.code` (physical key) **and** `event.key`; normalise digits; barcode input forced `dir="ltr"` |
| Scanner configured with CRLF prefix/suffix | strip `\r\n\t`, configurable prefix/suffix in settings |
| Scanner in "numpad" mode sends `Numpad0-9` | same normalisation path |
| Cashier types fast | 35 ms threshold + length ≥ 4 rules out human typing |
| Modal open / licence screen | queue scan in a 3-deep buffer, replay after close |
| Duplicate scan (double trigger) | 150 ms debounce per identical code |
| Very long codes (QR with URL/GS1) | accept up to 256 chars; GS1 `(01)…(10)` parsing later |

### 2.3 Lookup path (must be sub-millisecond)
- Table `product_barcodes(barcode PK, product_id, kind, pack_qty, is_primary)` — one product can carry many barcodes: unit, **case of 12**, supplier code, legacy code.
- **In-memory hashmap `barcode → (product_id, pack_qty)` loaded at startup** (10 000 SKUs ≈ ~1 MB), invalidated on any barcode write. SQLite is the truth; the map is a cache.
- Fallback to a unique index lookup (< 1 ms) if the map misses (e.g. barcode added on another screen).
- Validation on add: EAN-8/13, UPC-A, Code-128, ITF-14, QR; **check-digit verification** with a "this barcode looks invalid — save anyway?" override.
- Conflict detection: barcode already assigned to another product → hard error.

### 2.4 Behaviour on scan
| Situation | Behaviour |
|---|---|
| Product in catalog | add to cart · if already present, `qty += pack_qty` (setting: "increment" or "new line", default increment) |
| Product out of stock | added with red `no stock` badge + setting: `block` / `warn` / `allow` (**default warn** — never stop a sale, never lie about stock) |
| Product tracks batches | auto-pick **FEFO** batch, chip shows `exp 03/26`; tap chip to change (§3.6) |
| Unknown barcode | **`UnknownBarcodeDialog`**: name, price, category, stock, "create & add" in one step. This is the single most-used path in a real shop — it must take < 10 s |
| Barcode of a case (pack_qty = 12) | adds 12 units; cart line shows `×12` |
| Weighted item (scale label: prefix `2xxxx`, embedded weight/price) | parse and add fraction qty; v1.1 but hook designed now |
| Scanner of a product that was archived | "reactivate & add" inline |
| Same product, different current price | always use current price; never trust the code |

### 2.5 Producing barcodes
- **Generate internal EAN-13** with the in-store prefix range (e.g. `2xxxxx…`) + computed check digit, or Code-128 for internal SKUs.
- Label printing (A4 sheets of 24/40 labels) is **v1.1**; store the generated code on the product from day one so labels can be printed later without data migration.
- Supplier barcodes are kept as additional rows → a shop can scan either.

### 2.6 Effort
| Item | Days |
|---|---|
| Global capture buffer, normalisation, debounce, layout independence | 1.0 |
| Lookup map + barcode tables + CRUD UI + validation | 1.0 |
| Unknown-barcode quick create, out-of-stock policy, pack qty | 0.5 |
| Scan → batch auto-selection + picker sheet | 0.5 |
| **Total** | **3.0** |

---

## 3. Batches / lots

### 3.1 Why this matters for a real shop
Batch tracking is what turns "we have 40 units of yogurt" into "**12 units expire Friday and 20 already expired last week**". For a grocery, pharmacy, cosmetics or dairy shop it is not a nice-to-have: it's the difference between knowing your shrinkage and guessing at it. It's also what lets margin be computed **per supplier lot**, which is how you discover a supplier is quietly expensive.

### 3.2 Design rule: every product has ≥ 1 batch
```sql
products(id, ..., tracks_batches INTEGER NOT NULL DEFAULT 0, shelf_life_days, expiry_alert_days)

batches(
  id TEXT PRIMARY KEY,
  product_id TEXT NOT NULL REFERENCES products(id),
  batch_code TEXT NOT NULL DEFAULT '',        -- '' = implicit "no batch" lot
  supplier_id TEXT NULL, purchase_id TEXT NULL,
  received_day DATE NOT NULL,
  expiry_day   DATE NULL,
  qty_received INTEGER NOT NULL,
  qty_remaining INTEGER NOT NULL,             -- cache; must equal Σ movements
  unit_cost_millimes INTEGER NOT NULL,        -- cost of THIS lot
  status TEXT NOT NULL,                        -- active|exhausted|expired|written_off
  UNIQUE(product_id, batch_code, received_day)
)

stock_movements(..., batch_id TEXT NULL REFERENCES batches(id))
sale_lines(..., batch_id TEXT NULL, unit_cost_millimes INTEGER NOT NULL)
```
**The implicit-lot trick:** products with `tracks_batches = 0` still get one row (`batch_code = ''`, `expiry_day = NULL`). Every code path — stock, valuation, FEFO, returns, reports — then handles a single shape. No `if batches_enabled` branches sprinkled through the money code. That one decision is worth several days of bug-avoidance.

### 3.3 Costing
- **Per-lot cost** (`unit_cost_millimes` on the batch) is the truth for stock value: `Σ qty_remaining × unit_cost`.
- `sale_lines.unit_cost_millimes` freezes the cost at sale time → historical margin never drifts when a new, pricier lot arrives.
- Product-level moving average is *derived* from batches (for the reports that want one number), never stored separately as a competing truth.
- Two lots of the same product at different costs → **margin per lot** report: instantly shows which supplier lot actually made money.

### 3.4 Receiving a batch (`BatchReceiveForm`, the main daily flow)
```
Supplier [ ▾ ]   Invoice ref [ ABC-2231 ]   Date [ 20/10/2026 ]
─────────────────────────────────────────────────────────────
 scan barcode → product row auto-added
 [Product]        [Batch code]  [Expiry]   [Qty]  [Unit cost]  [Line total]
 Yogurt 500g       L-8831       03/11/26     24     1.250        30.000
 Cheese           (none)        —            10     4.800        48.000
─────────────────────────────────────────────────────────────
 quick actions: [copy expiry from line above] [last cost] [no batch]
 Invoice total: 78.000 TND     matches supplier doc? [✓] / [override + reason]
                                              [ Save & receive ]
```
Rules: expiry required if `product.shelf_life_days` set; expiry in the past → blocked; duplicate `(product, batch_code, received_day)` → merge or separate lot (default merge); the receive is **one SQLite transaction** producing N `stock_movements` (type `purchase`) + batch rows; a mismatch between the typed invoice total and the sum is allowed but audited.

### 3.5 Consumption policy
- **FEFO default** — `ORDER BY expiry_day ASC NULLS LAST, received_day ASC, id ASC`.
- FIFO available as a per-shop setting (some categories are not expiry-sensitive).
- Manual batch override from the cart: allowed always, **audited** (who, when, which lot) — needed for "customer wants the fresher one".
- Expired lot: excluded from auto-pick; adding it explicitly requires an owner override + reason, logged.
- Negative stock never silently allowed at lot level: if all lots are empty and the shop allows overselling, a single `''` lot goes negative so reports stay coherent.

### 3.6 Batch selection UI in the cart
`CartLine` shows a chip: `exp 03/26` (neutral), `exp 8d` (amber), `expired` (red). Tapping opens `BatchPickerSheet` listing all active lots for that product with expiry, qty, and unit cost hidden unless the user has cost permission. Default selection is the FEFO pick, so the fast path needs zero interaction.

### 3.7 Adjacent features that batch tracking unlocks
| Feature | Value |
|---|---|
| **Expiry report** (30/60/90 d aging) | the report a grocery owner actually opens every morning |
| **Write-off flow** (movement type `loss`, reason `expired`, batch-bound) | converts invisible shrinkage into a number on the P&L |
| **Returns restore to the original lot** (`sale_line.batch_id`) | correct stock and correct cost, no guessing |
| **Supplier quality score** (returned/expired rate per supplier) | negotiation leverage |
| **Batch recall** (list every sale from a lot) | food-safety answer in seconds |
| **Per-lot margin** | which purchases were actually profitable |
| **Expiry-aware discounting** (auto "30% off, 3 days left") | turns loss into revenue — v1.1 |

### 3.8 Integrity
- Invariant: `batches.qty_remaining == Σ stock_movements.qty_signed WHERE batch_id = …`, checked by `--doctor` and by a nightly internal check; mismatch → repair + audit entry.
- Batches are **never deleted**, only `exhausted`/`written_off` (history must survive).
- Migration for shops that already hold stock without batches: one implicit lot per product at the current moving-average cost.
- Sync: batches stay **local**. The cloud gets only aggregates (`stock_value_daily` split into `fresh` / `near_expiry` / `expired`, and `expiring_soon_count`) — expiry analytics without exporting lot-level or supplier data.

### 3.9 Effort
| Item | Days |
|---|---|
| Schema, migrations, implicit-lot backfill, `--doctor` invariant | 1.5 |
| Receive flow (multi-line, validation, transaction, audit) | 2.0 |
| FEFO consumption, override, expired handling, cart chips | 1.5 |
| Batch picker sheet, lot list per product | 1.0 |
| Expiry report + write-off + returns-to-lot | 1.5 |
| **Total** | **7.5** |

---

## 4. Schema changes vs `PLAN.md` §5

| Table | Change |
|---|---|
| `products` | `+ tracks_batches`, `+ shelf_life_days`, `+ expiry_alert_days`, `+ image_path` |
| `product_barcodes` | new — `barcode PK, product_id, kind, pack_qty, is_primary` |
| `batches` | new — see §3.2 |
| `stock_movements` | `+ batch_id` (nullable, indexed) |
| `sale_lines` | `+ batch_id`, `+ unit_cost_millimes` (already planned), `+ qty_pack` |
| `purchase_order_lines` | `+ batch_code`, `+ expiry_day` |
| `returns_line` | `+ batch_id` |
| `settings` | `+ barcode_prefix/suffix`, `+ qty_scan_mode`, `+ out_of_stock_policy`, `+ consumption_policy`, `+ theme`, `+ locale`, `+ numeral_system` |
| new indexes | `product_barcodes(barcode)`, `batches(product_id, expiry_day)`, `batches(status)`, `stock_movements(batch_id)` |

---

## 5. Performance implications
- Barcode scan → cart line: **< 2 ms** (hashmap + in-memory cart; no DB write until Save).
- Batch auto-pick: one indexed query on `batches(product_id, expiry_day)`; **< 1 ms**.
- Receiving 50 lines: one transaction, ~15–40 ms (fsync-bound); stays well inside a 100 ms UI budget.
- Product grid with stock+expiry chips: served from a single joined view, materialised per grid load; virtualised rendering > 200 items.
- Stock valuation query with 5 000 lots: < 20 ms via `batches(status)` partial index.

---

## 6. Testing
- Scan simulation: keyboard events at 5 ms intervals → asserts buffer detection, layout independence, Enter terminator, CRLF stripping.
- Property test: any sequence of receive/sale/return/write-off → batch invariant holds.
- FEFO: fixture lots with overlapping expiries, nulls, expired, and equal dates → deterministic order.
- Unknown barcode → product created → scan again resolves (cache invalidation test).
- RTL: scanning with `dir=rtl` active produces the same cart (ADR/design cross-test).
- E2E: receive lot → sell it → return it → assert stock back in the *same* lot.

---

## 7. Milestone placement

| Milestone | Addition | Days |
|---|---|---|
| M1 | barcode tables + lookup map + product card badges | +1.0 |
| M1.5 | **batches**: schema, receive flow, FEFO (must land before real data enters the till) | +5.0 |
| M2 | scan-to-cart, unknown-barcode dialog, cart batch chips, pack qty | +2.0 |
| M3 | expiry report, write-off, returns-to-lot, per-lot margin | +2.5 |
| **Total** | | **+10.5 d** |

**Ordering note:** batch schema must exist **before** the first real shop enters stock, otherwise you'll migrate live data later (doable, but a needless risk). If time is short, ship barcode selling in v1 and batches in v1.1 — but create the *implicit-lot* rows from day one so the migration is a no-op.

---

## 8. Risks
| Risk | Mitigation |
|---|---|
| Cashiers scan while another input is focused → missed scans | global capture buffer + queue + audible beep on every successful scan (trust is built by sound) |
| Scanner configured with a prefix that collides with typing | settings for prefix/suffix + a "test your scanner" screen |
| Shops never enter expiry dates → FEFO degrades to FIFO | make expiry mandatory when `shelf_life_days` set; default `shelf_life_days` per category |
| Duplicate/merged lots confuse stock | merge rule is explicit and audited; only `(product, code, day)` merges |
| Receiving 200 lines on a slow disk > 1 s | wrap in one transaction, show progress, keep the form recoverable on failure |
| Cloud dashboard expects lot data you deliberately don't export | dashboards show **aggregate** expiry buckets only — designed, not accidental |

---

## 9. What this does NOT include (v1)
Serial/IMEI tracking (per-unit, needed for phones/electronics — different model, ~+4 d), weighted-scale label parsing (+2 d), label printing (+2 d), batch-level transfers between tills, multi-warehouse, supplier return-to-vendor documents.

---

## 10. Decisions needed
1. **Confirm scope:** batches/barcode selling only on the local exe; cloud stays read-only analytics? (§1)
2. **Do your shops need expiry at all** (grocery/pharmacy/cosmetics), or is batch = *purchase lot only* (for costing) — much simpler UI if no expiry? This is the biggest simplification available.
3. **Consumption policy** default: FEFO (recommended) or FIFO?
4. **Out-of-stock at sale:** warn (recommended, never blocks a sale), allow silently, or block?
5. **Unknown barcode:** allow inline quick-create for cashiers, or owner-only? (Default: cashier can create with price, owner reviews later.)
6. **Batch required on receive** for batch-tracking products: yes (recommended) or optional?
