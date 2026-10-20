//! Persistence layer.
//!
//! Two rules enforced here:
//!  * **Every mutation goes through this module** and writes an `audit_events`
//!    row in the same transaction (ADR-011: everything editable, nothing
//!    anonymous).
//!  * **Money is `i64` millimes** everywhere (domain crate).
//!
//! v0.1 notes (tracked in PLAN.md):
//!  * SQLite is opened with the `bundled` feature; flipping to
//!    `bundled-sqlcipher-vendored-openssl` + `PRAGMA key` is the M0 encryption
//!    task. `doctor()` warns loudly while that is not done.
//!  * Batches/lots (ADR-005) and serialized units (ADR-006) land in P1; this
//!    slice uses `products.stock_qty` + a plain stock ledger so the sale path
//!    is already correct.

#![forbid(unsafe_code)]

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub const SCHEMA_SQL: &str = include_str!("../../../migrations/0001_init.sql");
pub const SCHEMA_SQL_V2: &str = include_str!("../../../migrations/0002_sessions.sql");
pub const SCHEMA_SQL_V3: &str = include_str!("../../../migrations/0003_expenses_and_scan.sql");

/// Re-exported so the web layer can hold the connection without depending
/// on rusqlite directly.
pub use rusqlite::Connection as Db;

// ---------------------------------------------------------------------------
// Opening / migrating / diagnostics
// ---------------------------------------------------------------------------

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let conn = Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous  = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA temp_store   = MEMORY;
         PRAGMA cache_size   = -65536;",
    )?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    // A fresh database has no schema_version table yet: treat any failure as 0.
    let mut version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(SCHEMA_SQL)
            .context("migration 0001_init.sql failed")?;
        version = 1;
    }
    if version < 2 {
        conn.execute_batch(SCHEMA_SQL_V2)
            .context("migration 0002_sessions.sql failed")?;
        version = 2;
    }
    if version < 3 {
        conn.execute_batch(SCHEMA_SQL_V3)
            .context("migration 0003_expenses_and_scan.sql failed")?;
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    let v = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .optional()?
        .unwrap_or(0);
    Ok(v)
}

/// Returns human-readable problems; empty = healthy.
pub fn doctor(conn: &Connection, db_path: &Path) -> Result<Vec<String>> {
    let mut problems = Vec::new();

    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        problems.push(format!("integrity_check: {integrity}"));
    }

    let cipher: Option<String> = conn
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .optional()
        .unwrap_or(None);
    if cipher.is_none() {
        problems.push(
            "Chiffrement désactivé : la base est un fichier SQLite standard. \
             (M0 — activer SQLCipher avant toute installation réelle.)"
                .to_string(),
        );
    }

    let free_mb = free_space_mb(db_path);
    if let Some(mb) = free_mb {
        if mb < 500 {
            problems.push(format!("Espace disque faible : {mb} Mo libres."));
        }
    }

    let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    if users == 0 {
        problems.push("Aucun utilisateur : la première ouverture demandera la configuration.".into());
    }

    Ok(problems)
}

fn free_space_mb(_path: &Path) -> Option<u64> {
    // std has no cross-platform free-space API; the real build uses `fs2`.
    None
}

// ---------------------------------------------------------------------------
// Time / ids
// ---------------------------------------------------------------------------

pub fn now_iso() -> String {
    Local::now().to_rfc3339()
}

pub fn today_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

// ---------------------------------------------------------------------------
// Passwords (Argon2id) — SEC-001
// ---------------------------------------------------------------------------

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sha2::{Digest, Sha256};

fn random_bytes(n: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| anyhow!("entropy: {e}"))?;
    Ok(buf)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hex(&hasher.finalize())
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt_bytes = random_bytes(16)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow!("salt: {e}"))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow!("hash: {e}"))
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Users & sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub role: String,
}

impl User {
    pub fn is_owner(&self) -> bool {
        self.role == "owner"
    }
}

/// The single shopkeeper account (identifiant unique, but the login form hides it).
pub fn owner_name(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT name FROM users ORDER BY created_at LIMIT 1", [], |r| r.get(0))
        .optional()?)
}

pub fn has_users(conn: &Connection) -> Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn create_user(conn: &Connection, name: &str, role: &str, password: &str) -> Result<User> {
    let id = new_id();
    let hash = hash_password(password)?;
    conn.execute(
        "INSERT INTO users (id, name, role, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, role, hash, now_iso()],
    )?;
    Ok(User { id, name: name.to_string(), role: role.to_string() })
}

pub fn authenticate(conn: &Connection, name: &str, password: &str) -> Result<Option<User>> {
    let row: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT id, name, role, password_hash FROM users WHERE is_active = 1 AND LOWER(name) = LOWER(?1)",
            params![name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    Ok(match row {
        Some((id, n, role, hash)) if verify_password(password, &hash) => {
            Some(User { id, name: n, role })
        }
        _ => None,
    })
}

/// Creates a session and returns the **raw** cookie value (never stored as-is).
pub fn start_session(conn: &Connection, user_id: &str) -> Result<String> {
    let raw = hex(&random_bytes(32)?);
    let id_hash = sha256_hex(raw.as_bytes());
    let now = Local::now();
    let expires = (now + chrono::Duration::hours(12)).to_rfc3339();
    conn.execute(
        "INSERT INTO sessions (id_hash, user_id, created_at, last_seen, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id_hash, user_id, now.to_rfc3339(), now.to_rfc3339(), expires],
    )?;
    Ok(raw)
}

pub fn session_user(conn: &Connection, raw_sid: &str) -> Result<Option<User>> {
    let id_hash = sha256_hex(raw_sid.as_bytes());
    let row: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT u.id, u.name, u.role, s.expires_at
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.id_hash = ?1 AND u.is_active = 1",
            params![id_hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    if let Some((id, name, role, expires_at)) = row {
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&expires_at) {
            if exp.with_timezone(&Local) < Local::now() {
                return Ok(None);
            }
        }
        conn.execute(
            "UPDATE sessions SET last_seen = ?1 WHERE id_hash = ?2",
            params![now_iso(), id_hash],
        )?;
        return Ok(Some(User { id, name, role }));
    }
    Ok(None)
}

pub fn end_session(conn: &Connection, raw_sid: &str) -> Result<()> {
    let id_hash = sha256_hex(raw_sid.as_bytes());
    conn.execute("DELETE FROM sessions WHERE id_hash = ?1", params![id_hash])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Audit — the single choke point (ADR-011)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn audit(
    conn: &Connection,
    actor: &User,
    kind: &str,
    entity: &str,
    entity_id: &str,
    summary: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_events
         (id, at_utc, business_day, actor_id, actor_name, actor_role, kind, entity, entity_id, summary, before_json, after_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            new_id(),
            now_iso(),
            today_key(),
            actor.id,
            actor.name,
            actor.role,
            kind,
            entity,
            entity_id,
            summary,
            before,
            after
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Products
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub barcode: Option<String>,
    pub price: i64,
    pub cost: i64,
    pub stock: i64,
    /// false = the shop never counted this item (created by an unknown scan).
    /// The till must then say nothing about stock instead of shouting "0".
    pub counted: bool,
}

/// A product may have no name yet (a scanned code, sheet left for later).
/// Every read path uses this, so the whole app speaks of "Code 123456".
const NAME_EXPR: &str =
    "COALESCE(NULLIF(TRIM(p.name), ''), 'Code ' || COALESCE(p.barcode, '?'))";

pub fn list_products(conn: &Connection, query: Option<&str>) -> Result<Vec<Product>> {
    let mut out = Vec::new();
    let map = |r: &rusqlite::Row| -> rusqlite::Result<Product> {
        Ok(Product {
            id: r.get(0)?,
            name: r.get(1)?,
            barcode: r.get(2)?,
            price: r.get(3)?,
            cost: r.get(4)?,
            stock: r.get(5)?,
            counted: r.get::<_, i64>(6)? != 0,
        })
    };

    match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{}%", q.to_lowercase());
            let mut stmt = conn.prepare(&format!(
                "SELECT p.id, {NAME_EXPR}, p.barcode, p.ref_price, p.last_cost, p.stock_qty,
                        p.tracks_stock
                 FROM products p WHERE p.is_active = 1 AND p.archived_at IS NULL
                   AND (LOWER(COALESCE(p.name, '')) LIKE ?1 OR p.barcode = ?2 OR p.barcode LIKE ?1)
                 ORDER BY {NAME_EXPR} LIMIT 60"
            ))?;
            let rows = stmt.query_map(params![like, q], map)?;
            for row in rows {
                out.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT p.id, {NAME_EXPR}, p.barcode, p.ref_price, p.last_cost, p.stock_qty,
                        p.tracks_stock
                 FROM products p WHERE p.is_active = 1 AND p.archived_at IS NULL
                 ORDER BY {NAME_EXPR} LIMIT 60"
            ))?;
            let rows = stmt.query_map([], map)?;
            for row in rows {
                out.push(row?);
            }
        }
    }
    Ok(out)
}

pub fn find_by_barcode(conn: &Connection, barcode: &str) -> Result<Option<Product>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT p.id, {NAME_EXPR}, p.barcode, p.ref_price, p.last_cost, p.stock_qty,
                        p.tracks_stock
                 FROM products p
                 WHERE p.is_active = 1 AND p.archived_at IS NULL AND p.barcode = ?1"
            ),
            params![barcode],
            |r| {
                Ok(Product {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    barcode: r.get(2)?,
                    price: r.get(3)?,
                    cost: r.get(4)?,
                    stock: r.get(5)?,
                    counted: r.get::<_, i64>(6)? != 0,
                })
            },
        )
        .optional()?)
}

pub fn create_product(
    conn: &Connection,
    actor: &User,
    name: &str,
    barcode: Option<String>,
    price: i64,
    cost: i64,
    qty: i64,
) -> Result<String> {
    let tx = conn.unchecked_transaction()?;
    let id = new_id();
    let now = now_iso();
    tx.execute(
        "INSERT INTO products (id, name, barcode, ref_price, last_cost, stock_qty, stock_avg_cost, created_at, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![id, name, barcode, price, cost, qty, cost, now, actor.id],
    )?;
    if qty != 0 {
        tx.execute(
            "INSERT INTO stock_movements (id, product_id, kind, qty, unit_cost, day, created_at, user_id, ref_id)
             VALUES (?1, ?2, 'adjust', ?3, ?4, ?5, ?6, ?7, NULL)",
            params![new_id(), id, qty, cost, today_key(), now, actor.id],
        )?;
    }
    audit(
        &tx,
        actor,
        "create",
        "product",
        &id,
        &format!("Produit créé : {name}"),
        None,
        Some(format!("{{\"name\":{name:?},\"price\":{price},\"cost\":{cost},\"stock\":{qty}}}").as_str()),
    )?;
    tx.commit()?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Draft cart (server-side: survives a crash, ADR-011/SEC-001)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DraftLine {
    pub product_id: String,
    pub name: String,
    pub qty: i64,
    pub unit_price: i64,
    pub unit_cost: i64,
    pub line_total: i64,
}

pub fn draft_lines(conn: &Connection, session_id: &str) -> Result<Vec<DraftLine>> {
    let mut stmt = conn.prepare(
        "SELECT d.product_id,
                COALESCE(NULLIF(TRIM(p.name), ''), 'Code ' || COALESCE(p.barcode, '?')),
                d.qty, d.unit_price, p.last_cost
         FROM draft_lines d JOIN products p ON p.id = d.product_id
         WHERE d.session_id = ?1 ORDER BY p.name",
    )?;
    let rows = stmt.query_map(params![session_id], |r| {
        let qty: i64 = r.get(2)?;
        let unit_price: i64 = r.get(3)?;
        Ok(DraftLine {
            product_id: r.get(0)?,
            name: r.get(1)?,
            qty,
            unit_price,
            unit_cost: r.get(4)?,
            line_total: qty * unit_price,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn draft_add(conn: &Connection, session_id: &str, product_id: &str) -> Result<()> {
    let price: i64 = conn.query_row(
        "SELECT ref_price FROM products WHERE id = ?1",
        params![product_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO draft_lines (session_id, product_id, qty, unit_price)
         VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(session_id, product_id) DO UPDATE SET qty = qty + 1",
        params![session_id, product_id, price],
    )?;
    Ok(())
}

pub fn draft_set_qty(conn: &Connection, session_id: &str, product_id: &str, qty: i64) -> Result<()> {
    if qty <= 0 {
        return draft_remove(conn, session_id, product_id);
    }
    conn.execute(
        "UPDATE draft_lines SET qty = ?3 WHERE session_id = ?1 AND product_id = ?2",
        params![session_id, product_id, qty],
    )?;
    Ok(())
}

pub fn draft_set_price(
    conn: &Connection,
    session_id: &str,
    product_id: &str,
    unit_price: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE draft_lines SET unit_price = ?3 WHERE session_id = ?1 AND product_id = ?2",
        params![session_id, product_id, unit_price],
    )?;
    Ok(())
}

pub fn draft_remove(conn: &Connection, session_id: &str, product_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM draft_lines WHERE session_id = ?1 AND product_id = ?2",
        params![session_id, product_id],
    )?;
    Ok(())
}

pub fn draft_clear(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute("DELETE FROM draft_lines WHERE session_id = ?1", params![session_id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Checkout
// ---------------------------------------------------------------------------

pub enum Payment {
    Cash,
    Card,
    Credit { name: String, phone: Option<String> },
}

impl Payment {
    fn as_str(&self) -> &'static str {
        match self {
            Payment::Cash => "cash",
            Payment::Card => "card",
            Payment::Credit { .. } => "credit",
        }
    }
}

#[derive(Debug)]
pub struct SaleResult {
    pub id: String,
    pub number: String,
    pub total: i64,
    pub credit: i64,
}

pub fn checkout(
    conn: &Connection,
    session_id: &str,
    actor: &User,
    payment: Payment,
) -> Result<SaleResult> {
    let tx = conn.unchecked_transaction()?;

    let lines = draft_lines(&tx, session_id)?;
    if lines.is_empty() {
        return Err(anyhow!("empty_cart"));
    }

    let total: i64 = lines.iter().map(|l| l.line_total).sum();
    let cost_total: i64 = lines.iter().map(|l| l.qty * l.unit_cost).sum();
    let credit = match &payment {
        Payment::Credit { .. } => total,
        _ => 0,
    };
    let paid = total - credit;

    // Careful: this is the *cash* session, not the browser session that owns
    // the draft cart. Shadowing `session_id` here used to make the final
    // `DELETE FROM draft_lines` target the wrong id — the cart stayed full
    // after a sale. Keep the two names apart, always.
    let cash_session = open_session_id(&tx)?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM sales", [], |r| r.get(0))?;
    let number = format!("V-{:05}", count + 1);
    let sale_id = new_id();
    let now = now_iso();
    let day = today_key();

    tx.execute(
        "INSERT INTO sales (id, number, business_day, created_at, user_id, total, paid, credit, cost_total, payment, status, session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'closed', ?11)",
        params![
            sale_id,
            number,
            day,
            now,
            actor.id,
            total,
            paid,
            credit,
            cost_total,
            payment.as_str(),
            cash_session
        ],
    )?;

    for line in &lines {
        tx.execute(
            "INSERT INTO sale_lines (id, sale_id, product_id, product_name, qty, unit_price, unit_cost, line_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new_id(),
                sale_id,
                line.product_id,
                line.name,
                line.qty,
                line.unit_price,
                line.unit_cost,
                line.line_total
            ],
        )?;
        tx.execute(
            "UPDATE products
                SET stock_qty = stock_qty - ?2, updated_at = ?3, updated_by = ?4, version = version + 1
              WHERE id = ?1",
            params![line.product_id, line.qty, now, actor.id],
        )?;
        tx.execute(
            "INSERT INTO stock_movements (id, product_id, kind, qty, unit_cost, day, created_at, user_id, ref_id)
             VALUES (?1, ?2, 'sale', ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new_id(),
                line.product_id,
                -line.qty,
                line.unit_cost,
                day,
                now,
                actor.id,
                sale_id
            ],
        )?;
    }

    if let Payment::Credit { name, phone } = &payment {
        let debt_id = new_id();
        tx.execute(
            "INSERT INTO debts (id, debtor_name, debtor_phone, sale_id, principal_millimes, opened_day, status, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?8)",
            params![debt_id, name, phone, sale_id, total, day, actor.id, now],
        )?;
        tx.execute(
            "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
             VALUES (?1, ?2, 'open', ?3, NULL, ?4, ?5, ?6)",
            params![new_id(), debt_id, total, day, now, actor.id],
        )?;
    }

    audit(
        &tx,
        actor,
        "sale",
        "sale",
        &sale_id,
        &format!("Vente {number} — {} article(s) — {}", lines.len(), domain::format_money_tnd(total)),
        None,
        None,
    )?;

    tx.execute("DELETE FROM draft_lines WHERE session_id = ?1", params![session_id])?;
    tx.commit()?;

    Ok(SaleResult { id: sale_id, number, total, credit })
}

// ---------------------------------------------------------------------------
// Debts / carnet (ADR-010)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DebtRow {
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub opened_day: String,
    pub principal: i64,
    pub paid: i64,
    pub balance: i64,
}

pub fn open_debts(conn: &Connection) -> Result<Vec<DebtRow>> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.debtor_name, d.debtor_phone, d.opened_day, d.principal_millimes,
                COALESCE(SUM(CASE WHEN l.amount_millimes < 0 THEN -l.amount_millimes ELSE 0 END), 0),
                COALESCE(SUM(l.amount_millimes), 0)
         FROM debts d
         LEFT JOIN debt_ledger l ON l.debt_id = d.id
         WHERE d.status = 'open'
         GROUP BY d.id
         HAVING COALESCE(SUM(l.amount_millimes), 0) > 0
         ORDER BY d.opened_day ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(DebtRow {
            id: r.get(0)?,
            name: r.get(1)?,
            phone: r.get(2)?,
            opened_day: r.get(3)?,
            principal: r.get(4)?,
            paid: r.get(5)?,
            balance: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn collect(
    conn: &Connection,
    debt_id: &str,
    amount: i64,
    method: &str,
    actor: &User,
) -> Result<()> {
    if amount <= 0 {
        return Err(anyhow!("invalid_amount"));
    }
    let tx = conn.unchecked_transaction()?;
    let name: String = tx.query_row(
        "SELECT debtor_name FROM debts WHERE id = ?1",
        params![debt_id],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
         VALUES (?1, ?2, 'payment', ?3, ?4, ?5, ?6, ?7)",
        params![new_id(), debt_id, -amount, method, today_key(), now_iso(), actor.id],
    )?;

    // The ledger is the truth: + at opening, - on every collection.
    let balance: i64 = tx.query_row(
        "SELECT COALESCE(SUM(amount_millimes), 0) FROM debt_ledger WHERE debt_id = ?1",
        params![debt_id],
        |r| r.get(0),
    )?;
    if balance <= 0 {
        tx.execute("UPDATE debts SET status = 'settled' WHERE id = ?1", params![debt_id])?;
    }

    audit(
        &tx,
        actor,
        "payment",
        "debt",
        debt_id,
        &format!("Encaissement {} — {}", domain::format_money_tnd(amount), name),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Small read helpers for the till header
// ---------------------------------------------------------------------------

/// (revenue today, ticket count today)
pub fn today_totals(conn: &Connection) -> Result<(i64, i64)> {
    let day = today_key();
    let row = conn.query_row(
        "SELECT COALESCE(SUM(total),0), COUNT(*) FROM sales WHERE business_day = ?1 AND status = 'closed'",
        params![day],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    Ok(row)
}


// ---------------------------------------------------------------------------
// Products: read one, edit, archive (ADR-011 — everything editable, audited)
// ---------------------------------------------------------------------------

pub fn get_product(conn: &Connection, id: &str) -> Result<Product> {
    let product = conn.query_row(
        &format!(
            "SELECT p.id, {NAME_EXPR}, p.barcode, p.ref_price, p.last_cost, p.stock_qty,
                    p.tracks_stock
               FROM products p WHERE p.id = ?1"
        ),
        params![id],
        |r| {
            Ok(Product {
                id: r.get(0)?,
                name: r.get(1)?,
                barcode: r.get(2)?,
                price: r.get(3)?,
                cost: r.get(4)?,
                stock: r.get(5)?,
                counted: r.get::<_, i64>(6)? != 0,
            })
        },
    )?;
    Ok(product)
}

#[allow(clippy::too_many_arguments)]
pub fn update_product(
    conn: &Connection,
    actor: &User,
    id: &str,
    name: &str,
    barcode: Option<String>,
    price: i64,
    cost: i64,
) -> Result<()> {
    let before = get_product(conn, id)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE products
            SET name = ?2, barcode = ?3, ref_price = ?4, last_cost = ?5,
                updated_at = ?6, updated_by = ?7, version = version + 1
          WHERE id = ?1",
        params![id, name, barcode, price, cost, now_iso(), actor.id],
    )?;

    let mut changes: Vec<String> = Vec::new();
    if before.name != name {
        changes.push(format!("nom: {} → {}", before.name, name));
    }
    if before.price != price {
        changes.push(format!(
            "prix: {} → {}",
            domain::format_money(before.price),
            domain::format_money(price)
        ));
    }
    if before.cost != cost {
        changes.push(format!(
            "prix d'achat: {} → {}",
            domain::format_money(before.cost),
            domain::format_money(cost)
        ));
    }
    if before.barcode != barcode {
        changes.push("code-barres modifié".to_string());
    }
    let summary = if changes.is_empty() {
        format!("Produit enregistré : {name} (aucun changement)")
    } else {
        format!("{} — {}", name, changes.join(" · "))
    };

    audit(
        &tx,
        actor,
        "update",
        "product",
        id,
        &summary,
        Some(&format!("{{\"name\":{:?},\"price\":{}}}", before.name, before.price)),
        Some(&format!("{{\"name\":{name:?},\"price\":{price}}}")),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn archive_product(conn: &Connection, actor: &User, id: &str, archive: bool) -> Result<()> {
    let product = get_product(conn, id)?;
    conn.execute(
        "UPDATE products SET archived_at = ?2, archived_by = ?3, version = version + 1,
                             updated_at = ?4, updated_by = ?3
          WHERE id = ?1",
        params![
            id,
            if archive { Some(now_iso()) } else { None },
            if archive { Some(actor.id.clone()) } else { None },
            now_iso()
        ],
    )?;
    audit(
        conn,
        actor,
        if archive { "delete" } else { "restore" },
        "product",
        id,
        &format!(
            "{} : {}",
            if archive { "Produit archivé" } else { "Produit restauré" },
            product.name
        ),
        None,
        None,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Audit trail — "Qui a fait quoi" (ADR-011)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuditRow {
    pub at: String,
    pub actor: String,
    pub role: String,
    pub kind: String,
    pub entity: String,
    pub summary: String,
}

pub fn recent_audit(conn: &Connection, limit: i64) -> Result<Vec<AuditRow>> {
    let mut stmt = conn.prepare(
        "SELECT at_utc, actor_name, actor_role, kind, entity, summary
           FROM audit_events ORDER BY at_utc DESC, rowid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(AuditRow {
            at: r.get(0)?,
            actor: r.get(1)?,
            role: r.get(2)?,
            kind: r.get(3)?,
            entity: r.get(4)?,
            summary: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Cash sessions — open the drawer, close it, Z report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CashSession {
    pub id: String,
    pub opened_at: String,
    pub opening_float: i64,
}

pub fn open_session_id(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM cash_sessions WHERE closed_at IS NULL ORDER BY opened_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn current_session(conn: &Connection) -> Result<Option<CashSession>> {
    Ok(conn
        .query_row(
            "SELECT id, opened_at, opening_float FROM cash_sessions
              WHERE closed_at IS NULL ORDER BY opened_at DESC LIMIT 1",
            [],
            |r| {
                Ok(CashSession {
                    id: r.get(0)?,
                    opened_at: r.get(1)?,
                    opening_float: r.get(2)?,
                })
            },
        )
        .optional()?)
}

pub fn open_session(conn: &Connection, actor: &User, opening_float: i64) -> Result<String> {
    if current_session(conn)?.is_some() {
        return Err(anyhow!("session_already_open"));
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO cash_sessions (id, opened_at, opened_by, opening_float)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, now_iso(), actor.id, opening_float],
    )?;
    audit(
        conn,
        actor,
        "create",
        "cash_session",
        &id,
        &format!("Caisse ouverte — fond de caisse {}", domain::format_money_tnd(opening_float)),
        None,
        None,
    )?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub struct ZReport {
    pub session_id: String,
    pub opened_at: String,
    pub closed_at: String,
    pub opening_float: i64,
    pub cash_in: i64,
    pub card_in: i64,
    pub credit_out: i64,
    pub collected: i64,
    pub tickets: i64,
    pub expected_cash: i64,
    pub counted_cash: Option<i64>,
    pub difference: Option<i64>,
}

/// Cash figures for a session. `collected` counts debt payments taken during it.
pub fn session_totals(conn: &Connection, session_id: &str) -> Result<ZReport> {
    let (opened_at, opening_float): (String, i64) = conn.query_row(
        "SELECT opened_at, opening_float FROM cash_sessions WHERE id = ?1",
        params![session_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let (cash_in, card_in, credit_out, tickets): (i64, i64, i64, i64) = conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN payment = 'cash' THEN paid ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN payment = 'card' THEN paid ELSE 0 END), 0),
            COALESCE(SUM(credit), 0),
            COUNT(*)
         FROM sales WHERE session_id = ?1 AND status = 'closed'",
        params![session_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;

    let collected: i64 = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN l.amount_millimes < 0 THEN -l.amount_millimes ELSE 0 END), 0)
           FROM debt_ledger l
          WHERE l.method = 'cash'
            AND l.at >= (SELECT opened_at FROM cash_sessions WHERE id = ?1)",
        params![session_id],
        |r| r.get(0),
    )?;

    let expected_cash = opening_float + cash_in + collected;

    let counted: Option<(Option<i64>, Option<i64>, Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT counted_cash, expected_cash, difference, closed_at FROM cash_sessions WHERE id = ?1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    let (counted_cash, difference, closed_at) = match counted {
        Some((c, _e, d, at)) => (c, d, at.unwrap_or_default()),
        None => (None, None, String::new()),
    };

    Ok(ZReport {
        session_id: session_id.to_string(),
        opened_at,
        closed_at,
        opening_float,
        cash_in,
        card_in,
        credit_out,
        collected,
        tickets,
        expected_cash,
        counted_cash,
        difference,
    })
}

pub fn close_session(conn: &Connection, actor: &User, counted_cash: i64) -> Result<ZReport> {
    let Some(session) = current_session(conn)? else {
        return Err(anyhow!("no_open_session"));
    };
    let report = session_totals(conn, &session.id)?;
    let difference = counted_cash - report.expected_cash;
    let now = now_iso();

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE cash_sessions
            SET closed_at = ?2, closed_by = ?3, counted_cash = ?4,
                expected_cash = ?5, difference = ?6
          WHERE id = ?1",
        params![
            session.id,
            now,
            actor.id,
            counted_cash,
            report.expected_cash,
            difference
        ],
    )?;
    audit(
        &tx,
        actor,
        "update",
        "cash_session",
        &session.id,
        &format!(
            "Caisse fermée — attendu {} · compté {} · écart {}",
            domain::format_money(report.expected_cash),
            domain::format_money(counted_cash),
            domain::format_money(difference)
        ),
        None,
        None,
    )?;
    tx.commit()?;

    session_totals(conn, &session.id)
}

pub fn last_closed_session(conn: &Connection) -> Result<Option<CashSession>> {
    Ok(conn
        .query_row(
            "SELECT id, opened_at, opening_float FROM cash_sessions
              WHERE closed_at IS NOT NULL ORDER BY closed_at DESC LIMIT 1",
            [],
            |r| {
                Ok(CashSession {
                    id: r.get(0)?,
                    opened_at: r.get(1)?,
                    opening_float: r.get(2)?,
                })
            },
        )
        .optional()?)
}

// ---------------------------------------------------------------------------
// Receipt (the page is printed by the browser — no driver needed in v0.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Receipt {
    pub number: String,
    pub created_at: String,
    pub seller: String,
    pub total: i64,
    pub paid: i64,
    pub credit: i64,
    pub payment: String,
    pub lines: Vec<(String, i64, i64, i64)>,
}

pub fn receipt(conn: &Connection, sale_id: &str) -> Result<Receipt> {
    let (number, created_at, seller, total, paid, credit, payment): (
        String,
        String,
        String,
        i64,
        i64,
        i64,
        String,
    ) = conn.query_row(
        "SELECT s.number, s.created_at, u.name, s.total, s.paid, s.credit, s.payment
           FROM sales s JOIN users u ON u.id = s.user_id
          WHERE s.id = ?1",
        params![sale_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    )?;

    let sale_id_owned = sale_id.to_string();
    let mut stmt = conn.prepare(
        "SELECT product_name, qty, unit_price, line_total
           FROM sale_lines WHERE sale_id = ?1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map(params![sale_id_owned], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })?;
    let mut lines = Vec::new();
    for row in rows {
        lines.push(row?);
    }

    Ok(Receipt {
        number,
        created_at,
        seller,
        total,
        paid,
        credit,
        payment,
        lines,
    })
}

pub fn last_sale_id(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM sales ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?)
}

// ---------------------------------------------------------------------------
// Free debt ("Ahmed took 500") and write-off (ADR-010)
// ---------------------------------------------------------------------------

pub fn create_debt(
    conn: &Connection,
    actor: &User,
    name: &str,
    phone: Option<String>,
    amount: i64,
    reason: Option<String>,
) -> Result<String> {
    if amount <= 0 {
        return Err(anyhow!("invalid_amount"));
    }
    let tx = conn.unchecked_transaction()?;
    let id = new_id();
    let now = now_iso();
    let day = today_key();
    tx.execute(
        "INSERT INTO debts (id, debtor_name, debtor_phone, sale_id, principal_millimes, opened_day, reason, status, created_by, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, 'open', ?7, ?8)",
        params![id, name, phone, amount, day, reason, actor.id, now],
    )?;
    tx.execute(
        "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
         VALUES (?1, ?2, 'open', ?3, NULL, ?4, ?5, ?6)",
        params![new_id(), id, amount, day, now, actor.id],
    )?;
    audit(
        &tx,
        actor,
        "create",
        "debt",
        &id,
        &format!("Dette ajoutée : {} — {}", name, domain::format_money_tnd(amount)),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(id)
}

pub fn write_off_debt(conn: &Connection, actor: &User, debt_id: &str, reason: &str) -> Result<()> {
    let (name, balance): (String, i64) = conn.query_row(
        "SELECT d.debtor_name, COALESCE(SUM(l.amount_millimes), 0)
           FROM debts d LEFT JOIN debt_ledger l ON l.debt_id = d.id
          WHERE d.id = ?1 GROUP BY d.id",
        params![debt_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
         VALUES (?1, ?2, 'write_off', ?3, NULL, ?4, ?5, ?6)",
        params![new_id(), debt_id, -balance, today_key(), now_iso(), actor.id],
    )?;
    tx.execute(
        "UPDATE debts SET status = 'written_off' WHERE id = ?1",
        params![debt_id],
    )?;
    audit(
        &tx,
        actor,
        "update",
        "debt",
        debt_id,
        &format!(
            "Dette passée en perte : {name} — {} ({reason})",
            domain::format_money_tnd(balance)
        ),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}


// ---------------------------------------------------------------------------
// Sales list — the shop's day book (needed for reprints and for "Ventes")
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SaleRow {
    pub id: String,
    pub number: String,
    pub created_at: String,
    pub total: i64,
    pub credit: i64,
    pub payment: String,
    pub seller: String,
    pub status: String,
}

pub fn recent_sales(conn: &Connection, limit: i64) -> Result<Vec<SaleRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.number, s.created_at, s.total, s.credit, s.payment, u.name, s.status
           FROM sales s JOIN users u ON u.id = s.user_id
          ORDER BY s.created_at DESC, s.rowid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(SaleRow {
            id: r.get(0)?,
            number: r.get(1)?,
            created_at: r.get(2)?,
            total: r.get(3)?,
            credit: r.get(4)?,
            payment: r.get(5)?,
            seller: r.get(6)?,
            status: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn day_totals(conn: &Connection) -> Result<(i64, i64, i64)> {
    let day = today_key();
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(total), 0), COALESCE(SUM(paid), 0), COUNT(*)
           FROM sales WHERE business_day = ?1 AND status = 'closed'",
        params![day],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?)
}


// ---------------------------------------------------------------------------
// Sale detail / edit / cancel  (ADR-011: everything editable, all attributed)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SaleLineRow {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub qty: i64,
    pub unit_price: i64,
    pub line_total: i64,
}

#[derive(Debug, Clone)]
pub struct SaleDetail {
    pub id: String,
    pub number: String,
    pub created_at: String,
    pub seller: String,
    pub payment: String,
    pub status: String,
    pub total: i64,
    pub paid: i64,
    pub credit: i64,
    pub lines: Vec<SaleLineRow>,
    pub debtor: Option<String>,
}

pub fn sale_detail(conn: &Connection, sale_id: &str) -> Result<SaleDetail> {
    let (number, created_at, seller, payment, status, total, paid, credit): (
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
    ) = conn.query_row(
        "SELECT s.number, s.created_at, u.name, s.payment, s.status, s.total, s.paid, s.credit
           FROM sales s JOIN users u ON u.id = s.user_id
          WHERE s.id = ?1",
        params![sale_id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        },
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, product_id, product_name, qty, unit_price, line_total
           FROM sale_lines WHERE sale_id = ?1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map(params![sale_id], |r| {
        Ok(SaleLineRow {
            id: r.get(0)?,
            product_id: r.get(1)?,
            name: r.get(2)?,
            qty: r.get(3)?,
            unit_price: r.get(4)?,
            line_total: r.get(5)?,
        })
    })?;
    let mut lines = Vec::new();
    for row in rows {
        lines.push(row?);
    }

    let debtor = conn
        .query_row(
            "SELECT debtor_name FROM debts WHERE sale_id = ?1 LIMIT 1",
            params![sale_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?;

    Ok(SaleDetail {
        id: sale_id.to_string(),
        number,
        created_at,
        seller,
        payment,
        status,
        total,
        paid,
        credit,
        lines,
        debtor,
    })
}

/// Edit quantities and unit prices of a closed sale.
/// `changes` = (line id, new qty, new unit price). Quantity 0 removes the line.
/// Stock is re-balanced by the difference, so the ledger always matches reality.
pub fn update_sale(
    conn: &Connection,
    actor: &User,
    sale_id: &str,
    changes: &[(String, i64, i64)],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;

    let (payment, status, old_total): (String, String, i64) = tx.query_row(
        "SELECT payment, status, total FROM sales WHERE id = ?1",
        params![sale_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if status != "closed" {
        return Err(anyhow!("sale_cancelled"));
    }

    let now = now_iso();
    let day = today_key();

    for (line_id, qty, unit_price) in changes {
        let (product_id, old_qty, unit_cost): (String, i64, i64) = tx.query_row(
            "SELECT product_id, qty, unit_cost FROM sale_lines WHERE id = ?1 AND sale_id = ?2",
            params![line_id, sale_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        let qty = (*qty).max(0);
        let unit_price = (*unit_price).max(0);
        let delta = old_qty - qty; // positive: stock comes back

        if delta != 0 {
            tx.execute(
                "UPDATE products
                    SET stock_qty = stock_qty + ?2, updated_at = ?3, updated_by = ?4,
                        version = version + 1
                  WHERE id = ?1",
                params![product_id, delta, now, actor.id],
            )?;
            tx.execute(
                "INSERT INTO stock_movements (id, product_id, kind, qty, unit_cost, day, created_at, user_id, ref_id)
                 VALUES (?1, ?2, 'adjust', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![new_id(), product_id, delta, unit_cost, day, now, actor.id, sale_id],
            )?;
        }

        if qty == 0 {
            tx.execute("DELETE FROM sale_lines WHERE id = ?1", params![line_id])?;
        } else {
            tx.execute(
                "UPDATE sale_lines SET qty = ?2, unit_price = ?3, line_total = ?4 WHERE id = ?1",
                params![line_id, qty, unit_price, qty * unit_price],
            )?;
        }
    }

    let (total, cost_total, remaining): (i64, i64, i64) = tx.query_row(
        "SELECT COALESCE(SUM(line_total), 0), COALESCE(SUM(qty * unit_cost), 0), COUNT(*)
           FROM sale_lines WHERE sale_id = ?1",
        params![sale_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if remaining == 0 {
        return Err(anyhow!("sale_empty"));
    }

    // The ledger is the truth: a customer may already have paid part of a
    // credit, so the new amount owed is `total - what was already paid` and
    // the sale's `credit` must match it (otherwise the carnet would lie).
    let debt: Option<(String, i64)> = tx
        .query_row(
            "SELECT id, principal_millimes FROM debts WHERE sale_id = ?1 LIMIT 1",
            params![sale_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let mut balance = 0i64;
    let mut already_paid = 0i64;
    if let Some((debt_id, principal)) = &debt {
        balance = tx.query_row(
            "SELECT COALESCE(SUM(amount_millimes), 0) FROM debt_ledger WHERE debt_id = ?1",
            params![debt_id],
            |r| r.get(0),
        )?;
        already_paid = (principal - balance).max(0);
    }

    let credit = if payment == "credit" {
        (total - already_paid).max(0)
    } else {
        0
    };
    let paid = total - credit;

    if let Some((debt_id, _)) = &debt {
        if credit != balance {
            tx.execute(
                "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
                 VALUES (?1, ?2, 'adjustment', ?3, NULL, ?4, ?5, ?6)",
                params![new_id(), debt_id, credit - balance, day, now, actor.id],
            )?;
        }
        tx.execute(
            "UPDATE debts SET principal_millimes = ?2, status = ?3 WHERE id = ?1",
            params![debt_id, credit, if credit == 0 { "settled" } else { "open" }],
        )?;
    }

    tx.execute(
        "UPDATE sales SET total = ?2, cost_total = ?3, paid = ?4, credit = ?5 WHERE id = ?1",
        params![sale_id, total, cost_total, paid, credit],
    )?;

    audit(
        &tx,
        actor,
        "update",
        "sale",
        sale_id,
        &format!(
            "Vente modifiée — {} (avant {})",
            domain::format_money_tnd(total),
            domain::format_money_tnd(old_total)
        ),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

/// Cancel a sale: stock returns to the shelf, the credit line is cleared, the
/// sale stays in the history marked as cancelled (never silently deleted).
pub fn cancel_sale(conn: &Connection, actor: &User, sale_id: &str, reason: Option<&str>) -> Result<()> {
    let tx = conn.unchecked_transaction()?;

    let (number, status, credit): (String, String, i64) = tx.query_row(
        "SELECT number, status, credit FROM sales WHERE id = ?1",
        params![sale_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if status != "closed" {
        return Err(anyhow!("sale_cancelled"));
    }

    let now = now_iso();
    let day = today_key();

    let lines: Vec<(String, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT product_id, qty, unit_cost FROM sale_lines WHERE sale_id = ?1",
        )?;
        let rows = stmt.query_map(params![sale_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut buf = Vec::new();
        for row in rows {
            buf.push(row?);
        }
        buf
    };

    for (product_id, qty, unit_cost) in lines {
        tx.execute(
            "UPDATE products
                SET stock_qty = stock_qty + ?2, updated_at = ?3, updated_by = ?4, version = version + 1
              WHERE id = ?1",
            params![product_id, qty, now, actor.id],
        )?;
        tx.execute(
            "INSERT INTO stock_movements (id, product_id, kind, qty, unit_cost, day, created_at, user_id, ref_id)
             VALUES (?1, ?2, 'cancel', ?3, ?4, ?5, ?6, ?7, ?8)",
            params![new_id(), product_id, qty, unit_cost, day, now, actor.id, sale_id],
        )?;
    }

    if credit > 0 {
        let debt_id: Option<String> = tx
            .query_row(
                "SELECT id FROM debts WHERE sale_id = ?1 LIMIT 1",
                params![sale_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(debt_id) = debt_id {
            let balance: i64 = tx.query_row(
                "SELECT COALESCE(SUM(amount_millimes), 0) FROM debt_ledger WHERE debt_id = ?1",
                params![debt_id],
                |r| r.get(0),
            )?;
            if balance != 0 {
                tx.execute(
                    "INSERT INTO debt_ledger (id, debt_id, kind, amount_millimes, method, day, at, user_id)
                     VALUES (?1, ?2, 'cancel', ?3, NULL, ?4, ?5, ?6)",
                    params![new_id(), debt_id, -balance, day, now, actor.id],
                )?;
            }
            tx.execute(
                "UPDATE debts SET status = 'cancelled' WHERE id = ?1",
                params![debt_id],
            )?;
        }
    }

    tx.execute(
        "UPDATE sales SET status = 'cancelled', note = ?2 WHERE id = ?1",
        params![sale_id, reason],
    )?;

    audit(
        &tx,
        actor,
        "cancel",
        "sale",
        sale_id,
        &format!("Vente {number} annulée — stock remis en magasin"),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Selling what is not in the system yet (inventory that fills itself by time)
// ---------------------------------------------------------------------------
//
// The shop does not enter its stock in advance. It scans, sells, and only then
// — when there is a quiet moment — completes the sheet. So a product may exist
// with: no name, no purchase price, and a stock that is not counted at all.
// Nothing here is ever a reason to refuse a sale.

/// Create a product straight from a barcode scanned at the till.
/// `name` may be empty (the till falls back to "Code 123456"), `cost` may be 0.
/// Stock is NOT tracked: `stock_qty` stays 0 and `tracks_stock` is 0 so the
/// shop is never told it has "0 in stock" — it never claimed to have any.
pub fn create_scanned_product(
    conn: &Connection,
    actor: &User,
    barcode: &str,
    name: &str,
    price: i64,
    cost: i64,
) -> Result<String> {
    let tx = conn.unchecked_transaction()?;
    let id = new_id();
    let now = now_iso();
    let name = name.trim();

    // Scanning the same code twice must not create a twin.
    if let Some(existing) = find_by_barcode(&tx, barcode)? {
        return Ok(existing.id);
    }

    tx.execute(
        "INSERT INTO products (id, name, barcode, ref_price, last_cost, stock_qty, stock_avg_cost,
                               tracks_stock, origin, created_at, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?5, 0, 'scan', ?6, ?6, ?7)",
        params![id, name, barcode, price, cost, now, actor.id],
    )?;

    let what = if name.is_empty() {
        format!("code {barcode}")
    } else {
        format!("{name} (code {barcode})")
    };
    audit(
        &tx,
        actor,
        "create",
        "product",
        &id,
        &format!(
            "Produit créé à la caisse : {what} — prix {}",
            domain::format_money_tnd(price)
        ),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub struct ProductStats {
    pub sold_qty: i64,
    pub revenue: i64,
    pub sales: i64,
    pub last_sold: Option<String>,
}

/// What this product actually did — the shop learns its own prices over time.
pub fn product_stats(conn: &Connection, product_id: &str) -> Result<ProductStats> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(sl.qty), 0), COALESCE(SUM(sl.line_total), 0),
                COUNT(DISTINCT sl.sale_id), MAX(s.created_at)
           FROM sale_lines sl JOIN sales s ON s.id = sl.sale_id
          WHERE sl.product_id = ?1 AND s.status = 'closed'",
        params![product_id],
        |r| {
            Ok(ProductStats {
                sold_qty: r.get(0)?,
                revenue: r.get(1)?,
                sales: r.get(2)?,
                last_sold: r.get(3)?,
            })
        },
    )?)
}

// ---------------------------------------------------------------------------
// Fixed charges: rent, water, power, maintenance — each with its own period
// ---------------------------------------------------------------------------

pub struct ExpenseRow {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub amount: i64,
    pub period: String,
    pub start_day: String,
    pub note: Option<String>,
    pub active: bool,
}

/// How many months one payment covers. This is the only place the rule lives.
pub fn period_months(period: &str) -> i64 {
    match period {
        "quarter" => 3,
        "year" => 12,
        _ => 1,
    }
}

pub fn normalize_period(period: &str) -> &'static str {
    match period {
        "quarter" => "quarter",
        "year" => "year",
        _ => "month",
    }
}

pub fn normalize_kind(kind: &str) -> &'static str {
    match kind {
        "rent" => "rent",
        "power" => "power",
        "water" => "water",
        "maintenance" => "maintenance",
        "telecom" => "telecom",
        _ => "other",
    }
}

pub fn list_expenses(conn: &Connection) -> Result<Vec<ExpenseRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, label, kind, amount_millimes, period, start_day, note, is_active
           FROM expenses
          ORDER BY is_active DESC, kind ASC, label ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ExpenseRow {
            id: r.get(0)?,
            label: r.get(1)?,
            kind: r.get(2)?,
            amount: r.get(3)?,
            period: r.get(4)?,
            start_day: r.get(5)?,
            note: r.get(6)?,
            active: r.get::<_, i64>(7)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// What the shop must earn every month just to keep the lights on.
/// Rounded down to the millime: never promise a cost lower than reality.
pub fn monthly_expenses(conn: &Connection) -> Result<i64> {
    let rows = list_expenses(conn)?;
    Ok(rows
        .iter()
        .filter(|e| e.active)
        .map(|e| e.amount / period_months(&e.period))
        .sum())
}

#[allow(clippy::too_many_arguments)]
pub fn create_expense(
    conn: &Connection,
    actor: &User,
    label: &str,
    kind: &str,
    amount: i64,
    period: &str,
    start_day: &str,
    note: Option<&str>,
) -> Result<String> {
    let tx = conn.unchecked_transaction()?;
    let id = new_id();
    let now = now_iso();
    let label = label.trim();
    let label = if label.is_empty() { "Charge" } else { label };

    tx.execute(
        "INSERT INTO expenses (id, label, kind, amount_millimes, period, start_day, note,
                               is_active, created_by, created_at, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9, ?8)",
        params![
            id,
            label,
            normalize_kind(kind),
            amount.max(0),
            normalize_period(period),
            start_day,
            note,
            actor.id,
            now
        ],
    )?;
    audit(
        &tx,
        actor,
        "create",
        "expense",
        &id,
        &format!(
            "Charge ajoutée : {label} — {} {}",
            domain::format_money_tnd(amount),
            match normalize_period(period) {
                "quarter" => "par trimestre",
                "year" => "par an",
                _ => "par mois",
            }
        ),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(id)
}

pub fn update_expense(
    conn: &Connection,
    actor: &User,
    id: &str,
    label: &str,
    kind: &str,
    amount: i64,
    period: &str,
    note: Option<&str>,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let label = label.trim();
    let label = if label.is_empty() { "Charge" } else { label };
    tx.execute(
        "UPDATE expenses
            SET label = ?2, kind = ?3, amount_millimes = ?4, period = ?5, note = ?6,
                updated_at = ?7, updated_by = ?8
          WHERE id = ?1",
        params![
            id,
            label,
            normalize_kind(kind),
            amount.max(0),
            normalize_period(period),
            note,
            now_iso(),
            actor.id
        ],
    )?;
    audit(
        &tx,
        actor,
        "update",
        "expense",
        id,
        &format!(
            "Charge modifiée : {label} — {}",
            domain::format_money_tnd(amount)
        ),
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

/// Stop a charge (shop closed a contract…). Never deleted: a past month must
/// stay explainable. `stopped_day` lets a later report cut it at the right date.
pub fn stop_expense(conn: &Connection, actor: &User, id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE expenses SET is_active = 0, stopped_day = ?2, updated_at = ?3, updated_by = ?4
          WHERE id = ?1",
        params![id, today_key(), now_iso(), actor.id],
    )?;
    audit(
        &tx,
        actor,
        "update",
        "expense",
        id,
        "Charge arrêtée",
        None,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Backups — a real file, not a promise
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BackupRow {
    pub path: PathBuf,
    pub at: String,
    pub bytes: u64,
}

/// Copy the whole database into `<data>/backups/store-YYYY-MM-DD_HHMMSS.db`.
/// `VACUUM INTO` is SQLite's own consistent-copy command: it works while the
/// till is running, and the copy is never half-written.
pub fn backup_into(conn: &Connection, data_dir: &Path) -> Result<PathBuf> {
    let dir = data_dir.join("backups");
    std::fs::create_dir_all(&dir).with_context(|| format!("créer {}", dir.display()))?;

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let target = dir.join(format!("store-{stamp}.db"));

    // Parameter binding keeps a path with spaces/quotes safe.
    conn.execute("VACUUM INTO ?1", params![target.to_string_lossy().to_string()])
        .context("VACUUM INTO (copie de la base)")?;

    prune_backups(&dir, 14)?;
    Ok(target)
}

/// Keep the last `keep` copies; older ones are removed. A shop that never
/// cleans up ends up with a full disk, which is its own way of losing data.
fn prune_backups(dir: &Path, keep: usize) -> Result<()> {
    let mut copies = list_backups(dir);
    if copies.len() <= keep {
        return Ok(());
    }
    copies.sort_by(|a, b| b.at.cmp(&a.at));
    for old in copies.into_iter().skip(keep) {
        let _ = std::fs::remove_file(&old.path);
    }
    Ok(())
}

/// Newest first. Missing folder = no backups yet, never an error.
pub fn backups(data_dir: &Path) -> Vec<BackupRow> {
    list_backups(&data_dir.join("backups"))
}

fn list_backups(dir: &Path) -> Vec<BackupRow> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let at = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_start_matches("store-").to_string())
            .unwrap_or_default();
        out.push(BackupRow { path, at, bytes });
    }
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

pub fn default_data_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payment_covers_its_period() {
        assert_eq!(period_months("month"), 1);
        assert_eq!(period_months("quarter"), 3);
        assert_eq!(period_months("year"), 12);
        // anything unknown must be treated as monthly, never as free
        assert_eq!(period_months("fortnight"), 1);
    }

    #[test]
    fn unknown_period_and_kind_are_tamed() {
        assert_eq!(normalize_period("week"), "month");
        assert_eq!(normalize_period("year"), "year");
        assert_eq!(normalize_kind("taxes"), "other");
        assert_eq!(normalize_kind("rent"), "rent");
    }

    #[test]
    fn a_quarter_costs_a_third_of_itself_every_month() {
        // 900 TND per quarter = 300 TND per month, to the millime
        assert_eq!(900_000 / period_months("quarter"), 300_000);
        assert_eq!(1_200_000 / period_months("year"), 100_000);
    }
}
