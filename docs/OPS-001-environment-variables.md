# OPS-001 — Environment variables & deployment contract

**Status:** spec — nothing to deploy yet, but this unblocks coding
**Date:** 2026-10-20
**Scope:** the **control plane** (Rust: axum + maud, `crates/control` — host-agnostic: Fly.io / Render / VPS; Next.js/Vercel only if Option B of `ADR-014` is chosen), the **Neon** database, and the few **build-time** constants the `.exe` needs.
**Relates to:** ADR-003 (control plane owns all DB creds), ADR-013 (Groq), SEC-001 (keys), STRATEGY-002 S4 (off-site backup)

---

## 1. The rule that decides everything

> **The shop's PC holds no cloud secrets. Every credential lives on Vercel.**

That's why the `.exe` needs **zero runtime secrets** (ADR-003): it authenticates with a revocable device token, never with a database password. So the entire environment contract below is **server-side only**, and a stolen shop PC cannot compromise the backend.

Second rule (Rust stack, `ADR-014`): **there is no `NEXT_PUBLIC_*` surface at all** — the browser receives finished HTML from maud, so *every* variable below is server-side by construction. In dev, load them with `dotenvy` from `.env`; in prod, real environment variables (Fly secrets / Docker env / systemd unit).

---

## 2. Complete variable list

Legend: **R** = required · **O** = optional (feature-gated) · **B** = build-time constant for the exe (not a Vercel var)

### 2.1 Core — required from day one

| Var | Req | Value / example | Notes |
|---|---|---|---|
| `DATABASE_URL` | **R** | `postgresql://control_plane:PASS@ep-xxx-pooler.eu-central-1.aws.neon.tech/neondb?sslmode=require` | **Neon *pooled* endpoint** (contains `-pooler`). Used by Next.js server runtime only. Serverless functions exhaust direct connections |
| `DATABASE_URL_UNPOOLED` | **R** | `postgresql://control_plane:PASS@ep-xxx.eu-central-1.aws.neon.tech/neondb?sslmode=require` | Direct endpoint — used **only** by migrations (`drizzle-kit`/`prisma migrate`). Never at runtime |
| `DASHBOARD_DATABASE_URL` | **R** | pooled URL for role `dashboard_reader` | Read-only role; every analytics query uses this. A bug in a chart cannot write |
| `SESSION_SECRET` | **R** | 32 random bytes, base64 (`openssl rand -base64 32`) | Signs/encrypts session cookies. Rotating it logs everyone out (acceptable) |
| `APP_URL` | **R** | `https://app.yourshop.tn` | Canonical origin: cookie scope, magic-link/redirect building, CORS-less same-origin checks |
| `NODE_ENV` | auto | set by Vercel | Never set manually to `development` in prod |

### 2.2 Identity, pairing & device tokens

| Var | Req | Value | Notes |
|---|---|---|---|
| `PAIRING_CODE_PEPPER` | **R** | 32 bytes base64 | HMAC for 6-digit pairing codes so a DB leak doesn't expose in-flight codes |
| `DEVICE_TOKEN_PEPPER` | **R** | 32 bytes base64 | Device tokens are stored **hashed**; this pepper prevents offline brute force |
| `TOTP_ISSUER` | O | `OneClick` | Label shown in the owner's authenticator app (ADR/SEC-001 §3 2FA) |
| `SESSION_IDLE_MINUTES` | O | `30` | Dashboard idle timeout (default 30) |
| `MAX_CONCURRENT_SESSIONS` | O | `1` | ADR-003: one dashboard session, new login revokes the old |

### 2.3 Licensing / entitlement (ADR-002, ADR-003)

| Var | Req | Value | Notes |
|---|---|---|---|
| `LICENCE_PRIVATE_KEY` | **R** | Ed25519 private key, base64 (PKCS#8) | **Signs lease tokens.** Server-only, never logged, never in the repo. If it leaks, leases can be forged |
| `LICENCE_PUBLIC_KEY` | **R** | Ed25519 public key, base64 | Also **baked into the exe at build** (`B`). Safe to expose |
| `LICENCE_KEY_ID` | O | `k1` | Lets you rotate signing keys later; the exe carries a keyring of public keys |
| `LEASE_DAYS` | O | `14` | Lease length for online shops |
| `GRACE_DAYS` | O | `14` | Offline tolerance → 28 days total before `LAPSED` (ADR-003 §1) |
| `SUSPEND_AFTER_DAYS` | O | `60` | `LAPSED` → `SUSPENDED` |

### 2.4 Sync ingestion

| Var | Req | Value | Notes |
|---|---|---|---|
| `INGEST_MAX_BYTES` | O | `262144` | 256 KB payload cap |
| `INGEST_MAX_DAYS_PER_BATCH` | O | `31` | Backfill chunk size |
| `SYNC_MIN_APP_VERSION` | O | `0.1.0` | Force-upgrade signal surfaced to old exes |

### 2.5 AI assistant (ADR-013) — omit all of these and the feature is simply off

| Var | Req | Value | Notes |
|---|---|---|---|
| `GROQ_API_KEY` | **O** | `gsk_...` | Server-side only; all calls go through `/api/ask` |
| `GROQ_MODEL` | O | `llama-3.1-8b-instant` | Intent extraction doesn't need a frontier model. Validate with the eval suite before changing |
| `GROQ_TIMEOUT_MS` | O | `4000` | After this, deterministic template answers instead |
| `AI_RATE_LIMIT_PER_HOUR` | O | `60` | Per shop |
| `AI_MONTHLY_BUDGET_CENTS` | O | `500` | Hard cap per shop per month (ADR-013 §6) |

**Graceful degradation is mandatory:** with `GROQ_API_KEY` absent, `/api/ask` must return the template-based answer, not a 500.

### 2.6 Off-site backup (STRATEGY-002 S4) — optional

| Var | Req | Value | Notes |
|---|---|---|---|
| `BACKUP_S3_ENDPOINT` | O | `https://<acct>.r2.cloudflarestorage.com` | Cloudflare R2 / Backblaze B2 / any S3-compatible |
| `BACKUP_S3_BUCKET` | O | `shops-backups` | |
| `BACKUP_S3_ACCESS_KEY_ID` | O | — | Scoped to that bucket only, write+read, no delete-listing beyond prefix |
| `BACKUP_S3_SECRET_ACCESS_KEY` | O | — | |
| `BACKUP_RETENTION_DAYS` | O | `90` | Server-side lifecycle policy should enforce the same |

**Zero-knowledge note:** these blobs are already encrypted *on the shop PC* with the shop's own key (SEC-001 §4.4). The bucket holds unreadable ciphertext — that property must never regress, so never add server-side decryption.

### 2.7 Notifications (daily 21:00 summary, STRATEGY-002 / STRATEGY-001 ③)

| Var | Req | Value | Notes |
|---|---|---|---|
| `TELEGRAM_BOT_TOKEN` | O | `123456:ABC...` | Free, automatic delivery |
| `RESEND_API_KEY` | O | `re_...` | Only if email summaries are wanted |
| `MAIL_FROM` | O | `noreply@yourshop.tn` | Must be a verified domain in Resend |
| `CRON_SECRET` | **R if crons exist** | 32 bytes base64 | Vercel sends `Authorization: Bearer $CRON_SECRET`; the route **must** verify it |

WhatsApp needs **no env**: receipts and statements are `wa.me` deep links built on the client (ADR-013 §7, STRATEGY-002 S7).

### 2.8 Ops & observability

| Var | Req | Value | Notes |
|---|---|---|---|
| `ADMIN_API_SECRET` | **R** | 32 bytes base64 | Guards `/api/admin/*` (set plan, extend grace, suspend, refunds). Separate from user auth on purpose |
| `LOG_LEVEL` | O | `info` | |
| `SENTRY_DSN` | O | — | Advisory: **do not send shop-identifying data**; aggregates only |
| `UPSTASH_REDIS_REST_URL` / `_TOKEN` | O | — | Only if you outgrow in-memory rate limits (serverless is stateless, so this becomes necessary for strict rate limits) |
| `FEATURE_CLOUD_DASHBOARD` | O | `true` | Cheap kill-switch for the whole dashboard during an incident |
| `FEATURE_AI` | O | `true` | Kill-switch for the AI assistant |
| `FEATURE_BACKUP_UPLOAD` | O | `false` | Off until S4 ships |

### 2.9 Exe-side constants (build-time, **not** Vercel vars)

| Var | Req | Baked where | Notes |
|---|---|---|---|
| `LICENCE_PUBLIC_KEY` | **B** | compiled into the binary | Same value as the Vercel var. A keyring of `{kid: pubkey}` supports rotation |
| `CONTROL_PLANE_URL` | **B** | default value, overridable per shop in `settings` | e.g. `https://api.yourshop.tn` |
| `APP_VERSION` | **B** | `build.rs` from git tag | Sent with every sync/licence call |

**The exe gets no `DATABASE_URL`, no `GROQ_API_KEY`, no S3 keys, no session secret. That's the design.**

---

## 3. Everything that must **never** be an env var in the client

| Never | Why |
|---|---|
| `NEXT_PUBLIC_DATABASE_URL`, `NEXT_PUBLIC_*_SECRET`, `NEXT_PUBLIC_GROQ_API_KEY` | Inlined into the JS bundle and readable by anyone with devtools |
| Neon *owner* credentials for the whole project | Only `control_plane` (RW on app tables) and `dashboard_reader` (SELECT) exist |
| The `LICENCE_PRIVATE_KEY` on the shop PC | Would let anyone mint licences |
| Any secret in `vercel.json`, `README`, a commit, a screenshot | Rotate immediately if it happens |

---

## 4. Environment matrix

| Var group | Local dev | Preview (PR) | Production |
|---|---|---|---|
| `DATABASE_URL*`, `DASHBOARD_DATABASE_URL` | Neon **dev branch** | Neon **preview branch** (auto per PR via the Neon↔Vercel integration) | Neon **main** |
| `SESSION_SECRET`, `*_PEPPER`, `ADMIN_API_SECRET` | dev values | **separate** values | prod values |
| `LICENCE_PRIVATE_KEY` | throwaway test key | test key (never prod) | prod key |
| `GROQ_API_KEY` | optional | shared (rate-limited) | prod key with budget cap |
| `TELEGRAM_BOT_TOKEN` | absent | absent | prod |
| Crons (`CRON_SECRET`) | n/a | **disabled** | enabled |

**Rule:** a preview deployment must never be able to touch production data or mint real licences. The Neon integration handles DB isolation per branch; the licence key and peppers are the two things you must set separately by hand.

---

## 5. Generation commands (run once, store in a password manager)

```bash
openssl rand -base64 32          # SESSION_SECRET, PAIRING_CODE_PEPPER, DEVICE_TOKEN_PEPPER, ADMIN_API_SECRET, CRON_SECRET

# Ed25519 licence keypair (private never leaves the server / your vault)
openssl genpkey -algorithm ed25519 -out licence_private.pem
openssl pkey -in licence_private.pem -pubout -out licence_public.pem
base64 -w0 licence_private.pem    # → LICENCE_PRIVATE_KEY  (single line)
base64 -w0 licence_public.pem     # → LICENCE_PUBLIC_KEY   (also baked into the exe)
```

Neon roles (least privilege):

```sql
-- run in the Neon SQL editor as the project owner
CREATE ROLE control_plane LOGIN PASSWORD 'STRONG';
CREATE ROLE dashboard_reader LOGIN PASSWORD 'STRONG';

GRANT USAGE ON SCHEMA public TO control_plane, dashboard_reader;

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES    IN SCHEMA public TO control_plane;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO control_plane;

GRANT SELECT ON ALL TABLES IN SCHEMA public TO dashboard_reader;

-- future tables created by migrations
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO control_plane;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO dashboard_reader;
-- dashboard_reader gets NO sequence grants and NO write grants: read-only by construction
```

---

## 6. Setting them on Vercel

```bash
# from web/
vercel link
vercel env add DATABASE_URL production
vercel env add DATABASE_URL preview
# ...repeat; or import all at once:
vercel env pull .env.local        # pull down for local work (never commit)

# bulk upload (one per line: NAME=value) is supported in the dashboard:
# Project → Settings → Environment Variables → paste .env format → choose environments
```

Mark every secret as **Sensitive** in the Vercel UI (write-only: it can't be read back in the dashboard, which is what you want for `LICENCE_PRIVATE_KEY`).

**Production-only**, set `CRON_SECRET` and add the cron (see §7).

---

## 7. `vercel.json` — cron schedule detail

```jsonc
{
  "crons": [
    { "path": "/api/cron/daily-summary", "schedule": "0 20 * * *" }
  ]
}
```

**Tunisia is UTC+1 all year (no DST since 2008)**, so **21:00 Tunis = 20:00 UTC**. Vercel cron runs in UTC — writing `0 21 * * *` would send the summary at 22:00 local. The cron route must:
1. verify `Authorization: Bearer $CRON_SECRET`,
2. be idempotent (safe to run twice — a unique `summary_sent(shop_id, day)` row),
3. cap work per invocation (Vercel function timeout), paginating shops across runs if needed.

---

## 8. Redeploy checklist (what "push and it works" actually requires)

| # | Check | Why it breaks otherwise |
|---|---|---|
| 1 | **Secrets never reach a template** | In Rust there's no bundle to leak into, but a careless `format!` into maud would print it — audit + CI grep |
| 2 | **Env validation at boot, with a clear message** | `envy`/`serde` struct parsed once at startup → one actionable error instead of 12 undefined later |
| 3 | **Optional features fail soft** | no `GROQ_API_KEY` → template answers; no S3 → backups skipped; no Telegram → summary logged and dropped |
| 4 | **Migrations run as a separate step** | two instances starting at once would race; use `DATABASE_URL_UNPOOLED` in a release task (`control-plane migrate`) before rolling out |
| 5 | **Every optional feature fails soft** | No `GROQ_API_KEY` → template answers; no S3 → backups skipped with a log; no Telegram → daily summary logged and dropped |
| 6 | **Preview never uses prod secrets** | §4 |
| 7 | **`NEXT_PUBLIC_*` audited** | Only `NEXT_PUBLIC_APP_URL` and feature-flag display values |
| 8 | **Post-deploy smoke test** | `GET /health` returns `{ db, migrations, ai, backups, crons }` — one curl tells you the deployment is real |

---

## 9. `.env.example` files in the repo

- `deploy/control-plane.env.example` — mirrors §2 with placeholders, committed. **Copy to `.env`, never commit `.env*`.**
- `.gitignore` — already updated to exclude every `.env*` variant except `.env.example`.

---

## 10. Decisions

1. **Domain** — `api.yourshop.tn` for the control plane and `app.yourshop.tn` for the dashboard, or a single Next.js app (recommended: **single app**, fewer moving parts, one env set).
2. **Neon region** — pick the one closest to Tunisia (currently **eu-central-1 / Frankfurt** is the practical choice; check what Neon offers at signup). Latency matters only for the dashboard, not the till.
3. **Licence key storage** — Vercel "Sensitive" env only, or additionally a KMS/vault? *(For one product, Sensitive env is enough; note that anyone with Vercel project access can mint licences.)*
4. **Cron schedule for summaries** — 21:00 local (recommended), and a second run at 09:00 for "yesterday's recap"?
