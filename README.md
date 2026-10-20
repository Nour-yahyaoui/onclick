# OneClick — caisse & gestion de magasin

Local-first POS for phone & accessories shops. **One binary, works offline, in French / Arabic / English.**

> « Il enregistre tout, chaque dinar. Il n'exige rien. » — *records every dinar, demands nothing.*

Planning and decisions live in [`PLAN.md`](PLAN.md) and [`docs/`](docs/README.md).
Current version: **v1.0.10** — a till you can run a full day on, plus the cloud half that starts as soon as you give it a `DATABASE_URL`.

---

## Installer la caisse (Windows)

Télécharger le zip de la dernière version :
**<https://github.com/Nour-yahyaoui/desktop/releases/latest>** — le dézipper, puis
double-cliquer sur **`OneClick.bat`**. Rien d'autre à installer ; les données restent
dans `data\`, à côté de `store.exe`.

Sortir une version soi-même : [`docs/OPS-002-release.md`](docs/OPS-002-release.md).
Icône, démarrage et alerte SmartScreen (« Windows a protégé votre PC ») :
[`docs/OPS-003-windows-startup.md`](docs/OPS-003-windows-startup.md)
(un tag `vX.Y.Z` suffit — GitHub construit le `.exe` et publie la release).

---

## Le magasin qui n'a jamais saisi son stock

C'est le cas normal, pas l'exception. OneClick part de là :

1. le client pose un article sur le comptoir, vous scannez son code-barres ;
2. le code est inconnu ? Une feuille s'ouvre : **nom (facultatif)**, **prix de
   vente**, prix d'achat (facultatif). Deux champs suffisent, l'article part
   dans la vente ;
3. la fiche existe désormais. Sans nom, elle s'appelle « Code 123456 » ;
   son stock n'est pas compté, donc la caisse ne vous dira jamais « rupture » ;
4. plus tard, au calme, la fiche vous montre **ce que l'article a réellement
   vendu** — c'est avec ça que vous fixez le prix et le prix d'achat.

Et pour connaître votre vrai coût de revient : **Charges** (loyer, eau,
électricité, entretien), chacune avec sa périodicité — par mois, par trimestre
ou par an. L'écran affiche le total mensuel et **ce qu'il faut encaisser chaque
jour** pour les couvrir. Décision complète : [`docs/ADR-016`](docs/ADR-016-inventory-fills-itself.md).

---

## Status at a glance

| Area | v0.5 |
|---|---|
| **Till (offline)** | ✅ usable for a day: sell, credit, debts, cash session, Z report, receipt, product edit/archive, audit journal |
| **Cloud control plane** | ✅ runs standalone: pairing, lease stub, sync ingest, read-only dashboard, `/health` |
| **Encryption at rest** | ⚠️ not yet (SQLCipher is next; `--doctor` says so out loud) |
| **Roles / cashier PIN** | ⚠️ not yet — single "Patron" account |
| **Selling what is not in the system** | ✅ scan an unknown code → two questions → the sheet exists, the sale goes through |
| **Fixed charges (rent, water, power…)** | ✅ per month / per quarter / per year, each with its own rhythm |
| **Batches, IMEI, images, deliveries** | ❌ the phone-shop stock layer is not built yet |
| **Exe → cloud sync** | ❌ endpoints exist, the agent inside the exe does not |

---

## What works today (v0.5)

| Feature | |
|---|---|
| Single Rust binary, local server on `127.0.0.1:8642`, no Node, no DB server | ✅ |
| Server-rendered UI — **maud macros in `.rs` files, no `.html` anywhere** (ADR-014) | ✅ |
| French / Arabic (RTL) / English, one tap to switch | ✅ |
| Light + dark theme (rebuilt palette), persisted per device | ✅ |
| Bismillah rendered in its own strip under the header | ✅ |
| Content capped at 1180 px (forms 640 px) — never edge-to-edge | ✅ |
| Settings as a **modal** (58%, over the till) instead of a full page | ✅ |
| **Toasts** after every action + **Réessayer** on errors | ✅ |
| **Sidebar**: quick sell, search/scan, Vendre · Ventes · Carnet · Produits, Caisse/Z · Journal · Réglages | ✅ |
| **Ventes** page: the day book, one tap to reprint a ticket | ✅ |
| **Sell what you never entered**: unknown barcode → name (optional) + price only, added to the sale | ✅ |
| **Inventory that fills itself**: unnamed items show as « Code 123456 », never « rupture » | ✅ |
| **Product sheet** shows what it really sold, to fix the price later | ✅ |
| **Charges**: rent / water / power / maintenance, per month, quarter or year | ✅ |
| **Product pictures**: upload your own, or pick one of 8 built-in drawings — stored locally | ✅ |
| Bismillah masthead (Amiri/naskh stack) on every page | ✅ |
| First run: 3 plain questions → a working till with 6 sample products | ✅ |
| Owner password (Argon2id), session ids stored hashed | ✅ |
| Products: create, **edit, archive**, search by name or barcode, opening stock | ✅ |
| Sell: scan/type → cart → **price entered per line** (ADR-009) → cash / card / **credit** | ✅ |
| **Printable receipt** after every sale (browser print — no driver needed) | ✅ |
| **Cash session**: open with float → counted cash on close → expected vs counted vs difference (Z) | ✅ |
| **Debts / carnet**: credit sales, **free debts**, balances, collect, **write-off to loss** | ✅ |
| **Audit everywhere**: setup, login, failed login, logout, product create/edit/archive, sale, collect, write-off, settings | ✅ |
| **« Qui a fait quoi »** journal — last 200 actions, who/what/when | ✅ |
| **Cloud control plane** (`crates/control`): pairing, lease stub, sync ingest, read-only dashboard | ✅ |
| `--doctor` self-check, `/health`, `--port`, `--data` | ✅ |
| Money as `i64` millimes, 3 decimals, never floats | ✅ |
| Unit tests on money formatting/parsing, cart maths, margins, VAT | ✅ |

### Known shortcuts, stated plainly

* **SQLite is not encrypted yet** — `data/store.db` is a normal SQLite file until the SQLCipher switch (M0).
* **Roles are not enforced** — one "Patron" account; every screen is reachable, costs are visible to anyone who logs in.
* **Lease tokens are not signed yet** — `/api/licence/lease` answers `signed:false` on purpose, so a client can refuse it.
* **The exe does not sync yet** — the ingest endpoint works and is tested by hand; the agent inside `store.exe` is M5.
* **No CSRF / Host / Origin hardening yet** on the local app.

### Not built yet (see `PLAN.md` §14)

Batches/lots + IMEI units · product images · supplier deliveries with landed cost & FX · instalment schedules & aging buckets · charts/dashboard · ESC/POS thermal printing · CSV/Excel import · EAN check digits · roles & cashier PINs · backups & restore · AI assistant · htmx partial swaps.

---

## Build & run — the till (offline)

Requires **Rust stable 1.79+** and nothing else.

```bash
# From the repository root (the folder containing Cargo.toml):
cargo run                # build + start on http://127.0.0.1:8642
cargo run -p store       # explicit, identical
```

> Do **not** run `cargo run` inside `src/` — Cargo looks for `Cargo.toml` in the
> current directory and its parents, and `src/` has none of its own.
> `crates/control` is a **separate workspace** (the cloud half): build it from
> inside `crates/control`, never from the root.

The browser opens manually — go to the URL printed in the console.
First launch: language → shop name → VAT question → password, then you can sell.

```bash
cargo test               # domain unit tests (money, cart, margin, VAT)
cargo run -- --doctor    # integrity check + encryption/disk warnings
cargo run -- --port 9000 --data D:\shop
cargo build --release    # store.exe in target/release/
```

Data lives in `./data/store.db` (WAL). Delete that folder to start over.

## Build & run — the cloud half (optional)

A separate crate with its own workspace, so the till always builds even if this one does not.

```bash
psql "$DATABASE_URL_UNPOOLED" -f deploy/cloud/0001_init.sql   # once, on Neon
cd crates/control
cp ../../deploy/control-plane.env.example .env                 # set DATABASE_URL
cargo run                                                     # http://localhost:8080
```

With no `DATABASE_URL` it still starts, and `/health` reports `db=down` instead of crashing.
Endpoints: `POST /api/device/pair`, `POST /api/licence/lease`, `POST /api/sync/ingest` (device token as `Authorization: Bearer …`).

> **Windows note:** `x86_64-pc-windows-msvc` builds need the MSVC linker (Visual Studio Build Tools). Static-CRT / single-exe packaging is M7; today the release binary still links the system CRT.

---

## Repository layout

```
Cargo.toml                  workspace root + `store` binary
migrations/0001_init.sql    schema v1 (identity, catalog, sales, debts, audit)
migrations/0002_sessions.sql schema v2 (cash sessions, archiving)
src/main.rs                 CLI: serve | --doctor | --port | --data
crates/domain/              pure money & business maths (no I/O, unit-tested)
crates/storage/             SQLite, repositories, audit choke point
crates/ui/                  maud components, i18n, embedded CSS  (no .html)
crates/app/                 axum routes: sell, products, carnet, session, journal, auth
crates/control/             cloud control plane — separate workspace
deploy/cloud/0001_init.sql  Neon schema (shops, devices, daily_agg, sync_health)
deploy/control-plane.env.example
docs/                       21 decision records (ADR / DESIGN / SEC / OPS / STRATEGY)
```

### Why the structure looks like this

* **`domain` has no I/O** — the till, the control plane and the tests compute money the same way. One function, one answer, no local/cloud drift.
* **`storage` is the only place that writes** — and every write also writes an `audit_events` row, so "who did what, when" is never missing (ADR-011).
* **`ui` owns every piece of markup** — Rust functions returning `Markup`. No template engine, no `.html` file.
* **`app` holds no business logic** — handlers load, call `storage`, render.

---

## Build status — honest

This code was written in an environment **without a Rust toolchain or crates.io access**, so compilation happens on your machine and in CI (`.github/workflows/ci.yml`, Ubuntu + Windows).

Verified so far from the first real build (issue #1):

* `domain` and `ui` compiled; `serde`, `axum`, `maud` resolve correctly ✔
* a genuine maud bug was found and fixed — an element may not combine the shorthand class with an explicit `class` attribute (fixed in `c5dc29a`) ✔
* `storage` and `app` have **not been type-checked yet** — they were never reached by the compiler

If the build stops again, paste the error: it will be a concrete API detail, not an architectural one.

---

## Versioning

**Every commit bumps the version** — one command, all six manifests:

```bash
python3 scripts/bump_version.py          # 0.5.6 -> 0.5.7
python3 scripts/bump_version.py minor    # 0.5.7 -> 0.6.0
python3 scripts/bump_version.py 1.0.0    # explicit
```

The running build tells you which one it is, so a shop can answer *"which version?"*
without a developer: the **menu** (… → bottom line) shows `OneClick 0.5.6`, and
`store --version` prints the same string.

---

## The rules this code follows

1. **The till never waits on the network** — no network call exists in any request path.
2. **Record everything, require nothing** — cost, price, payment split and seller stored on every sale; no VAT, no invoice number, no tax ID required (ADR-007).
3. **Everything editable, nothing anonymous** — audit rows written in the same transaction, with actor and summary; product edits keep `version`/`updated_by` (ADR-011).
4. **Every dinar is exact** — `i64` millimes, 3 decimals, no floats, no hidden rounding.
5. **A non-technical shopkeeper can use it** — no jargon on screen, one primary button per screen, errors say what to do next (DESIGN-003).

---

## Version history

| Version | What landed |
|---|---|
| **v1.0.10** | Photos produits opérationnelles : `multipart` lu **avant** le verrou de la base (sinon axum refuse le handler), icône de barre latérale restaurée, et un CI qui publie enfin les erreurs de compilation en clair |
| **v1.0.4** | **Photos produits** : votre propre photo (JPEG/PNG/WebP, copiée dans `data/photos`, jamais envoyée nulle part) ou l'une des **8 images d'accessoires** embarquées dans l'exe ; vignettes dans la grille de vente et dans la liste. **Icônes de barre latérale refaites** avec la bibliothèque Lucide (26 icônes générées par script, plus aucun tracé à la main) |
| **v1.0.3** | `winresource` déclaré comme dépendance de **build** (l'icône ne compilait pas sur Windows) |
| **v1.0.2** | **Icône OneClick gravée dans `store.exe`** (Explorateur, barre des tâches, raccourcis) + fiche du fichier renseignée ; `store --open` démarre et ouvre le navigateur ; la fenêtre noire ne survit plus à l'app. Note honnête sur l'alerte SmartScreen dans `OPS-003` |
| **v1.0.1** | **Sauvegarde réelle** (bouton « Sauvegarder maintenant », copie cohérente via `VACUUM INTO`, 14 dernières gardées) ; le réglage ne promet plus « sauvegarde automatique chaque soir » ni un chiffrement qui n'existe pas |
| **v1.0** | **Sell what you never entered.** Scanning an unknown barcode no longer fails: name (optional) + price only, the sheet is created and goes straight into the sale; unnamed items show as « Code 123456 » and never as « rupture ». Product sheets show what they really sold, so prices settle over time. **Charges**: rent, water, power, maintenance — each with its own rhythm (month, quarter, year) and a per-day break-even |
| **v0.6.2** | **Control plane compiles again** (`tokio-postgres` was missing the `chrono` feature, so every `DATE` column failed to map) ; the cloud half can no longer paint the shop's pipeline red |
| **v0.6.1** | **Le panier se vide après la vente** (il gardait les articles : le vidage visait la mauvaise session), et le ticket propose « Nouvelle vente ». Les pages ne sont plus rejouées depuis le cache du navigateur |
| **v0.6.0** | **Publication automatique** : un tag `vX.Y.Z` fait construire `store.exe` par GitHub et publie la release (zip + lanceur double-clic + SHA-256) — voir `docs/OPS-002-release.md` |
| **v0.5.9** | One sale, in full: every article with its quantity and price; **edit** a sale (stock is re-balanced by the difference); **cancel** a sale and the stock comes back to the shelf — the sale stays in the history. Debt maths fixed: the carnet no longer counts twice, cancelled sales leave the day total |
| **v0.5.8** | Visible drag handle for the sidebar (replaces the invisible CSS grip), a real fold/unfold chevron, no scrollbar inside the rail |
| **v0.5.7** | **Blank-page fix** (one grid column for two children), resizable sidebar, collapsible to an icon rail with every icon kept, nav icons |
| **v0.5.6** | Login hint removed; the failed-login screen reuses the same card; version shown in the menu and via `store --version` |
| **v0.5.5** | Build fixes: restored `LoginForm`, `Option<Markup>` no longer rendered bare, `pos_body` signature cleaned; linter now also catches bare `Option` renders |
| **v0.5.4** | Sign-in card redesigned (password only, brand, language/theme in the footer); setup uses the same card; unused-variable warning fixed (#3) |
| **v0.5.3** | Width caps, settings modal, toasts, retry button |
| **v0.5.2** | Persistent sidebar (quick sell, search, sales, debts, menu) + sales day-book page |
| **v0.5.1** | Visual rebuild: new light/dark palette (warm paper / deep slate), no side rail, Bismillah moved under the app bar, two-column POS, new cart panel — see `docs/assets/design-pos-{light,dark}.png` |
| **v0.5** | Renamed to **OneClick**; docs + UI + migrations aligned; version bumped across the workspace |
| v0.2 | Product edit/archive, audit everywhere + « Qui a fait quoi », cash session + Z report, printable receipt, free debts & write-offs, cloud control plane |
| v0.1 | First slice: workspace, server-rendered maud UI, offline till, sales, stock ledger, credit sales, carnet, Argon2id auth, fr/ar/en, dark mode |
