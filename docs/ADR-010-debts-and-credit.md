# ADR-010 — Debt management (the carnet): sale debts, free debts, tranches, settlement

**Status:** ✅ **ACCEPTED** — `credit_only` default debtor mode, opening balances in v1, instalments in v1, overpayment blocked.
**Date:** 2026-10-20
**Relates to:** ADR-009 §5 (customer modes — **partially superseded here**), ADR-007 (record everything), ADR-003 (PII stays local), `PLAN.md` §5/§7

---

## 1. The model in one sentence

> **A debt is a numbered ledger with a principal and a stream of movements.** It can be born from a sale or from nothing at all, it can be paid in any number of tranches of any size, in any order, and it is **never edited** — only extended by new entries.

Everything else (tranches, partial payments, early settlement, write-off, aging) is a *view* over that one ledger, which is why this stays simple to build and impossible to get inconsistent.

---

## 2. Reconciliation with ADR-009 (§5 superseded)

ADR-009 said "no customer records by default". **A debt without a person is not collectable**, so debt management requires a minimal debtor record. The rules become:

| Mode | Debts allowed? | Data stored |
|---|---|---|
| **`credit_only`** ⭐ **new default** | ✅ yes | `name/nickname` + `phone` + optional note. **Nothing else** — no purchase history browsing, no marketing, no export, no cloud |
| `none` | ❌ no credit sales at all (cash only) | anonymous tickets |
| `full` | ✅ | real customer records |

- A `credit_only` record is created **the first time a debt is opened**, in one field-blur: `Nom ▸ Téléphone`. Never a customer form, never a profile.
- Debtor records are **local-only by hard rule**: ADR-003 sync exports aggregates (totals, aging buckets, counts) and **never names or phones**.
- Deleting a debtor with a non-zero balance is refused; archive only.

---

## 3. Schema

```sql
-- minimal debtor = "carnet page"
debtors(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,                 -- free text, nickname ok
  phone TEXT NULL,                    -- required if you want WhatsApp reminders
  note TEXT NULL,
  opened_day DATE NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  created_by TEXT NOT NULL
);

-- the debt itself (a header; the truth is the ledger)
debts(
  id TEXT PRIMARY KEY,
  debtor_id TEXT NOT NULL REFERENCES debtors(id),
  origin TEXT NOT NULL,               -- sale | manual | opening_balance
  sale_id TEXT NULL REFERENCES sales(id),
  category TEXT NULL,                 -- manual: cash_loan | old_balance | advance | other
  principal_millimes INTEGER NOT NULL,
  opened_day DATE NOT NULL,
  due_day DATE NULL,                  -- optional (lump-sum deadline)
  currency TEXT NOT NULL DEFAULT 'TND',
  fx_rate_micros INTEGER NULL,        -- if the debt was agreed in USD/EUR
  reason TEXT NULL,
  status TEXT NOT NULL,               -- open | settled | written_off | cancelled
  created_by TEXT NOT NULL,
  UNIQUE(sale_id)                     -- at most one debt per sale
);

-- append-only ledger: this is the money
debt_ledger(
  id TEXT PRIMARY KEY,
  debt_id TEXT NOT NULL REFERENCES debts(id),
  kind TEXT NOT NULL,                 -- open | payment | adjustment | write_off | cancel
  amount_millimes INTEGER NOT NULL,   -- signed: + increases what is owed, − reduces
  method TEXT NULL,                   -- cash | card | transfer | in_kind
  day DATE NOT NULL,                  -- business day
  at TEXT NOT NULL,                   -- exact timestamp
  cash_session_id TEXT NULL,          -- if method = cash → ties into drawer
  instalment_id TEXT NULL,
  note TEXT NULL,
  user_id TEXT NOT NULL,
  reverses_ledger_id TEXT NULL        -- corrections reference what they undo
);

-- optional schedule (tranches)
instalments(
  id TEXT PRIMARY KEY,
  debt_id TEXT NOT NULL REFERENCES debts(id),
  seq INTEGER NOT NULL,
  due_day DATE NOT NULL,
  amount_millimes INTEGER NOT NULL,
  state TEXT NOT NULL,                -- pending | partial | paid | waived
  UNIQUE(debt_id, seq)
);
```

**Balance** is never stored as an editable field. It is `SUM(debt_ledger.amount_millimes)` — cached in a `v_debt_balance` view/index for speed, recomputed on every write in the same transaction. `--doctor` verifies cache == sum.

---

## 4. The four ways a debt is born

### 4.1 From a sale, fully unpaid
`total 5,000` → `paid 0` → debt `5,000`, `origin=sale`, linked to `sales.id`. The receipt prints: `À crédit · reste 5,000.000 · carnet #D-2026-0142`.

### 4.2 From a sale, partially paid (the common case)
`total 5,000`, customer hands `3,000` cash + `2,000` on credit → sale records `paid 3,000`, debt opens at `2,000`. **Cash session sees only the 3,000** (correct for drawer reconciliation); the 2,000 is a receivable, not cash.

### 4.3 A free debt (no sale)
"Ahmed took 500 in cash", "old balance from the notebook", "advance to a cousin". Fields: debtor, amount, day, optional reason/due day, category. Recorded exactly like a sale debt, `origin=manual`. This is what makes the app replace the paper carnet on day one.

### 4.4 Opening balance import (onboarding) ⭐
The shop arrives with 30 pages of notebook. A dedicated screen:
```
Debtor            Phone          Amount       Since        Note
Ahmed Ben Ali     21 555 123     1,250.000    12/08/2026   écran cassé
Sami              98 222 456       400.000    03/09/2026   accessoires
                                   ─────────
                        total importé 1,650.000
                                     [ Ajouter au carnet ]
```
Bulk paste or CSV. Creates `debtors` + `debts(origin=opening_balance)` with one `open` ledger entry each. **Without this feature, no shop can switch mid-year** — it's the single most important onboarding step for this vertical.

---

## 5. Payments: tranches, partial, full, early

### 5.1 Any amount, any time
`[ Encaisser ]` on a debt → keypad → any amount from 1 millime to the balance. Tranches are **not enforced** as a rule; a schedule is a *plan*, not a constraint. The row shows:

```
D-2026-0142 · Ahmed Ben Ali · iPhone 13 128GB
principal 5,000.000   payé 2,400.000   reste 2,600.000
échéancier : 3 × 1,666.667  (1 payée · 1 partielle · 1 en retard depuis 6j)
                                    [ Encaisser ]  [ Détail ]  [ WhatsApp ]
```

### 5.2 Allocation rule (deterministic, always explainable)
A payment is applied in this order:
1. to the **oldest unpaid instalment** (if a schedule exists), then
2. any remainder against **principal** (unscheduled debt).

Partial instalment → state `partial`. Fully paid → `paid`. All paid → debt `settled`. **FIFO allocation, never "smart"**, because the customer must be able to reproduce the arithmetic on paper — that's what avoids arguments.

### 5.3 Full payment at once
Same screen, `[ Tout encaisser ]` pre-fills the remaining balance. One tap for a handshake "here's everything".

### 5.4 Early settlement with a discount (very common)
"Pay 4,500 now and we're square on 5,000."
- `[ Règlement anticipé ]` → enters the amount received (4,500) → the remaining 500 is posted as a **separate `adjustment` entry** with reason `settlement_discount`, so:
  - the ledger balance reaches 0 **honestly**,
  - the 500 appears as a real loss in P&L (it *is* one),
  - the original principal is never rewritten.
- Audited, and reported as `remises de règlement` per month.

### 5.5 Overpayment
Blocked by default with a clear message (`le reste est 2,600`). Optional setting: overflow becomes a **positive balance / avoir** for that debtor, shown in the carnet and consumable by a future sale. No silent negative debts.

### 5.6 Multi-currency payment
A debt agreed in USD can be paid in TND (or vice versa): `method + currency + fx_rate_micros`, rate **frozen at the payment**, both amounts stored and printed. Matches ADR-008/009 multi-currency cash.

### 5.7 Cash and the drawer
Any payment with `method = cash` **must** be attached to the open `cash_session` (ADR: cash sessions in PLAN §6). Consequences:
- expected-drawer math includes debt collections automatically;
- a session cannot be closed with unattributed cash collections;
- a payment inside a **closed** session is immutable — corrections are new entries dated in the current session, referencing the old one.

---

## 6. Effects elsewhere (the parts that must not be forgotten)

| Area | Rule |
|---|---|
| **Receipt** | credit sale prints `payé X / reste Y`, debt number, and a due date if scheduled |
| **Returns** | a return on a sale with an open debt first **reduces the debt** (ledger `adjustment`), then refunds cash for any excess. Never refund cash while debt remains — that's a classic cash-out leak |
| **Sale deletion/void** | forbidden if a debt exists and has payments; otherwise the debt is `cancelled` via ledger entry, never deleted |
| **P&L / revenue** | revenue is recognised at sale time (full amount), independent of collection. Collection is a **cash-flow** event, not a revenue event — two separate reports, never mixed |
| **Bad debt write-off** | `write_off` entry + reason → hits P&L in the `créances irrécouvrables` expense line. Recoverable later by a new `payment` entry (re-opening the balance), keeping history |
| **Credit limit** | optional per debtor (`max_open_millimes`); exceeding it → **warn only** (ADR-007: require nothing) |
| **No interest** | explicitly out of scope: no riba, no late fees, no compounding. Late debts are flagged, not charged |
| **Sync** | only aggregates leave the machine: `total_receivables`, aging buckets, `collected_today`, `open_debts_count`. **Names, phones, ledger entries stay local** |
| **Backup** | debts are in the same SQLite file — nightly backup covers them; `--doctor` verifies ledger integrity (sum == cache, no orphan ledger rows) |

---

## 7. Reports (local + dashboard)

| Report | Content |
|---|---|
| **Créances** (aging) | `0-30 / 31-60 / 61-90 / 90+` buckets, total, count, by debtor |
| **Carnet par personne** | printable / WhatsApp statement: every debt, every payment, running balance, dates, Arabic/French |
| **Encaissements** | collected per day/week/month, by method, in vs out of schedule |
| **Recouvrement** | % of scheduled tranches paid on time, avg days late, worst debtors (by amount × age) |
| **Pertes** | write-offs, settlement discounts, bad-debt ratio vs revenue |
| **Prévision de trésorerie** | expected collections next 7/30 days from due dates — the report a phone shop actually needs to buy stock |
| **Daily 21:00 summary** | `Encaissé 1,240 · Reste dû 8,900 · 3 en retard (+15j) · 2 échéances demain` |

**WhatsApp statement** follows STRATEGY-001 ①: a `wa.me` deep link with a pre-formatted message — no API, no cloud, one tap, works offline until you send.

---

## 8. Implementation order & effort

| Item | Days |
|---|---|
| Schema (`debtors`, `debts`, `debt_ledger`, `instalments`) + balance cache + `--doctor` invariants | 1.5 |
| Sale → debt integration (unpaid / partial), receipt lines, void rules | 1.0 |
| Carnet UI: debtor list, debt detail, ledger timeline, collect sheet | 2.0 |
| Tranches: schedule generator (N equal / custom / weekly-monthly), states, FIFO allocation | 1.5 |
| Settlement discount, overpay guard, multi-currency payment | 1.0 |
| Opening balance import (bulk paste + CSV) | 1.0 |
| Aging + carnet statements + WhatsApp/print | 1.0 |
| Cash-session integration + closed-session immutability | 0.5 |
| Write-off + P&L wiring + collection reports + dashboard tiles | 1.0 |
| **Total** | **10.5 d** |

**Placement:** this is core, not a nice-to-have — it goes in **M3 (finance core)**, which grows to ~20 d. It also unblocks STRATEGY-001 ① (the credit book) and the replacement of the paper carnet, which is your strongest demo.

---

## 9. Decisions

1. **Default debtor mode** — confirm `credit_only` (minimal name + phone) becomes the default, since debts require it. `none` would mean no credit sales at all.
2. **Receivables sync** — send only aggregates (total, aging buckets, counts) to the cloud dashboard, **never** names/phones. Confirm? *(Recommend yes — it gives the remote "who owes me" total without exposing identities.)*
3. **Opening balances in v1?** *(Strongly recommend yes — without it no shop can migrate mid-year.)*
4. **Instalment schedule** in v1, or v1.1? *(Recommend v1: tranches are the daily reality of this trade.)*
5. **Overpayment** — block (recommended) or keep as an `avoir` credit on the debtor?
