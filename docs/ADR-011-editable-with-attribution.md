# ADR-011 — Everything editable, everything attributed (the audit rule)

**Status:** ✅ **ACCEPTED** — everything editable, everything attributed; supersedes "immutable financial facts".
**Date:** 2026-10-20
**Relates to:** PLAN §5/§7, ADR-007 (record everything), ADR-010 (debts), DESIGN-002 (simplicity)

---

## 1. The rule

> **Everything is editable. Nothing is anonymous.**
> Who edited what, when. Who sold what, when. Who did what, when. Always.

Two obligations that must be built together:

| Obligation | Means |
|---|---|
| **Editability** | Any field on any record can be corrected — a wrong price, a wrong IMEI, a wrong quantity, a wrong debtor, a wrong date. Mistakes happen daily in a cash shop; a system that refuses corrections gets worked around with paper — which is exactly what we're replacing |
| **Attribution** | Every write is stamped with **actor + timestamp + device + what changed (before → after)**. Edits never erase; they add a version. Deletes are soft and audited |

I previously designed for immutability ("reverse, never edit"). Your rule is better for this market, and it's implementable without losing trust — provided history is a **first-class screen**, not a hidden table. It is.

---

## 2. The honest trade-off (and how it's contained)

Editable history has one real cost: **yesterday's reported number can change today.** If an owner edits a sale from last week, last week's profit was "wrong" — legally and psychologically uncomfortable.

Contained with three mechanisms, none of which block edits:

| Mechanism | Behaviour |
|---|---|
| **`as_reported` snapshots** | Each **daily closing (Z)** stores an immutable snapshot of every figure shown that day. It is never recalculated |
| **Post-close variance report** | Any edit to a closed day produces a visible line: `Journée du 12/10 clôturée à 4,820.000 — aujourd'hui 4,690.000 (écart −130.000, modifié par Nour le 15/10 14:22)`. The shop always sees *what changed after the fact*, and why |
| **Version, not overwrite** | The original value is always one click away in the history drawer, with the author's name on the change |

So: **the record is editable, the truth is traceable, and no number silently moves.**

---

## 3. Schema

### 3.1 Universal revision log (append-only, never edited, never deleted)
```sql
audit_events(
  id            TEXT PRIMARY KEY,          -- uuidv7 (time-sortable)
  at_utc        TEXT NOT NULL,             -- ISO-8601
  business_day  DATE NOT NULL,             -- Africa/Tunis
  actor_user_id TEXT NOT NULL,             -- who (never null; system actions use 'system')
  actor_role    TEXT NOT NULL,             -- owner | manager | cashier | system
  device_id     TEXT NOT NULL,
  session_id    TEXT NULL,                 -- cash session, links to drawer
  kind          TEXT NOT NULL,             -- create|update|delete|restore|sale|payment|login|export|login_fail|print|settings
  entity        TEXT NOT NULL,             -- sale | sale_line | product | batch | unit | debt | payment | debtor | expense | ...
  entity_id     TEXT NOT NULL,
  parent_id     TEXT NULL,                 -- e.g. sale_id for a line edit → easy timeline queries
  before_json   TEXT NULL,                 -- full prior row state (only changed fields + keys)
  after_json    TEXT NULL,
  diff_fields   TEXT NULL,                 -- 'price,total,qty' — cheap filtering/indexing
  reason        TEXT NULL,                 -- optional, never required (ADR-007)
  ip            TEXT NULL,                 -- localhost or LAN
  hash          TEXT NOT NULL,             -- sha256(prev_hash || this row canonical) → tamper-evident chain
  prev_hash     TEXT NULL
);
CREATE INDEX ix_audit_entity  ON audit_events(entity, entity_id, at_utc DESC);
CREATE INDEX ix_audit_actor   ON audit_events(actor_user_id, at_utc DESC);
CREATE INDEX ix_audit_day     ON audit_events(business_day, kind);
CREATE INDEX ix_audit_parent  ON audit_events(parent_id, at_utc DESC);
```

**Every table that a human can edit** carries `version INTEGER NOT NULL DEFAULT 1`, `updated_at`, `updated_by`, and soft-delete columns (`deleted_at`, `deleted_by`, `delete_reason`). That includes `sales`, `sale_lines`, `products`, `batches`, `units`, `debts`, `debt_ledger`, `expenses`, `debtors`, `purchases`.

### 3.2 Hash chain (tamper-evidence)
`hash = sha256(prev_hash || canonical_json(row))` — a broken chain proves history was rewritten outside the app.
**Honest limit:** the shop owns the machine and the code is theirs, so a determined person can recompute the chain. It protects against *quiet* edits (cashier or employee poking the DB), not against the owner with a hex editor. Say so in the docs; don't oversell it.

### 3.3 Written by one function, never by hand
```rust
// single choke point — every mutation goes through it
fn mutate(ctx, entity, id, changes, reason) -> Result<Row> {
    read_before(); validate(); apply();            // in ONE transaction
    insert audit_event(before, after, diff, actor, device, session);
    bump version; write stock/cash/ledger side-effects;
    update hash chain; commit;
}
```
No handler writes to a business table directly. Enforced by a lint/review rule + a test that greps the codebase for raw `UPDATE sales` outside the mutation module.

---

## 4. What "editable" means, per entity

| Entity | Editable? | Side-effects handled automatically |
|---|---|---|
| **Sale** | ✅ header (date*, customer, note), lines, prices, quantities, payment split | stock re-applied (reverse + re-apply), lot/unit re-consumption, cash session corrected, debts recalculated, Z variance logged |
| **Sale line (serialized)** | ✅ swap unit/IMEI, cost, price | old unit → `returned`, new unit → `sold`, both events attributed |
| **Product** | ✅ everything, incl. price/cost reference | price change → `price_history` entry, same audit event |
| **Batch / delivery** | ✅ cost, FX, charges, expiry, qty | landed cost recomputed; **warn** if units already sold from it (retro cost impact shown) |
| **Unit / IMEI** | ✅ | duplicate-IMEI check re-run, warranty recomputed |
| **Debt / payment** | ✅ amount, date, method, debtor | balance recomputed from ledger, cash session corrected, aging recalculated |
| **Expense** | ✅ | P&L recalculated, variance flagged if in a closed day |
| **Cash session** | ✅ only while open; closed sessions get *adjustments*, not edits | adjustment attributed + linked to the original session |
| **User account** | ✅ name, role, PIN reset | role change is always audited; PIN never logged (only "changed") |
| **Delete** | ✅ soft delete everywhere | row hidden, effects reversed, `delete` event with reason; restore = `restore` event |
| **Audit log itself** | ❌ **never** | append-only; no UI, no API, no export path can modify it |

\* Editing a sale's **date** moves it to another business day: both days' aggregates are marked dirty for sync (ADR-003) and both appear in the variance report. Allowed, attributed.

**Irreversible actions** (delete a sale, cancel a debt with payments) can optionally require an **owner PIN** (`settings.pin_for_destructive`, default on). The PIN is never stored in the event — only that it was used.

---

## 5. The two screens this rule requires

### 5.1 History drawer — per record
Available from **every** record (sale, product, debt, expense, unit, batch). One tap:

```
Historique — Vente #V-1042 · iPhone 13 128GB
─────────────────────────────────────────────────────────────
20/10 14:22  Nour (owner)      Modifié   prix 1,500 → 1,420
                                       raison : remise client
                                       impact : total −80.000 · marge 31% → 26%
20/10 11:05  Sami (caissier)   Créée     vente de 1,500.000 · 1 unité · L2
─────────────────────────────────────────────────────────────
                                        [ Voir la vente ] [ Exporter ]
```
Each entry shows: **who, when, action, field-level before → after, and the money impact**. No technical jargon, no JSON on screen (raw JSON behind "Détails techniques" for support).

### 5.2 Journal d'activité — the global screen ("who did what when")
Owner-only. Filters: **user · date range · action kind · entity · amount range · device**. Presets: *Aujourd'hui*, *Ce caissier*, *Modifications seulement*, *Suppressions*, *Après clôture*, *Prix changés*, *Connexions*.

Answers the four questions directly:
- **Who sold what when** → filter `kind=sale`, grouped by seller with totals per seller per day
- **Who edited what when** → `kind=update`, with diffs
- **Who did what when** → everything, chronological
- **What changed after closing** → `post-close` preset (the variance list)

Export: CSV/XLSX for the owner's own records; **print-friendly page** for a dispute. Local only, never uploaded (ADR-003).

---

## 6. Reports that get new columns (because attribution now exists)

| Report | New |
|---|---|
| Sales by seller | `sales count · revenue · avg price vs shop avg · % below reference · deviation` (ADR-009 §4.3) |
| Edits by user | `count · by type · by amount affected` — surfaces a cashier who keeps "correcting" the same kind of record |
| Post-close variance | per closed day: original vs current, delta, author of each change |
| Deleted records | what, when, by whom, why (if given), restored? |
| Price changes | who changed which price, old → new, when (already `price_history`, now attributed) |
| Discounts granted | per seller, per day, total given away |

**This is the real product of your rule:** a phone shop can now answer "why is the cash 300 short" and "who sold the last three phones at 15% below list" **without accusing anyone by hand**. That's worth more than the reports I had before.

---

## 7. Consequences & risks

| Risk | Handling |
|---|---|
| **Reports drift from previously reported numbers** | `as_reported` snapshots + post-close variance (§2). Never silent |
| **Edit storm** (a cashier "fixing" data to hide something) | Every edit attributed; `edits by user` report; optional PIN on destructive ops |
| **Audit log grows** | ~1–3 KB/event; a busy shop ≈ 200–800 events/day ≈ 60–300 MB/year. Trivial; add monthly partitioning by `business_day` and an archive-to-file option after 3 years |
| **Performance** | Audit insert is one row in the same transaction; JSON is ~1 KB. Measured budget: sale + audit ≤ 20 ms total (from 15 ms) |
| **Someone edits the DB directly** | Hash chain flags it on next launch (`--doctor` + banner). Not bulletproof — documented as such |
| **Privilege: cashier editing own sales** | `settings.cashier_edit_scope`: `own_today` (default) · `own_any` · `none`. Owner always full |
| **Legal/fiscal view** | Edited records show their full history, which is *more* defensible in a control than a "clean" book. Nothing is hidden — that was the ADR-007 rule too |
| **Sync** | Audit events stay **local**. Cloud gets aggregates + a `last_edit_count`/`post_close_variance_total` figure for the dashboard tile — no names, no diffs |

---

## 8. Effort

| Item | Days |
|---|---|
| `audit_events` table + hash chain + `mutate()` choke point + migration of existing writes | 2.0 |
| Version/soft-delete columns on all editable tables + `--doctor` chain/integrity checks | 1.0 |
| Side-effect engine for sale edits (stock, lots, units, cash, debts) + recalc tests | 2.0 |
| History drawer (timeline + field diffs + money impact) | 1.5 |
| Journal d'activité screen (filters, presets, seller totals, export, print) | 1.5 |
| `as_reported` snapshots at closing + post-close variance report | 1.0 |
| Permission scopes (`cashier_edit_scope`, PIN for destructive) + audit of role changes | 0.5 |
| Report columns (edits by user, deletions, discounts by seller) | 0.5 |
| **Total** | **10 d** |

**Placement:** the audit core (`mutate()`, `audit_events`, versions) must land in **M0** — retrofitting it after 30 tables exist is 3× the work and loses history. The drawer/journal screens land in **M4**.

---

## 9. Decisions

1. **Cashier edit scope** — default `own_today` (can fix today's own sales), `own_any`, or `none`?
2. **PIN for destructive actions** (delete sale, cancel debt) — required by default, or off?
3. **Reason field on edits** — always optional (my recommendation, per ADR-007) or mandatory for edits to already-closed days?
4. **Audit retention** — keep forever (recommended, it's their proof), or archive rows older than 3 years to a local file?
5. Confirm the **post-close variance** approach (snapshot + delta list) rather than forbidding edits to closed days.
