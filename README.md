# NovaShop — caisse & gestion de magasin

Local-first POS for phone & accessories shops. **One binary, works offline, in French / Arabic / English.**

> « Il enregistre tout, chaque dinar. Il n'exige rien. »

Planning and decisions live in [`PLAN.md`](PLAN.md) and [`docs/`](docs/README.md).
This repository is currently at **v0.2 — the till is usable for a day; the cloud half runs with `DATABASE_URL`**.

---

## What works today (v0.1)

| Feature | Status |
|---|---|
| Single Rust binary, local HTTP server on `127.0.0.1:8642` | ✅ |
| Server-rendered UI — **maud macros in `.rs` files, no `.html` anywhere** (ADR-014) | ✅ |
| French / Arabic (RTL) / English, one tap to switch | ✅ |
| Light + dark theme, persisted per device | ✅ |
| Bismillah masthead (Amiri/naskh stack) on every page | ✅ |
| First run: 3 questions → a working till with 6 sample products | ✅ |
| Owner password (Argon2id), hashed sessions in SQLite | ✅ |
| Products: create, search by name or barcode, opening stock | ✅ |
| Sell: scan/type → cart → **price entered per line** (ADR-009) → cash / card / **credit** | ✅ |
| Stock ledger + product version bump on every sale | ✅ |
| **Debts / carnet**: credit sales, balances, collect a payment (ADR-010) | ✅ |
| **Audit**: every product/sale/payment/settings change written to `audit_events` (ADR-011) | ✅ |
| **Product edit + archive** (soft delete, audited) | ✅ |
| **Audit everywhere**: setup, login, login-failure, logout, product create/edit/archive, sale, debt collect, write-off, settings | ✅ |
| **« Qui a fait quoi »** journal screen (last 200 actions) | ✅ |
| **Cash session**: open with float → close with counted cash → expected vs counted vs difference (Z report) | ✅ |
| **Printable receipt** after every sale (browser print, no driver) | ✅ |
| **Free debt** ("Ahmed took 500") and **write-off to loss** | ✅ |
| **Cloud control plane** (separate crate): pairing, lease stub, sync ingest, read-only dashboard, `/health` | ✅ |
| `--doctor` self-check, `/health` endpoint | ✅ |
| Money as `i64` millimes, 3 decimals, never floats | ✅ |
| Unit tests for money formatting/parsing, cart maths, margins, VAT | ✅ |

### Not yet (tracked in `PLAN.md` §14)

SQLCipher encryption at rest (M0) · batches/lots + IMEI units (P1) · product images · supplier deliveries · instalment schedules & aging · charts/dashboard · **exe → control-plane sync client** · AI assistant · ESC/POS thermal printing · CSV/Excel import · EAN check digits · roles & cashier PINs · CSRF/rate-limiting hardening · htmx partial swaps.

Known shortcuts in this version, stated plainly:

* **SQLite is not encrypted yet** — `--doctor` says so out loud.
* **Roles are not enforced** (single "Patron" user; every screen is reachable).
* **Lease tokens are not signed yet** — `/api/licence/lease` returns `signed:false` so the client can refuse it.
* **The exe does not sync yet** — the endpoint exists and is tested by hand; the agent inside `store.exe` is M5.

---

## Build & run — the till (offline)

Requires **Rust stable 1.79+** and nothing else (no Node, no database server).

```bash
cargo run                # build + start on http://127.0.0.1:8642
```

The browser opens manually — go to the URL printed in the console.
First launch shows the setup screen: language → shop name → VAT question → password. Then you can sell.

```bash
cargo test               # domain unit tests (money, cart, margin, VAT)
cargo run -- --doctor    # integrity check + encryption/disk warnings
cargo run -- --port 9000 --data D:\shop
cargo build --release    # store.exe in target/release/
```

Data lives in `./data/store.db` (WAL). Delete that folder to start over.

## Build & run — the cloud half (optional)

It is a **separate crate with its own workspace**, so the till always builds even if this one does not.

```bash
cd crates/control
cp ../../deploy/control-plane.env.example .env   # then set DATABASE_URL to your Neon string
cargo run                                        # http://localhost:8080
```

First run against an empty Neon database:

```bash
psql "$DATABASE_URL_UNPOOLED" -f deploy/cloud/0001_init.sql
```

Then open `http://localhost:8080` — with no data it shows an empty dashboard; `/health` reports `db=up|down`.
Endpoints: `POST /api/device/pair`, `POST /api/licence/lease`, `POST /api/sync/ingest` (device token as `Authorization: Bearer …`).

> **Windows note:** `x86_64-pc-windows-msvc` builds need the MSVC linker (Visual Studio Build Tools). The static-CRT / single-exe packaging work is M7; today the release binary still links the system CRT.

---

## Repository layout

```
Cargo.toml                  workspace root + `store` binary
migrations/0001_init.sql    schema v1
src/main.rs                 CLI: serve | --doctor | --port | --data
crates/domain/              pure money & business maths (no I/O, unit-tested)
crates/storage/             SQLite, repositories, audit choke point
crates/ui/                  maud components, i18n, embedded CSS  (no .html)
crates/app/                 axum routes: sell, products, carnet, auth, settings
docs/                       16 decision records (ADR / DESIGN / SEC / OPS / STRATEGY)
```

### Why the structure looks like this

* **`domain` has no dependencies on I/O** — the till, the future control plane and the tests all compute money the same way. One function, one answer.
* **`storage` is the only place that writes** — and every write also writes an `audit_events` row, so "who did what, when" is never missing (ADR-011).
* **`ui` owns every piece of markup** — as Rust functions returning `Markup`. There is no template engine and no `.html` file.
* **`app` holds no business logic** — handlers load, call `storage`, render.

---

## Verified status

The sandbox this was written in has **no Rust toolchain and no access to crates.io**, so the code has **not been compiled here**. Everything uses stable, minimal API surface (axum 0.8 · maud 0.26 · rusqlite 0.32 · argon2 0.5), but expect the usual first-build fixes:

```bash
cargo build 2>&1 | head -40
```

Likely candidates on the first build: a maud attribute detail, an unused import warning, or a version bump in `Cargo.toml`. None of them are architectural.

---

## The rules this code already follows

1. **The till never waits on the network** — no network call exists in any request path.
2. **Record everything, require nothing** — every sale stores cost, price, payment split and seller; no VAT, no invoice number, no tax ID required.
3. **Everything editable, nothing anonymous** — audit rows in the same transaction, with actor and summary; edits are versioned (`version`, `updated_by`, soft-delete columns are next).
4. **Every dinar is exact** — `i64` millimes, 3 decimals, no floats, no hidden rounding.
5. **A non-technical shopkeeper can use it** — no jargon on screen, one primary button per screen, errors say what to do next.
