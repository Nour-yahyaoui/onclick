# Design documentation — shop management system

Implementation has started (**OneClick v0.5**, see `../README.md`). These records are the decisions it follows:

| Doc | What it settles |
|---|---|
| **[../PLAN.md](../PLAN.md)** | Master plan: architecture, schema, KPIs, sync, milestones, open decisions |
| `ADR-001-backend-stack.md` | Actix vs Axum vs Go vs .NET, single-exe guarantees, frontend choice |
| `ADR-002-licensing.md` | Trial, signed offline licence, why "block everything" is wrong |
| `ADR-003-control-plane-and-entitlement.md` | Exe → **Rust control plane** → Neon; licensing, device binding, offline sync |
| `DESIGN-001-theme-i18n.md` | "Simple POS" visual system, dark mode, Arabic/RTL, fonts, components |
| `ADR-005-barcode-and-batches.md` | Barcode-selling capture, lots/batches, FEFO, expiry |
| `ADR-006-phone-retail-vertical.md` | Serialized units, IMEI, variants, warranty, SAV, trade-in |
| `ADR-007-recording-modes.md` | **Record everything, require nothing** — compliance profiles A/B/C/D |
| `ADR-008-costing-when-prices-move.md` | Per-lot cost, FX, landed cost, repricing, replacement margin |
| `ADR-009-negotiated-pricing.md` | Price entered per sale, price dispersion, seller deviation, anonymous clients |
| `ADR-010-debts-and-credit.md` | The carnet: sale debts, free debts, tranches, settlement, aging |
| `DESIGN-002-simplicity-and-brand.md` | Simplicity doctrine, optional fields, money display, Bismillah masthead |
| `ADR-011-editable-with-attribution.md` | **Everything editable, everything attributed** — audit log, history drawer, journal |
| `ADR-012-local-image-upload.md` | PC image upload: validation, EXIF strip, WebP thumbs, encrypted storage, never synced |
| `ADR-013-ai-assistant-groq.md` | Groq assistant: derja/Arabizi intent extraction, numbers computed by our code, whitelisted metrics, fallbacks |
| `SEC-001-auth-and-encryption.md` | Dashboard password/roles/sessions, SQLCipher at rest, key hierarchy + recovery code, localhost hardening, 2FA |
| `STRATEGY-001-market-positioning.md` | Competitive position, TEIF/NACEF compliance wedge, pricing, signature features |
| `DESIGN-003-non-tech-users.md` | ✅ **Binding UX law for non-technical users**: jargon blacklist, button budget, error patterns, 6-item settings, 3-question first run, usability-test gate |
| `ADR-015-webapp-vs-desktop.md` | **Web app vs desktop app**: why the window is cosmetic, app-mode vs wry vs Tauri vs native comparison, "feels like an app" checklist |
| `ADR-014-rust-only-frontend.md` | **Rust repo rule**: no `.html` files, markup via `maud` in `.rs`, no Node; workspace layout; Rust control plane (Option A) |
| `OPS-001-environment-variables.md` | **Env contract for Vercel/Neon**: every var, generator commands, Neon roles, preview-vs-prod matrix, redeploy checklist |
| `STRATEGY-002-superpowers.md` | **My recommendations**: zero-loss journal, off-site backup, WhatsApp receipt, restock/cash advisor, carnet OCR import, LAN second till, and the pilot-slice argument |

## The rules that govern every decision

1. **The till never stops.** No network call on the sale path; read-only degradation at worst (`ADR-003`).
2. **Record everything, require nothing.** Total capture; no mandatory field the shop doesn't have (`ADR-007`).
3. **Everything editable, nothing anonymous.** Every change attributed: who, what, when, before → after (`ADR-011`).
4. **Every dinar is exact.** `i64` millimes, 3 decimals, no hidden rounding (`DESIGN-002`).
5. **The files are encrypted and the door has a lock.** SQLCipher at rest with a recoverable key; password + roles on the UI (`SEC-001`).
6. **A non-technical shopkeeper can use it unaided.** No jargon, ≤3 buttons, nothing to configure, explain the failure, a human one tap away (`DESIGN-003`).

## Status

**All decisions accepted (2026-10-20).** Stack locked: **Rust · axum · maud + htmx · SQLCipher · shared `domain` crate · Rust control plane (Option A) · web app in app-mode window.** Every ADR below is marked ✅ ACCEPTED; the remaining open questions are implementation details, not architecture.

- Roadmap restructured (PLAN §14): **Track P (pilot ≈68 d) · Track M (full v1 ≈111 d) · Track S (superpowers ≈22 d)**. Superpowers S1–S6 are folded into P0/P2/M7; S7–S14 unlock only after a shop has been live ~30 days.

- Decided by owner: recording modes (`ADR-007`), editable+attributed (`ADR-011`), phone vertical, negotiated pricing, debts, simplicity doctrine, Bismillah masthead.
- Still open: backend language, licensing model details, target segment, several per-feature defaults (each ADR ends with its own decision list).

## Reference assets

`assets/reference-simple-pos.png` — the visual reference the theme is derived from.
- [ADR-016](ADR-016-inventory-fills-itself.md) — the inventory fills itself; fixed charges
