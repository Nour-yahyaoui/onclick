# DESIGN-001 — Visual system, dark mode, Arabic / RTL

**Status:** ✅ **ACCEPTED** — system-default theme with a toggle, Western digits, `fr`/`en` live with reviewed `ar` at M7, Arabic receipts deferred.
**Reference:** `docs/assets/reference-simple-pos.png` ("Simple POS")
**Date:** 2026-10-20

---

## 1. What we keep from the reference

Its identity is strong and cheap to reproduce: **green header + black category rail + white product cards + a red total bar + two big save buttons.** A cashier reads that layout in one glance, which is worth more than novelty.

**Layout skeleton (unchanged):**

```
┌──────────────────────────────────────────────────────────────┐
│  HEADER  (green)                    [ New Invoice ] [ ⚙ ]     │
├───────────────────────────────────────────────┬──────────────┤
│  SEARCH / SCAN FIELD                          │ Invoice No.  │
├────────────┬──────────────────────────────────┤  ───────────  │
│ CATEGORY   │  PRODUCT GRID (cards)            │  CART LINES  │
│ RAIL       │  [img] Code / Name / Price       │  − 1 +  Price│
│ (black)    │                                  │  ───────────  │
│            │                                  │              │
│ ─ Add cat  │                                  │  SubTotal    │
│ ─ Add item │                                  │  TVA         │
│ ─ History  │                                  │  TOTAL (red) │
│            │                                  │ [Save][Print]│
└────────────┴──────────────────────────────────┴──────────────┘
```

## 2. What we must fix (it looks good but fails in a real shop)

| # | Issue in the reference | Fix |
|---|---|---|
| 1 | White text on `#41B14B` = **2.76:1** (fails AA) | Keep the hue, use **near-black text on green** (6.97:1) — see §3 |
| 2 | Red TOTAL bar `#FE2914` + white = **3.79:1** | Deepen to `#C62212` (5.77:1) or keep bright red with black text |
| 3 | Grey buttons `#BBBBBB` + white = **1.92:1** (unreadable) | `#6B7280` → 4.83:1, with a clear border |
| 4 | No visible focus ring; mouse-first | 2px focus ring + **full keyboard/shortcut model** (§6) |
| 5 | Tap targets ~28 px | **44 px minimum** (touch + scanner workflow) |
| 6 | Prices not aligned; no tabular figures | Right-aligned `font-variant-numeric: tabular-nums` money column |
| 7 | `$` currency, CGST/IGST tax | **TND, 3 decimals (millimes)**, **TVA 19/13/7%**, HT/TTC toggle |
| 8 | Cart line has no expiry/stock context | Batch chip (`exp 03/26`), stock badge, warning states |
| 9 | No "unknown barcode" path | Inline quick-create (§ ADR-005) |
| 10 | Fixed pixel layout | Responsive: cart → bottom sheet under 1200 px; rail → icons-only under 900 px |

Nothing here fights the style — it's the same design, made legible and operable.

---

## 3. Design tokens (one source of truth, two themes)

Single CSS variable set consumed by **both** the local app and the Vercel dashboard, so they look like one product. Light is the reference identity; dark is a real second theme, not an inverted filter.

### 3.1 Raw palette
| Token | Light | Dark | Note |
|---|---|---|---|
| `--brand-green` | `#41B14B` | `#22B866` | from the reference; brightened in dark for legibility |
| `--brand-green-deep` | `#1E8E3E` | `#17A65A` | hover/pressed |
| `--brand-red` | `#FE2914` | `#FF4B36` | TOTAL bar accent |
| `--brand-red-deep` | `#C62212` | `#D93A24` | used when white text sits on it |
| `--brand-blue` | `#1B6EF3` | `#4C8DFF` | Save & Print |
| `--chrome-black` | `#000000` | `#0A0D11` | category rail |
| `--neutral-100…900` | `#F1F1F1 → #12161B` | inverted per §3.2 | greys |

### 3.2 Semantic tokens
| Semantic | Light | Dark |
|---|---|---|
| `--bg` | `#F1F1F1` | `#0E1116` |
| `--surface` | `#FFFFFF` | `#161B22` |
| `--surface-2` (cards, table stripes) | `#FAFAFA` | `#1C232C` |
| `--rail-bg` / `--rail-fg` | `#000000` / `#F2F4F6` | `#0A0D11` / `#E6EAEF` |
| `--text` | `#12161B` | `#E9EDF2` |
| `--text-muted` | `#5B6672` (5.85:1) | `#9AA5B1` |
| `--border` | `#E4E4E4` | `#2A323D` |
| `--focus-ring` | `#1B6EF3` + 2px offset | `#6EA8FF` + 2px offset |
| `--primary-bg` / `--primary-fg` | `#41B14B` / `#0B0F14` | `#22B866` / `#08130C` |
| `--danger-bg` / `--danger-fg` | `#C62212` / `#FFF` | `#D93A24` / `#FFF` |
| `--money-bg` (TOTAL row) | `#FE2914` + `#0B0F14` text | `#FF4B36` + `#0B0F14` text |
| `--shadow-e1` | `0 1px 2px rgb(0 0 0 / .06)` | none (use border instead) |

**Contrast discipline:** body text ≥ 4.5:1, UI/large ≥ 3:1, **the cart's money column ≥ 7:1 in both themes** — a cashier reads totals at arm's length on a cheap panel. Verified ratios: green+dark-text 6.97:1 · deep red+white 5.77:1 · blue+white 4.59:1 · slate+white 4.83:1 · dark-green+dark-text 7.32:1.

### 3.3 Dark-mode rules (not just colours)
- Backgrounds step `#0E1116 → #161B22 → #1C232C`; **elevation = lighter surface**, never a shadow.
- Never pure `#000` for large areas (smearing on TN/VA panels); the rail is the only near-black element.
- Product thumbnails are **not** inverted or dimmed; add a 1px `--border` ring so white-background photos don't glow.
- Charts get a separate categorical palette (7 colours) validated against both themes — not the brand green repeated.
- Green buttons take **dark text** in dark mode (`#22B866` + `#08130C` = 7.32:1); white-on-green would be 2.59:1.
- Theme selection: `system | light | dark`, stored in the SQLite `settings` table (per device) so the till reopens in the right theme; `prefers-color-scheme` only as the initial default.

---

## 4. Arabic + RTL

### 4.1 Two separate problems (often confused)
1. **Translation** — finding the words.
2. **Bidi layout** — the whole layout must mirror, and mixed Arabic/Latin/numeric strings must not scramble.

Both are designed in now; **the CSS cost is ~0 if done from day 1 and 2–3 days if retrofitted.** This is the single most important sequencing note in this document.

### 4.2 Language set
`ar` (RTL) · `fr` (LTR) · `en` (LTR) — all three are needed in Tunisia; product names, supplier names and receipt text are mixed by nature. Catalogs are JSON, embedded in the binary, no network, no CDN:

```
locales/ar.json · fr.json · en.json      // namespaced: pos.cart.total, inv.batch.expiry
```
Missing-key policy: fall back `ar → fr → en`, log the missing key, never render a raw key to a cashier.

### 4.3 Direction mechanics
- `<html lang="ar" dir="rtl">` set from the active locale at boot.
- **CSS logical properties only** — `margin-inline-start`, `padding-inline`, `inset-inline-end`, `border-start-start-radius`. **No `left`/`right`/`margin-left` anywhere in app CSS** (enforce with stylelint rule `csstools/use-logical`). Flex/grid then mirror automatically; the category rail jumps to the right and the cart panel to the left with zero extra CSS.
- Icons that encode direction (back arrow, chevron) get `transform: scaleX(-1)` in RTL only; the logo, gear, trash and +/- stay unmirrored. The +/- stepper order *does* mirror (− then + reads naturally).
- Barcode/search input is **always `dir="ltr"`** regardless of locale (barcodes are Latin alphanumerics).
- **Numbers and money stay LTR**: every money cell, quantity, date and code gets `dir="ltr"` + `unicode-bidi: isolate` so `1,250.500 TND` never becomes `TND 500.250,1`. Mixed product names use `<bdi>` to isolate Latin fragments inside Arabic strings.

### 4.4 Arabic typography
| Concern | Decision |
|---|---|
| Font | **Cairo** (UI, OFL) with **Noto Naskh Arabic** fallback for receipts; Latin pairs with Inter. **Amiri** (OFL, naskh) for the Bismillah masthead only — see `DESIGN-002` §6 |
| Delivery | **Self-hosted WOFF2 subset embedded in the exe** (no Google Fonts — the app is offline). Arabic subset ≈ 60–110 KB, Latin ≈ 20 KB. Subset with `glyphhanger`/`fonttools` from the actual catalogs |
| Metrics | Arabic needs more line-height (`1.7`) and no letter-spacing; **never `text-transform: uppercase`** (destroys Arabic); no italics for emphasis — use weight |
| Width | Arabic strings run **15–30% longer** than English → all buttons/labels flexible width, sidebar labels allowed 2 lines, no fixed `width` on action buttons |
| Numerals | **Western digits (0–9) by default** (standard in Tunisian commerce); optional Arabic-Indic `٠١٢٣` as a setting |
| Formatting | `Intl.NumberFormat` / `DateTimeFormat` per locale, currency `TND` (3 decimals), dates `dd/MM/yyyy`, day boundary `Africa/Tunis` |
| Alignment | Text alignment follows direction; **numbers always right-aligned in the money column in LTR mode, left-aligned in RTL mode** (logical `text-align: end`) |

### 4.5 Things that silently break in RTL — watch list
1. Hardcoded `left`/`right` in CSS (the classic; prevented by the lint rule).
2. `flex-direction: row-reverse` hacks fighting the mirrored flow.
3. Chart axes: keep **time flowing left→right always**; only labels translate. Arabic financial charts read LTR for time.
4. Percentage/discount `-10%` punching direction; use `bdi`.
5. Numeric input fields in RTL: set `dir="ltr"` on all quantity/price inputs or keystrokes appear reversed.
6. **Receipts**: thermal printers do not shape Arabic. Two options — (a) printer with an Arabic code page (rare on cheap 58 mm units), (b) **render the receipt to a bitmap and print as raster (ESC/POS `GS v 0`)** — heavier but universal. Decide in M8; do not promise Arabic receipts in v1 (see §9).
7. Pseudo-localisation test with `عربي عربي` + long German-style strings to expose overflow before shipping.

---

## 5. Component inventory (shared by till and dashboard)

**Implementation rule (`ADR-014`): every component is a Rust function returning `maud::Markup` in a `.rs` file — no template files.** e.g. `fn money(m: i64) -> Markup`, `fn product_card(p: &Product) -> Markup`, `fn bismillah_band() -> Markup`.

`AppShell` · **`BismillahBand`** (large Arabic masthead on dashboard/login/print — `DESIGN-002` §6) · `HeaderBar` (brand, invoice counter, new-invoice, settings) · `ScanField` (barcode capture + text search in one box) · `CategoryRail` (sticky, count badges) · `ProductCard` (thumb, code, name, price, stock chip, expiry chip, `out-of-stock` state) · `ProductGrid` (virtualised > 200 cards) · `CartPanel` · `CartLine` (thumb, name, batch chip, stepper, line total, remove) · `TotalsBlock` (subtotal, TVA breakdown, discount, TOTAL row) · `ActionBar` (Save / Save & Print + keyboard hints) · `BatchPickerSheet` · `BatchReceiveForm` (multi-line) · `ExpiryBadge` (`ok` / `soon` / `expired`) · `UnknownBarcodeDialog` · `Toast` / `OfflineChip` (`Synced 2m ago` · `Offline — 3 days queued`) · `LicenceBanner` (ADR-002/003 states) · `CommandPalette` (Ctrl+K).

Every screen also has to pass the ten-point **design review gate** in `DESIGN-002` §7 (one primary action, ≤7 fields, keyboard-complete, optional-field defaults, money precision).

Primitives: `Button` (primary/danger/secondary/ghost), `Input`, `Stepper`, `Table`, `Dialog`, `Sheet`, `Tabs`, `Badge`, `Skeleton`. Every primitive is RTL-safe by construction and theme-aware by token — no per-page colour literals.

### Product images (offline, must be fast)
Stored as runtime data in `data/images/<hash>.webp` (not in the binary), referenced from SQLite. On import: resize to **240 px** thumb (grid) + **1024 px** (product page), strip metadata. Grid uses `loading="lazy"`, `decoding="async"`, fixed aspect box to prevent layout shift. Placeholder = category-coloured initial tile, so a shop with no photos still looks intentional.

---

## 6. Keyboard & touch model

A till with a scanner and no mouse is the primary case.

| Key | Action |
|---|---|
| `F2` / `/` | focus scan/search |
| `F4` | new invoice |
| `F9` | save |
| `F10` / `Ctrl+P` | save & print |
| `↑ ↓` | move in product results / cart |
| `Enter` | add focused product · confirm |
| `+ / −` | qty of focused line |
| `Del` | remove focused line |
| `Esc` | close dialog / clear search |
| `Ctrl+K` | command palette |
| `Ctrl+Shift+D` | toggle theme |

Hover must never reveal essential information (tablets have none). All interactive states have `:focus-visible` rings. A **touch mode** toggle enlarges steppers to 48 px and disables keyboard-hint chrome.

---

## 7. Where the redesign lands in the milestones

| Milestone | Work | Days |
|---|---|---|
| M0 | Token file, logical-property CSS reset, stylelint RTL rule, i18n scaffolding, **embedded Cairo font** | 1.5 |
| M1 | `ProductCard/Grid/Rail` in both themes | 1.0 |
| M2 | `CartPanel`, `TotalsBlock`, `ActionBar`, keyboard model, dark-mode QA of POS | 1.5 |
| M4 | Dashboard charts (dual-theme palette), drill-downs, RTL tables | 1.5 |
| M6 | Same tokens on the Vercel dashboard (shared package) | 0.5 |
| M7 | Arabic catalog completion (`ar.json`), pseudo-locale pass, RTL screendiff tests, receipt-language decision | 2.5 |
| **Total** | | **~8.5 d** (+~1.5 if RTL is retrofitted later instead of M0) |

**Sequencing advice:** put `fr` + `en` live in v1 and treat `ar` as the M7 deliverable — but **build every component RTL-safe from M0**, because that's the cheap moment. Ship Arabic when the translation is reviewed by a native speaker, not before (machine-translated POS buttons undermine trust in a money app).

---

## 8. Testing

- **Visual:** Playwright screenshots of every screen in `light × fr`, `dark × fr`, `dark × ar`, `light × ar` — 4 baselines, diff on CI.
- **Bidi:** assert money cells render LTR inside RTL (regex check on computed text + bounding box ordering).
- **Contrast:** automated axe run per theme; fail under 4.5:1 for text.
- **i18n:** pseudo-locale build in CI to catch truncation; missing-key test that fails the build on any key rendered raw.
- **Font:** assert both WOFF2 files are embedded (no network request to any font host — grep the built binary/assets).

---

## 9. Decisions needed

1. **Dark mode default:** follow the OS, or force light in the shop (bright rooms) and let the owner pick? *I'd default to system, with a settings toggle.*
2. **Arabic digits:** Western `0-9` (recommended) or Arabic-Indic `٠-٩`, or a per-user setting?
3. **Languages in v1:** `fr` + `en` first (Arabic at M7), or all three at launch? *(Recommendation: fr/en live, ar reviewed before first sale to a shop that wants it.)*
4. **Arabic receipts:** needed in v1? If yes, that's the raster-print path and **+3 days** in M8 — worth knowing now.
5. **Brand:** keep the green/red/blue identity as-is, or move to a slightly deeper green (`#1E8E3E`) so white-on-green passes AA? *I'd keep `#41B14B` and use dark text — it preserves the look exactly.*
