# SEC-001 — Dashboard authentication & data encryption

**Status:** ✅ **ACCEPTED** (encryption + password + roles). Defaults chosen: **unattended DPAPI**, **6-digit PIN**, **10 min POS / 30 min dashboard**, **2FA in v1.1**, **exports plain-with-warning**.
**Date:** 2026-10-20
**Relates to:** PLAN §12, ADR-011 (audit), ADR-003 (control plane), DESIGN-001

---

## 1. Threat model — what we're actually defending against

Encryption decisions are meaningless without naming the adversary. Ranked by real likelihood in a Tunisian phone shop:

| # | Threat | Likelihood | Defended by |
|---|---|---|---|
| 1 | **The PC is stolen / the drive is pulled** | medium | **encryption at rest** (§4) |
| 2 | Backup copy (USB / Drive) leaks or is lost | medium | encrypted backups (§4.5) |
| 3 | Employee / relative opens the till and reads margins, debts, or changes prices | **high** | **auth + roles** (§2) |
| 4 | Another user account on the same PC opens a browser and hits `127.0.0.1:8642` | high | session auth + `Host`/`Origin` checks (§2.6) |
| 5 | Malicious webpage in another tab POSTs to localhost (CSRF / DNS rebinding) | medium | Origin + custom-header + Host validation |
| 6 | Eavesdropping on the sync channel | medium | TLS + device token (§5) |
| 7 | Someone with local admin + the exe decrypts the DB | low | **cannot be stopped** — documented honestly (§4.6) |
| 8 | Owner/partner dispute: "who changed this?" | high | ADR-011 audit log, not encryption |

**Design consequence:** the password protects the **UI and the operating session**, encryption protects the **file**. Neither protects against a local administrator with the running binary — no offline-first product can, and claiming otherwise would be dishonest.

---

## 2. Local dashboard authentication

### 2.1 Accounts & roles
| Role | Sees | Can do |
|---|---|---|
| **Owner** | everything | all edits, settings, journal, pricing, debts, closing, user management, export |
| **Manager** | no encryption/key settings, no user management | sales, stock, deliveries, debts, reports, price changes (audited) |
| **Cashier** | **no cost, no margin, no profit, no other users' history** | sell, scan, returns (own day), debt collect, own session |

Roles are enforced **server-side on every endpoint** (a `require(Role::Owner)` guard on the route, not a hidden button). A cashier calling `/api/journal` gets `403`, not an empty page.

### 2.2 Password & PIN strategy (two speeds, on purpose)
- **Owner / manager → password.** Minimum 8 chars, no forced complexity theatre, block the 1 000 most common passwords + the shop name. Hash: **Argon2id**, `m = 64 MiB, t = 3, p = 1`, 16-byte random salt, PHC string stored. Verify on a fixed thread pool (never on the HTTP worker).
- **Cashier → 6-digit PIN** for speed (a cashier cannot type a password 200×/day). Stored with Argon2id as well (`m = 19 MiB, t = 2`). PINs are only usable to *open a session for that cashier*; they cannot reach settings, costs, journal, or exports.
- First run: owner sets the password **before** the app opens the till — a 2-field screen (password, confirm), skippable only into a locked state (see §2.7).
- Password change requires the old password; reset requires the **recovery code** (§4.4), never "email us".

### 2.3 Sessions
| Property | Value |
|---|---|
| Session id | 256-bit CSPRNG, **stored hashed** (SHA-256) server-side; the raw value never touches disk |
| Cookie | `HttpOnly; SameSite=Strict; Path=/` (no `Domain`). `Secure` only when TLS is on (LAN mode) |
| Idle timeout | configurable: 5 min (cashier) / 30 min (owner), default **10 min POS, 30 min dashboard** |
| Absolute lifetime | 12 h, then re-login |
| Concurrency | max 5 sessions; "log out other devices" button |
| Storage | `sessions(id_hash, user_id, created_at, last_seen, expires_at, device_id, ip, user_agent)` |
| Revoke | on password/PIN change, on role change, on "logout all" |

**Auto-lock** on the POS is essential in a shop: after idle timeout the screen shows a lock cover with the cashier's name and a PIN field. In-progress cart is preserved (encrypted in memory, flushed to a `draft_cart` row so a crash keeps it).

### 2.4 Login throttling (safe against lockout DoS)
- Per-account: 5 failures → 30 s, then 1 m, 5 m, 15 m (exponential, jitter), **max 15 min**. Never permanent lockout.
- Per-IP (loopback is shared, so this is mostly a LAN feature) and a global cap.
- Every attempt logged in `audit_events` (`kind = login | login_fail`) with user + IP + device.
- **Owner recovery:** recovery code → immediate unlock. A refused login must never be able to brick a shop's till for a day.

### 2.5 CSRF / DNS-rebinding / localhost hardening
A localhost web app is reachable from **any page the cashier opens** — this is a real, commonly-ignored attack surface:

| Control | Implementation |
|---|---|
| **Host header validation** | reject any request whose `Host` is not `127.0.0.1:<port>` or `localhost:<port>` → kills DNS rebinding |
| **No CORS** | `Access-Control-Allow-Origin` never sent; preflights rejected |
| **Origin/Referer check** on every non-GET | must match the app origin, else `403` |
| **Custom header requirement** | all mutations need `X-Store-Client: 1` — an HTML form/`<img>` cannot set it |
| **CSRF token** (double-submit) | belt-and-braces on top of the above |
| **Bind loopback only** | `127.0.0.1`, never `0.0.0.0`, in v1 |
| **CSP** | `default-src 'self'; img-src 'self' data:; script-src 'self'` — no inline scripts, no CDNs (everything is embedded anyway) |
| **`X-Content-Type-Options: nosniff`**, `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY` | boilerplate, free |

### 2.6 Sensitive-action re-auth
Owner PIN/password re-prompt (not a new session) for: deleting a sale, exporting full data, changing encryption settings, disabling the audit log view, changing a user's role. Logged as `kind = reauth` + the action.

### 2.7 Locked state (first run without a password)
If the owner skips password setup, the app runs in **`locked_setup`**: selling is possible **only** with a temporary cashier PIN printed on the setup screen, and all owner screens (dashboard, journal, settings, costs) are inaccessible until a password is set. The till never becomes unusable — consistent with ADR-002's "never block the work" rule.

---

## 3. Two dashboards, one standard

| | Local dashboard (`store.exe`) | Web dashboard (Vercel) |
|---|---|---|
| Auth | session cookie, Argon2id | same + **optional TOTP 2FA** (recommended) |
| Sessions | 1 owner session typical | **1 concurrent session** (ADR-003), revocable |
| Transport | loopback HTTP (see §5) | HTTPS only, HSTS |
| Role data | full local DB | **aggregates only**, no names/phones (ADR-003) |
| Cookies | `HttpOnly; SameSite=Strict` | `__Host-` prefix + `Secure` + `HttpOnly` + `SameSite=Lax` |
| Extra | — | rate-limited login, CSP, no public signup, admin actions behind a separate secret |

2FA on the web dashboard is cheap (TOTP, 6 digits, ±1 window) and worth it: that dashboard exposes every shop's revenue.

---

## 4. Encryption at rest (the important half)

### 4.1 Options considered
| Option | Protects | Cost | Verdict |
|---|---|---|---|
| **SQLCipher** (whole-DB AES-256, page level) — `rusqlite` `bundled-sqlcipher` | *everything*: tables, indexes, WAL, free pages | ~3–8 % write overhead; needs key management | ✅ **chosen** |
| App-level field encryption (AES-GCM on selected columns) | only the columns you remember to encrypt | fragile: a new query leaks plaintext | ❌ alone (kept as an extra for notes/ID photos) |
| Rely on BitLocker | full disk | zero code | ❌ cannot be assumed; off on many shops, and doesn't protect the backup |
| Plain DB + password on the UI only | nothing at rest | zero | ❌ **that's a screen lock, not encryption** |

**Chosen: SQLCipher for the database + encrypted backups + encrypted image/attachment blobs.** A stolen PC or a copied `.db` file is unreadable.

### 4.2 Key hierarchy
```
                    ┌──────────────────────────────────────┐
                    │  K_db  (32 random bytes, SQLCipher)  │  ← never leaves RAM
                    └───────────────▲──────────────────────┘
                                    │ unwrapped at startup
        ┌───────────────────────────┴────────────────────────────┐
        │  Master key (32 bytes)                                 │
        │  = DPAPI(Protect) on Windows  →  data/.keyring         │
        │  (optional) + Argon2id(passphrase) wrap, if enabled    │
        └───────────────────────────▲────────────────────────────┘
                                    │ printed once at setup
                       Recovery code (Crockford base32, 8×5 chars)
```
- **Default (unattended):** master key is wrapped with **Windows DPAPI (machine + user scope)** → the shop PC reboots after a power cut and the till opens by itself. No prompt, no forgotten password.
- **Paranoid mode (optional, per shop):** master key is additionally wrapped with **Argon2id(owner passphrase)**; the app asks once per boot. Trade-off stated plainly in the UI: *"if you enable this, a reboot requires the passphrase — a power cut at 08:00 means nobody sells until the owner arrives."*
- **Recovery code** shown exactly once at setup with a "I wrote it down" confirmation, printable (PDF) and suitable for a safe. It unwraps the DB on a new machine.
- Keys held in `zeroize`-wrapped buffers; never logged, never in a crash dump (set `panic = abort` already), never in the URL.

### 4.3 What SQLCipher covers
`store.db`, `store.db-wal`, `store.db-shm` (page-level), plus:
- **`data/images/*`** (product photos, trade-in ID photos, warranty scans) → AES-GCM per file, key derived from `K_db`; filenames are random hashes, not product IDs.
- **Logs** → no PII by rule; still encrypted if `settings.encrypt_logs`.
- **Export files** → user-initiated exports are written **unencrypted by default** (the owner needs to open them) with a warning, or optionally password-protected (`.zip` + AES-256) — one checkbox.

### 4.4 Backup, rotation, recovery
| Item | Rule |
|---|---|
| Backup format | SQLCipher DB copy (`VACUUM INTO` on the unlocked key) → **already encrypted**; or `age`/AES-GCM archive if exported off-box |
| Backup key | same `K_db` derived from the master key → **restoring on another machine requires the recovery code** (must be documented loudly at setup) |
| Verify | `--doctor` opens each recent backup with the key and runs `PRAGMA integrity_check` + a row count; a backup that can't be opened is a *failure*, surfaced in the UI |
| Rotation (`--rekey`) | re-encrypt with a new key (SQLCipher `PRAGMA rekey`), rotate the recovery code, invalidate old sessions |
| Lost key | **unrecoverable — by design.** The setup flow forces the recovery code to be acknowledged before the first sale; the dashboard shows a persistent warning chip until a code is confirmed stored |
| Corrupt/restore | `--restore <file>` + `--doctor` verifies the chain (ADR-011) after restore |

### 4.5 Honest boundaries
- **Stolen disk / stolen PC → protected.** ✅
- **Local admin with the exe and the DPAPI user session → not protected.** Documented, not hidden. Mitigation is physical + Windows account hygiene, not crypto.
- **A RAM dump while the app runs** → not protected.
- **The shop's own recovery code leaked** → protected only until someone finds it. The UI says so.

### 4.6 Performance budget
| Operation | Before | With SQLCipher (target) |
|---|---|---|
| Sale commit (10 lines + audit + stock) | ~15–20 ms | **≤ 25 ms** |
| Product grid query (10 k SKUs) | ~5–8 ms | **≤ 10 ms** |
| Startup (open + decrypt + integrity quick check) | <150 ms | **≤ 250 ms** |
| Backup of a 500 MB DB | — | ~3–6 s, background, never on the sale path |

Measured in CI on a cheap shop PC profile (2-core, SATA SSD); a crypto regression fails the build.

---

## 5. Encryption in transit

| Channel | Decision |
|---|---|
| **Browser ↔ local app (v1)** | HTTP on **loopback only**. Rationale: traffic never leaves the machine; TLS with a self-signed cert produces browser warnings that scare shopkeepers more than it protects. Protections are cookie + Origin/Host/CSRF (§2.5), and the port is bound to `127.0.0.1` |
| **LAN mode (v2: phone stock count)** | **HTTPS required**, self-signed cert pinned at pairing, plus a pairing PIN. No plain HTTP off-loopback, ever |
| **Exe ↔ control plane (sync/licence)** | TLS 1.2+ via rustls, certificate verification enforced, **device token** auth, payload ≤ 256 KB, no DB credentials on the client (ADR-003) |
| **Control plane ↔ Neon** | TLS `sslmode=require`, pooled endpoint, least-privilege roles |
| **Vercel ↔ browser** | HTTPS + HSTS + secure cookies |

---

## 6. Secrets inventory (where each lives)

| Secret | Location | Protection |
|---|---|---|
| Owner password / cashier PIN | SQLite `users` | Argon2id hash (never reversible) |
| `K_db` | RAM only | `zeroize`, never persisted in plaintext |
| Master key | `data/.keyring` | DPAPI (+ optional passphrase wrap) |
| Recovery code | owner's safe (paper/PDF) | out of band, the *only* copy |
| Device token (sync) | SQLite `settings` | encrypted by `K_db` at rest, never in config |
| Dashboard session secret | local DB | encrypted by `K_db` |
| Neon credentials | **never on the client** | server-side env only |
| Admin API secret | Vercel env | server-side only |

---

## 7. Audit integration (ADR-011)
Every security-relevant event goes to `audit_events` with no exceptions: `login`, `login_fail`, `logout`, `reauth`, `password_change`, `pin_change`, `role_change`, `user_create/disable`, `session_revoke`, `export`, `backup`, `restore`, `rekey`, `key_recovery`, `settings_change`. A security section in the owner's Journal with presets: *failed logins*, *permission changes*, *exports*, *key operations*.

---

## 8. UX cost control (DESIGN-002 R1)
- Login = **one field + Enter**, remembered username per device.
- Cashier locking = **PIN only, numeric pad, 4 taps**. No username.
- No password expiry, no forced rotation (it produces `12345678` variations), no security questions.
- Never more than **one** auth prompt per action; re-auth only on destructive/export actions.
- A forgotten password is a **recovery-code** flow, offline, 30 seconds — not a support ticket.

---

## 9. Effort

| Item | Days |
|---|---|
| Users/roles/sessions + Argon2id + throttling + lock screen + re-auth | 2.0 |
| Localhost hardening (Host/Origin/CSRF/custom header/CSP) + tests | 0.5 |
| SQLCipher integration (bundled), migration of a plaintext DB, pragma tuning | 1.5 |
| Key hierarchy: DPAPI wrap, optional passphrase, recovery code, `--rekey`, lock/unlock UI | 2.0 |
| Encrypted images/blobs + encrypted backups + `--doctor` verify + restore flow | 1.0 |
| Web dashboard auth hardening + optional TOTP 2FA | 1.0 |
| Security test suite (CSRF, rebinding, brute force, key loss, restore, perf budget) | 1.0 |
| **Total** | **9.0 d** |

**Placement:** auth + SQLCipher + key hierarchy go in **M0** (a plaintext DB must never exist in the field, and migrating later means re-encrypting live shop data). 2FA/journal-security presets land with M6/M4.

---

## 10. Decisions

1. **Default keys:** unattended DPAPI (reboot → till opens, recommended) or passphrase-at-boot (safer, but a power cut locks the shop)?
2. **Cashier PIN length:** 6 digits (recommended) or 4 (faster, weaker)?
3. **Idle lock timeout:** 10 min POS / 30 min dashboard — or shorter for the dashboard?
4. **Web dashboard 2FA:** build TOTP in v1, or v1.1?
5. **Export encryption:** default plain files with a warning (recommended) or password-protected by default?
