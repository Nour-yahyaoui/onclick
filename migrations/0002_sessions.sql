-- ============================================================================
--  OneClick — schema v2
--  Cash sessions (drawer / Z report), sale↔session link, product archiving.
-- ============================================================================

PRAGMA foreign_keys = ON;

-- ── Cash sessions: open the drawer, count it, close it (Z report) ───────────
CREATE TABLE IF NOT EXISTS cash_sessions (
    id             TEXT PRIMARY KEY,
    opened_at      TEXT NOT NULL,
    opened_by      TEXT NOT NULL REFERENCES users(id),
    opening_float  INTEGER NOT NULL DEFAULT 0,
    closed_at      TEXT,
    closed_by      TEXT REFERENCES users(id),
    counted_cash   INTEGER,
    expected_cash  INTEGER,
    difference     INTEGER,
    note           TEXT
);
CREATE INDEX IF NOT EXISTS ix_sessions_open
    ON cash_sessions(closed_at) WHERE closed_at IS NULL;

-- ── Link sales to the session they happened in (nullable: never block a sale) ─
ALTER TABLE sales ADD COLUMN session_id TEXT REFERENCES cash_sessions(id);
CREATE INDEX IF NOT EXISTS ix_sales_session ON sales(session_id);

-- ── Products: archiving (soft delete) is an explicit, audited action ───────
ALTER TABLE products ADD COLUMN archived_at TEXT;
ALTER TABLE products ADD COLUMN archived_by TEXT;

UPDATE schema_version SET version = 2;
