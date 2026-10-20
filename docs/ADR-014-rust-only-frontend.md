# ADR-014 — Rust-only repository, HTML written in `.rs` files

**Status:** ✅ **ACCEPTED** — binding rule for the whole repo. Axum chosen; **Option A (full Rust monorepo, Rust control plane)** accepted.
**Date:** 2026-10-20
**Supersedes:** `ADR-001` SPA/TypeScript frontend sections · `PLAN.md` §2 diagram, §3 stack, §11 caching · `DESIGN-001` §5 component inventory
**Relates to:** ADR-003 (control plane), OPS-001 (env)

---

## 1. The rule

> **No `.html` files. No HTML templating files. No Node/npm toolchain.**
> Markup is written **inside `.rs` files**, with the Rust type system and default escaping.

What this means concretely:

| Not allowed | Instead |
|---|---|
| `index.html`, `*.html` templates, `layout.tera`, `*.hbs` | **`maud!` macro** — HTML as Rust code in a `.rs` file, escaped by default |
| React/Next/Vue/Svelte, JSX, `node_modules`, `package.json` | Server-rendered Rust handlers returning `Markup` |
| Client-side router, client state store | **htmx partial swaps** (vendored single file) + a tiny inline script for scanner/keyboard |
| Tailwind/PostCSS build step | Hand-authored CSS with the DESIGN-001 tokens, embedded via `include_str!` |

**Escaping is structural:** `maud!` escapes interpolations by default, so an XSS in a product name is not a code review question — it's impossible without an explicit `PreEscaped`. That alone is worth the rule.

---

## 2. The stack, now settled

| Layer | Choice | Note |
|---|---|---|
| Language | **Rust** (edition 2021, MSRV pinned) | decided |
| HTTP | **Axum 0.7+ + tower** | recommended over Actix: typed extractors, tower middleware, simpler generics, first-class `Markup` responses. *Actix remains acceptable* — this is the only open detail in this ADR |
| HTML | **`maud` 0.26** (compile-time macro) | no template files, no runtime parse, `impl Into<Markup>` components |
| Interactivity | **htmx 2.x**, vendored (~14 KB, embedded) | no framework, no build |
| Tiny JS | one embedded script (~150 lines) for scanner buffer, keyboard shortcuts, theme toggle | the only JS in the repo |
| CSS | hand-authored, one file, CSS custom properties, logical properties (RTL) | embedded via `include_str!` |
| Fonts | Cairo + Amiri WOFF2 subsets | embedded via `include_bytes!` |
| DB | `rusqlite` + `bundled-sqlcipher` | SEC-001 |
| Async | `tokio` | |
| HTTP client | `reqwest` + rustls | sync/lease |
| Testing | `cargo test`, maud snapshot tests, `chromiumoxide` (Rust) for E2E | **no Playwright/Node** |

**Total build toolchain: `cargo` and `rustc`. Nothing else.** That is the practical meaning of "a Rust repo".

---

## 3. Repository layout (one workspace, two binaries)

```
desktop/
├── Cargo.toml                  # [workspace]
├── crates/
│   ├── domain/                 # PURE Rust: money (i64 millimes), lots, FX, debts,
│   │                           # aggregates, pricing, margins. NO I/O. NO async.
│   ├── storage/                # sqlcipher SQLite, migrations, repositories,
│   │                           # audit_events + mutate() choke point, backups
│   ├── ui/                     # maud components, layout, tokens, i18n, forms
│   │   ├── components.rs       # CartLine(), ProductCard(), Money(), BismillahBand()
│   │   └── assets.rs           # include_str!/include_bytes! for css, htmx, fonts
│   ├── app/                    # the till: axum router, handlers, htmx fragments, auth
│   ├── sync/                   # outbox, lease client, backoff, circuit breaker
│   ├── control/                # control plane server: pairing, lease, ingest,
│   │                           # analytics (read-only) — SAME stack, SAME ui crate
│   └── cli/                    # store.exe: serve | --doctor | --backup | --repair | --rekey
├── assets/                     # .css, htmx.min.js, fonts, icons (embedded at build)
├── migrations/                 # SQL only
└── docs/
```
**Two binaries, one language:** `store.exe` (shop PC) and `control-plane` (cloud).

### 3.1 Why the shared `domain` crate matters more than the HTML rule
`domain` holds the **money math**: aggregation, margin, debt ledgers, replacement cost. The till uses it for local reports; the control plane uses it to validate and roll up ingest payloads; the tests use it as the single source of truth.

> That kills the most common SaaS bug in this design: **local numbers and cloud numbers disagreeing** because two codebases computed the same figure differently. One function, one answer, everywhere.

---

## 4. Consequence: the cloud dashboard must also be Rust (decision needed)

The previous plan had the control plane + dashboard on **Next.js/Vercel** (`ADR-003`, `OPS-001`). Next.js *is* HTML+TS, so it violates your rule. Two honest options:

| | **Option A — full Rust monorepo** ⭐ *my recommendation* | **Option B — keep Next.js for the cloud half** |
|---|---|---|
| Repo | 100% Rust, one `cargo` workspace | Rust exe + a `web/` Next.js app |
| Control plane | `crates/control` (axum + maud), deployed on **Fly.io / Render / Hetzner VPS** | Next.js on Vercel |
| Dashboard UI | maud + htmx, same components as the till | React |
| Money math | **shared `domain` crate** — zero drift | duplicated in TS (the bug risk above) |
| Toolchain | `cargo` only | `cargo` + `node` |
| Hosting cost | ~$0–7/mo (Fly shared-cpu, or free tier on Render) | Vercel free tier |
| Deploy ergonomics | `fly deploy` / Docker push, or GitHub Actions → SSH | `git push` (automatic) |
| Cron (21:00 summary) | `tokio` interval in the control plane, **or** Fly scheduled machine | Vercel cron |
| Loses | Vercel's zero-config CDN/preview deploys | your rule, plus domain-code duplication |

**Recommendation: Option A.** The shared `domain` crate is worth more than Vercel's convenience, and deploys stay one-command (`fly deploy` or an Actions workflow building a Docker image). Neon, the env contract, and everything in `OPS-001` survive unchanged — only the *host* and `NEXT_PUBLIC_*` disappear.

**If you prefer zero-ops**, Option B is defensible: keep `web/` Next.js and accept a small TS surface + duplicated aggregate logic. Say which and I'll align every doc.

---

## 5. What changes in the other documents

| Doc | Change |
|---|---|
| `ADR-001` | Language **decided: Rust**. Axum recommended; Actix acceptable. The SPA/htmx comparison collapses to **maud + htmx** |
| `PLAN.md` §2 | Diagram: server-rendered; no asset pipeline, no SPA bundle, no browser-state layer |
| `PLAN.md` §3 | Stack table: `maud`, `htmx` vendored, `chromiumoxide` for E2E; no Node |
| `PLAN.md` §11 | Perf: full-page render ~1–3 ms; htmx fragment swap 10–30 ms round trip on localhost; no hydration cost |
| `DESIGN-001` §5 | Components become **functions**: `fn product_card(p: &Product) -> Markup`, `fn money(m: i64) -> Markup` |
| `DESIGN-002` | unchanged — the doctrine is UI-agnostic |
| `ADR-013` (AI) | unchanged: `/api/ask` returns JSON; the answer card is a maud fragment |
| `OPS-001` | drop `NEXT_PUBLIC_*`; document host choice (Fly/Render) if Option A |
| `web/.env.example` | moves to `deploy/control-plane.env.example` (same variables, host-agnostic) |

---

## 6. Risks of maud + htmx (and the answers)

| Risk | Answer |
|---|---|
| Full-page reloads feel slow | They don't: localhost render is 1–3 ms and htmx swaps only fragments. Budget: **≤ 30 ms** per interaction including network |
| No client-side state to hide latency | The POS is a *form*, not an app: cart lives server-side in `draft_cart` (already needed for crash recovery, SEC/ADR-011) |
| Rich widgets (date pickers, charts) | Charts: server-rendered SVG or a tiny embedded chart lib (`uplot` ~40 KB, vendored, no build). Date pickers: `<input type="date">` + ISO parsing |
| htmx + CSRF | htmx sends the same headers; our `X-Store-Client` + Origin checks apply unchanged (SEC-001 §2.5) |
| Testing HTML | `maud` snapshots + `chromiumoxide` for real-browser E2E (no Node) |
| Recompile times as UI grows | Split `ui` crate; use `cargo-watch` + `axum` on a dev server; LTO only in release |
| RTL | unchanged: logical CSS properties; maud emits attributes normally, `dir` stays on `<html>` |
| Arabic fonts | unchanged: embedded subsets |

---

## 7. The one detail to confirm

**Axum or Actix** — both satisfy the rule. I recommend **Axum** for extractors and tower middleware; everything in this document works identically on Actix if you prefer it. Not worth more than a paragraph of deliberation.

---

## 8. Effort impact

| Item | Delta |
|---|---|
| No SPA build pipeline, no client router, no local JSON API layer for the till UI | **−3 d** |
| maud component library instead of component framework | **−1 d** (lighter, typed) |
| Control plane in Rust instead of Next.js (Option A) | **+2 d** (auth/cookies/SSR in axum instead of framework defaults) |
| Shared `domain` crate discipline | 0 (it was already required for correct money math) |
| **Net** | **≈ −2 d** → full v1 ≈ **109 d**, pilot ≈ **66 d** |

The rule costs nothing. It removes a toolchain and a class of bugs.

## Icônes (v1.0.4)

Les sept tracés de barre latérale écrits à la main ont été remplacés par un jeu
réel : **Lucide 1.60.0** (licence ISC), 26 icônes, converties en Rust par
`scripts/make_icons.py` → `crates/ui/src/icons.rs`. Elles héritent de
`currentColor`, donc le mode sombre ne demande aucun travail. Régénérer :

```bash
python3 scripts/make_icons.py --fetch   # retélécharge les .svg voulus
python3 scripts/make_icons.py           # réécrit icons.rs
```
