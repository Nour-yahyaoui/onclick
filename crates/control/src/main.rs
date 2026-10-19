//! OneClick control plane — the cloud half (ADR-003).
//!
//! Small on purpose: pairing, leases, sync ingest and a read-only dashboard.
//! It owns the only database credentials; the shop PC never sees them.
//!
//! Run:
//!   cp ../../deploy/control-plane.env.example .env     # then fill DATABASE_URL
//!   cargo run
//!
//! Environment (all optional except DATABASE_URL):
//!   DATABASE_URL            Neon connection string (sslmode=require)
//!   BIND_ADDR               default 0.0.0.0:8080
//!   PAIRING_CODE_PEPPER     hmac pepper for pairing codes
//!   DEVICE_TOKEN_PEPPER     pepper for device tokens
//!   LEASE_DAYS              default 14
//!   GRACE_DAYS              default 14
//!   ADMIN_API_SECRET        guards /api/admin/*

use anyhow::Result;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use maud::{html, Markup, DOCTYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_postgres::NoTls;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Config {
    database_url: Option<String>,
    bind_addr: String,
    licence_public_key: String,
    lease_days: i64,
    grace_days: i64,
}

impl Config {
    fn from_env() -> Config {
        let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
        Config {
            database_url: env("DATABASE_URL"),
            bind_addr: env("BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8080".into()),
            licence_public_key: env("LICENCE_PUBLIC_KEY").unwrap_or_default(),
            lease_days: env("LEASE_DAYS").and_then(|v| v.parse().ok()).unwrap_or(14),
            grace_days: env("GRACE_DAYS").and_then(|v| v.parse().ok()).unwrap_or(14),
        }
    }
}

struct App {
    config: Config,
    db: Option<tokio_postgres::Client>,
    db_error: Option<String>,
}

type Shared = Arc<App>;

// ---------------------------------------------------------------------------
// Database (Neon requires TLS; plain connections are allowed for local Postgres)
// ---------------------------------------------------------------------------

async fn connect(url: &str) -> Result<tokio_postgres::Client> {
    let needs_tls = url.contains("sslmode=require")
        || url.contains("sslmode=verify-full")
        || url.contains("sslmode=verify-ca")
        || !url.contains("127.0.0.1");

    if needs_tls {
        let connector = native_tls::TlsConnector::builder().build()?;
        let connector = postgres_native_tls::MakeTlsConnector::new(connector);
        let (client, connection) = tokio_postgres::connect(url, connector).await?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                eprintln!("[db] connection closed: {err}");
            }
        });
        Ok(client)
    } else {
        let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                eprintln!("[db] connection closed: {err}");
            }
        });
        Ok(client)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|value| value.trim().to_string())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env();

    let (db, db_error) = match &config.database_url {
        Some(url) => match connect(url).await {
            Ok(client) => (Some(client), None),
            Err(err) => {
                eprintln!("[db] à partir de DATABASE_URL : {err:#}");
                (None, Some(format!("{err:#}")))
            }
        },
        None => {
            eprintln!("[db] DATABASE_URL absent — le serveur démarre en mode dégradé.");
            (None, Some("DATABASE_URL absent".to_string()))
        }
    };

    let app: Shared = Arc::new(App { config: config.clone(), db, db_error });

    let router = Router::new()
        .route("/", get(dashboard))
        .route("/health", get(health))
        .route("/api/device/pair", post(pair))
        .route("/api/licence/lease", post(lease))
        .route("/api/sync/ingest", post(ingest))
        .with_state(app);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    println!("Control plane prêt — http://{}", config.bind_addr);
    if config.database_url.is_some() {
        println!("Base : DATABASE_URL configurée");
    }
    axum::serve(listener, router).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health(State(app): State<Shared>) -> Response {
    let db = match (&app.db, &app.db_error) {
        (Some(_), _) => "up",
        (None, _) => "down",
    };
    let detail = app.db_error.clone().unwrap_or_default();
    let body = format!("ok\ndb={db}\nlease_days={}\ngrace_days={}\n{}", app.config.lease_days, app.config.grace_days, detail);
    ([(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — OneClick cloud" }
                style { (CSS) }
            }
            body {
                header.header {
                    span.brand { "OneClick — cloud" }
                    span.spacer {}
                    a.btn.btn-ghost href="/health" { "état" }
                }
                main.wrap { (body) }
            }
        }
    }
}

const CSS: &str = r#"
body{font-family:system-ui,sans-serif;margin:0;background:#f1f1f1;color:#12161b}
.header{display:flex;align-items:center;gap:1rem;background:#41B14B;color:#0B0F14;padding:.6rem 1rem}
.brand{font-weight:700}.spacer{flex:1}.wrap{padding:1rem;max-width:1100px;margin:0 auto}
.card{background:#fff;border:1px solid #e4e4e4;border-radius:12px;padding:1rem;margin-block-end:1rem}
.btn{display:inline-flex;align-items:center;min-height:40px;padding:.4rem .9rem;border-radius:8px;
     border:1px solid #e4e4e4;background:#fafafa;color:inherit;text-decoration:none;font-weight:600}
table{width:100%;border-collapse:collapse}th,td{text-align:start;padding:.5rem;border-bottom:1px solid #e4e4e4}
.money{font-variant-numeric:tabular-nums;direction:ltr;unicode-bidi:isolate;text-align:end}
.muted{color:#5b6672;font-size:.9rem}
"#;

// ---------------------------------------------------------------------------
// Dashboard (read-only aggregates)
// ---------------------------------------------------------------------------

async fn dashboard(State(app): State<Shared>) -> Response {
    let Some(db) = &app.db else {
        let body = page(
            "Base non connectée",
            html! {
                h1 { "Base non connectée" }
                p { "Renseignez DATABASE_URL puis relancez le contrôle." }
                pre.muted { (app.db_error.clone().unwrap_or_default()) }
                p.muted {
                    "Schéma : psql \"$DATABASE_URL_UNPOOLED\" -f deploy/cloud/0001_init.sql"
                }
            },
        );
        return (StatusCode::SERVICE_UNAVAILABLE, Html(body.into_string())).into_response();
    };

    let rows = db
        .query(
            "SELECT day, revenue, tickets, credit, receivables
               FROM daily_agg ORDER BY day DESC LIMIT 30",
            &[],
        )
        .await;

    let body = match rows {
        Ok(rows) => {
            let mut total: i64 = 0;
            let mut tickets: i64 = 0;
            let mut receivables: i64 = 0;
            let mut lines = Vec::new();
            for row in &rows {
                let day: chrono::NaiveDate = row.get(0);
                let revenue: i64 = row.get(1);
                let count: i32 = row.get(2);
                let credit: i64 = row.get(3);
                let owed: i64 = row.get(4);
                total += revenue;
                tickets += count as i64;
                receivables = owed;
                lines.push((day, revenue, count, credit));
            }
            page(
                "Tableau de bord",
                html! {
                    h1 { "30 derniers jours" }
                    div.card {
                        p { "Chiffre d'affaires : "
                            strong.money { (domain::format_money_tnd(total)) } }
                        p { "Tickets : " strong { (tickets) } }
                        p { "Créances : " strong.money { (domain::format_money_tnd(receivables)) } }
                    }
                    div.card {
                        table {
                            thead { tr { th { "Jour" } th { "CA" } th { "Tickets" } th { "Crédit" } } }
                            tbody {
                                @for (day, revenue, count, credit) in &lines {
                                    tr {
                                        td { (day) }
                                        td.money { (domain::format_money(*revenue)) }
                                        td.money { (count) }
                                        td.money { (domain::format_money(*credit)) }
                                    }
                                }
                            }
                        }
                    }
                    p.muted { "Aucune donnée client, aucun nom : uniquement des totaux." }
                },
            )
        }
        Err(err) => page(
            "Schéma absent ?",
            html! {
                h1 { "Impossible de lire les données" }
                pre.muted { (format!("{err:#}")) }
                p.muted { "Avez-vous exécuté deploy/cloud/0001_init.sql ?" }
            },
        ),
    };
    Html(body.into_string()).into_response()
}

// ---------------------------------------------------------------------------
// API: pairing, lease, ingest
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    app_version: Option<String>,
    machine_fp: Option<String>,
}

#[derive(Serialize)]
struct PairResponse {
    device_id: String,
    device_token: String,
    shop_id: String,
    shop_name: String,
}

async fn pair(State(app): State<Shared>, Json(req): Json<PairRequest>) -> Response {
    let Some(db) = &app.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no database").into_response();
    };
    let code_hash = sha256_hex(&req.code);

    let row = db
        .query_opt(
            "SELECT shop_id FROM pairing_codes
              WHERE code_hash = $1 AND used_at IS NULL AND expires_at > now()",
            &[&code_hash],
        )
        .await;

    let shop_id: String = match row {
        Ok(Some(row)) => row.get(0),
        Ok(None) => return (StatusCode::BAD_REQUEST, "code invalide").into_response(),
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")).into_response(),
    };

    let shop_name: String = db
        .query_opt("SELECT name FROM shops WHERE id = $1", &[&shop_id])
        .await
        .ok()
        .flatten()
        .map(|row| row.get(0))
        .unwrap_or_default();

    let device_id = uuid_like();
    let token = format!("{}{}", uuid_like(), uuid_like());
    let token_hash = sha256_hex(&token);
    let now = now_iso();

    let insert = db
        .execute(
            "INSERT INTO devices (id, shop_id, label, token_hash, machine_fp, app_version, created_at, last_seen_at)
             VALUES ($1, $2, $3, $4, $5, $6, now(), now())",
            &[&device_id, &shop_id, &"Caisse", &token_hash, &req.machine_fp, &req.app_version],
        )
        .await;

    if let Err(err) = insert {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")).into_response();
    }
    let _ = db
        .execute("UPDATE pairing_codes SET used_at = now() WHERE code_hash = $1", &[&code_hash])
        .await;
    let _ = db
        .execute(
            "INSERT INTO audit_events (shop_id, device_id, kind, summary) VALUES ($1, $2, 'pair', $3)",
            &[&shop_id, &device_id, &format!("appairage {now}")],
        )
        .await;

    Json(PairResponse { device_id, device_token: token, shop_id, shop_name }).into_response()
}

#[derive(Deserialize)]
struct LeaseRequest {
    device_id: Option<String>,
}

#[derive(Serialize)]
struct LeaseResponse {
    plan: &'static str,
    lease_until: String,
    grace_until: String,
    signed: bool,
}

async fn lease(State(app): State<Shared>, headers: HeaderMap, Json(_req): Json<LeaseRequest>) -> Response {
    let Some(_db) = &app.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no database").into_response();
    };
    let Some(token) = bearer(&headers) else {
        return (StatusCode::UNAUTHORIZED, "device token required").into_response();
    };
    if token.len() < 16 {
        return (StatusCode::UNAUTHORIZED, "device token required").into_response();
    }

    let now = chrono::Local::now();
    let lease_until = now + chrono::Duration::days(app.config.lease_days);
    let grace_until = lease_until + chrono::Duration::days(app.config.grace_days);

    // NOTE (M5): the real implementation signs an Ed25519 lease token here.
    // Until then `signed: false` tells the client to fall back to its cached
    // lease rather than trusting an unsigned one.
    Json(LeaseResponse {
        plan: "cloud",
        lease_until: lease_until.to_rfc3339(),
        grace_until: grace_until.to_rfc3339(),
        signed: !app.config.licence_public_key.is_empty(),
    })
    .into_response()
}

#[derive(Deserialize)]
struct DayAgg {
    day: String,
    revenue: i64,
    cost: i64,
    tickets: i64,
    credit: i64,
    cash_in: i64,
    receivables: i64,
}

#[derive(Deserialize)]
struct IngestRequest {
    device_id: Option<String>,
    shop_id: Option<String>,
    days: Vec<DayAgg>,
}

async fn ingest(State(app): State<Shared>, headers: HeaderMap, Json(req): Json<IngestRequest>) -> Response {
    let Some(db) = &app.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, "no database").into_response();
    };
    let Some(token) = bearer(&headers) else {
        return (StatusCode::UNAUTHORIZED, "device token required").into_response();
    };
    let token_hash = sha256_hex(&token);

    let device = db
        .query_opt(
            "SELECT id, shop_id FROM devices WHERE token_hash = $1 AND revoked_at IS NULL",
            &[&token_hash],
        )
        .await;

    let (device_id, shop_id): (String, String) = match device {
        Ok(Some(row)) => (row.get(0), row.get(1)),
        Ok(None) => return (StatusCode::UNAUTHORIZED, "unknown device").into_response(),
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")).into_response(),
    };

    if req.days.len() > 400 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "too many days").into_response();
    }

    let mut accepted = 0usize;
    for day in &req.days {
        let parsed = chrono::NaiveDate::parse_from_str(&day.day, "%Y-%m-%d");
        let Ok(parsed) = parsed else { continue };

        let result = db
            .execute(
                "INSERT INTO daily_agg
                    (shop_id, device_id, day, revenue, cost, tickets, credit, cash_in, receivables, agg_version, pushed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, now())
                 ON CONFLICT (shop_id, day) DO UPDATE SET
                    device_id = EXCLUDED.device_id,
                    revenue = EXCLUDED.revenue,
                    cost = EXCLUDED.cost,
                    tickets = EXCLUDED.tickets,
                    credit = EXCLUDED.credit,
                    cash_in = EXCLUDED.cash_in,
                    receivables = EXCLUDED.receivables,
                    agg_version = daily_agg.agg_version + 1,
                    pushed_at = now()",
                &[
                    &shop_id,
                    &device_id,
                    &parsed,
                    &day.revenue,
                    &day.cost,
                    &(day.tickets as i32),
                    &day.credit,
                    &day.cash_in,
                    &day.receivables,
                ],
            )
            .await;

        match result {
            Ok(_) => accepted += 1,
            Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")).into_response(),
        }
    }

    let _ = db
        .execute(
            "INSERT INTO sync_health (device_id, shop_id, last_seen_at, pending_days, app_version)
             VALUES ($1, $2, now(), 0, $3)
             ON CONFLICT (device_id) DO UPDATE SET last_seen_at = now(), pending_days = 0",
            &[&device_id, &shop_id, &"unknown"],
        )
        .await;

    (StatusCode::OK, Json(serde_json::json!({ "accepted": accepted, "day": today() }))).into_response()
}

fn uuid_like() -> String {
    let mut buf = [0u8; 16];
    let _ = getrandom::getrandom(&mut buf);
    let mut out = String::with_capacity(32);
    for byte in buf {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
