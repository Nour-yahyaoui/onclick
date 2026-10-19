# Shop Management System — Architecture & Delivery Plan

**Status:** plan only, no code yet
**Target:** one portable `store.exe`, runs offline on the shop PC, local-first, with a cloud dashboard that shows financial figures only.

---

## 0. Executive summary

A **single portable `store.exe`** (no installer, no runtime, no DLLs) that:

1. Runs an **embedded web server on `127.0.0.1:8642`** and opens the app in the browser in app mode (framework still to be chosen — `ADR-001`).

3. Owns **all** data in a **SQLCipher-encrypted SQLite** file — the single source of truth. Works with the internet unplugged, forever.
4. Records **everything** (unit costs, lots, IMEI, negotiated prices, debts, cash sessions) and **requires nothing** (no TVA, no supplier invoice, no tax ID by default).
5. Stamps **every write** with actor + timestamp + before→after in an append-only audit log — everything editable, nothing anonymous.
6. Runs a **background sync agent** that pushes aggregate figures only to a **Next.js control plane on Vercel**, which owns all database credentials and serves a read-only analytics dashboard (+ a derja/Arabizi AI assistant that computes nothing itself).
7. Degrades gracefully: no network → full speed; unpaid → the *owner* is blocked, **the till is not**; never deletes or hides data.

**Principles:** local is truth · cloud is a mirror of numbers · no PII ever leaves the machine · every dinar is exact (i64 millimes) · the till never goes down · **and every screen must be usable by a shopkeeper who is not tech-friendly** (`DESIGN-003`).

**Scope & effort:** pilot slice for the first 3 shops **≈ 70 dev-days** (fast pilot ≈ 35 d), full v1 **≈ 115 d**, superpowers track **≈ 22 d** (§14). 15 supporting decision records live in `docs/`.

---

## 1. Assumptions (correct me where wrong)

| # | Assumption | Impact if wrong |
|---|---|---|
| A1 | Windows 10/11 x64 is the deployment target (hence `.exe`) | Linux build is trivial; packaging section changes |
| A2 | **One till / one PC** in v1; maybe a second later | Multi-till is already designed for (§8.6) — no rewrite |
| A3 | Shop is in Tunisia → **TND, 3 decimals (millimes), TVA 19/13/7%**, day boundary `Africa/Tunis` | Currency/tax table is config, not hardcoded |
| A4 | Owner-only access to the cloud dashboard | If several users: add accounts/roles on Vercel side |
| A5 | Neon = aggregates only, owned by exactly one shop | If several shops share one Neon DB, add `shop_id` scoping (already in schema) |
| A6 | No card-terminal / fiscal-printer integration in v1 (ESC/POS receipt printing = v1.1) | Adds ~5 days |
| A7 | **Vertical = phone & accessories retail** (serialized units, IMEI, variants, warranty) — see `docs/ADR-006-phone-retail-vertical.md` | Changes the inventory model: units, not just quantities |

**Open decisions I need from you:** see §15.

---

## 2. Architecture

```
┌─────────────────────── SHOP PC (offline-first, local-only truth) ────────────────────────┐
│                                                                                          │
│   store.exe  (single Rust binary, no runtime, no .html files)                            │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐  │
│  │ axum (127.0.0.1:8642)                                                              │  │
│  │   ├─ handlers ──► domain (money math) ──► storage (SQLCipher SQLite)               │  │
│  │   ├─ maud!()  → HTML rendered server-side (typed, escaped, no template files)       │  │
│  │   ├─ htmx partial swaps + ~150 lines of embedded JS (scanner, keys, theme)          │  │
│  │   └─ assets: CSS + Cairo/Amiri fonts + htmx, embedded via include_str!/include_bytes!│  │
│  │                                                                                     │  │
│  │ Sync agent (background, isolated, 1 request at a time)                              │  │
│  │   outbox → batch → POST /api/sync/ingest   +   POST /api/licence/lease              │  │
│  │   connect 2s / total 5s timeout · circuit breaker · backoff 5m→6h                   │  │
│  └────────────────────────────────────────────────────────────────────────────────────┘  │
│         │                                    ▲                                           │
│         ▼                                    │ outbound HTTPS only (best-effort)         │
│   data/store.db (SQLCipher) + images         │ no DB credentials on this machine         │
└──────────────────────────────────────────────┼───────────────────────────────────────────┘
                                               │
                                    ┌──────────▼───────────────────────────┐
                                    │  CONTROL PLANE (Rust: axum + maud)    │
                                    │  same workspace, shared `domain`     │
                                    │  /device/pair · /licence/lease       │
                                    │  /sync/ingest · /sync/health         │
                                    │  /dashboard (read-only, htmx)        │
                                    │  /api/ask (AI) · /admin · cron       │
                                    └──────────┬───────────────────────────┘
                                               │ pooled TLS, least-privilege roles
                                    ┌──────────▼──────────┐
                                    │  Neon Postgres      │  daily_agg, monthly_agg,
                                    │  (aggregates only)  │  top_products, stock_value,
                                    └──────────┬──────────┘  entitlement, sync_health
                                               │ SELECT only
                                    ┌──────────▼──────────┐
                                    │ OWNER (browser)     │  1 concurrent session,
                                    │ analytics only      │  no CRUD in cloud
                                    └─────────────────────┘
```

**Key rules:** the exe **never talks to Neon**; **there are no `.html` files** — markup is written in `.rs` via `maud` (`ADR-014`); the cloud half is Rust too (shared `domain` crate = one money math, zero local/cloud drift). It talks to the control plane, which validates, rate-limits, and owns all DB credentials. The till's own requests never await the network — see `docs/ADR-003-control-plane-and-entitlement.md`.

### Window shell — web app, not a native GUI (`ADR-015`)

The shop sees a **chrome-less app window** (`msedge --app=http://127.0.0.1:8642 --user-data-dir=...`, own icon/title, no URL bar, dedicated profile) launched by `store.exe`. No Electron, no Tauri, no WebView2 dependency, no native GUI toolkit.

Why: the HTTP server is required anyway (LAN second till, phone-camera stock count, sync, ESC/POS printing), the cloud dashboard already forces HTML/maud — so a native local UI would mean **two UIs forever** — and browser text engines are the only ones that handle **Arabic + RTL + diacritics** properly. An optional `--window` mode using `wry` (~200 lines, same server, same pages) can be added later if a shop insists on a "real window".

### Why no Electron / Tauri
The UI is a web app served from the binary and opened in the OS browser in app mode (`msedge --app=http://127.0.0.1:8642` / `chrome --app=...`, fallback plain browser). Zero bundled browser runtime, zero extra installs, instant startup. A WebView2 wrapper can be added later as a *second*, optional thin shell if a real "window" is ever required — it would not change the server or the UI.

---

## 3. Stack & crate choices

**Rust only — `cargo` is the entire toolchain (`ADR-014`).**

| Concern | Choice | Why |
|---|---|---|
| HTTP server | **axum** + tower (Actix acceptable) | typed extractors, tower middleware; renders `Markup` directly |
| HTML | **`maud`** macro — markup **inside `.rs` files** | no `.html` files, compile-time, **escaped by default** |
| Interactivity | **htmx 2** vendored (~14 KB, embedded) + ~150 lines of embedded JS | no framework, no build, no client state |
| CSS | hand-authored, tokens as CSS custom properties, logical properties | no Tailwind/PostCSS; embedded via `include_str!` |
| Fonts | Cairo + Amiri WOFF2 subsets | embedded via `include_bytes!`, offline |
| Async runtime | `tokio` | |
| Local DB | `rusqlite` + **`bundled-sqlcipher`** | SQLite C compiled in, AES-256 at rest (SEC-001) |
| Pool | `r2d2_sqlite` | small, predictable; WAL allows concurrent readers |
| Cloud DB | `tokio-postgres` or `sqlx` (rustls) — **control plane only** | never on the shop PC |
| HTTP client | `reqwest` + rustls | sync + lease, 5 s timeout |
| JSON | `serde` / `serde_json` | |
| IDs | `uuid` v7 | time-sortable |
| Time | `chrono` + `chrono-tz` (`Africa/Tunis`) | explicit day boundaries |
| Money | **`i64` millimes**, never float/Decimal | exact, fast |
| Images | `image` + WebP encoder | local upload pipeline (ADR-012) |
| Auth | `argon2`, `cookie`/`tower-cookies` | SEC-001 |
| Crypto | `ed25519-dalek` (licences), `aes-gcm` (blobs), `sqlcipher` (DB) | |
| Logging | `tracing` + file rotation | |
| Migrations | `include_str!` SQL + `schema_version` (or `refinery`) | shipped inside the binary |
| E2E tests | **`chromiumoxide`** (Rust) | no Node, no Playwright |

### 3.1 Workspace crates
```
crates/domain   pure money/business math (no I/O)  ← shared by till AND control plane
crates/storage  SQLCipher, repositories, audit choke point
crates/ui       maud components + embedded assets
crates/app      the till (axum handlers, htmx fragments, auth)
crates/sync     outbox, lease client, backoff
crates/control  control plane server (same stack)
crates/cli      store.exe: serve | --doctor | --backup | --repair | --rekey
```
**Rule:** the hot path (a sale) touches only `storage` + `domain` + in-memory data. No network, no telemetry, no licence call, no ORM.

## 4. The "single .exe, nothing else needed" checklist

This is the part that most often breaks. Concretely:

| Requirement | How |
|---|---|
| No `sqlite3.dll` | `rusqlite` `bundled` feature (C amalgamation compiled in) |
| No OpenSSL DLL | rustls everywhere (Actix + sqlx + any HTTP client) |
| No VC++ redistributable | build with `RUSTFLAGS=-C target-feature=+crt-static` (MSVC static CRT) |
| No folder of frontend files | `rust-embed` (assets inside exe) |
| No installer, no service | portable exe; optional `--install-autostart` writes a Startup shortcut / Task Scheduler entry |
| No config file required | first run writes `data/config.toml` with sane defaults |
| No `.env` | Neon URL stored encrypted-at-rest in SQLite settings (Windows DPAPI via `windows` crate, or AES-GCM with a key derived from a machine+user secret) |
| Reproducible release | one `build-release.ps1`: `cargo build --release --target x86_64-pc-windows-msvc`, strip, LTO, embed icon/version via `winres` |

Release profile: `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"` → expect **8–14 MB**, cold start **< 150 ms**, RSS **< 60 MB**.
**Do not UPX** (triggers antivirus heuristics). Authenticode signing is optional (~$100–300/yr) if SmartScreen warnings annoy the owner.

Runtime layout (portable mode, data stays next to the exe or in `%APPDATA%` — configurable):

```
store.exe
data/
  store.db
  store.db-wal
  config.toml
  logs/store-YYYYMMDD.log
  backups/store-YYYYMMDD-HHMM.db
```

---

## 5. Data model (local SQLite — source of truth)

Conventions:
- **Money:** `INTEGER` millimes (1 TND = 1000). All totals computed in integers.
- **Timestamps:** `TEXT` ISO-8601 UTC; **business day** stored separately as `DATE` in shop timezone.
- **Everything is editable, nothing is anonymous.** Any record can be corrected; every write is versioned and stamped with **actor + timestamp + device + before→after diff** in an append-only `audit_events` log (hash-chained). Deletes are soft. Closed days keep an `as_reported` snapshot, so every post-close edit appears as an explicit variance. See `docs/ADR-011-editable-with-attribution.md`.
- **Price is entered, not looked up:** catalog prices are *reference* only (nullable); every sale line stores the **actual** negotiated `unit_price_millimes` plus the seller and the delta vs reference. Products have no mandatory price. See `docs/ADR-009-negotiated-pricing.md`.
- **No customer records by default:** sales are anonymous; a lightweight credit contact is created only when credit is given (mode `none` | `credit_only` | `full`, a setting, not a migration).
- **Costing:** **cost belongs to the lot, never to the product.** `batches.unit_cost_millimes` (with FX + landed charges), *specific identification* for serialized units, FIFO/FEFO for quantity items. Each sale line freezes `unit_cost_millimes` so historical profit never drifts when the market moves. See `docs/ADR-008-costing-when-prices-move.md`.

### Core tables (~28)

| Group | Tables |
|---|---|
| Identity | `settings`, `users`, `cash_sessions` (shifts / cash drawer open-close) |
| Media | `media` (hash, dimensions, AES-GCM blob refs, attribution), `media_links` (product / variant / unit / trade-in / SAV) |
| Catalog | `categories`, `products`, `product_variants`, `product_barcodes` (multi-barcode + pack qty), `product_suppliers`, `price_history` |
| Stock | `stock_movements` (single ledger: purchase/sale/return/adjust/loss/transfer, batch-bound), `stock_levels` (materialized qty + WAC, derived), **`batches`** (lot, expiry, per-lot cost, FEFO), **`units`** (one row per physical device: IMEI, condition, state, warranty), `unit_events` |
| After-sales | `sav_tickets`, `sav_events`, `warranty_claims`, `dealer_purchases` (trade-in), `instalment_plans` |
| Purchasing | `suppliers`, `purchase_orders`, `purchase_order_lines` (FX + currency), `purchase_charges` (freight/customs allocation), `supplier_payments` |
| Selling | `sales`, `sale_lines`, `sale_payments`, `returns`, `return_lines` |
| Credit / debts | `debtors` (minimal name+phone, **never leaves the machine**), `debts` (from sale · manual · opening balance), **`debt_ledger`** (append-only: open/payment/adjustment/write_off), `instalments` (tranches) — see `docs/ADR-010-debts-and-credit.md` |
| Finance | `expenses`, `expense_categories`, `tax_rates`, `daily_closings` (Z-report per day) |
| Sync | `dirty_days`, `sync_state`, `sync_log`, `agg_version` |
| Ops · audit | **`audit_events`** (append-only, hash-chained: actor, device, kind, entity, before/after JSON, diff, reason), `report_snapshots` (`as_reported` per closing), `schema_version`, `backup_log` |

Notes:
- `stock_movements` is append-only; `stock_levels` is rebuilt/updated transactionally so reads are O(1).
- `dirty_days(day, version)` is the sync work queue — any write that affects money or stock bumps it.
- Indexes: `sales(day)`, `sale_lines(sale_id)`, `sale_lines(product_id)`, `stock_movements(product_id, created_at)`, `products(barcode)`, `expenses(day)`.
- SQLite handles 100k+ sales/year trivially; the realistic ceiling here is disk space, not the engine.

---

## 6. Functional scope

### v1 — the money loop (must have)
- Product catalog + **multi-barcode** support (USB scanner = keyboard wedge, no driver), CSV/Excel import for initial setup.
- **Barcode selling**: global scan capture, unknown-barcode quick create, pack/case barcodes — see `docs/ADR-005-barcode-and-batches.md`.
- **Batch / lot tracking**: receive with batch code + expiry, FEFO consumption, expiry alerts, write-offs, returns-to-lot.
- **UI system**: "Simple POS" theme, light **+ dark**, `fr`/`en` + **Arabic with full RTL** — see `docs/DESIGN-001-theme-i18n.md`.
- **Local media**: product/unit/ID photos picked from the PC, EXIF-stripped, resized to WebP thumbs, **AES-GCM encrypted, never uploaded** (no cloud, no AI) — see `docs/ADR-012-local-image-upload.md`.
- **AI assistant** (web dashboard): derja/Arabizi/fr/en questions → **LLM extracts the intent only, our code computes the numbers**; Groq, JSON-mode, whitelisted metrics, deterministic fallback — see `docs/ADR-013-ai-assistant-groq.md`.
- **Non-technical users are the primary audience**: zero jargon (a blacklist of banned words), ≤ 3 visible buttons, nothing to configure, warn-never-block, errors that say what to do, a human one tap away. Every screen passes the `DESIGN-003` gate.
- **Simplicity doctrine**: one primary action per screen, ≤7 visible fields, nothing required unless the record is impossible without it, notes always optional, money never rounded on display — see `docs/DESIGN-002-simplicity-and-brand.md`.
- **Bismillah masthead** (بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ) in Amiri, large and centred on the dashboard, login screen and printed documents.
- **Negotiated pricing**: free per-line price entry (reference price optional, no validation), price dispersion + seller-deviation reports, anonymous sales with three customer modes — see `docs/ADR-009-negotiated-pricing.md`.
- **Audit & attribution**: everything editable, every change attributed (who/what/when, before→after), history drawer on every record, owner-only *Journal d'activité* (who sold/edited/deleted what and when), post-close variance — see `docs/ADR-011-editable-with-attribution.md`.
- **Recording policy**: capture every detail (cost, discounts, payment split, IMEI, credit) and require **no** compliance field by default — TVA/TEIF are opt-in profiles — see `docs/ADR-007-recording-modes.md`.
- **POS screen**: scan/search → cart → discount → payment (cash / card / credit) → receipt view. Keyboard-first, sub-50 ms per action.
- Stock: automatic movements on sale/purchase/return, manual adjust with reason, low-stock alerts.
- Purchases: supplier, PO, receive stock, supplier payments.
- Customers: credit sales, outstanding balance, settle.
- Expenses: categorized, dated, attached note.
- **Cash sessions**: open with float, close with counted cash → expected vs actual difference (this is where shrinkage shows up).
- **Daily closing (Z)**: freezes the day's numbers.
- Reports: daily/monthly sales, margin per product/category, top products, dead stock, stock value, expense breakdown, cash flow, profit & loss.
- **Customer credit book** — now implemented properly in `docs/ADR-010-debts-and-credit.md`: sale debts, free debts, tranches, early settlement, aging, WhatsApp statements.
- Local dashboard + remote dashboard, same SPA code, two data sources.
- Users: owner (full) + cashier (sell-only, no cost/margin visibility — important).
- Backup: nightly `VACUUM INTO`, 30-day rotation, one-click restore, `--doctor` self-check.

### After the pilot — priority order across all tracks

The backlog is no longer a flat "v1.1 list"; it is ordered by when it earns money. Milestone mapping in §14.

| Order | Item | Track | Why now |
|---|---|---|---|
| 1 | TEIF e-invoicing + fiscal journal | M8 | the compliance moat (`STRATEGY-001` ②) |
| 2 | 21:00 WhatsApp/Telegram daily summary | M8 | retention: they open it every day |
| 3 | WhatsApp digital receipt | **S7** | free distribution on every sale |
| 4 | Carnet photo → OCR import, supplier price-list import | **S8, S9** | onboarding days → minutes |
| 5 | Restock + cash-flow advisor, dead-cash detector | **S10, S12** | the "it earns money" features |
| 6 | Mobile second till + phone-camera stock count | **S11** | no second PC needed |
| 7 | Arabic receipts (raster print), ESC/POS, A4 invoices | M8 | required by some shops, not all |
| 8 | Basket affinity, multi-till full sync, scale items | **S13**, M9 | after ~20 shops |
| 9 | Loyalty, bundles, multi-warehouse, accountant export pack | M9 | when a shop asks |

Explicitly **not built**: general ledger/accounting, e-commerce storefront, card terminals, payroll, loyalty points — export to the accountant instead.

### Explicitly out of scope (v1)
Card terminal integration, payroll, e-commerce storefront, native mobile app, general ledger. (E-invoicing/TEIF is *in* as a module — see ADR-007 profiles and `STRATEGY-001`.)

---

## 7. Financial model — exact KPI definitions

Ambiguity here is what makes dashboards lie, so each figure gets a formula. All amounts TTC unless marked HT; VAT split stored per line.

| KPI | Formula |
|---|---|
| Revenue (CA TTC) | `Σ sale_lines.total_millimes` − discounts − returns (same day) |
| Revenue HT | `Σ line.total / (1 + rate)` per line's `tax_rate` |
| COGS | `Σ (qty × unit_cost_at_sale)` for lines sold in period |
| **Gross margin** | `Revenue HT − COGS`; margin % = gross / revenue HT |
| **Replacement margin** | `(price − current buy price) / price` — the *real* margin in a moving market; gap vs displayed margin is a headline report |
| **Realized price dispersion** | per product: min / avg / max actual sold price, % of sales below reference, and **deviation by seller** (cash-haggling leak detection) |
| **Edits by user** | count + types + money affected, per user per period — surfaces a cashier who keeps "correcting" the same record |
| **Post-close variance** | per closed day: figure as reported vs as it stands now, with the author of each change |
| **Receivables aging** | open debt per debtor in 0-30 / 31-60 / 61-90 / 90+ buckets; collections vs schedule; bad-debt ratio |
| Expenses | `Σ expenses.amount` in period (by category) |
| **Net profit** | `Gross margin − Expenses` |
| Transactions | count of `sales` (returns counted separately, never netted silently) |
| AOV | `Revenue TTC / Transactions` |
| Cash in / out | from `sale_payments` + `supplier_payments` + `expenses` by method (cash/card/credit) |
| Expected drawer cash | float + cash sales − cash refunds − cash expenses − supplier cash payments ± manual in/out |
| Drawer variance | counted − expected (shown per shift, per cashier) |
| Stock value | `Σ (qty × WAC)` |
| Dead stock | products with qty > 0 and no sale in 60/90 days; value at WAC |
| Dues / receivables | `Σ customer_ledger` open balances |
| Break-even day | `fixed_costs_estimate / (margin_rate)` — optional, owner sets fixed costs |

Dashboard views: **Today** (live), **This week/month**, **Yearly trend**, **Profit & loss**, **Cash**, **Stock health**, **Top/dead products**. Every card clickable → drill-down report → CSV export.

---

## 8. Sync & entitlement (control-plane model)

Full design: `docs/ADR-003-control-plane-and-entitlement.md`. Summary:

- **Outbox pattern** — every cloud-bound fact is written to `outbox` inside the *same* SQLite transaction as the business write. A background agent drains it. Zero network calls in any request handler.
- **Unit of sync = one day's aggregates** keyed `(shop_id, device_id, day, agg_version)`. Derived numbers only, so there are no conflicts and retries are always safe (`Idempotency-Key` + versioned upsert).
- **Hard timeouts:** connect 2 s, total 5 s, one request at a time, circuit breaker after 3 failures, backoff 5 m → 6 h capped.
- **Backfill:** 30 days/batch, newest first, so the dashboard is useful within seconds of reconnecting.
- **Same channel carries entitlement:** `POST /api/licence/lease` returns a 14-day **signed lease** (Ed25519, verified offline). Offline-safe 28 days (lease + grace). Never blocks a sale — see ADR-003 §1.
- **No DB credentials on customer machines.** Device tokens only, encrypted at rest (DPAPI), revocable server-side in one row.

## 9. Neon schema (cloud, aggregates + entitlement only)

| Group | Tables |
|---|---|
| Tenancy | `shops`, `devices` (1 active POS per shop), `users` (dashboard logins, roles) |
| Aggregates | `daily_agg`, `monthly_agg`, `top_products_daily`, `stock_value_daily`, `cash_session_agg` |
| Entitlement | `licences` (plan, `lease_seq`, `paid_until`), `lease_events`, `pairing_codes` |
| Ops | `sync_health` (last seen, pending days), `edit_summary` (counts + post-close variance only — **no diffs, no names**), `rate_limits` |

Rules:
- App traffic goes through the **Next.js control plane**, never straight from the exe — so no `store_writer` credentials leave your server.
- Neon roles: `control_plane` (RW on the above) used only by the Next.js server; `dashboard_reader` (SELECT only) used only by dashboard queries.
- Connection string uses Neon's **`-pooler`** endpoint (PgBouncer) — required for serverless/Vercel functions.
- TLS mandatory (`sslmode=require`).
- Neon free tier (0.5 GB, scale-to-zero ~5 min) is enough for many years of aggregates.
- **Data minimisation:** aggregates only — no customer names, phones, invoice lines, or product photos. Privacy claim stays true.

---

## 10. Web dashboard (Vercel) — analytics only, read-only

- **Next.js (App Router)** on Vercel; server components query Neon with the `dashboard_reader` role via `@neondatabase/serverless`.
- **No CRUD in the cloud.** The dashboard lists, charts and exports — it never creates a product, a sale, or an expense. All business writes happen on the till, in its own SQLite. This is what keeps the system conflict-free and the cloud DB disposable.
- Auth: owner login (password + signed HttpOnly session, or magic link). **1 concurrent session** by default (new login revokes the old one), optional 2-device allowlist for an accountant.
- Caching: 30–60 s revalidation; `Last synced: 3 min ago · 0 days pending` driven by `sync_health`.
- Charts: lightweight (`recharts` or hand-rolled SVG) — revenue trend, margin, expense donut, cash-flow bars, top/dead products.
- Reads only aggregates → no PII exposure even if the Vercel project leaks.
- Mobile-friendly: owner checks the day's numbers from a phone while away.
- **Degrades by state:** `ACTIVE`/`GRACE` → full dashboard; `LAPSED` → analytics locked behind a payment screen for the owner (see ADR-003 §1).

The **local dashboard is the same SPA** served by `store.exe` and reading `/api/*` directly — zero network, always the freshest data. Same components, two data sources.

---

## 11. Performance plan

The brief is "very fast". Concrete targets, measured, not vibes:

| Metric | Target |
|---|---|
| Cold start to accepting requests | < 150 ms |
| Exe size | < 15 MB |
| Idle RSS | < 60 MB |
| `GET /api/products?q=` (10k SKUs) | p99 < 8 ms |
| Commit a sale (cart + lines + stock + dirty day) | p99 < 15 ms |
| Daily/monthly report (1 year of data) | p99 < 30 ms |
| First page render (server-side maud, no hydration) | < 120 ms |
| htmx fragment swap round trip (localhost) | < 30 ms |
| Concurrent cashier UI use | trivially 5+ |

How:
- SQLite PRAGMAs: `journal_mode=WAL`, `synchronous=NORMAL`, `busy_timeout=5000`, `foreign_keys=ON`, `temp_store=MEMORY`, `mmap_size=268435456`, `cache_size=-65536`.
- Prepared statement cache per connection; no dynamic SQL on hot path.
- Report queries hit **pre-aggregated tables** (`daily_sales_agg` maintained on write) instead of scanning `sale_lines`.
- One SQLite connection per tokio task for reads + a **single writer connection** (SQLite allows one writer) accessed via `spawn_blocking`.
- Assets served from RAM (`include_str!`/`include_bytes!`, baked into the binary), `Cache-Control: immutable`, no compression on localhost (gzip costs more than localhost bandwidth saves).
- **No client bundle, no hydration, no framework boot**: the browser receives finished HTML. Median interaction = one htmx request + one fragment swap.
- Release build: LTO fat, `codegen-units=1`, `panic=abort`.
- Logging off `info` in release; no request logs by default.
- **Benchmark harness** (`criterion` for the query layer, `oha` against a seeded 100k-sale DB, and render-time benches for maud components) run in CI so regressions are caught.

---

## 12. Security & operations

Full detail: **`docs/SEC-001-auth-and-encryption.md`**. Summary:

- **Auth & roles.** Owner/manager = password (**Argon2id**, m=64 MiB t=3); cashier = 6-digit PIN (Argon2id, for speed). Server-side guards on every endpoint — a cashier hitting `/api/journal` gets 403, and never sees cost, margin or profit.
- **Sessions.** 256-bit ids stored *hashed*, `HttpOnly; SameSite=Strict`, idle lock 10 min (POS) / 30 min (dashboard), max 5 sessions, revoke-all on password or role change. In-progress cart survives the lock and a crash.
- **Localhost hardening.** `Host` validation (kills DNS rebinding), `Origin` check, mandatory `X-Store-Client` header, CSRF token, no CORS, strict CSP, `X-Frame-Options: DENY`. Bind `127.0.0.1` only.
- **Encryption at rest: SQLCipher** (AES-256, `bundled-sqlcipher`) for the whole DB + WAL + free pages; AES-GCM for images/ID photos; encrypted backups. **Key hierarchy:** `K_db` ← master key ← Windows **DPAPI** (unattended reboot keeps the till working), optional Argon2id passphrase wrap, **printed recovery code** shown once at setup (the only copy — losing it loses the data, and the UI says so).
- **Refresh-proofing.** `--doctor` opens and verifies each backup, checks the audit hash chain, and fails the build on a crypto perf regression (sale commit ≤ 25 ms).
- **Throttling without lockout DoS.** Exponential delays capped at 15 min, never permanent; owner unlocks with the recovery code.
- **Secrets.** Device sync token and session secret encrypted by `K_db`; **Neon credentials never on the client**; dashboard admin secret server-side only.
- **Web dashboard:** same session model, **optional TOTP 2FA** (recommended — it exposes every shop's revenue), `__Host-` cookies, HSTS.
- **Audit** of every security event: login/fail, reauth, password/PIN/role change, export, backup, restore, rekey, key recovery (ADR-011).
- **Single instance:** the port bind *is* the lock. Second launch detects an existing instance and just opens the browser to it.
- Backups: nightly `VACUUM INTO` + keep 30; `--backup-now`, `--restore <file>`, `--doctor` (integrity check, disk space, port, sync config, Neon reachability).
- Auto-start: optional Startup shortcut / Scheduled Task at logon; tray-less (it's a server + browser tab).
- Logs rotate daily, keep 14 days; no PII in logs.
- Updates: manual replace of the exe (data dir is separate), or optional `--update` that verifies a signed manifest.
- **Licensing / trial:** offline trial state + optional Ed25519-signed license file; enforcement degrades to **read-only**, never to data loss. See `docs/ADR-002-licensing.md` (decision pending). Slots into M7, ~3.5–4.5 days.

---

## 13. Testing strategy

| Layer | Approach |
|---|---|
| Money math | unit tests on integer millimes, rounding rules, VAT split, discounts |
| Stock ledger | property tests: any sequence of movements → `stock_levels` == recomputed from ledger |
| Aggregates | golden tests: fixture DB → expected day/month aggregates; the same function feeds local reports *and* sync (one code path = one truth) |
| Sync | integration test against a real Neon **branch** (Neon branching = disposable DB per CI run); idempotency test (push same day 3×), backfill test, version-conflict test |
| API | `actix_web::test` + a seeded in-memory SQLite |
| E2E | Playwright against the built exe: scan → sell → close day → dashboard numbers match |
| Packaging | CI builds the exe on Windows, runs `--doctor`, asserts size and cold-start budget |

---

## 14. Milestones

Two tracks on purpose: **P = what one real shop needs to start working** (sells product, so you get feedback and money), **M = what makes it a business**. Nothing in M is lost — it is sequenced *after* the first installs.

### 14.1 Track P — pilot slice (the thing to actually build first)

| # | Milestone | Deliverable | Est. |
|---|---|---|---|
| **P0** | Foundation | Rust workspace, axum + maud skeleton, HTTP server, **app-mode window launcher (single-instance)** + SQLite + migrations + `--doctor`; design tokens, logical-property CSS (RTL-safe), i18n scaffolding, Cairo + Amiri, Bismillah masthead, first-run flow, money-format utils; **audit core** (`mutate()`, `audit_events`, hash chain, version + soft-delete); **auth + roles + sessions, SQLCipher + key hierarchy + recovery code**; **instrumentation** (per-screen latency, error ring buffer, slow queries, backup status) | 18 d |
| **P1** | Catalog & stock | products/variants, multi-barcode + lookup map, import, stock ledger, levels, **batches (receive + FEFO)**, **image upload (encrypted)**, **serialized units + IMEI + warranty lookup** | 23 d |
| **P2** | POS & sales | cart, **scan-to-cart**, negotiated price entry, payments (cash/card/credit), receipts view, returns, cash sessions + Z, **theme + keyboard model**, **crash-proof sale journal** | 13 d |
| **P3** | Debts & closing | debt carnet (sale debts, free debts, tranches, settlement, aging), opening-balance import, daily closing + reports, expenses | 14 d |
| | | **P total** | **≈ 70 d** |

> **Honest observation:** the "pilot" is 68 days, not 35 — because encryption, audit and debts are non-negotiable in your spec, and they're front-loaded. If you need to install sooner, the only real cut is: **P0 + P2 with a minimal catalog (no batches, no IMEI) ≈ 35 d**, then add P1/P3 *on that shop's data*. That's a legitimate way to start; it just means touching live data twice.

### 14.2 Track M — full v1 (after the first shops)

| # | Milestone | Deliverable | Est. |
|---|---|---|---|
| M3 | Finance core | suppliers/POs, landed cost + FX, P&L, expiry report + write-offs, per-seller deviation | 8 d |
| M4 | Local dashboard | KPI cards, dual-theme charts, drill-downs, **history drawer + Journal d'activité + post-close variance**, CSV export, RTL tables, offline question parser | 9.5 d |
| M5 | Sync + control plane | outbox, aggregation, ingest API, pairing, lease tokens, backoff, health UI | 9 d |
| M6 | Control plane + web dashboard | **Rust (axum + maud)**, auth (1 session + optional TOTP 2FA), analytics-only server-rendered views, shared `domain` crate, admin ops, AI assistant (Groq, intent-only + eval suite), deploy pipeline | 15.5 d |
| M7 | Packaging & hardening | static CRT, icons, build script, benchmarks, E2E, degradation ladder, Arabic catalog + RTL visual tests, **signed daily hash**, **zero-knowledge off-site backup**, **health bundle**, **one-click Réparer + printer/scanner self-test**, **in-app "Appeler / Envoyer un problème" + laminated support card** | 19.5 d |

**Full v1 ≈ 115 dev-days** (~5 months solo). **Pilot slice ≈ 70 d** (~3 months). **Fast pilot ≈ 35 d.**

### 14.3 Track S — superpowers (CHOSEN: folded into the plan)

From `docs/STRATEGY-002-superpowers.md`. Sequenced by leverage, not by size. Items marked ⚡ are already folded into P/M above; the rest are post-pilot.

| # | Item | Effort | Status | Why |
|---|---|---|---|---|
| S1 | **Crash-proof sale journal** (fsync'd line per sale, `--repair` replay) | 2 d | ⚡ **P2** | "Aucune vente perdue. Jamais." — provable |
| S2 | **Instrumentation** (latency, error ring buffer, slow queries) | 1 d | ⚡ **P0** | You can only promise fast if you can see it |
| S3 | **Signed daily hash** (Ed25519 over closing figures, printed on Z) | 1 d | ⚡ **M7** | Provable non-alteration after closing |
| S4 | **Zero-knowledge off-site backup** (encrypted client-side, R2/Backblaze) | 3 d | ⚡ **M7** | Kills the #1 fear; paid tier; ~0 cost to you |
| S5 | **Health bundle** (aggregate, PII-free, consented) | 2 d | ⚡ **M7** | Support 50 shops alone |
| S6 | **One-click Réparer + printer/scanner self-test** | 1 d | ⚡ **M7** | Most tickets are 4 known problems |
| S7 | **WhatsApp digital receipt** (`wa.me`, free, offline) | 1 d | **M8** | Every customer sees your name |
| S8 | **Carnet page photo → OCR import** (local tesseract, names never upload) | 3 d | **M8** | Turns the worst onboarding into 20 min |
| S9 | **Supplier price-list import** (Excel paste → catalog) | 1.5 d | **M8** | A phone shop's catalog *is* its supplier list |
| S10 | **Restock + cash-flow advisor** (velocity + lead time + debt position) | 2.5 d | **M8** | Answers "can I afford this shipment?" |
| S11 | **Mobile second till + phone camera scanner** (LAN, TLS + pairing PIN) | 3 d | **M9** | Stock count 4 h → 20 min; no second PC |
| S12 | **Dead-cash detector** (frozen capital at replacement cost) | 1 d | M9 | Turns a report into a discount decision |
| S13 | **Basket affinity** (tickets only, no customers) | 1.5 d | M9 | Bundle suggestions at the till |
| S14 | **In-app "Réparer" + `--doctor` UI** (already S6) | — | — | — |
| | **Track S total** | **≈ 22 d** | | |

**Rule for S:** S1–S6 are folded into the pilot and full v1 (already counted). S7–S14 are only unlocked once a shop has been live for ~30 days, and each must be justified by something that shop actually asked for.

### 14.4 Sequencing summary

```
P0 (18) ─► P1 (23) ─► P2 (13) ─► P3 (14) ─► [ FIRST 3 SHOPS LIVE ]
                                          │
                        ┌─────────────────┴──────────────────┐
                        ▼                                    ▼
                 M3–M4 local depth (17.5)              M8 "sell faster"
                        │                              (S7–S10, 8 d)
                        ▼
                 M5–M6 cloud + AI (22.5)
                        │
                        ▼
                 M7 hardening (18.5) ─► M9 LAN second till + insights (5.5)
```

**Two rules I'd hold you to:** (1) no screen ships without passing the `DESIGN-003` §12 gate, including a real non-technical person completing the task unaided; (2) do not start M5/M6 (cloud + AI) until three shops have used the pilot for a month. The cloud is the expensive half and the easiest half to build for nobody.

## 15. Open decisions — tell me before I start

**UI / i18n / inventory** (details in `DESIGN-001` §9 and `ADR-005` §10):

1. **Tills:** one PC now, or plan for 2–3 at launch? (changes nothing structurally, but changes v1.1 priority)
2. **Tax:** real Tunisian TVA per product (19/13/7%) with HT/TTC display, or just record TTC for now?
3. **Cash sessions (shifts):** do you need shift opening/closing with drawer variance in v1, or only end-of-day closing?
4. **Remote dashboard:** owner-only private view (simplest), or does an accountant/partner need their own read-only login?
5. **Receipts:** is a printed receipt needed for v1, or is an on-screen/browser-printable receipt enough? (ESC/POS adds hardware coupling)
6. **Product count:** roughly how many SKUs and sales/day? (sizes the import + report strategy; the design handles 10k SKUs easily)
7. **Neon project:** already created, or should I plan the setup steps (roles, pooler endpoint, branch for staging)?
8. **Batches:** is expiry real for your shops (grocery/pharmacy/cosmetics), or is a batch just a *purchase lot* for costing? (Biggest UI simplification available.)
9. **Arabic receipts:** needed in v1? (Requires raster printing on thermal printers, +3 d.)
10. **Languages at launch:** `fr`+`en` with Arabic at M7, or all three?
11. **Barcode selling scope:** local exe only (recommended) — or also from the cloud dashboard? (The latter breaks the read-only, conflict-free model.)

---

## 16. Risks

| Risk | Mitigation |
|---|---|
| Windows SmartScreen / antivirus flags an unsigned single exe | no UPX, stable signing cert later, publish hash + SHA-256 on release |
| Static-CRT build hiccups with a crate (rare) | fallback: ship `vcruntime` as optional; validate in M0, not M7 |
| Neon cold start makes the dashboard feel slow | keepalive ping every 4 min (or accept ~1 s first load); cache in Next.js |
| Owner's PC dies → data loss | nightly local backups + documented weekly copy to an external drive/Drive; add optional Drive backup in v1.1 |
| Cost drift when prices change | `unit_cost_at_sale` frozen per line — historical profit never rewrites itself |
| Scope creep into full ERP | milestones are strict; anything not in §6 v1 goes to the backlog |

---

## 17. Immediate next steps when you say go

1. Confirm §15 answers.
2. M0: repo scaffold, exe proof-of-concept (embedded SPA + SQLite + static CRT) built and measured on Windows.
3. Lock the schema in §5 into migrations.
4. Build M2 (POS) as the first vertical slice so you can start entering real data early.
