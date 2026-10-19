# ADR-015 — Local web app vs desktop app (which window, not which architecture)

**Status:** ✅ **ACCEPTED** — Option **A** (local web app in app-mode window); `wry --window` kept as a documented later fallback.
**Date:** 2026-10-20
**Relates to:** ADR-014 (maud, Rust-only), ADR-003 (control plane), STRATEGY-002 S11 (LAN second till)

---

## 1. Reframing the question

"Web app or desktop app?" is usually a UI question. In this product, **three of your requirements already force an HTTP server on the shop PC**, whichever UI you pick:

| Requirement | Needs an HTTP server? |
|---|---|
| **LAN second till / phone camera stock count** (STRATEGY-002 S11) | ✅ the phone must *reach* the till over WiFi |
| **Cloud dashboard + sync** | ✅ server-side aggregation before upload (and the exe must keep serving while syncing) |
| **Thermal receipt printing, cash drawer, backups, `--doctor`** | ✅ these live in the Rust process, not in a UI toolkit |
| Barcode scanner | ❌ keyboard wedge, works anywhere |

So the real question is narrower:

> **Which window shows the UI?** The engine behind it is `store.exe` + axum + maud either way.

This matters because it means **the decision is reversible in days**, not weeks — and you should stop worrying about it.

---

## 2. The four real options

| | **A. Browser in app mode** ⭐ *current plan* | **B. `wry` webview shell** (~200 lines) | **C. Tauri 2** | **D. Native GUI** (egui/Iced/Slint) |
|---|---|---|---|---|
| What the shop sees | a chrome-less window with your icon/title, no URL bar | same | same | a real native window |
| Extra dependency | **none** (uses installed Edge/Chrome) | WebView2 runtime | WebView2 + Node/Tauri CLI | none |
| Single exe | ✅ | ✅ (still needs WebView2 present) | ⚠️ needs bootstrapper on old Win10 | ✅ |
| **Arabic + RTL quality** | ✅ **excellent** (browser text engine, bidi, shaping, Amiri) | ✅ same engine | ✅ same engine | ⚠️ **weak** (see §3) |
| UI code shared with cloud dashboard | ✅ **same maud components** | ✅ same | ✅ same | ❌ **second UI to build** |
| LAN second till | ✅ free | ✅ (server still runs) | ✅ | ✅ server, but native UI can't serve LAN |
| Thermal printing / drawer | ✅ raw ESC/POS from Rust | ✅ | ✅ | ✅ |
| Dev velocity | ✅ DevTools, snapshot tests, instant reload | ✅ | ⚠️ framework overhead | ❌ slow iteration |
| Perf (this app) | ✅ 120 ms render, 30 ms swaps | ✅ same | ✅ same | ⚠️ faster frames, *slower to build* |
| Kiosk / tray / global hotkeys | ⚠️ browser limits (kiosk mode works) | ⚠️ limited | ✅ | ✅ best |
| Risk of "feels like a website" | 🟡 the only real downside | ✅ low | ✅ low | ✅ none |

---

## 3. Why native GUI is the weakest option *for this product*

Not a knock on Rust GUI — a mismatch with your requirements:

1. **Arabic/RTL is where immediate-mode GUIs still struggle.** `egui` has limited bidi/shaping quality; RTL layout is manual. Your app is *Arabic-first with diacritics* (the Bismillah alone requires proper naskh shaping) and needs full RTL mirroring. Browser engines have had 20 years of investment here; you'd be fighting the toolkit on your most visible screen.
2. **You already need HTML for the cloud dashboard.** Choosing native locally means **maintaining two UIs forever** (maud for the cloud + egui/iced for the till) — double the design work, double the RTL work, double the bugs, and the two will drift visually.
3. **Native doesn't remove the server** (LAN till, sync, printing) — so you pay the server cost *and* the second-UI cost.
4. **Everything else is a wash:** the DB and rendering aren't your bottleneck; a browser renders your pages in ~120 ms and htmx swaps in ~30 ms.
5. Slint is the one native toolkit with decent RTL/Arabic — but it's still a second UI, still weaker than the browser text engine, and adds a markup language file type (`.slint`), which brushes against your "HTML only in `.rs`" rule in spirit.

---

## 4. Recommendation

> **A — local web app, opened in app mode.** Keep exactly the planned architecture.
> **Add B (`--window`) later, only if a shop asks for a real window.** It is a ~200-line wrapper around the same server and the same maud pages: no rewrite, no second UI.

Why A wins on the merits:
- **Zero extra dependency** — Edge ships with every Windows 10/11. No WebView2-version roulette on old shop PCs.
- **One UI codebase** shared between the till and the cloud dashboard.
- **Best-in-class Arabic/RTL/bidi** for free.
- **LAN second till and phone scanning work by construction** — the differentiator.
- The "app" feel is a **launch detail**, not an architecture detail (§5).

### 4.1 Why not Tauri/Electron
Tauri 2 is genuinely good, but for an app that already runs its own HTTP server it adds a framework, a build pipeline, a WebView2 dependency and a second config surface to gain… a window. Electron adds 100 MB+ and a JS toolchain — ruled out by ADR-014.

---

## 5. Making A feel like a real desktop app (the actual work, ~1 day)

| Concern | How |
|---|---|
| Shopkeeper expects a double-clickable app | `store.exe --window` behaviour by default: start server → launch `msedge --app=http://127.0.0.1:8642 --window-size=1366,768` → **chrome-less window with your title and icon** |
| Browser profile pollution | launch with a dedicated `--user-data-dir=%LOCALAPPDATA%\OneClick\browser` → own history, no extensions, own zoom, no "restore pages" prompt |
| Accidental close | JS `beforeunload` guard *only* while a cart has items ("panier non enregistré") |
| They closed the tab and think it crashed | the server keeps running; the desktop shortcut re-opens the window instantly; the tray-less design means **the port is the lock** (second launch just re-opens the UI) |
| Kiosk / fullscreen | `--start-fullscreen` or `--kiosk` flag `settings.kiosk = true`, exit with a key combo + PIN |
| Branding | `.exe` icon, window title = shop name, custom `theme-color`, no visible URL to confuse them |
| Old PC with an ancient browser | fallback chain: Edge → Chrome → default browser; the page is plain server-rendered HTML (no ES2022 features), so even an old engine works |
| "Is it really offline?" | it is: the UI is served from the same machine; DevTools network tab shows zero external calls |

That's the entire cost of not using a native window: ~1 day of launcher polish.

---

## 6. Failure modes (and why they're mild)

| Failure | Impact | Mitigation |
|---|---|---|
| Browser auto-update mid-session | none — server unaffected, reopen window | single-instance re-launch |
| Shop has locked-down browser policy (GPO) | `--app` flag blocked | fallback to normal tab, or ship `--window` (`wry`) build for that shop |
| Windows 10 LTSC without WebView2 | option B/C would break; **A doesn't** | another reason A is the safest default |
| No browser at all | practically impossible on Win10/11 | `--doctor` detects and reports |
| Multiple tabs open | two views of the same DB — harmless (server-side state) | single-instance focuses the existing window; add a "another window is open" notice |
| Shop wants a Windows Service (runs before login) | not supported by a browser UI | out of scope for v1; a service + LAN till is the v2 answer |

---

## 7. Decision & cost

**Decision needed:** confirm **A** (local web app, app-mode window) as the default, with **B** as a documented later fallback.

| Path | Effort | Note |
|---|---|---|
| A (planned) | ~1 d launcher polish | included in M7 packaging |
| + B `wry --window` mode, later | ~2 d | same server, same pages, no UI change |
| C Tauri | ~4–5 d + dependency risk | not recommended |
| D native GUI | **+30 d** and a second UI forever | not recommended |

**Net effect on the plan: zero.** No milestone changes, no schema changes, no doc rewrites. That's the point — the question looked architectural and turned out to be cosmetic.

---

## 8. The one sentence

> **You are building a web app that happens to ship as a double-clickable `.exe`.** The server is the product; the browser is just the window — and it's the only window that gives you Arabic, RTL, LAN tills and one UI codebase for free.
