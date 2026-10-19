-- ============================================================================
--  OneClick — schema v1 (first vertical slice)
--  Everything editable, nothing anonymous (ADR-011).
--  Money is INTEGER millimes everywhere (1 TND = 1000).
-- ============================================================================

PRAGMA foreign_keys = ON;

-- ── Identity ────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('owner','manager','cashier')),
    password_hash TEXT NOT NULL,                 -- Argon2id PHC string
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id_hash    TEXT PRIMARY KEY,                 -- sha256 of the cookie value
    user_id    TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    last_seen  TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    device     TEXT
);

-- ── Settings (one row per key; every key has a default in code) ─────────────
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ── Catalog ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS products (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    barcode       TEXT UNIQUE,
    -- reference price is OPTIONAL (ADR-009: price is entered at the sale)
    ref_price     INTEGER NOT NULL DEFAULT 0,
    last_cost     INTEGER NOT NULL DEFAULT 0,
    stock_qty     INTEGER NOT NULL DEFAULT 0,
    stock_avg_cost INTEGER NOT NULL DEFAULT 0,
    tracks_stock  INTEGER NOT NULL DEFAULT 1,
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    updated_by    TEXT,
    version       INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS ix_products_barcode ON products(barcode);
CREATE INDEX IF NOT EXISTS ix_products_name    ON products(name);

-- ── Sales ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS sales (
    id            TEXT PRIMARY KEY,
    number        TEXT NOT NULL UNIQUE,
    business_day  TEXT NOT NULL,                 -- YYYY-MM-DD, Africa/Tunis
    created_at    TEXT NOT NULL,
    user_id       TEXT NOT NULL REFERENCES users(id),
    total         INTEGER NOT NULL,
    paid          INTEGER NOT NULL DEFAULT 0,
    credit        INTEGER NOT NULL DEFAULT 0,
    cost_total    INTEGER NOT NULL DEFAULT 0,    -- frozen cost of what was sold
    payment       TEXT NOT NULL,                 -- cash | credit | card
    status        TEXT NOT NULL DEFAULT 'closed',
    note          TEXT
);
CREATE INDEX IF NOT EXISTS ix_sales_day ON sales(business_day);

CREATE TABLE IF NOT EXISTS sale_lines (
    id          TEXT PRIMARY KEY,
    sale_id     TEXT NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    product_id  TEXT NOT NULL REFERENCES products(id),
    product_name TEXT NOT NULL,                  -- snapshot: receipts never change
    qty         INTEGER NOT NULL,
    unit_price  INTEGER NOT NULL,                -- what was actually agreed
    unit_cost   INTEGER NOT NULL,                -- frozen lot cost (ADR-008)
    line_total  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_sale_lines_sale ON sale_lines(sale_id);

-- ── Stock ledger (append-only) ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS stock_movements (
    id         TEXT PRIMARY KEY,
    product_id TEXT NOT NULL REFERENCES products(id),
    kind       TEXT NOT NULL,                    -- purchase | sale | return | adjust | loss
    qty        INTEGER NOT NULL,                 -- signed
    unit_cost  INTEGER NOT NULL DEFAULT 0,
    day        TEXT NOT NULL,
    created_at TEXT NOT NULL,
    user_id    TEXT,
    ref_id     TEXT
);
CREATE INDEX IF NOT EXISTS ix_moves_product ON stock_movements(product_id, created_at);

-- ── Draft cart (server-side, survives a crash — ADR-011/SEC-001) ────────────
CREATE TABLE IF NOT EXISTS draft_lines (
    session_id  TEXT NOT NULL,
    product_id  TEXT NOT NULL REFERENCES products(id),
    qty         INTEGER NOT NULL DEFAULT 1,
    unit_price  INTEGER NOT NULL,
    PRIMARY KEY (session_id, product_id)
);

-- ── Debts / carnet (ADR-010) ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS debts (
    id                TEXT PRIMARY KEY,
    debtor_name       TEXT NOT NULL,
    debtor_phone      TEXT,
    sale_id           TEXT REFERENCES sales(id),
    principal_millimes INTEGER NOT NULL,
    opened_day        TEXT NOT NULL,
    due_day           TEXT,
    reason            TEXT,
    status            TEXT NOT NULL DEFAULT 'open',  -- open | settled | written_off
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_debts_status ON debts(status);

CREATE TABLE IF NOT EXISTS debt_ledger (
    id             TEXT PRIMARY KEY,
    debt_id        TEXT NOT NULL REFERENCES debts(id),
    kind           TEXT NOT NULL,                -- open | payment | adjustment | write_off
    amount_millimes INTEGER NOT NULL,            -- signed: + increases what is owed
    method         TEXT,                         -- cash | card | transfer
    day            TEXT NOT NULL,
    at             TEXT NOT NULL,
    user_id        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_ledger_debt ON debt_ledger(debt_id, at);

-- ── Audit (ADR-011: append-only, never edited, never deleted) ───────────────
CREATE TABLE IF NOT EXISTS audit_events (
    id           TEXT PRIMARY KEY,
    at_utc       TEXT NOT NULL,
    business_day TEXT NOT NULL,
    actor_id     TEXT NOT NULL,
    actor_name   TEXT NOT NULL,
    actor_role   TEXT NOT NULL,
    kind         TEXT NOT NULL,      -- create | update | delete | sale | payment | login | setup
    entity       TEXT NOT NULL,      -- sale | product | debt | user | session | settings
    entity_id    TEXT NOT NULL,
    summary      TEXT NOT NULL,      -- human readable, shown in "Qui a fait quoi"
    before_json  TEXT,
    after_json   TEXT,
    device       TEXT
);
CREATE INDEX IF NOT EXISTS ix_audit_entity ON audit_events(entity, entity_id, at_utc DESC);
CREATE INDEX IF NOT EXISTS ix_audit_day    ON audit_events(business_day);
CREATE INDEX IF NOT EXISTS ix_audit_actor  ON audit_events(actor_id, at_utc DESC);

-- ── Schema version ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);
INSERT INTO schema_version (version)
SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM schema_version);
