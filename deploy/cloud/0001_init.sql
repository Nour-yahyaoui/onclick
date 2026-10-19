-- ============================================================================
--  OneClick control plane — cloud schema (Neon Postgres)
--  Run once against the Neon database:
--     psql "$DATABASE_URL_UNPOOLED" -f deploy/cloud/0001_init.sql
--  Aggregates + entitlement ONLY. No customer names, no phones, no invoices.
-- ============================================================================

CREATE TABLE IF NOT EXISTS shops (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    plan          TEXT NOT NULL DEFAULT 'trial',       -- trial | local | cloud | compliance
    paid_until    TIMESTAMPTZ,
    status        TEXT NOT NULL DEFAULT 'active'       -- active | lapsed | suspended
);

CREATE TABLE IF NOT EXISTS devices (
    id            TEXT PRIMARY KEY,
    shop_id       TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    label         TEXT,
    token_hash    TEXT NOT NULL,                       -- sha256(token + pepper)
    machine_fp    TEXT,
    app_version   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ,
    revoked_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_devices_shop ON devices(shop_id);

CREATE TABLE IF NOT EXISTS pairing_codes (
    code_hash     TEXT PRIMARY KEY,                    -- sha256(code + pepper)
    shop_id       TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    expires_at    TIMESTAMPTZ NOT NULL,
    used_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One row per shop/day. Idempotent upsert key: (shop_id, day).
CREATE TABLE IF NOT EXISTS daily_agg (
    shop_id       TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    device_id     TEXT,
    day           DATE NOT NULL,
    revenue       BIGINT NOT NULL DEFAULT 0,           -- millimes
    cost          BIGINT NOT NULL DEFAULT 0,
    tickets       INTEGER NOT NULL DEFAULT 0,
    credit        BIGINT NOT NULL DEFAULT 0,
    cash_in       BIGINT NOT NULL DEFAULT 0,
    receivables   BIGINT NOT NULL DEFAULT 0,
    agg_version   BIGINT NOT NULL DEFAULT 1,
    pushed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (shop_id, day)
);
CREATE INDEX IF NOT EXISTS ix_daily_agg_day ON daily_agg(day DESC);

CREATE TABLE IF NOT EXISTS top_products_daily (
    shop_id       TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    day           DATE NOT NULL,
    product_name  TEXT NOT NULL,
    qty           INTEGER NOT NULL,
    revenue       BIGINT NOT NULL,
    PRIMARY KEY (shop_id, day, product_name)
);

CREATE TABLE IF NOT EXISTS sync_health (
    device_id     TEXT PRIMARY KEY,
    shop_id       TEXT NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    pending_days  INTEGER NOT NULL DEFAULT 0,
    app_version   TEXT
);

CREATE TABLE IF NOT EXISTS audit_events (
    id            BIGSERIAL PRIMARY KEY,
    at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    shop_id       TEXT,
    device_id     TEXT,
    kind          TEXT NOT NULL,                        -- pair | lease | ingest | admin
    summary       TEXT NOT NULL
);
