# ADR-007 — Recording & compliance modes (capture everything, require nothing)

**Status:** ✅ **ACCEPTED** — record everything, require nothing; default profile **A (Simple)**.
**Date:** 2026-10-20
**Supersedes:** the framing in ADR-006 §1
**Relates to:** `PLAN.md` §5/§7, ADR-005, STRATEGY-001

---

## 1. The principle

> **Record every dinar, every unit, every detail — and never demand a paper the shop doesn't have.**

Two independent axes, previously conflated:

| Axis | Decision |
|---|---|
| **How much is recorded** | **Maximum.** Line-level cost, per-line discount, payment split, cash session, credit ledger, IMEI, warranty, supplier, per-delivery costs, expenses, every profit figure. Nothing is summarised away, nothing is editable after the fact. |
| **How much is *required*** | **The shop's choice.** No mandatory TVA rate, no mandatory supplier invoice number, no mandatory tax ID, no mandatory e-invoice. Those are switch-on features, not gates. |

The app is a **complete record of the business**, not a tax form with a cart attached. Full recording is also your strongest commercial argument — *"every dinar is traceable, and you can prove your margins to a bank"* — and it costs nothing to build: the schema stores everything anyway.

**One thing that stays out**, unchanged from before: no tool whose purpose is to delete, hide, or falsify a recorded sale. Total capture makes that a non-issue — there's simply nothing to conceal because nothing is optional to *record*.

---

## 2. Profiles (chosen once at setup, changeable later)

| Profile | Who | Receipt | Tax fields | E-invoice |
|---|---|---|---|---|
| **A. Simple** *(default)* | unregistered / small shop | thermal ticket, no TVA line | **off** — price is the price | none |
| **B. Forfaitaire** | flat-regime registered | ticket + shop NIF | single rate, no per-line split | none |
| **C. Réel simple** | VAT-registered, receipt-based | ticket + HT/TVA/TTC | **per-product rate** (19/13/7/0) | manual |
| **D. Réel + TEIF** | full compliance | + TEIF XML archive | per-product + signature | **on** (El Fatoora/TTN) |

**Default is A.** A shop turns on more only when *it* wants to. No nag screens, no "you should enable TVA" prompts, no forced wizard after the 10th sale.

### 2.1 Setup wizard (30 seconds, 3 questions)
```
1. Êtes-vous assujetti à la TVA ?        ( ) Non  ( ) Oui, taux unique  ( ) Oui, par produit
2. Émettez-vous des factures électroniques TEIF ?   ( ) Non  ( ) Oui
3. Avez-vous un NIF à imprimer sur le ticket ?      ( ) Non  ( ) Oui, saisir
```
Everything else defaults off. Skippable entirely with one "Plus tard".

---

## 3. Field matrix — captured vs required

`M` = mandatory · `O` = optional, shown if enabled · `—` = hidden in that profile (**still stored if it arrives**)

| Field | A Simple | B Forfait | C Réel | D TEIF |
|---|---|---|---|---|
| Product, qty, unit price, line total | M | M | M | M |
| Line discount / global discount | O | O | O | O |
| **Unit cost at time of sale** | **M** (silent — needed for margin) | M | M | M |
| Payment method + split | M | M | M | M |
| Customer (name/phone) on credit sales | M if credit | M if credit | M if credit | M if credit |
| **IMEI / serial** (phone products) | **M** | M | M | M |
| Supplier + delivery reference | O | O | O | O (TEIF needs it) |
| **TVA rate per line** | — | single rate | **M** | **M** |
| HT/TTC toggle on receipt | — | — | O | M |
| Shop NIF on ticket | — | M | M | M |
| Client NIF / matricule | — | — | O | M (B2B) |
| Supplier invoice number | — | — | O | M |
| TEIF XML + signature | — | — | — | M |
| Fiscal journal entry | — | — | O | M |
| Cash session / drawer variance | O | O | O | O |
| Notes, photos (trade-in) | O | O | O | O |

**Annotations:**
- *Unit cost is always captured*, never printed, never asked of the cashier — it's derived from the received lot. Margin reports therefore work in **every** profile, including A. This is the feature shops actually want.
- A field hidden by profile is **not** a field rejected: if a supplier ref or a rate arrives (import, API, manual), it is stored. Turning a profile up later reuses the history instead of losing it.
- Profile change upward applies **forward** (new sales use the new rules). Historical sales keep exactly what was stored — no retroactive rewriting, so past margins and past tickets never change.

---

## 4. Consequences in code (small, contained)

| Area | Rule |
|---|---|
| `settings` | `+ compliance_profile`, `+ default_tax_rate`, `+ shop_nif`, `+ show_ht_ttc`, `+ teif_enabled`, `+ print_nif` |
| `sale_lines.tax_rate` | **nullable** — `NULL` = no tax (profile A). Never default to 19 silently |
| Money math | `total = qty × price − discount`; only profiles C/D compute `ht = total / (1 + rate)` for display. A single `fn line_totals(line)` handles both, one code path |
| Reports | Margin, P&L, cash flow, stock value all computed from `total` + `unit_cost` → **identical across profiles**; VAT appears as an extra breakdown row only when rates exist |
| Receipt templates | 4 templates selected by profile, same data model. IMEI, warranty, credit balance still printed in profile A — *more* detail than a legal invoice, not less |
| Daily closing (Z) | Profile-independent: cash in/out, drawer variance, per-method totals, credit taken |
| TEIF module | Compiled in, **inert** unless profile D. No network calls, no UI, no dependency on it for selling |
| `--doctor` | Warns if profile C/D is selected but rates are missing — never blocks a sale |

**Storage invariant:** every sale row is written **once**, completely, in one transaction, and is immutable afterwards (reversal, not edit). That's what makes the record trustworthy in profile A as much as D.

---

## 5. Why this is the right call commercially

1. **Bigger market.** Profile A opens every shop in the country, registered or not.
2. **No dead end.** A shop that later registers flips to C/D without re-entering history — you keep the customer through their formalisation, which is exactly when they'd otherwise switch software.
3. **Better than the competitors on both ends.** Cloud POS tools assume registered shops; accounting packages assume full formal books. Nobody serves "records everything, requires nothing".
4. **Your margin reports work from day one** in every profile — the report that proves the software earns money.
5. **No legal exposure:** nothing deleted, nothing falsified, nothing hidden — and the compliance module is there the day they need it.

---

## 6. Effort

| Item | Days |
|---|---|
| `compliance_profile` + settings UI + 3-question wizard | 1.0 |
| Nullable tax rates + unified `line_totals` + report adjustments | 1.5 |
| 4 receipt templates (shared engine) | 1.5 |
| TEIF module gating / inert-mode tests | 0.5 |
| Profile-switch migration tests (A→C, C→D historical integrity) | 1.0 |
| **Total** | **5.5 d** (largely replaces parts already budgeted for receipts/TEIF UI) |

**Net add to v1: ~+3 days.** v1 ≈ **55 dev-days** with the phone vertical and the §9 cut list applied.

---

## 7. Open items
1. Confirm default profile **A (Simple)** for new installs.
2. Confirm receipt templates needed in v1: A + B only (recommended), C/D at v1.1 with TEIF.
3. NIF/RC fields: print on ticket in profile B from v1, or defer? *(recommend print from v1 — it's one line of config)*
