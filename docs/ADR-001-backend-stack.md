# ADR-001 — Backend language & framework

**Status:** ✅ **ACCEPTED** — Rust, **axum** (owner delegated the choice). Frontend closed by `ADR-014` (maud + htmx, no `.html` files).
**Date:** 2026-10-20
**Supersedes:** the Actix Web assumption in `PLAN.md` §3 (still valid as *one* option)

---

## 1. What we're actually optimising for

The brief says "very fast Actix web server". Before comparing, one correction to the framing:

**HTTP framework performance is not our bottleneck, and won't be at any realistic shop scale.**

| Reality | Number |
|---|---|
| Shop doing 300 sales/day, 12h open | ~0.007 req/s average, ~2–3 req/s peak |
| Actix Web vs Axum vs Go on a JSON endpoint | all under **0.5 ms** framework overhead |
| A SQLite-backed read on 100k sales, indexed | **1–10 ms** |
| Round-trip for one sale write (WAL) | **2–15 ms** |

At 2 req/s, a framework 3× faster than another saves **microseconds we will never perceive**. TechEmpower-style benchmarks (millions of plaintext req/s) measure a regime this app never enters. Choosing on raw throughput is choosing on the wrong axis.

What *does* determine whether this app feels fast and whether the project succeeds:

1. **Time-to-first-frame and per-action latency** → dominated by SQLite query design, index quality, and pre-aggregated tables — not by the framework.
2. **Packaging risk** — "one `store.exe`, nothing else" is the genuinely hard requirement, and it's where languages actually differ.
3. **Your velocity as a solo dev** — the single biggest variable in a ~43-day build.

So the comparison below weights packaging + velocity + footprint over benchmark scores.

---

## 2. Option matrix

Scores 1–5, weighted for *this* project. Weights: single-exe quality 20, build friction 15, solo velocity 20, perf/footprint 15, ecosystem 15, maintainability 15.

| Stack | Single exe | Build friction | Velocity | Perf/RAM | Ecosystem | Maintain | **Score** |
|---|---|---|---|---|---|---|---|
| **Rust + Axum** | 5 | 3 | 3 | 5 | 3.5 | 4 | **78%** |
| Rust + Actix Web | 5 | 3 | 2.5 | 5 | 3.5 | 3.5 | **75%** |
| **Go + chi/Echo** | 5 | 5 | 4.5 | 4 | 4 | 4.5 | **90%** |
| **C# .NET NativeAOT** | 4.5 | 4 | 4.5 | 4 | 4.5 | 4.5 | **87%** |
| Bun/Node + TS | 4 | 4.5 | 5 | 2.5 | 5 | 3.5 | **82%** |
| Java/Kotlin + Quarkus native | 4 | 2 | 3 | 4 | 4 | 4 | **70%** |
| Python + FastAPI | 2 | 3 | 5 | 1.5 | 4.5 | 3 | **64%** |

The scores are *my* weights for *your* constraints. If you're fluent in one of these, that stack effectively gains ~10–15 points on velocity — which reorders the top four. Comfort beats theory here.

---

## 3. Each option honestly

### Rust + Axum — *recommended Rust option*
- **Exe:** ~8–14 MB static, no DLLs, no runtime. Startup <150 ms, RSS <60 MB.
- **Pros:** best possible footprint; `sqlx`/`rusqlite` excellent; `rust-embed` + `askama`/`maud` make "everything in one binary" idiomatic; compile-time correctness pays off across a financial domain where a silent bug costs money.
- **Cons:** slowest to *write* for a solo dev; MSVC toolchain + static-CRT setup on first build (mitigable: the `x86_64-pc-windows-gnu` target with mingw needs no MSVC at all, or just build on a GitHub Actions `windows-latest` runner); long compile times as the app grows.
- **When it wins:** you already know Rust, or you value the smallest/fastest binary and airtight types for money math.

### Rust + Actix Web — *fine, but not better here*
- Functionally equivalent outcome, ~5–15% more raw throughput, slightly larger learning surface (`web::Data`, generic extractor signatures, per-worker state), historically a more complex runtime model (actix-web 4 is a normal tokio-based framework now — the old actor criticism is outdated, but the API still feels heavier).
- **Rule: if you already know Actix, use Actix.** Switching frameworks for zero user-visible gain is waste. There is no performance argument for changing.

### **Go — the lowest-risk path to the same product**
- **Exe:** ~12–18 MB, fully static, **cross-compiles to Windows from any OS with one command** (`GOOS=windows go build`) using the pure-Go SQLite driver (`modernc.org/sqlite`) — no cgo, no MSVC, no static-CRT puzzle, no Windows dev machine needed.
- **Pros:** kills the single biggest packaging risk outright; ~10–20× faster compile than Rust; goroutines make the sync worker trivial; `database/sql` + `embed` are textbook for this exact app; one binary, no runtime.
- **Cons:** less expressive domain modelling (no sum types/enums with payloads → more `switch` boilerplate); reports/codegen are more manual; error handling is verbose.
- **When it wins:** you're not already a Rust dev. It gets you the identical product with less toolchain pain — this is the pragmatic recommendation for most solo builders.

### C# / .NET 9 NativeAOT
- **Exe:** ~15–25 MB, self-contained single file, startup ~30–60 ms.
- **Pros:** best-in-class Windows integration (tray, autostart, Windows Service, DPAPI for the Neon credential), superb tooling/debugger, `Microsoft.Data.Sqlite` solid, ADO.NET + Dapper is pleasant, EF Core if you want migrations for free. Very high velocity if you know C#.
- **Cons:** NativeAOT has trimming/reflection sharp edges (needs care with serializers); slightly larger binary; you may end up fighting AOT warnings rather than the app.
- **When it wins:** you already know C# / have a .NET background. Strongly preferred over Rust in that case.

### Bun / Node + TypeScript
- **Exe:** 50–100 MB (bundled runtime), startup ~40–80 ms, RSS 120–200 MB (`bun build --compile` is genuinely good; Node SEA is clumsier).
- **Pros:** one language for backend + SPA, best dashboard/component ecosystem, `bun:sqlite` is fast, fastest to prototype the whole thing.
- **Cons:** biggest footprint and RAM by far; "single exe" is technically met but heavier and more fragile on antivirus; long-term dependency churn.
- **When it wins:** you're a JS/TS developer and the 100 MB binary doesn't bother you.

### Python / FastAPI — *not recommended*
- PyInstaller/Nuitka exe 25–50 MB, **startup 0.5–2 s** (bad first impression every launch), highest antivirus false-positive rate of any option, awkward to guarantee "no other file needed". Dev velocity is great; the delivery story isn't. Rejected on the hard requirement, not on taste.

### Java/Kotlin, Elixir, Deno, Tauri — rejected for v1
- JVM: 40–80 MB + slower startup (GraalVM native fixes perf but adds a hard build pipeline).
- Elixir/Phoenix: `mix release` works offline and is robust, but it's an exotic stack for a shop app and overkill for one till.
- Tauri/Electron: these are *desktop shells*, not servers. They contradict "server on localhost opened in the browser" and add a whole frontend-runtime layer for nothing.

---

## 4. The decision that actually matters more

**Frontend architecture has a bigger effect on perceived speed than the backend language.**

Three viable paths:

| Path | Assets | Notes |
|---|---|---|
| **Server-rendered HTML + htmx** (Askama/Maud for Rust, `html/template` for Go) | ~150–300 KB | One language, zero JS build step, zero node_modules, instant. Charts need a small JS lib (ECharts ~150 KB). Best raw simplicity for a POS. |
| **SvelteKit / SolidJS static export** embedded via `rust-embed`/`embed` | ~200–400 KB | Real SPA, tiny runtime, good for the dashboard half. Needs a Node build step in CI only. |
| **React/Next SPA** | ~800 KB–1.5 MB | Only if you want the same component ecosystem as the Vercel dashboard. Slower first paint, more tooling. |

**My recommendation: htmx + server-rendered for the POS/till screens** (keyboard-driven, form-heavy, latency-sensitive) **and a small SPA or ECharts page for the analytics dashboard.** This is the "very fast" answer — and it means one language end-to-end.

---

## 5. Recommendation

> **If you're not already fluent in Rust: Go.**
> **If you are fluent in Rust: Axum.**
> **If you're a C# dev: .NET NativeAOT.**
> **Actix specifically: only if you already know it.**

Ranked by expected outcome for a solo build of this exact app:

1. **Go + chi + `modernc.org/sqlite` + `embed`** — 90. One static exe, cross-compiled from anywhere, fastest to ship, zero packaging surprises. *Best risk-adjusted choice.*
2. **.NET 9 NativeAOT** — 87. Best if C# comfort and Windows-native tooling matter.
3. **Rust + Axum** — 78. Best engineering ceiling, smallest binary, steepest ramp. Choose if Rust is known or desired as a skill investment.
4. **Bun/TS** — 82 on velocity, but 100 MB and churn. Choose only on JS familiarity.
5. Rust + Actix — 75. Equivalent outcome to Axum with a heavier API; keep only if already familiar.

**And the honest meta-point:** at 2 req/s, *all* of these are fast enough. The difference that will actually decide whether this system is pleasant to use is (a) index/pre-aggregation design, (b) the POS keyboard flow, and (c) how boring the exe build is. Only (c) meaningfully differs across languages — and Go wins it.

---

## 6. This decision is cheap to reverse — deliberately

Because `PLAN.md` puts all framework coupling in a thin layer, the swap cost is days, not a rewrite:

- Handlers stay thin: parse → call a **pure domain function** → serialize. No framework types in business logic.
- All SQL in a **repository module** behind traits/interfaces (`rusqlite`/`database/sql`/Dapper are swappable).
- **The API is the contract**, not the framework: `/api/v1/*`, JSON only, documented. The SPA / htmx frontend never learns which server it's talking to.
- Aggregation code (§7–§8 of `PLAN.md`) is **pure functions over rows** — identical in any language, and it's the part that must never drift.

So we are not betting the project on this choice. Pick on comfort; the architecture insulates us.

---

## 7. What I need from you

1. **Which languages are you actually fluent in?** (Real answer dominates all scoring above. "None yet, but willing to learn" → Go.)
2. **Do you have a Windows machine with a working build toolchain**, or should the exe be built in GitHub Actions and downloaded? (If CI: the friction difference between stacks nearly vanishes.)
3. **Server-rendered + htmx, or a real SPA** for the UI? (This matters more than the backend.)
4. **Confirm Actix is still wanted** — or accept the recommendation above.

Once you answer, I'll update `PLAN.md` §3 and record the decision here as *accepted*, with the crate/module list and the build recipe for the chosen stack.
