# DESIGN-002 — Simplicity doctrine, optional fields, the Bismillah masthead

**Status:** ✅ **ACCEPTED** — Bismillah default on for ar/fr, receipts at v1.1, sample data on first run.
**Date:** 2026-10-20
**Relates to:** DESIGN-001 (tokens, RTL), ADR-007 (record everything, require nothing), ADR-010 (debts)

---

## 1. The doctrine

> **Every screen has one job. Every field is optional unless the record is impossible without it. Every dinar is shown exactly, never rounded away.**

Complexity is not a cost you pay once — it's a tax on every sale, every day, forever. A cashier who hesitates for two seconds costs more than a feature ever earns. So simplicity is a **build gate**, not a taste.

---

## 2. Hard rules (enforced in review, not debated per screen)

| # | Rule | Why |
|---|---|---|
| **R1** | **One primary action per screen.** Exactly one green button. Everything else is secondary or hidden | Ambiguity is the slowest thing in a POS |
| **R2** | **Common sale path ≤ 2 inputs**: scan (or tap product) → `F9`. Price entry optional | Speed is the product |
| **R3** | **Max 7 visible fields per form**, everything else behind "Plus d'options" (progressive disclosure) | Working-memory limit; more fields = more skipping = worse data |
| **R4** | **No field is required** unless the record is meaningless without it (see §4) | ADR-007, applied to UI |
| **R5** | **Every field has a sensible default** — blank inputs are bugs | A cashier should be able to hold `Enter` |
| **R6** | **No nested modals, no wizards longer than 3 steps**, no "next → next → next" | Wizards are where work gets abandoned |
| **R7** | **No blocking validation.** Warn inline, allow the action, record the reason | Never stop a sale (ADR-007) |
| **R8** | **No empty-state dead ends.** Every "no data" screen offers the action that fixes it | First-run experience decides adoption |
| **R9** | **Keyboard-complete.** Every screen reachable without a mouse, hints shown inline | Scanner-first shops have no hands free |
| **R10** | **No animation over 120 ms**, none on the sale path | Perceived speed |
| **R11** | **Labels are short and localised**, never abbreviations invented by us | Arabic/French length differences |
| **R12** | **One concept, one word.** "Client", "carnet", "crédit", "ticket" — fixed glossary, no synonyms | Confusion is a support call |

**Screen budget:** a new screen is *rejected* if it exceeds R1/R3 without a written justification in the PR. That's how this stays true six months from now.

### 2.1 The five screens that carry the product
If these five are fast, the app wins. Everything else is secondary.

| Screen | Job | Target |
|---|---|---|
| **Sale** | scan → pay | ≤ 2 inputs, no scrolling |
| **Carnet** (debts) | see who owes, collect | open a debt ≤ 3 taps |
| **Delivery / receive** | stock in with cost | scan → qty → cost → Enter |
| **Dashboard** | "how did today go" in one glance | 1 screen, no scroll on 1080p |
| **Product** | create a sellable item | ≤ 5 fields, rest optional |

---

## 3. First-run experience (the highest-leverage UX work)

A shopkeeper judges the software in the first 5 minutes and never revisits the judgement.

```
1. Language        [العربية] [Français] [English]        ← one tap, big buttons
2. Shop name       [_______________]                      ← typing, optional
3. Device pairing  [ 4 8 2 1 9 3 ] or "Plus tard"         ← skippable, offline works
4. ─────────────────────────────────────────────
   [ Tester une vente maintenant ]   ← goes straight to a sample sale
   [ Importer mon stock (Excel) ]
2 — read a code-barres, decide later
```

Rules: **no account creation, no email verification, no tour overlay, no feature checklist.** Sample products are pre-loaded (deletable in one tap) so the very first action is a *successful sale*, not a configuration screen. The import wizard is available but never forced.

---

## 4. Optional fields — explicit list

| Field | Required? | Default |
|---|---|---|
| Product name | **yes** (only way to identify it) | — |
| Price | **no** — asked at sale (ADR-009) | empty |
| Barcode | no | generated if wanted |
| Category | no | "Divers" |
| Cost | no | from received lot, else 0 + a soft "marge inconnue" flag |
| Supplier | no | "—" |
| **Notes / description on anything** | **never** | empty |
| Customer name / phone | only when opening a debt (ADR-010) | empty |
| Due date, instalments | no | none (free-form payments) |
| Tax rate | no (profile-dependent, ADR-007) | off |
| Batch/expiry | no unless the product declares a shelf life | none |
| IMEI | **yes for serialized phones** (it *is* the item) | — |
| Reason / override note | no — appears only when a rule was crossed | — |
| Attachment / photo | no | none |

**Two rules for notes specifically:**
1. **Optional, always.** No "explain why" box on the happy path.
2. **Never lost when typed.** A note typed on a sale, a debt, a price change or a write-off is stored and searchable. `notes` is `TEXT NULL` everywhere, never rejected, never truncated by UI.

Exception that is *not* a field: **audit reasons**. When someone crosses a floor (below cost, expired batch, discount above X), the system records *that it happened* and *by whom* automatically. It does not interrogate the user.

---

## 5. "Care about every dinar" — money display rules

Precision is a trust feature. A shop that catches the software rounding 500 millimes will never trust its profit numbers.

| Rule | Detail |
|---|---|
| **Never round in storage or calculation** | `i64` millimes everywhere; no floats (PLAN §5) |
| **Always show 3 decimals on money** | `1,250.500 TND` — never `1 250` or `1 250.5` |
| **Never hide a rounding** | Cash rounding to 0.100 appears as an explicit line `Arrondi −0.020`, never silently absorbed |
| **Separate the currencies of the screen** | revenue / cash collected / debt / profit are four different numbers, never one "total" |
| **Show the delta, not just the value** | `1,300 (‑200 vs prix affiché)` — an amount without context is noise |
| **Tabular figures, right-aligned** | columns line up so errors are visible by eye |
| **Never abbreviate money** (`1.2k`) | except in sparkline tooltips, where it's labelled |
| **Ties are shown** | a debt's balance always reconciles to the ledger sum, and the UI can show the arithmetic (ADR-010 §5.2) |
| **Zero is written `0.000`**, not `—` | `—` means "not applicable", and the difference matters in a report |

---

## 6. The Bismillah masthead

> **بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ**

Requested by the shop owner audience, and it belongs to the product's tone: this is a money app in a Muslim market, and the shopkeeper wants his till to open with it. It is a **greeting, not decoration** — so it gets the best typography in the app and the most restrained placement.

### 6.1 Placement (exactly four places, never more)

| Where | Size | Notes |
|---|---|---|
| **Dashboard (local + web)** | **large**, centred band above the KPI cards | the one requested; ~`clamp(22px, 3.2vw, 38px)` |
| **Login / pairing screen** | large, centred, same treatment | first impression |
| **Printed receipt** | small, top centre | optional per shop; needs Arabic-capable printing (DESIGN-001 §4.5 item 6) |
| **Carnet / invoices / statements (print)** | small, top centre | optional |

**Never** on: the POS sale grid (space and focus), dialogs, error messages, buttons, or as a loading spinner. It is not a brand lockup and must never compete with the header.

### 6.2 Typography & rendering
- **Font: Amiri** (OFL, naskh calligraphic) for the Bismillah only — *not* Cairo, which is a UI sans and would flatten it. Subset to the phrase's glyphs → **~15–25 KB** WOFF2, embedded in the binary (offline, no CDN).
- Full tashkeel (diacritics) rendered exactly: `بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ`. Amiri handles the stacked marks; a sans font will collide them.
- `line-height: 1.9`, `letter-spacing: 0`, `font-feature-settings: "liga" 1, "calt" 1`, `text-rendering: optimizeLegibility`.
- `dir="rtl"` + `<bdi>`, never concatenated into a Latin string.
- Colour: `--brand-green-deep` in light, `--brand-green` at reduced opacity in dark — **muted**, never shouting. No gold, no gradients, no glow.
- Never pseudo-bold, never italic (Arabic has no italic), never letter-spaced, never uppercase-transformed.
- Do not rasterise or screenshot it into an image: text stays selectable, searchable and accessible.

### 6.3 Behaviour
- **Toggle in settings** (`settings.show_bismillah`, default **on** for Arabic/French, off for English UI by default — the shop can flip it either way). Respecting it as optional is part of respecting it.
- On screen it is **static**: no animation, no fade, no parallax.
- `prefers-reduced-motion` irrelevant (nothing moves), but `aria-hidden` is **wrong** — it's meaningful text, so it gets an accessible label and no ARIA tricks.
- Same component in the Vercel dashboard and in print CSS (a single `BismillahBand` used by both, so the product feels like one thing).

### 6.4 Copy accuracy matters
Get the text approved by a native speaker once and store it as a single constant. Misspelling or missing diacritics on a religious phrase is the fastest way to lose a shop's trust — this is the one string in the app that cannot be "good enough".

---

## 7. Design review gate (one page, used on every screen)

Before a screen ships, answer **yes** to all:

- [ ] One primary action, visually dominant?
- [ ] ≤ 7 visible fields, rest behind "Plus d'options"?
- [ ] Works with keyboard only, hints visible?
- [ ] Every field optional except the identifying minimum, and defaulted?
- [ ] No nested modal, ≤ 3 steps if it's a wizard?
- [ ] Empty state offers the action that fixes it?
- [ ] Money shown with 3 decimals, tabular, aligned, deltas visible?
- [ ] Warns instead of blocking when a rule is crossed?
- [ ] RTL-safe (logical properties, no hardcoded left/right) and readable in dark mode?
- [ ] No label wider than its container in Arabic?

Ten checkboxes. If it fails one, it isn't done.

---

## 8. Decisions

1. **Bismillah default** — on for Arabic/French installs, off for English? *(Recommend yes, with a toggle.)* Or always on?
2. **Bismillah on receipts in v1** — needs the Arabic raster-print path (DESIGN-001 §4.5, +3 d) or wait for v1.1?
3. **Sample data on first run** — pre-load ~8 demo products so the first action is a successful sale? *(Recommend yes, deletable in one tap.)*
4. Confirm **notes never required anywhere**, including deliveries, debts and write-offs.

---

## 9. Effort

| Item | Days |
|---|---|
| `BismillahBand` component + Amiri subset + placement on dashboard/login/print + toggle | 0.5 |
| First-run flow (4 steps, skippable, sample data) | 1.0 |
| Money-format utilities (`3 decimals, tabular, delta`) shared by all screens | 0.5 |
| Design-review checklist applied to existing screens (M1–M3 pass) | 1.0 |
| "Plus d'options" progressive-disclosure pattern on forms | 0.5 |
| **Total** | **3.5 d** |
