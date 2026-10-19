# ADR-009 — Negotiated pricing & anonymous sales

**Status:** ✅ **ACCEPTED** — free price entry, warn-only floor, multi-currency cash in v1.
**Date:** 2026-10-20
**Relates to:** ADR-007 (require nothing), ADR-008 (costing, replacement margin), ADR-006 (phone vertical)

---

## 1. The rule

> **Price is entered, not looked up.** The catalog holds a *reference* price at most. What the customer actually paid is the only truth, and it is recorded per line, per sale, forever.

Consequences, all of which remove constraints rather than add them:

| Assumption in most POS | Here |
|---|---|
| Product has a price; sale uses it | Product has an **optional reference price**; sale records the **actual price** |
| Discount = reference − sale (a separate concept) | **A discount is just a price that moved.** One field, not two mechanisms |
| Price is validated against catalog | No validation. Below cost → warn, never block (ADR-007) |
| Same product sells at one price | Reported as a **distribution** (min / avg / max), not a single number |
| Customer profile drives pricing | **No customer records by default** (§5) — prices differ because the deal differed, not because a profile said so |

---

## 2. Model

```sql
sale_lines(
  ...,
  reference_price_millimes INTEGER NULL,   -- what the shop "usually" asks (may be NULL)
  unit_price_millimes      INTEGER NOT NULL, -- what was ACTUALLY agreed  ← the truth
  price_source             TEXT NOT NULL,  -- reference | manual | package | promo
  price_delta_millimes     INTEGER NULL,   -- computed: actual − reference (for reports)
  override_reason          TEXT NULL,      -- only when below floor (ADR-008 §5.2)
  user_id                  TEXT NOT NULL,  -- WHO sold at that price  ← key for §4
  unit_cost_millimes       INTEGER NOT NULL -- frozen at sale (ADR-005/008)
);
```

`price_delta` is **derived and stored**, so "how much did we give away" is a single indexed column instead of a join-time calculation. Products keep `default_price` (nullable) and `min_price` (nullable) — both advisory.

**Nothing else changes:** stock, lots, costing, margins, reports all read `unit_price_millimes` exactly as before. Negotiation is not a feature bolted on — it's the assumption the schema already had (the earlier plans never required a catalog price to sell).

---

## 3. POS experience (this is where it's won or lost)

Negotiated sales die when the software makes the cashier fight it. Design accordingly:

1. **Price field is a first-class input** on the cart line, keyboard-reachable (`Tab`/`F3`), numeric keypad on touch. Typing a price is as fast as scanning.
2. **One tap to accept the reference**, so the common "no haggling" sale stays two keystrokes.
3. **Live feedback while typing** (owner/manager role only): `1,300 → marge 12%` · `900 → vente à perte` in colour. Never a modal, never a block.
4. **Quick-price chips** for the habitual haggling ladder: `5,500 · 5,200 · 4,900` (configurable per product, "prix marchandage"). One tap instead of typing.
5. **Optional floor with owner PIN** (`settings.price_floor_mode`): cashier cannot go under `min_price` **unless an owner PIN is entered**, which is logged with the name. Default = *warn only*; hard floor is opt-in.
6. **Full price, no reference:** product with no reference price simply asks for an amount. Works for one-off items, second-hand units, bundles.
7. **Per-line, per-sale:** a 3-line sale can mix paid-at-list and heavily negotiated lines. Totals stay exact (integer millimes).

**Optional multi-currency cash** (common in this trade — the example was *300 dollars*): `sale_payments` gains `currency`, `amount_foreign_minor`, `fx_rate_micros`; the till accepts USD/EUR cash and stores the TND equivalent plus the rate used. Rate is typed or fetched (ADR-008), frozen per payment. Receipt shows both lines. **Never required** — a shop that only takes dinars never sees it.

---

## 4. ⭐ The report that makes this a feature: price dispersion + deviation

Once actual prices are recorded per line with the seller's name, you can see what no paper notebook can:

### 4.1 Price dispersion per product
```
iPhone 13 128GB — ventes ce mois : 14
  min 320.000 · moyen 412.500 · max 520.000
  marge réalisée : 8% … 34%  (moyenne 19%)
  prix de référence : 500.000   → 11 ventes en dessous
```
An owner who haggles "by feel" finally sees his own pricing behaviour. It is also the input for §4.3.

### 4.2 Realized vs nominal margin
Report on **actual** price − **actual** cost, per sale. Not "what the catalog says I earn", but what hit the drawer. Combined with ADR-008's replacement-cost gap, this is the honest picture of a negotiated business.

### 4.3 Employee price-deviation detection ⭐ *the real reason to build it*
In cash + haggling businesses, the classic leak is **selling low on paper and pocketing the difference** (`300 on the ticket, 450 paid, 150 in the pocket`). You cannot detect that without recording *who* sold at *what* price versus what the product normally fetches.

- Per user, per period: `sales count · avg price vs shop avg · % below reference · % below cost · total deviation in TND`.
- Flag: cashier consistently 15–20% under the shop's average on the same product → surfaced in the owner's daily summary.
- **Not an accusation engine:** it shows numbers and lets the owner judge. No automated punishment, no hidden scoring.
- Works entirely off local data, offline. No cloud needed.

**Honest limit:** it cannot catch a cashier who never rings the sale at all. No POS can. It does catch the most common form of manipulation in a negotiated-cash trade, which paper cannot catch at all.

---

## 5. "Don't save clients" — three customer modes

Default: **no client records.** Recorded honestly:

| Mode | Behaviour | Cost of the choice |
|---|---|---|
| **`none`** *(default)* | Sales are anonymous. No customer field anywhere. Credit, if used, is a **free-text name + phone typed on the sale** (`credit_ref`), stored with that ticket only | No per-customer aging or statement. You can list *open credit tickets* by name, but not "total owed by Ahmed" reliably |
| **`credit_only`** | A lightweight **credit contact** is created *only* when the shop gives credit: nickname + phone, nothing else. No history browsing, no marketing use, no export | You get proper aging + one-tap WhatsApp statement (STRATEGY-001 ①) — the feature that gets debts paid |
| **`full`** | Real customer records with purchase history | Only if a shop asks. Never the default, never required |

**My recommendation:** ship `none` as default (respects your requirement literally), and let a shop switch to `credit_only` the first time it gives credit — because a credit book without a key is just a list of numbers. All three modes are schema-compatible (`sales.customer_id` nullable, `sale_lines` unaffected), so **switching is a setting, not a migration.**

**Nothing about clients leaves the machine** in any mode: ADR-003 sends aggregates only, and `credit_only` contacts are explicitly excluded from sync.

---

## 6. What this changes in earlier documents

| Document | Change |
|---|---|
| PLAN §5 | `sale_lines` gains `reference_price_millimes`, `price_source`, `price_delta_millimes`, `user_id` (already implied by cash sessions); `products.default_price` made **nullable**; `sales.customer_id` / `credit_ref` |
| PLAN §7 KPIs | `+ realized avg price`, `+ price dispersion`, `+ deviation vs reference`, `+ % ventes sous le coût` |
| ADR-005 §3.3 | unchanged — per-lot cost still the truth |
| ADR-006 §4.7 | instalments/credit now explicitly depend on customer mode |
| ADR-007 | unchanged, reinforced: price is a free field, not a validated one |
| ADR-008 §5.3 | bulk repricing now updates **reference** prices only — actual prices are always per sale |
| DESIGN-001 §5 | `+ PriceEntry` (numeric, chips, live margin), `+ PriceDispersionChart`, `+ DeviationTable` |

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| Owners love it, **accountants** dislike negotiated pricing on invoices | Record it truthfully anyway; invoices show the paid price. It's reality, not an invention |
| Cashiers resent the deviation report | Ship it as a *shop* metric first (price dispersion for the owner's own decisions), deviation-by-user second |
| Reports get noisy with per-sale variance | Report distributions, not point values; use medians and bands, not "the price" |
| Mode `none` + credit = messy receivables | Warn once at setup: "credit without a contact = you can't chase it". Then respect the choice |
| Multi-currency cash invites rate disputes | Freeze the rate per payment and print it on the receipt |
| Price floor with owner PIN gets bypassed by sharing the PIN | Log every PIN override with user + timestamp; audit list in the daily summary |

---

## 8. Effort & decisions

| Item | Days |
|---|---|
| Schema (`reference_price`, `price_delta`, `price_source`), nullable product price | 0.5 |
| POS price entry: keypad, chips, live margin, optional floor + PIN override | 1.5 |
| Price dispersion + realized-margin + below-cost reports | 1.0 |
| User deviation report + daily-summary flag | 1.0 |
| Customer modes (`none` / `credit_only` / `full`) + credit-ref path | 1.0 |
| *(optional)* multi-currency cash payments | 1.0 |
| **Total** | **5.0 d** (+1.0 if multi-currency is in v1) |

**Decisions:**
1. **Customer mode default** — `none` (literal "don't save clients") or `credit_only` (needed for a usable credit book)? *My pick: `none` default, with a one-click switch offered the first time credit is given.*
2. **Price floor** — warn only (recommended), or owner-PIN hard floor for cashiers?
3. **Multi-currency cash** (USD/EUR) in v1, or TND only?
4. **Deviation report** — build in v1, or only the shop-level dispersion report first? *(I'd ship both: the deviation report is the one that pays for the licence.)*
