# ADR-008 — Costing when prices move (per-lot cost, FX, landed cost, repricing)

**Status:** ✅ **ACCEPTED** — per-lot cost, FX + landed cost **in v1**, FIFO consumption, warn-only margin floor.
**Date:** 2026-10-20
**Confirms:** ADR-005 §3.3 (per-lot cost) · **Extends:** ADR-006 (phone vertical: FX + import lots)
**Relates to:** `PLAN.md` §7 (KPI definitions)

---

## 1. Your statement, as a design rule

> "The price is not constant. The same product can be bought at a different price in a different batch, because the market moves."

**Confirmed — this is already the core of the model, not a patch:**

| Layer | Rule |
|---|---|
| **Every delivery = its own lot** | `batches.unit_cost_millimes` — cost belongs to the *lot*, never to the product |
| **Every serialized device = its own cost** | `units.purchase_cost_millimes` — the truest possible cost (specific identification) |
| **Every sale line freezes what it consumed** | `sale_lines.unit_cost_millimes` + `batch_id` + `unit_id` — written at sale time, **never recomputed** |
| **Product has no single cost** | `products` stores no cost field at all. Only derived views: `last_cost`, `avg_cost`, `current_stock_value` |

So a price change tomorrow **cannot** rewrite yesterday's profit. That was deliberate in ADR-005; nothing changes here.

### Worked example (same product, two purchase prices)
```
10/10  receive 5 × iPhone 13 128GB @ 1,000.000   (lot L1)
14/10  market moves ↑
18/10  receive 5 × iPhone 13 128GB @ 1,200.000   (lot L2)

20/10  sell 6 units @ 1,500.000 each  →  revenue 9,000.000

FIFO / specific identification:
  COGS = 5 × 1,000.000  (L1)  +  1 × 1,200.000  (L2)  =  6,200.000
  gross margin = 9,000.000 − 6,200.000 = 2,800.000  →  31.1%

If the app had stored one product cost (say 1,100 avg):
  COGS = 6 × 1,100 = 6,600.000  →  margin 2,400.000  →  26.7%   ← wrong by 400 TND
```
**400 TND of invisible error on a single sale line** is exactly the drift that makes generic POS margin reports untrustworthy in a moving market.

---

## 2. Consumption order (which cost actually leaves the shelf)

| Product type | Method | Why |
|---|---|---|
| **Serialized** (phones, with IMEI) | **Specific identification** — the cashier picks the actual units (or the system picks and shows it) | The cost of *this* device is known exactly. No estimation at all |
| **Quantity items** (accessories) | **FIFO by default** (FEFO when expiry exists, per ADR-005 §3.5) | Matches how a shop physically sells — oldest delivery first |
| Per-shop setting | `fifo` \| `fefo` \| `average` | `average` exists for shops whose accountant insists, and is clearly labelled as an estimate |

**Never silent:** the cart line shows which lot(s) it consumed (`L2 · 18/10`), and the sale detail shows the split when one line spans two lots. A cashier who wants the cheaper old units can pick them explicitly — that override is audited.

---

## 3. Gap 1 — foreign-currency purchases (phones from China: essential)

Cost today isn't in dinars. It arrives as `USD 285` + freight, converted at some rate, plus customs. Without this, margin on imported phones is fiction.

```sql
batches(
  ...,
  purchase_currency   TEXT    NOT NULL DEFAULT 'TND',   -- TND | USD | EUR | CNY
  cost_foreign_minor  INTEGER NULL,                     -- e.g. 28500 = 285.00 USD
  fx_rate_micros      INTEGER NULL,                     -- 3.120500 TND/USD × 1e6 (integer, no float)
  fx_rate_source      TEXT    NULL,                     -- manual | fetched | invoice
  fx_day              DATE    NULL,
  unit_cost_millimes  INTEGER NOT NULL                  -- final TND landed cost (truth)
)
```
- **Offline by default:** the rate is typed (or copied from the supplier invoice). If online, an optional fetch can pre-fill it — never required, never blocking (consistent with the whole design).
- FX stored as `i64` micros → integer math, no float drift, same discipline as millimes.
- Rate is **frozen per lot**: a shop that bought at 3.12 isn't retroactively affected by today's 3.35.
- Display: `285.00 USD × 3.1205 = 889.343 TND` on the lot screen, so the owner can *see* what FX did to his cost.

---

## 4. Gap 2 — landed cost (freight, customs, transport, fees)

A phone that costs `USD 285` doesn't cost you `889 TND`. It costs that **plus** shipping, customs duty, clearance, local transport, and the courier's cut. Spread over the shipment, sometimes only over some units (a broken one, a demo unit).

```sql
purchase_charges(
  id, batch_id, kind,            -- freight | customs | clearance | transport | insurance | other
  amount_millimes, currency, fx_rate_micros,
  allocation TEXT                -- by_value | by_qty | manual
);
```
- Allocation (`by_value` default) distributes charges across the lot's units → **landed unit cost** is what the batch stores.
- `manual` lets the owner load one unit with a bigger share (e.g. the demo unit).
- UI: receive screen has a **"Charges"** sub-panel — one line each for freight/customs; the landed cost updates live per unit.
- **Result:** the margin report finally answers "did I actually make money on that shipment?", not "did the invoice price look nice".

---

## 5. Selling price when the cost moved

The buying side is solved by costing. The **selling** side is where shops lose money, so it gets first-class support:

### 5.1 Cost visibility at the point of sale
On the product card and the unit picker, the cashier/owner sees (cost hidden for cashier role):
```
last buy:  1,200.000   ·  stock avg: 1,180.000  ·  current price: 1,500.000  → +27%
```
and for serialized units, the **exact** cost of the unit being sold: `L2 · cost 1,200.000`.

### 5.2 Margin floor guard
- Per-product or per-category **minimum margin** (`settings` + `products.min_margin_pct`).
- Selling below it → **warn** with the number (`marge 3% — sous le seuil 10%`), with an audited override + reason. **Never blocks** (ADR-007: require nothing, record everything).
- Selling **below lot cost** → stronger warning (`vente à perte`) — still allowed, always recorded, always appears in the report.

### 5.3 Bulk repricing when the market moves
The real workflow: *"my supplier raised prices 8% this week — reprice the catalog."*
- `PriceChange` screen: filter by category / supplier / brand / low-margin-only → **apply +X%** (or set target margin) → **preview table of every old→new price** → apply.
- Every change writes `price_history(product_id, old, new, at, user, reason, source)` — reversible and auditable.
- Rounding rules per shop (`to 0.500`, `to 1.000`, `psychological .900`) so prices stay shop-like.
- **Price lists** (retail / wholesale / friend) come in v1.1 — the schema has room.

### 5.4 ⭐ The metric nobody else computes: replacement-cost gap
In a moving market, paper profit can hide real capital loss:
```
displayed margin      = (price − lot cost) / price
replacement margin    = (price − last/current buy price) / price
gap = displayed − replacement
```
Report line: **« Marge affichée 31% — marge de remplacement 20%. Vous vendez bien, mais vous vous appauvrissez à chaque vente. »**

Worked: you sell at 1,500 with 1,000 cost (33%), but the next unit now costs 1,200 → you're really making 20%. Ten sales later, you can't afford to restock the shelf. **This single report is a differentiator** (STRATEGY-001 ⑤ upgraded): it turns your POS into the only tool that warns a shopkeeper his prices are stale.

Alerting: flag products where `replacement margin < target − 5 pts` in the daily 21:00 summary. Cheap rules, high value.

---

## 6. Inventory valuation: two numbers, one truth

| Report | Method | Audience |
|---|---|---|
| **Margin / profit** (operational truth) | **actual lot cost** (specific id / FIFO) | the owner, daily |
| **Stock value (accounting)** | selectable: `fifo` **(default)** or `weighted average (CMP)` | the accountant, monthly |

Both are generated from the same lot ledger, side by side with the delta shown. No silent switch: if the accountant prefers CMP, the setting changes the *valuation report only* — the margin report never changes, so past P&L stays comparable. This avoids the classic trap where changing a valuation method retroactively rewrites profit.

---

## 7. Schema & code changes summary

| Table | Change |
|---|---|
| `batches` | `+ purchase_currency, cost_foreign_minor, fx_rate_micros, fx_rate_source, fx_day` |
| `purchase_charges` | **new** — freight/customs/allocation, per batch |
| `units` | `+ purchase_cost_millimes` (exists), `+ landed_cost_millimes` (allocated charges share) |
| `products` | `+ min_margin_pct`, `+ target_margin_pct`; **no cost column, ever** |
| `price_history` | `+ reason, + source, + user_id` (already planned, now used by bulk repricing) |
| `sale_lines` | unchanged (`unit_cost_millimes`, `batch_id`, `unit_id` already frozen) |
| new views | `v_product_cost` (last / avg / replacement cost), `v_margin_gap` |
| settings | `+ valuation_method`, `+ default_target_margin`, `+ rounding_rule`, `+ margin_floor_action` |

Code rules: one `fn lot_cost_tnd(batch) = cost_foreign × fx / 1e6 + allocated_charges` — **single function, integer math**, used by receiving, valuation and reporting. No cost is ever computed in two places.

---

## 8. Effort & decisions

| Item | Days |
|---|---|
| Lot cost + landed charges + FX (receive UI, allocation, integer math) | 1.5 |
| Specific identification / FIFO consumption display + audited override | 1.0 |
| Cost visibility at POS + margin floor guard (warn + audit) | 0.5 |
| Bulk repricing screen + preview + `price_history` | 1.0 |
| Replacement-cost gap report + daily-summary rule | 0.5 |
| Dual valuation report (FIFO / CMP) | 0.5 |
| **Total** | **5.0 d** |

**Decisions:**
1. **Do your shops import in USD/EUR (China/UAE/Turkey)?** If yes, FX + landed cost is **v1**, non-negotiable — otherwise margin is fiction. If they buy locally in TND, it can be v1.1.
2. **Consumption order** for accessories: FIFO (recommended) or weighted average?
3. **Valuation method** default for reports: FIFO (recommended) or CMP (more common in Tunisian accounting — pick whatever your accountant will accept).
4. **Margin floor behaviour:** warn + audit (recommended, never blocks) or hard block below cost?
