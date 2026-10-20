//! The till — axum handlers serving **server-rendered maud pages**.
//!
//! No `.html` files, no client framework, no JS bundle (ADR-014).
//! Everything the cashier does is a plain form POST + a re-render (~1–3 ms).

#![forbid(unsafe_code)]

use anyhow::Result;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use domain::{format_money, parse_money};
use maud::{html, Markup};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use storage::{Db, Payment, User};
use ui::{Chrome, Lang, Nav, Theme};

/// Built-in product pictures, shipped inside store.exe (about 41 KB in total).
/// A shop that has never taken a photo still gets something better than an
/// empty box — and can replace it with its own picture at any time.
static DEFAULT_PHOTOS: &[(&str, &[u8])] = &[
    ("phone", include_bytes!("../../ui/assets/defaults/phone.jpg")),
    ("charger", include_bytes!("../../ui/assets/defaults/charger.jpg")),
    ("cable", include_bytes!("../../ui/assets/defaults/cable.jpg")),
    ("case", include_bytes!("../../ui/assets/defaults/case.jpg")),
    ("earphones", include_bytes!("../../ui/assets/defaults/earphones.jpg")),
    ("headset", include_bytes!("../../ui/assets/defaults/headset.jpg")),
    ("powerbank", include_bytes!("../../ui/assets/defaults/powerbank.jpg")),
    ("protector", include_bytes!("../../ui/assets/defaults/protector.jpg")),
];

/// "d:charger" -> /photos/default/charger, "f:p_1.jpg" -> /photos/p_1.jpg
fn photo_url(photo: &str) -> Option<String> {
    let (kind, rest) = photo.split_once(':')?;
    match kind {
        "d" => Some(format!("/photos/default/{rest}")),
        "f" => Some(format!("/photos/{rest}")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    /// Where the database and its backups live. Needed to copy the file.
    pub data_dir: Arc<PathBuf>,
}

pub async fn serve(data_dir: PathBuf, port: u16) -> Result<()> {
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("store.db");
    let conn = storage::open(&db_path)?;
    storage::migrate(&conn)?;

    // Loud, honest warning while encryption is still the M0 task.
    for problem in storage::doctor(&conn, &db_path).unwrap_or_default() {
        eprintln!("[attention] {problem}");
    }

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        data_dir: Arc::new(data_dir.clone()),
    };
    let app = routes(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("OneClick prêt — http://{addr}");
    println!("Données : {}", db_path.display());
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", post(logout))
        .route("/setup", get(setup_page).post(setup_submit))
        .route("/theme", get(toggle_theme))
        .route("/sidebar", get(toggle_sidebar))
        .route("/lang/{code}", get(set_lang))
        .route("/sales", get(sales_page))
        .route("/sales/{id}", get(sale_detail_page))
        .route("/sales/{id}/edit", get(sale_edit_page).post(sale_update))
        .route("/sales/{id}/cancel", post(sale_cancel))
        .route("/products", get(products_page))
        .route("/products/new", get(product_new_page).post(product_create))
        .route("/products/{id}/edit", get(product_edit_page).post(product_update))
        .route("/products/{id}/archive", post(product_archive))
        .route("/journal", get(journal_page))
        .route("/session", get(session_page))
        .route("/session/open", post(session_open))
        .route("/session/close", post(session_close))
        .route("/receipt/{id}", get(receipt_page))
        .route("/products/quick", get(product_quick_page).post(product_quick_create))
        .route("/products/{id}/photo", post(product_photo))
        .route("/photos/{name}", get(photo_file))
        .route("/photos/default/{key}", get(photo_default))
        .route("/expenses", get(expenses_page).post(expense_create))
        .route("/expenses/{id}", post(expense_update))
        .route("/expenses/{id}/stop", post(expense_stop))
        .route("/cart/add", post(cart_add))
        .route("/cart/scan", post(cart_scan))
        .route("/cart/qty", post(cart_qty))
        .route("/cart/price", post(cart_price))
        .route("/checkout", post(checkout))
        .route("/carnet", get(carnet_page))
        .route("/carnet/collect", post(carnet_collect))
        .route("/carnet/new", post(carnet_new))
        .route("/carnet/writeoff", post(carnet_writeoff))
        .route("/settings", get(settings_page).post(settings_save))
        .route("/settings/backup", post(settings_backup))
        .route("/assets/store.css", get(stylesheet))
        .route("/health", get(health))
        // A photo taken with a phone is easily 3-4 MB; the default 2 MB would
        // reject it with a confusing error. 12 MB is generous but bounded.
        .layer(DefaultBodyLimit::max(12 * 1024 * 1024))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Small helpers: cookies, language, theme, responses
// ---------------------------------------------------------------------------

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut kv = part.trim().splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == name {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn cookie_header(name: &str, value: &str, max_age_secs: i64) -> String {
    format!("{name}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_secs}")
}

fn append_cookie(mut resp: Response, cookie: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        resp.headers_mut().append(header::SET_COOKIE, value);
    }
    resp
}

fn lang_of(headers: &HeaderMap) -> Lang {
    Lang::from_code(&cookie(headers, "lang").unwrap_or_else(|| "fr".to_string()))
}

fn sidebar_collapsed(headers: &HeaderMap) -> bool {
    cookie(headers, "side").as_deref() == Some("collapsed")
}

fn theme_of(headers: &HeaderMap) -> Theme {
    Theme::from_str(&cookie(headers, "theme").unwrap_or_default())
}

fn user_of(conn: &Db, headers: &HeaderMap) -> Option<User> {
    let sid = cookie(headers, "sid")?;
    storage::session_user(conn, &sid).ok().flatten()
}

fn shop_name(conn: &Db) -> String {
    storage::get_setting(conn, "shop_name")
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn page_response(markup: Markup) -> Response {
    let mut response = Html(markup.into_string()).into_response();
    // The till is stateful: never let the browser replay an old page (and an
    // old cart) from its back/forward cache after a sale.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );
    response
}

fn back(request: &HeaderMap) -> String {
    request
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "/".to_string())
}

/// DESIGN-003 §8: say what happened, what to do, and never lose work.
fn failure(lang: Lang, err: anyhow::Error) -> Response {
    eprintln!("[erreur] {err:#}");
    let body = html! {
        (ui::auth_bismillah())
        h1 { "Une erreur est survenue" }
        p { "Rien n'est perdu : vos ventes sont enregistrées." }
        p.muted { "Appuyez sur « Réessayer ». Si cela continue, appelez l'assistance." }
        div style="display:grid;gap:.5rem;margin-block-start:1rem" {
            button.btn.btn-primary.btn-block.btn-lg type="button"
                   onclick="location.reload()" { "Réessayer" }
            a.btn.btn-ghost.btn-block href="/" { "Retour au comptoir" }
        }
    };
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(ui::auth_page(lang, Theme::Light, "Erreur", body).into_string()),
    )
        .into_response()
}

fn chrome(
    conn: &Db,
    headers: &HeaderMap,
    user: &User,
    active: Nav,
    cart_count: usize,
    title: &str,
) -> Chrome {
    Chrome {
        lang: lang_of(headers),
        theme: theme_of(headers),
        shop: shop_name(conn),
        user: Some(user.name.clone()),
        active: Some(active),
        cart_count,
        title: title.to_string(),
        collapsed: sidebar_collapsed(headers),
    }
}

// ---------------------------------------------------------------------------
// Sell / POS
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    done: Option<String>,
}

/// Every action that redirects carries a short `done=` code, which becomes a
/// toast on the next screen (DESIGN-003: always say what just happened).
#[derive(Deserialize, Default)]
struct DoneQuery {
    done: Option<String>,
    saved: Option<String>,
}

fn toast_text(lang: Lang, code: &str) -> Option<&'static str> {
    let text = match code {
        "sale" => ui::t(lang, "toast.sale"),
        "credit" => ui::t(lang, "toast.credit"),
        "saved" => ui::t(lang, "toast.saved"),
        "created" => ui::t(lang, "toast.created"),
        "archived" => ui::t(lang, "toast.archived"),
        "collected" => ui::t(lang, "toast.collected"),
        "debt" => ui::t(lang, "toast.debt"),
        "written_off" => ui::t(lang, "toast.written_off"),
        "opened" => ui::t(lang, "toast.opened"),
        "closed" => ui::t(lang, "toast.closed"),
        "added" => ui::t(lang, "toast.added"),
        "backup" => ui::t(lang, "toast.backup"),
        "photo" => ui::t(lang, "toast.photo"),
        "photo_bad" => ui::t(lang, "photo.bad"),
        "expense" => ui::t(lang, "toast.expense"),
        "expense_updated" => ui::t(lang, "toast.expense_updated"),
        "expense_stopped" => ui::t(lang, "toast.expense_stopped"),
        "edited" => ui::t(lang, "toast.edited"),
        "cancelled" => ui::t(lang, "toast.cancelled"),
        _ => return None,
    };
    Some(text)
}

fn toast_from(query: &DoneQuery, lang: Lang) -> Option<Markup> {
    let code = query.done.clone().or_else(|| {
        query.saved.as_ref().map(|s| {
            if s == "1" || s == "true" { "saved".to_string() } else { s.clone() }
        })
    })?;
    toast_text(lang, &code).map(|text| ui::toast(text, "ok"))
}

async fn index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();

    if !storage::has_users(&conn).unwrap_or(false) {
        return Redirect::to("/setup").into_response();
    }
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    match render_index(&conn, &headers, &user, params.q.as_deref(), params.done.as_deref()) {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

fn pos_body(
    conn: &Db,
    headers: &HeaderMap,
    query: Option<&str>,
    done: Option<&str>,
) -> Result<Markup> {
    let sid = cookie(headers, "sid").unwrap_or_default();
    let lang = lang_of(headers);
    let drafts = storage::draft_lines(conn, &sid)?;
    let total: i64 = drafts.iter().map(|d| d.line_total).sum();
    let products = storage::list_products(conn, query)?;
    let (revenue, tickets) = storage::today_totals(conn)?;
    let session = storage::current_session(conn)?;
    let notice = done.and_then(|code| toast_text(lang, code));

    let body = html! {
        @if let Some(text) = notice {
            (ui::toast(text, "ok"))
        }

        div.page-head {
            h1 { (ui::t(lang, "nav.sell")) }
            span.muted {
                (ui::t(lang, "pos.today")) " " (ui::money_tnd(revenue))
                " · " (tickets) " " (ui::t(lang, "pos.tickets"))
            }
            span.spacer {}
            a.btn.btn-ghost href="/products/new" { (ui::t(lang, "pos.new_product")) }
            @if let Some(session) = &session {
                a.btn.btn-ghost href="/session" {
                    "Caisse ouverte depuis " (session.opened_at.get(11..16).unwrap_or(""))
                }
            } @else {
                a.btn.btn-ghost href="/session" { "Caisse fermée — ouvrir" }
            }
        }

        div.pos {
            section.pos-main {
                @if products.is_empty() {
                    div.card.card-pad {
                        (ui::empty_state(ui::t(lang, "products.empty")))
                        a.btn.btn-ghost href="/products/new" { (ui::t(lang, "pos.new_product")) }
                    }
                } @else {
                    div.grid {
                        @for p in &products {
                            (ui::product_card(&ui::ProductView {
                                id: p.id.as_str(),
                                name: p.name.as_str(),
                                price: p.price,
                                stock: p.stock,
                                counted: p.counted,
                                photo: p.photo.as_deref().and_then(photo_url),
                            }))
                        }
                    }
                }
            }

            aside.cart-panel {
                div.cart-head {
                    h2 { (ui::t(lang, "nav.sell")) }
                    @if !drafts.is_empty() {
                        span.count { (drafts.len()) }
                    }
                }

                /* The barcode machine always has somewhere to type.
                   Unknown code -> /sales? no: a two-question sheet, never a refusal. */
                form.scan-box method="post" action="/cart/scan" autocomplete="off" {
                    input type="text" name="code" dir="ltr" inputmode="numeric"
                          placeholder=(ui::t(lang, "scan.placeholder"))
                          aria-label=(ui::t(lang, "scan.placeholder"))
                          autofocus;
                    button.btn.btn-primary type="submit" { (ui::t(lang, "scan.add")) }
                }
                p.scan-hint { (ui::t(lang, "scan.hint")) }

                @if drafts.is_empty() {
                    (ui::empty_state(ui::t(lang, "pos.empty_cart")))
                } @else {
                    div.cart-lines {
                        @for line in &drafts {
                            (ui::draft_row(&ui::DraftView {
                                product_id: line.product_id.as_str(),
                                name: line.name.as_str(),
                                qty: line.qty,
                                unit_price: line.unit_price,
                                line_total: line.line_total,
                            }, lang))
                        }
                    }

                    div.cart-foot {
                        div.totals {
                            div.row {
                                span { (drafts.len()) " " (ui::t(lang, "pos.articles")) }
                                span.money-cell { (ui::money(total)) }
                            }
                            div.grand {
                                span { (ui::t(lang, "common.total")) }
                                span.money dir="ltr" {
                                    (format_money(total))
                                    span.unit { " DT" }
                                }
                            }
                        }

                        div.sell-actions {
                            form method="post" action="/checkout" {
                                button.btn.btn-primary.btn-block.btn-lg type="submit"
                                       name="payment" value="cash" {
                                    (ui::t(lang, "pos.charge"))
                                }
                            }
                            div.split {
                                form method="post" action="/checkout" {
                                    input type="hidden" name="payment" value="card";
                                    button.btn.btn-block type="submit" { (ui::t(lang, "pos.card")) }
                                }
                                form method="post" action="/checkout" {
                                    input type="hidden" name="payment" value="credit";
                                    button.btn.btn-block type="submit" { (ui::t(lang, "pos.credit")) }
                                }
                            }
                            details.no-print {
                                summary.muted style="cursor:pointer;padding-block:.35rem" {
                                    (ui::t(lang, "pos.credit")) " — nom du client"
                                }
                                form method="post" action="/checkout" {
                                    input type="hidden" name="payment" value="credit";
                                    label for="debtor" { (ui::t(lang, "pos.debtor")) }
                                    input type="text" id="debtor" name="debtor";
                                    label for="phone" { (ui::t(lang, "pos.phone")) }
                                    input type="text" id="phone" name="phone" dir="ltr";
                                    button.btn.btn-block type="submit" style="margin-block-start:.5rem" {
                                        (ui::t(lang, "pos.credit"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    Ok(body)
}

fn render_index(
    conn: &Db,
    headers: &HeaderMap,
    user: &User,
    query: Option<&str>,
    done: Option<&str>,
) -> Result<Markup> {
    let lang = lang_of(headers);
    let sid = cookie(headers, "sid").unwrap_or_default();
    let cart = storage::draft_lines(conn, &sid)?.len();
    let body = pos_body(conn, headers, query, done)?;
    Ok(ui::page(
        &chrome(conn, headers, user, Nav::Sell, cart, ui::t(lang, "nav.sell")),
        body,
    ))
}

// ---------------------------------------------------------------------------
// Cart
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AddForm {
    product_id: String,
}

async fn cart_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AddForm>,
) -> Response {
    let conn = state.db.lock().unwrap();
    if let Some(sid) = cookie(&headers, "sid") {
        if let Err(err) = storage::draft_add(&conn, &sid, &form.product_id) {
            return failure(lang_of(&headers), err);
        }
    }
    Redirect::to("/").into_response()
}

#[derive(Deserialize)]
struct QtyForm {
    product_id: String,
    qty: String,
}

async fn cart_qty(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<QtyForm>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let qty: i64 = form.qty.trim().parse().unwrap_or(1);
    if let Some(sid) = cookie(&headers, "sid") {
        if let Err(err) = storage::draft_set_qty(&conn, &sid, &form.product_id, qty) {
            return failure(lang_of(&headers), err);
        }
    }
    Redirect::to("/").into_response()
}

#[derive(Deserialize)]
struct PriceForm {
    product_id: String,
    unit_price: String,
}

async fn cart_price(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<PriceForm>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let price = parse_money(&form.unit_price).unwrap_or(0);
    if let Some(sid) = cookie(&headers, "sid") {
        if let Err(err) = storage::draft_set_price(&conn, &sid, &form.product_id, price) {
            return failure(lang_of(&headers), err);
        }
    }
    Redirect::to("/").into_response()
}

#[derive(Deserialize)]
struct CheckoutForm {
    payment: String,
    debtor: Option<String>,
    phone: Option<String>,
}

async fn checkout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CheckoutForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let Some(sid) = cookie(&headers, "sid") else {
        return Redirect::to("/login").into_response();
    };

    let payment = match form.payment.as_str() {
        "card" => Payment::Card,
        "credit" => {
            let name = form.debtor.unwrap_or_default();
            let name = name.trim().to_string();
            if name.is_empty() {
                // Never block the sale — record it as cash and let the cashier fix it.
                Payment::Cash
            } else {
                Payment::Credit {
                    name,
                    phone: form.phone.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()),
                }
            }
        }
        _ => Payment::Cash,
    };

    let is_credit = matches!(payment, Payment::Credit { .. });
    match storage::checkout(&conn, &sid, &user, payment) {
        // Straight to the ticket: the cashier hands it over or prints it.
        Ok(sale) => {
            let _ = is_credit;
            Redirect::to(&format!("/receipt/{}", sale.id)).into_response()
        }
        Err(err) => {
            if err.to_string().contains("empty_cart") {
                Redirect::to("/").into_response()
            } else {
                failure(lang, err)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Products
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProductSearch {
    done: Option<String>,
    q: Option<String>,
}

async fn products_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ProductSearch>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    if !storage::has_users(&conn).unwrap_or(false) {
        return Redirect::to("/setup").into_response();
    }
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let notice = params
        .done
        .as_deref()
        .and_then(|code| toast_text(lang, code))
        .map(|text| ui::toast(text, "ok"));

    let result = (|| -> Result<Markup> {
        let products = storage::list_products(&conn, params.q.as_deref())?;
        let body = html! {
            @if let Some(toast) = notice { (toast) }
            div style="display:flex;justify-content:space-between;align-items:center;margin-block-end:.75rem" {
                h1 { (ui::t(lang, "products.title")) }
                a.btn.btn-primary href="/products/new" { (ui::t(lang, "products.new")) }
            }
            form.scan method="get" action="/products" {
                input type="search" name="q" placeholder=(ui::t(lang, "pos.search"))
                      value=(params.q.clone().unwrap_or_default());
                button.btn type="submit" { "🔍" }
            }
            p.muted style="margin-block:0 12px" { (ui::t(lang, "products.fills_itself")) }
            div.card {
                table.list {
                    thead {
                        tr {
                            th { (ui::t(lang, "common.name")) }
                            th { (ui::t(lang, "products.barcode")) }
                            th.money-cell { (ui::t(lang, "common.price")) }
                            th.money-cell { (ui::t(lang, "common.stock")) }
                            th { "" }
                        }
                    }
                    tbody {
                        @for p in &products {
                            tr {
                                td {
                                    @if let Some(src) = p.photo.as_deref().and_then(photo_url) {
                                        img.row-photo src=(src) alt="" loading="lazy";
                                    }
                                    (p.name)
                                    @if p.price <= 0 {
                                        span.pill.credit { (ui::t(lang, "products.no_price")) }
                                    }
                                }
                                td.muted dir="ltr" { (p.barcode.clone().unwrap_or_default()) }
                                td.money-cell {
                                    @if p.price <= 0 { "—" } @else { (ui::money(p.price)) }
                                }
                                td.money-cell {
                                    @if p.counted { (p.stock) } @else { "—" }
                                }
                                td {
                                    a.btn.btn-ghost href=(format!("/products/{}/edit", p.id)) {
                                        (ui::t(lang, "common.edit"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Products, 0, ui::t(lang, "products.title")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

async fn product_new_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let body = html! {
        h1 { (ui::t(lang, "products.new")) }
        form method="post" action="/products/new" {
            label for="name" { (ui::t(lang, "common.name")) }
            input type="text" id="name" name="name" required autofocus;

            label for="barcode" { (ui::t(lang, "products.barcode")) }
            input type="text" id="barcode" name="barcode" dir="ltr";

            label for="price" { (ui::t(lang, "common.price")) }
            input type="text" id="price" name="price" dir="ltr" inputmode="decimal" placeholder="1 500.000";

            label for="cost" { (ui::t(lang, "common.cost")) }
            input type="text" id="cost" name="cost" dir="ltr" inputmode="decimal" placeholder="1 200.000";

            label for="qty" { (ui::t(lang, "products.qty")) }
            input type="number" id="qty" name="qty" value="1" min="0" dir="ltr";

            button.btn.btn-primary.btn-block.btn-lg type="submit" style="margin-block-start:1rem" {
                (ui::t(lang, "products.create"))
            }
        }
    };

    page_response(ui::page(
        &chrome(&conn, &headers, &user, Nav::Products, 0, ui::t(lang, "products.new")),
        body,
    ))
}

#[derive(Deserialize)]
struct ProductForm {
    name: String,
    barcode: Option<String>,
    price: Option<String>,
    cost: Option<String>,
    qty: Option<String>,
}

async fn product_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ProductForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let price = form.price.as_deref().and_then(parse_money).unwrap_or(0);
    let cost = form.cost.as_deref().and_then(parse_money).unwrap_or(0);
    let qty = form.qty.as_deref().and_then(|q| q.trim().parse::<i64>().ok()).unwrap_or(0);
    let barcode = form
        .barcode
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty());

    match storage::create_product(&conn, &user, form.name.trim(), barcode, price, cost, qty) {
        Ok(_) => Redirect::to("/products?done=created").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// One sale: header, its items, edit, cancel (stock comes back)
// ---------------------------------------------------------------------------

async fn sale_detail_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Query(query): Query<DoneQuery>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let notice = toast_from(&query, lang);
    let result = (|| -> Result<Markup> {
        let sale = storage::sale_detail(&conn, &path.id)?;
        let cancelled = sale.status != "closed";

        let body = html! {
            @if let Some(toast) = notice { (toast) }

            div.page-head {
                h1 { (ui::t(lang, "sales.one")) " " (sale.number) }
                span.muted dir="ltr" {
                    (sale.created_at.get(0..16).unwrap_or(&sale.created_at))
                }
                span.spacer {}
                a.btn.btn-ghost href="/sales" { (ui::t(lang, "sales.back")) }
            }

            div.card {
                table.list {
                    thead {
                        tr {
                            th { (ui::t(lang, "sales.item")) }
                            th.money-cell { (ui::t(lang, "sales.qty")) }
                            th.money-cell { (ui::t(lang, "sales.unit")) }
                            th.money-cell { (ui::t(lang, "common.total")) }
                        }
                    }
                    tbody {
                        @for line in &sale.lines {
                            tr {
                                td { (line.name) }
                                td.money-cell { (line.qty) }
                                td.money-cell { (format_money(line.unit_price)) }
                                td.money-cell { (format_money(line.line_total)) }
                            }
                        }
                    }
                }
                div.card-pad {
                    div.totals {
                        div.grand {
                            span { (ui::t(lang, "common.total")) }
                            span.money dir="ltr" { (format_money(sale.total)) }
                        }
                    }
                }
            }

            div.card.card-pad {
                table.list {
                    tbody {
                        tr {
                            td.muted { (ui::t(lang, "sales.seller")) }
                            td { (sale.seller) }
                        }
                        tr {
                            td.muted { (ui::t(lang, "sales.payment")) }
                            td { (payment_label(lang, &sale.payment)) }
                        }
                        @if let Some(debtor) = &sale.debtor {
                            tr { td.muted { (ui::t(lang, "sales.customer")) } td { (debtor) } }
                        }
                        @if sale.credit > 0 {
                            tr {
                                td.muted { (ui::t(lang, "carnet.paid")) }
                                td.money-cell { (format_money(sale.paid)) }
                            }
                            tr {
                                td.muted { (ui::t(lang, "carnet.balance")) }
                                td.money-cell { (format_money(sale.credit)) }
                            }
                        }
                        tr {
                            td.muted { (ui::t(lang, "sales.status")) }
                            td {
                                @if cancelled {
                                    span.pill.cancelled { (ui::t(lang, "sales.cancelled")) }
                                } @else {
                                    span.pill { (ui::t(lang, "sales.closed")) }
                                }
                            }
                        }
                    }
                }
            }

            @if !cancelled {
                div.page-actions {
                    a.btn.btn-primary href=(format!("/sales/{}/edit", sale.id)) {
                        (ui::t(lang, "sales.edit"))
                    }
                    a.btn.btn-ghost href=(format!("/receipt/{}", sale.id)) {
                        (ui::t(lang, "sales.ticket"))
                    }
                    form method="post" action=(format!("/sales/{}/cancel", sale.id))
                         onsubmit=(format!(
                            "return confirm('{}')",
                            ui::t(lang, "sales.cancel_confirm")
                         )) {
                        details.reason-box {
                            summary { (ui::t(lang, "sales.reason")) }
                            textarea name="reason" rows="2" {}
                        }
                        button.btn.btn-danger type="submit" { (ui::t(lang, "sales.cancel")) }
                    }
                }
                p.muted { (ui::t(lang, "sales.cancel_hint")) }
            }
        };

        let title = format!("{} {}", ui::t(lang, "sales.one"), sale.number);
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Sales, 0, &title),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

async fn sale_edit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let result = (|| -> Result<Markup> {
        let sale = storage::sale_detail(&conn, &path.id)?;
        let body = html! {
            div.page-head {
                h1 { (ui::t(lang, "sales.edit_title")) " " (sale.number) }
                span.spacer {}
                a.btn.btn-ghost href=(format!("/sales/{}", sale.id)) {
                    (ui::t(lang, "common.cancel"))
                }
            }

            form method="post" action=(format!("/sales/{}/edit", sale.id)) {
                div.card {
                    table.list {
                        thead {
                            tr {
                                th { (ui::t(lang, "sales.item")) }
                                th.money-cell { (ui::t(lang, "sales.qty")) }
                                th.money-cell { (ui::t(lang, "sales.unit")) }
                            }
                        }
                        tbody {
                            @for line in &sale.lines {
                                tr {
                                    td { (line.name) }
                                    td.money-cell {
                                        input.qty-input type="number" min="0" step="1"
                                            name=(format!("qty_{}", line.id))
                                            value=(line.qty) dir="ltr";
                                    }
                                    td.money-cell {
                                        input.price-input type="text" dir="ltr" inputmode="decimal"
                                            name=(format!("price_{}", line.id))
                                            value=(format_money(line.unit_price));
                                    }
                                }
                            }
                        }
                    }
                }

                p.muted style="margin-block-start:10px" { (ui::t(lang, "sales.qty_hint")) }

                div.page-actions {
                    button.btn.btn-primary type="submit" { (ui::t(lang, "sales.save")) }
                    a.btn.btn-ghost href=(format!("/sales/{}", sale.id)) { (ui::t(lang, "common.cancel")) }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Sales, 0, ui::t(lang, "sales.edit_title")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

async fn sale_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let mut changes: Vec<(String, i64, i64)> = Vec::new();
    for (key, value) in &form {
        if let Some(line_id) = key.strip_prefix("qty_") {
            let qty: i64 = value.trim().parse().unwrap_or(1).max(0);
            let price = form
                .get(&format!("price_{line_id}"))
                .and_then(|raw| parse_money(raw))
                .unwrap_or(0);
            changes.push((line_id.to_string(), qty, price));
        }
    }

    match storage::update_sale(&conn, &user, &path.id, &changes) {
        Ok(()) => Redirect::to(&format!("/sales/{}?done=edited", path.id)).into_response(),
        Err(err) => failure(lang, err),
    }
}

async fn sale_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let reason = form
        .get("reason")
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    match storage::cancel_sale(&conn, &user, &path.id, reason.as_deref()) {
        Ok(()) => Redirect::to(&format!("/sales/{}?done=cancelled", path.id)).into_response(),
        Err(err) => failure(lang, err),
    }
}

fn payment_label(lang: Lang, payment: &str) -> &'static str {
    match payment {
        "card" => ui::t(lang, "pos.card"),
        "credit" => ui::t(lang, "pos.credit"),
        _ => ui::t(lang, "pos.cash"),
    }
}


// ---------------------------------------------------------------------------
// Scan at the till — including codes the shop never entered
// ---------------------------------------------------------------------------
//
// The rule of this shop: the barcode machine must always work. If the code is
// known, it drops in the cart. If it is unknown, we do not refuse and we do not
// lecture: we ask for a name (optional) and a selling price (only), create the
// product, and put it in the cart. The sheet can be completed later.

/// Percent-encode the few characters a barcode could contain in a query.
fn urlencode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Deserialize)]
struct ScanForm {
    code: String,
}

async fn cart_scan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ScanForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(sid) = cookie(&headers, "sid") else {
        return Redirect::to("/login").into_response();
    };
    if user_of(&conn, &headers).is_none() {
        return Redirect::to("/login").into_response();
    }

    let code = form.code.trim();
    if code.is_empty() {
        return Redirect::to("/").into_response();
    }

    match storage::find_by_barcode(&conn, code) {
        Ok(Some(product)) => match storage::draft_add(&conn, &sid, &product.id) {
            Ok(()) => Redirect::to("/?done=added").into_response(),
            Err(err) => failure(lang, err),
        },
        // Unknown code: never a dead end — open the two-question sheet.
        Ok(None) => {
            Redirect::to(&format!("/products/quick?code={}", urlencode(code))).into_response()
        }
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct QuickQuery {
    code: Option<String>,
}

async fn product_quick_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<QuickQuery>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let code = query.code.unwrap_or_default().trim().to_string();

    let body = html! {
        div.page-head {
            h1 { (ui::t(lang, "quick.title")) }
            span.spacer {}
            a.btn.btn-ghost href="/" { (ui::t(lang, "quick.later")) }
        }

        div.card.card-pad style="max-inline-size:560px" {
            p.muted { (ui::t(lang, "quick.intro")) }
            div.quick-code dir="ltr" { (code) }

            form method="post" action="/products/quick" {
                input type="hidden" name="barcode" value=(code);

                label for="name" { (ui::t(lang, "quick.name")) }
                input type="text" id="name" name="name" autofocus
                      placeholder=(ui::t(lang, "quick.name_hint"));

                label for="price" { (ui::t(lang, "quick.price")) }
                input type="text" id="price" name="price" dir="ltr" inputmode="decimal"
                      placeholder="0.000";

                label for="cost" { (ui::t(lang, "quick.cost")) }
                input type="text" id="cost" name="cost" dir="ltr" inputmode="decimal"
                      placeholder="0.000";

                label for="qty" { (ui::t(lang, "quick.qty")) }
                input type="number" id="qty" name="qty" value="1" min="1" dir="ltr";

                button.btn.btn-primary.btn-block.btn-lg type="submit"
                       style="margin-block-start:1rem" {
                    (ui::t(lang, "quick.add"))
                }
            }
            p.muted style="margin-block-start:.75rem" { (ui::t(lang, "quick.note")) }
        }
    };

    page_response(ui::page(
        &chrome(&conn, &headers, &user, Nav::Sell, 0, ui::t(lang, "quick.title")),
        body,
    ))
}

#[derive(Deserialize)]
struct QuickForm {
    barcode: String,
    name: Option<String>,
    price: Option<String>,
    cost: Option<String>,
    qty: Option<String>,
}

async fn product_quick_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<QuickForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let Some(sid) = cookie(&headers, "sid") else {
        return Redirect::to("/login").into_response();
    };

    let barcode = form.barcode.trim().to_string();
    if barcode.is_empty() {
        return Redirect::to("/").into_response();
    }
    let name = form.name.unwrap_or_default();
    let price = form.price.as_deref().and_then(parse_money).unwrap_or(0);
    let cost = form.cost.as_deref().and_then(parse_money).unwrap_or(0);
    let qty = form
        .qty
        .as_deref()
        .and_then(|q| q.trim().parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);

    match storage::create_scanned_product(&conn, &user, &barcode, &name, price, cost) {
        Ok(product_id) => {
            let _ = storage::draft_set_qty(&conn, &sid, &product_id, qty);
            Redirect::to("/?done=added").into_response()
        }
        Err(err) => failure(lang, err),
    }
}


// ---------------------------------------------------------------------------
// Product photos: one file, on this machine, plus eight ready-made drawings
// ---------------------------------------------------------------------------

async fn photo_default(Path(path): Path<KeyPath>) -> Response {
    match DEFAULT_PHOTOS.iter().find(|(key, _)| *key == path.key) {
        Some((_, bytes)) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes.to_vec(),
        )
            .into_response(),
        // A missing key is not an error worth a page: no picture is.
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct KeyPath {
    key: String,
}

async fn photo_file(State(state): State<AppState>, Path(path): Path<NamePath>) -> Response {
    let Some(file) = storage::photo_file(&state.data_dir, &path.name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = std::fs::read(&file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let kind = match file.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    (
        [
            (header::CONTENT_TYPE, kind),
            // Re-uploading the same name must show the new picture at once.
            (header::CACHE_CONTROL, "private, max-age=60"),
        ],
        bytes,
    )
        .into_response()
}

#[derive(Deserialize)]
struct NamePath {
    name: String,
}

/// Accepts three intents in one POST: a new file, a built-in picture, or none.
///
/// The body is read *before* the database lock is taken: a `std::sync::Mutex`
/// guard held across an `.await` makes the handler future non-`Send`, and axum
/// then refuses the handler outright.
async fn product_photo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    mut multipart: Multipart,
) -> Response {
    let lang = lang_of(&headers);

    let mut chosen_default: Option<String> = None;
    let mut uploaded: Option<Vec<u8>> = None;
    let mut clear = false;
    let mut bad_format = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "default" => {
                if let Ok(value) = field.text().await {
                    let value = value.trim().to_string();
                    if !value.is_empty() {
                        chosen_default = Some(value);
                    }
                }
            }
            "clear" => clear = true,
            "file" => match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => {
                    // Trust the first bytes, not the file name: a phone photo
                    // is still called IMG_0042.
                    if storage::image_extension(&bytes).is_some() {
                        uploaded = Some(bytes.to_vec());
                    } else {
                        bad_format = true;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    if bad_format {
        return Redirect::to(&format!("/products/{}?done=photo_bad", path.id)).into_response();
    }

    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let outcome = if let Some(bytes) = uploaded {
        storage::save_product_photo(&conn, &user, &state.data_dir, &path.id, &bytes).map(|_| ())
    } else if let Some(key) = chosen_default {
        // Only the keys we actually ship are accepted.
        if DEFAULT_PHOTOS.iter().any(|(k, _)| *k == key) {
            storage::set_product_photo(&conn, &user, &path.id, Some(&format!("d:{key}")))
        } else {
            Ok(())
        }
    } else if clear {
        storage::set_product_photo(&conn, &user, &path.id, None)
    } else {
        Ok(())
    };

    match outcome {
        Ok(()) => Redirect::to(&format!("/products/{}?done=photo", path.id)).into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Fixed charges: the rent, the water, the power, the maintenance
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ExpenseForm {
    label: Option<String>,
    kind: Option<String>,
    amount: Option<String>,
    period: Option<String>,
    note: Option<String>,
    start_day: Option<String>,
}

fn expense_from(form: &ExpenseForm) -> (String, String, i64, String, Option<String>, String) {
    let label = form.label.clone().unwrap_or_default().trim().to_string();
    let kind = form.kind.clone().unwrap_or_else(|| "other".to_string());
    let amount = form.amount.as_deref().and_then(parse_money).unwrap_or(0);
    let period = form.period.clone().unwrap_or_else(|| "month".to_string());
    let note = form
        .note
        .clone()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    let start_day = form
        .start_day
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    let start_day = if start_day.is_empty() {
        storage::today_key()
    } else {
        start_day
    };
    (label, kind, amount, period, note, start_day)
}

fn kind_label(lang: Lang, kind: &str) -> &'static str {
    match kind {
        "rent" => ui::t(lang, "expense.kind.rent"),
        "power" => ui::t(lang, "expense.kind.power"),
        "water" => ui::t(lang, "expense.kind.water"),
        "maintenance" => ui::t(lang, "expense.kind.maintenance"),
        "telecom" => ui::t(lang, "expense.kind.telecom"),
        _ => ui::t(lang, "expense.kind.other"),
    }
}

fn period_label(lang: Lang, period: &str) -> &'static str {
    match period {
        "quarter" => ui::t(lang, "expense.period.quarter"),
        "year" => ui::t(lang, "expense.period.year"),
        _ => ui::t(lang, "expense.period.month"),
    }
}

async fn expenses_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DoneQuery>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let notice = toast_from(&query, lang);
    let result = (|| -> Result<Markup> {
        let rows = storage::list_expenses(&conn)?;
        let monthly = storage::monthly_expenses(&conn)?;
        let per_day = monthly / 30;

        let body = html! {
            @if let Some(toast) = notice { (toast) }

            div.page-head {
                h1 { (ui::t(lang, "expense.title")) }
                span.muted { (ui::t(lang, "expense.intro")) }
            }

            div.cards-row {
                div.card.card-pad {
                    div.muted { (ui::t(lang, "expense.per_month")) }
                    div.big-money { (ui::money_tnd(monthly)) }
                }
                div.card.card-pad {
                    div.muted { (ui::t(lang, "expense.per_day")) }
                    div.big-money { (ui::money_tnd(per_day)) }
                    div.muted { (ui::t(lang, "expense.per_day_hint")) }
                }
            }

            div.card.card-pad {
                h2 { (ui::t(lang, "expense.add")) }
                form method="post" action="/expenses" {
                    div.field-row {
                        div {
                            label for="label" { (ui::t(lang, "expense.label")) }
                            input type="text" id="label" name="label"
                                  placeholder=(ui::t(lang, "expense.label_hint"));
                        }
                        div {
                            label for="kind" { (ui::t(lang, "expense.kind")) }
                            select id="kind" name="kind" {
                                option value="rent" { (ui::t(lang, "expense.kind.rent")) }
                                option value="power" { (ui::t(lang, "expense.kind.power")) }
                                option value="water" { (ui::t(lang, "expense.kind.water")) }
                                option value="maintenance" { (ui::t(lang, "expense.kind.maintenance")) }
                                option value="telecom" { (ui::t(lang, "expense.kind.telecom")) }
                                option value="other" { (ui::t(lang, "expense.kind.other")) }
                            }
                        }
                    }
                    div.field-row {
                        div {
                            label for="amount" { (ui::t(lang, "expense.amount")) }
                            input type="text" id="amount" name="amount" dir="ltr"
                                  inputmode="decimal" placeholder="0.000";
                        }
                        div {
                            label for="period" { (ui::t(lang, "expense.period")) }
                            select id="period" name="period" {
                                option value="month" { (ui::t(lang, "expense.period.month")) }
                                option value="quarter" { (ui::t(lang, "expense.period.quarter")) }
                                option value="year" { (ui::t(lang, "expense.period.year")) }
                            }
                        }
                    }
                    button.btn.btn-primary type="submit"
                           style="margin-block-start:1rem" {
                        (ui::t(lang, "expense.save"))
                    }
                }
            }

            @if rows.is_empty() {
                div.card.card-pad { (ui::empty_state(ui::t(lang, "expense.empty"))) }
            } @else {
                div.card {
                    table.list {
                        thead {
                            tr {
                                th { (ui::t(lang, "expense.label")) }
                                th { (ui::t(lang, "expense.period")) }
                                th.money-cell { (ui::t(lang, "expense.amount")) }
                                th.money-cell { (ui::t(lang, "expense.per_month")) }
                                th { "" }
                            }
                        }
                        tbody {
                            @for row in &rows {
                                tr class=(if row.active { "" } else { "row-off" }) {
                                    td {
                                        strong { (row.label) }
                                        span.muted { " · " (kind_label(lang, &row.kind)) }
                                    }
                                    td { (period_label(lang, &row.period)) }
                                    td.money-cell { (format_money(row.amount)) }
                                    td.money-cell {
                                        (format_money(row.amount / storage::period_months(&row.period)))
                                    }
                                    td {
                                        @if row.active {
                                            details.inline-edit {
                                                summary { (ui::t(lang, "common.edit")) }
                                                form method="post" action=(format!("/expenses/{}", row.id)) {
                                                    div.field-row {
                                                        div {
                                                            label { (ui::t(lang, "expense.label")) }
                                                            input type="text" name="label" value=(row.label);
                                                        }
                                                        div {
                                                            label { (ui::t(lang, "expense.amount")) }
                                                            input type="text" name="amount" dir="ltr"
                                                                  inputmode="decimal"
                                                                  value=(format_money(row.amount));
                                                        }
                                                    }
                                                    div.field-row {
                                                        div {
                                                            label { (ui::t(lang, "expense.period")) }
                                                            select name="period" {
                                                                option value="month" selected[row.period == "month"] {
                                                                    (ui::t(lang, "expense.period.month"))
                                                                }
                                                                option value="quarter" selected[row.period == "quarter"] {
                                                                    (ui::t(lang, "expense.period.quarter"))
                                                                }
                                                                option value="year" selected[row.period == "year"] {
                                                                    (ui::t(lang, "expense.period.year"))
                                                                }
                                                            }
                                                        }
                                                        div {
                                                            label { (ui::t(lang, "expense.kind")) }
                                                            select name="kind" {
                                                                option value="rent" { (ui::t(lang, "expense.kind.rent")) }
                                                                option value="power" { (ui::t(lang, "expense.kind.power")) }
                                                                option value="water" { (ui::t(lang, "expense.kind.water")) }
                                                                option value="maintenance" { (ui::t(lang, "expense.kind.maintenance")) }
                                                                option value="telecom" { (ui::t(lang, "expense.kind.telecom")) }
                                                                option value="other" { (ui::t(lang, "expense.kind.other")) }
                                                            }
                                                        }
                                                    }
                                                    button.btn.btn-primary type="submit" {
                                                        (ui::t(lang, "common.save"))
                                                    }
                                                }
                                            }
                                            form method="post"
                                                 action=(format!("/expenses/{}/stop", row.id)) {
                                                button.btn.btn-ghost type="submit" {
                                                    (ui::t(lang, "expense.stop"))
                                                }
                                            }
                                        } @else {
                                            span.pill.cancelled { (ui::t(lang, "expense.stopped")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Expenses, 0, ui::t(lang, "expense.title")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

async fn expense_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ExpenseForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let (label, kind, amount, period, note, start_day) = expense_from(&form);
    match storage::create_expense(
        &conn,
        &user,
        &label,
        &kind,
        amount,
        &period,
        &start_day,
        note.as_deref(),
    ) {
        Ok(_) => Redirect::to("/expenses?done=expense").into_response(),
        Err(err) => failure(lang, err),
    }
}

async fn expense_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Form(form): Form<ExpenseForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let (label, kind, amount, period, note, _start) = expense_from(&form);
    match storage::update_expense(
        &conn,
        &user,
        &path.id,
        &label,
        &kind,
        amount,
        &period,
        note.as_deref(),
    ) {
        Ok(()) => Redirect::to("/expenses?done=expense_updated").into_response(),
        Err(err) => failure(lang, err),
    }
}

async fn expense_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match storage::stop_expense(&conn, &user, &path.id) {
        Ok(()) => Redirect::to("/expenses?done=expense_stopped").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Sales — the day book (reprint a ticket, see what was taken)
// ---------------------------------------------------------------------------

async fn sales_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let result = (|| -> Result<Markup> {
        let rows = storage::recent_sales(&conn, 100)?;
        let (revenue, _paid, tickets) = storage::day_totals(&conn)?;

        let body = html! {
            div.page-head {
                h1 { (ui::t(lang, "nav.sales")) }
                span.muted {
                    (ui::t(lang, "pos.today")) " " (ui::money_tnd(revenue))
                    " · " (tickets) " " (ui::t(lang, "pos.tickets"))
                }
            }

            @if rows.is_empty() {
                div.card.card-pad { (ui::empty_state("Aucune vente pour le moment.")) }
            } @else {
                div.card {
                    table.list {
                        thead {
                            tr {
                                th { "N°" }
                                th { "Heure" }
                                th { "Paiement" }
                                th { "Qui" }
                                th.money-cell { "Total" }
                                th { "" }
                            }
                        }
                        tbody {
                            @for sale in &rows {
                                tr {
                                    td dir="ltr" {
                                        a href=(format!("/sales/{}", sale.id)) { (sale.number) }
                                    }
                                    td dir="ltr" { (sale.created_at.get(11..16).unwrap_or("")) }
                                    td {
                                        @if sale.status != "closed" {
                                            span.pill.cancelled { (ui::t(lang, "sales.cancelled")) }
                                        } @else if sale.credit > 0 {
                                            span.pill.credit { (ui::t(lang, "pos.credit")) }
                                        } @else if sale.payment == "card" {
                                            span.pill { (ui::t(lang, "pos.card")) }
                                        } @else {
                                            span.pill { (ui::t(lang, "pos.cash")) }
                                        }
                                    }
                                    td.muted { (sale.seller) }
                                    td.money-cell { (ui::money(sale.total)) }
                                    td {
                                        a.btn.btn-ghost href=(format!("/receipt/{}", sale.id)) {
                                            "Ticket"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Sales, 0, ui::t(lang, "nav.sales")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Product edit / archive  (ADR-011: everything editable, everything attributed)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct IdPath {
    id: String,
}

async fn product_edit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let result = (|| -> Result<Markup> {
        let product = storage::get_product(&conn, &path.id)?;
        let stats = storage::product_stats(&conn, &path.id)?;
        let default_keys: Vec<&str> = DEFAULT_PHOTOS.iter().map(|(key, _)| *key).collect();

        let body = html! {
            p { a href="/products" { "← " (ui::t(lang, "products.title")) } }
            h1 { (product.name) }

            /* Photo: their own picture, or one of the eight ready-made ones. */
            div.card.card-pad style="margin-block-end:14px" {
                div.photo-row {
                    div.photo-preview {
                        @if let Some(src) = product.photo.as_deref().and_then(photo_url) {
                            img src=(src) alt="" ;
                        } @else {
                            div.photo-blank { (ui::icon("image")) }
                        }
                    }
                    div.photo-actions {
                        h3 { (ui::t(lang, "photo.title")) }
                        p.muted style="margin:0 0 8px" { (ui::t(lang, "photo.hint")) }

                        form method="post" action=(format!("/products/{}/photo", product.id))
                             enctype="multipart/form-data" {
                            label.btn.btn-primary for="photo-file" {
                                (ui::icon("camera"))
                                (ui::t(lang, "photo.upload"))
                            }
                            input type="file" id="photo-file" name="file" accept="image/*"
                                  onchange="this.form.submit()"
                                  style="display:none";
                        }

                        details.photo-defaults {
                            summary { (ui::t(lang, "photo.choose_default")) }
                            form method="post" action=(format!("/products/{}/photo", product.id)) {
                                div.photo-grid {
                                    @for key in default_keys.iter().copied() {
                                        button.photo-pick type="submit" name="default" value=(key)
                                               title=(ui::t(lang, &format!("photo.type.{key}"))) {
                                            img src=(format!("/photos/default/{key}")) alt="";
                                        }
                                    }
                                }
                            }
                        }

                        @if product.photo.is_some() {
                            form method="post" action=(format!("/products/{}/photo", product.id)) {
                                input type="hidden" name="clear" value="1";
                                button.btn.btn-ghost type="submit" {
                                    (ui::icon("trash"))
                                    (ui::t(lang, "photo.remove"))
                                }
                            }
                        }
                    }
                }
            }

            /* What this product actually did — the shop learns its own prices. */
            div.card.card-pad style="margin-block-end:14px" {
                @if stats.sales == 0 {
                    p.muted style="margin:0" { (ui::t(lang, "product.never_sold")) }
                } @else {
                    div.cards-row style="margin:0" {
                        div {
                            div.muted { (ui::t(lang, "product.sold")) }
                            div.big-money { (stats.sold_qty) }
                        }
                        div {
                            div.muted { (ui::t(lang, "product.revenue")) }
                            div.big-money { (ui::money_tnd(stats.revenue)) }
                        }
                        @if let Some(last) = &stats.last_sold {
                            div {
                                div.muted { (ui::t(lang, "product.last_sold")) }
                                div.muted dir="ltr" {
                                    (last.get(0..10).unwrap_or(last.as_str()))
                                }
                            }
                        }
                    }
                }
            }

            form.form-card method="post" action=(format!("/products/{}/edit", product.id)) {
                label for="name" { (ui::t(lang, "common.name")) }
                input type="text" id="name" name="name" value=(product.name)
                      placeholder=(ui::t(lang, "quick.name_hint")) autofocus;
                p.muted style="margin:0" { (ui::t(lang, "quick.note")) }

                label for="barcode" { (ui::t(lang, "products.barcode")) }
                input type="text" id="barcode" name="barcode" dir="ltr"
                      value=(product.barcode.clone().unwrap_or_default());

                label for="price" { (ui::t(lang, "common.price")) }
                input type="text" id="price" name="price" dir="ltr" inputmode="decimal"
                      value=(format_money(product.price));

                label for="cost" { (ui::t(lang, "common.cost")) }
                input type="text" id="cost" name="cost" dir="ltr" inputmode="decimal"
                      value=(format_money(product.cost));

                div.muted style="margin-block-start:.5rem" {
                    @if product.counted {
                        (ui::t(lang, "common.stock")) ": " (product.stock)
                    } @else {
                        (ui::t(lang, "product.not_counted"))
                    }
                }

                button.btn.btn-primary.btn-block.btn-lg type="submit" style="margin-block-start:1rem" {
                    (ui::t(lang, "common.save"))
                }
            }

            form method="post" action=(format!("/products/{}/archive", product.id))
                 style="margin-block-start:1.5rem" {
                button.btn.btn-ghost.btn-block type="submit" {
                    "Archiver ce produit"
                }
                p.muted { "Le produit disparaît des ventes. L'historique reste intact." }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Products, 0, "Modifier un produit"),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct ProductEditForm {
    name: String,
    barcode: Option<String>,
    price: Option<String>,
    cost: Option<String>,
}

async fn product_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Form(form): Form<ProductEditForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let price = form.price.as_deref().and_then(parse_money).unwrap_or(0);
    let cost = form.cost.as_deref().and_then(parse_money).unwrap_or(0);
    let barcode = form
        .barcode
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty());

    match storage::update_product(&conn, &user, &path.id, form.name.trim(), barcode, price, cost) {
        Ok(()) => Redirect::to("/products?done=saved").into_response(),
        Err(err) => failure(lang, err),
    }
}

async fn product_archive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    match storage::archive_product(&conn, &user, &path.id, true) {
        Ok(()) => Redirect::to("/products?done=archived").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Journal — "Qui a fait quoi" (ADR-011 §5.2)
// ---------------------------------------------------------------------------

async fn journal_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let result = (|| -> Result<Markup> {
        let rows = storage::recent_audit(&conn, 200)?;
        let body = html! {
            h1 { "Qui a fait quoi" }
            p.muted { "Les 200 dernières actions, de la plus récente à la plus ancienne." }

            @if rows.is_empty() {
                div.card.card-pad { (ui::empty_state("Rien à afficher pour l'instant.")) }
            } @else {
                div.card {
                    table.list {
                        thead {
                            tr {
                                th { "Quand" }
                                th { "Qui" }
                                th { "Quoi" }
                            }
                        }
                        tbody {
                            @for row in &rows {
                                tr {
                                    td.muted dir="ltr" { (row.at.get(0..16).unwrap_or(&row.at)) }
                                    td {
                                        strong { (row.actor) }
                                        div.muted { (row.role) }
                                    }
                                    td { (row.summary) }
                                }
                            }
                        }
                    }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Settings, 0, "Qui a fait quoi"),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Cash session + Z report
// ---------------------------------------------------------------------------

async fn session_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DoneQuery>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let notice = toast_from(&query, lang);

    let result = (|| -> Result<Markup> {
        let current = storage::current_session(&conn)?;
        let last_report = match storage::last_closed_session(&conn)? {
            Some(last) => Some(storage::session_totals(&conn, &last.id)?),
            None => None,
        };
        let body = match &current {
            Some(session) => {
                let report = storage::session_totals(&conn, &session.id)?;
                html! {
                    h1 { "Caisse ouverte" }
                    p.muted { "Ouverte le " (report.opened_at.get(0..16).unwrap_or(&report.opened_at)) }

                    div.card.card-pad {
                        table.list {
                            tbody {
                                tr { td { "Fond de caisse" } td.money-cell { (ui::money(report.opening_float)) } }
                                tr { td { "Ventes espèces" } td.money-cell { (ui::money(report.cash_in)) } }
                                tr { td { "Ventes carte" } td.money-cell { (ui::money(report.card_in)) } }
                                tr { td { "Ventes à crédit" } td.money-cell { (ui::money(report.credit_out)) } }
                                tr { td { "Dettes encaissées" } td.money-cell { (ui::money(report.collected)) } }
                                tr { td { "Tickets" } td.money-cell { (report.tickets) } }
                            }
                        }
                        div.totals {
                            div.grand {
                                span { "Attendu en caisse" }
                                span.money { (format_money(report.expected_cash)) }
                            }
                        }
                    }

                    div.card.card-pad style="margin-block-start:1rem" {
                        h2 { "Fermer la caisse (Z)" }
                        form method="post" action="/session/close" {
                            label for="counted" { "Montant compté dans le tiroir" }
                            input type="text" id="counted" name="counted" dir="ltr"
                                  inputmode="decimal" value=(format_money(report.expected_cash));
                            button.btn.btn-primary.btn-block.btn-lg type="submit" style="margin-block-start:.75rem" {
                                "Fermer la caisse"
                            }
                        }
                    }
                }
            }
            None => html! {
                h1 { "Caisse fermée" }
                p { "Ouvrez la caisse pour commencer la journée." }
                form method="post" action="/session/open" {
                    label for="float" { "Fond de caisse (espèces dans le tiroir)" }
                    input type="text" id="float" name="float" dir="ltr" inputmode="decimal" value="0.000";
                    button.btn.btn-primary.btn-block.btn-lg type="submit" style="margin-block-start:.75rem" {
                        "Ouvrir la caisse"
                    }
                }

                @if let Some(report) = last_report {
                    div.card.card-pad style="margin-block-start:1.5rem" {
                        h2 { "Dernier Z" }
                        table.list {
                            tbody {
                                tr { td { "Attendu" } td.money-cell { (ui::money(report.expected_cash)) } }
                                tr {
                                    td { "Compté" }
                                    td.money-cell { (ui::money(report.counted_cash.unwrap_or(0))) }
                                }
                                tr {
                                    td { "Écart" }
                                    td.money-cell { (ui::money(report.difference.unwrap_or(0))) }
                                }
                            }
                        }
                    }
                }
            },
        };
        let body = html! {
            @if let Some(toast) = notice { (toast) }
            (body)
        };
        Ok(ui::page(&chrome(&conn, &headers, &user, Nav::Sell, 0, "Caisse"), body))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct OpenSessionForm {
    float: Option<String>,
}

async fn session_open(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<OpenSessionForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let float = form.float.as_deref().and_then(parse_money).unwrap_or(0);
    match storage::open_session(&conn, &user, float) {
        Ok(_) => Redirect::to("/session?done=opened").into_response(),
        Err(err) => {
            // Already open -> simply show the screen, never an error page.
            if err.to_string().contains("already_open") {
                Redirect::to("/session").into_response()
            } else {
                failure(lang, err)
            }
        }
    }
}

#[derive(Deserialize)]
struct CloseSessionForm {
    counted: Option<String>,
}

async fn session_close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CloseSessionForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let counted = form.counted.as_deref().and_then(parse_money).unwrap_or(0);
    match storage::close_session(&conn, &user, counted) {
        Ok(_) => Redirect::to("/session?done=closed").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Receipt — printed by the browser (no driver needed in v0.2)
// ---------------------------------------------------------------------------

async fn receipt_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    if user_of(&conn, &headers).is_none() {
        return Redirect::to("/login").into_response();
    }

    let result = (|| -> Result<Markup> {
        let receipt = storage::receipt(&conn, &path.id)?;
        let shop = shop_name(&conn);
        let body = html! {
            (ui::toast(ui::t(lang, "toast.sale"), "ok"))
            div.auth style="max-inline-size:520px;margin:0 auto" {
                div.card.card-pad {
                    div style="text-align:center" {
                        @if !shop.is_empty() { h2 { (shop) } }
                        div.muted dir="ltr" { (receipt.created_at.get(0..16).unwrap_or(&receipt.created_at)) }
                    }
                    table.list style="margin-block-start:1rem" {
                        thead {
                            tr {
                                th { "Article" }
                                th.money-cell { "Qté" }
                                th.money-cell { "Prix" }
                                th.money-cell { "Total" }
                            }
                        }
                        tbody {
                            @for (name, qty, price, total) in &receipt.lines {
                                tr {
                                    td { (name) }
                                    td.money-cell { (qty) }
                                    td.money-cell { (ui::money(*price)) }
                                    td.money-cell { (ui::money(*total)) }
                                }
                            }
                        }
                    }
                    div.totals {
                        div.grand {
                            span { (ui::t(lang, "common.total")) }
                            span.money { (format_money(receipt.total)) }
                        }
                        @if receipt.credit > 0 {
                            div.row {
                                span { (ui::t(lang, "pos.credit")) }
                                span.money-cell { (ui::money(receipt.credit)) }
                            }
                        }
                    }
                    p.muted style="text-align:center;margin-block-start:1rem" {
                        "Ticket " (receipt.number) " · " (receipt.seller)
                    }
                }
                div style="display:flex;gap:.5rem;margin-block-start:1rem" {
                    a.btn.btn-primary href="/" { (ui::t(lang, "pos.new_sale")) }
                    button.btn.btn-ghost type="button" onclick="window.print()" { "Imprimer" }
                }
            }
        };
        Ok(ui::auth_page(lang, theme_of(&headers), "Ticket", body))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Carnet (debts)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CarnetParams {
    done: Option<String>,
}

async fn carnet_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CarnetParams>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let notice = params
        .done
        .as_deref()
        .and_then(|code| toast_text(lang, code))
        .map(|text| ui::toast(text, "ok"));

    let result = (|| -> Result<Markup> {
        let debts = storage::open_debts(&conn)?;
        let total: i64 = debts.iter().map(|d| d.balance).sum();
        let body = html! {
            @if let Some(toast) = notice { (toast) }
            @if params.done.is_some() {
                (ui::banner("ok", ui::t(lang, "done.credit"), ""))
            }
            h1 { (ui::t(lang, "carnet.title")) }
            p.muted { (ui::t(lang, "carnet.balance")) ": " (ui::money_tnd(total)) }

            div.card.card-pad style="margin-block-end:1rem" {
                h2 { "Ajouter une dette (sans vente)" }
                form method="post" action="/carnet/new" {
                    label for="name" { (ui::t(lang, "pos.debtor")) }
                    input type="text" id="name" name="name" required;
                    label for="phone" { (ui::t(lang, "pos.phone")) }
                    input type="text" id="phone" name="phone" dir="ltr";
                    label for="amount" { "Montant" }
                    input type="text" id="amount" name="amount" dir="ltr" inputmode="decimal" required;
                    label for="reason" { "Motif (optionnel)" }
                    input type="text" id="reason" name="reason";
                    button.btn.btn-primary.btn-block type="submit" style="margin-block-start:.5rem" {
                        "Ajouter au carnet"
                    }
                }
            }

            @if debts.is_empty() {
                div.card.card-pad { (ui::empty_state(ui::t(lang, "carnet.empty"))) }
            } @else {
                div.card {
                    table.list {
                        thead {
                            tr {
                                th { (ui::t(lang, "carnet.debtor")) }
                                th { (ui::t(lang, "carnet.opened")) }
                                th.money-cell { (ui::t(lang, "carnet.balance")) }
                                th { "" }
                            }
                        }
                        tbody {
                            @for d in &debts {
                                tr {
                                    td {
                                        strong { (d.name) }
                                        @if let Some(phone) = &d.phone {
                                            div.muted dir="ltr" { (phone) }
                                        }
                                    }
                                    td.muted { (d.opened_day) }
                                    td.money-cell { (ui::money(d.balance)) }
                                    td {
                                        form method="post" action="/carnet/collect" style="display:flex;gap:.4rem" {
                                            input type="hidden" name="debt_id" value=(d.id);
                                            input type="text" name="amount" dir="ltr"
                                                  value=(format_money(d.balance));
                                            button.btn.btn-primary type="submit" {
                                                (ui::t(lang, "carnet.collect"))
                                            }
                                        }
                                        form method="post" action="/carnet/writeoff" style="margin-block-start:.35rem" {
                                            input type="hidden" name="debt_id" value=(d.id);
                                            input type="hidden" name="reason" value="irrécouvrable";
                                            button.btn.btn-ghost type="submit" style="min-height:36px;font-size:.9rem" {
                                                "Passer en perte"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Carnet, 0, ui::t(lang, "carnet.title")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct CollectForm {
    debt_id: String,
    amount: String,
}

async fn carnet_collect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CollectForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let amount = parse_money(&form.amount).unwrap_or(0);
    match storage::collect(&conn, &form.debt_id, amount, "cash", &user) {
        Ok(()) => Redirect::to("/carnet?done=collected").into_response(),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct NewDebtForm {
    name: String,
    phone: Option<String>,
    amount: Option<String>,
    reason: Option<String>,
}

async fn carnet_new(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<NewDebtForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let amount = form.amount.as_deref().and_then(parse_money).unwrap_or(0);
    let phone = form
        .phone
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    match storage::create_debt(
        &conn,
        &user,
        form.name.trim(),
        phone,
        amount,
        form.reason.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
    ) {
        Ok(_) => Redirect::to("/carnet?done=debt").into_response(),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct WriteOffForm {
    debt_id: String,
    reason: Option<String>,
}

async fn carnet_writeoff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<WriteOffForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };
    let reason = form.reason.unwrap_or_else(|| "irrécouvrable".to_string());
    match storage::write_off_debt(&conn, &user, &form.debt_id, &reason) {
        Ok(()) => Redirect::to("/carnet?done=written_off").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Auth: setup, login, logout
// ---------------------------------------------------------------------------

async fn setup_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    if storage::has_users(&conn).unwrap_or(false) {
        return Redirect::to("/login").into_response();
    }
    drop(conn);

    let body = html! {
        (ui::auth_brand(""))
        (ui::auth_bismillah())

        h1 { (ui::t(lang, "setup.title")) }
        p.muted { (ui::t(lang, "setup.intro")) }

        form method="post" action="/setup" {
            label for="shop_name" { (ui::t(lang, "setup.shop")) }
            input type="text" id="shop_name" name="shop_name" autofocus
                  autocomplete="organization" placeholder="Écran Bleu";

            label { (ui::t(lang, "setup.vat")) }
            div.choices {
                label.choice {
                    input type="radio" name="vat" value="no" checked;
                    span { (ui::t(lang, "setup.vat_no")) }
                }
                label.choice {
                    input type="radio" name="vat" value="single";
                    span { (ui::t(lang, "setup.vat_single")) }
                }
                label.choice {
                    input type="radio" name="vat" value="per_product";
                    span { (ui::t(lang, "setup.vat_per_product")) }
                }
            }

            label for="password" { (ui::t(lang, "setup.password")) }
            input type="password" id="password" name="password" required minlength="6"
                  autocomplete="new-password" placeholder="••••••••";

            button.btn.btn-primary.btn-block.btn-lg type="submit" {
                (ui::t(lang, "setup.start"))
            }
        }

        div.auth-foot {
            div.seg {
                @for l in Lang::ALL {
                    a href=(format!("/lang/{}", l.code()))
                        class=(if l == lang { "seg-item active" } else { "seg-item" }) {
                        (l.code().to_uppercase())
                    }
                }
            }
            a.btn.btn-ghost href="/theme" { (ui::t(lang, "theme.toggle")) }
        }
    };

    page_response(ui::auth_page(lang, theme_of(&headers), "Configuration", body))
}

#[derive(Deserialize)]
struct SetupForm {
    shop_name: Option<String>,
    vat: Option<String>,
    password: String,
}

async fn setup_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SetupForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();

    if storage::has_users(&conn).unwrap_or(false) {
        return Redirect::to("/login").into_response();
    }
    if form.password.trim().len() < 6 {
        return Redirect::to("/setup").into_response();
    }

    let result = (|| -> Result<Response> {
        let owner = storage::create_user(&conn, "Patron", "owner", form.password.trim())?;
        storage::set_setting(&conn, "shop_name", form.shop_name.unwrap_or_default().trim())?;
        storage::set_setting(&conn, "lang", lang.code())?;
        storage::set_setting(
            &conn,
            "compliance_profile",
            match form.vat.as_deref() {
                Some("single") => "forfaitaire",
                Some("per_product") => "reel",
                _ => "simple",
            },
        )?;

        // Six sample products so the very first action is a successful sale
        // (DESIGN-003 §7). Deletable in one tap.
        let samples: [(&str, i64, i64); 6] = [
            ("Chargeur USB-C", 4_500, 2_800),
            ("Câble USB-C 1m", 3_500, 1_900),
            ("Coque iPhone 15", 6_000, 3_200),
            ("Écouteurs filaires", 5_500, 3_000),
            ("Verre trempé", 3_000, 1_200),
            ("Powerbank 10 000", 18_000, 12_500),
        ];
        for (name, price, cost) in samples {
            storage::create_product(&conn, &owner, name, None, price, cost, 5)?;
        }

        storage::audit(
            &conn,
            &owner,
            "setup",
            "settings",
            "install",
            "Première configuration du magasin",
            None,
            None,
        )?;
        let sid = storage::start_session(&conn, &owner.id)?;
        let resp = Redirect::to("/").into_response();
        Ok(append_cookie(
            resp,
            &cookie_header("sid", &sid, 12 * 3600),
        ))
    })();

    match result {
        Ok(resp) => resp,
        Err(err) => failure(lang, err),
    }
}

async fn login_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    if !storage::has_users(&conn).unwrap_or(false) {
        return Redirect::to("/setup").into_response();
    }

    let shop = shop_name(&conn);
    let owner = storage::owner_name(&conn).unwrap_or(None).unwrap_or_else(|| "Patron".into());
    drop(conn);

    let body = html! {
        (ui::auth_brand(&shop))
        (ui::auth_bismillah())

        h1 { (ui::t(lang, "login.title")) }

        form method="post" action="/login" {
            input type="hidden" name="name" value=(owner);

            label for="password" { (ui::t(lang, "login.password")) }
            input type="password" id="password" name="password"
                  required autofocus autocomplete="current-password"
                  placeholder="••••••••";

            button.btn.btn-primary.btn-block.btn-lg type="submit" {
                (ui::t(lang, "login.enter"))
            }
        }

        div.auth-foot {
            div.seg {
                @for l in Lang::ALL {
                    a href=(format!("/lang/{}", l.code()))
                        class=(if l == lang { "seg-item active" } else { "seg-item" }) {
                        (l.code().to_uppercase())
                    }
                }
            }
            a.icon-btn href="/theme" title=(ui::t(lang, "theme.toggle")) {
                (ui::t(lang, "theme.short"))
            }
        }
    };

    page_response(ui::auth_page(lang, theme_of(&headers), ui::t(lang, "login.enter"), body))
}

#[derive(Deserialize)]
struct LoginForm {
    name: Option<String>,
    password: String,
}

async fn login_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let name = form.name.unwrap_or_else(|| "Patron".to_string());
    let name = if name.trim().is_empty() { "Patron".to_string() } else { name };

    match storage::authenticate(&conn, name.trim(), &form.password) {
        Ok(Some(user)) => {
            let _ = storage::audit(
                &conn,
                &user,
                "login",
                "user",
                &user.id,
                &format!("Connexion : {}", user.name),
                None,
                None,
            );
            let sid = storage::start_session(&conn, &user.id).unwrap_or_default();
            let resp = Redirect::to("/").into_response();
            append_cookie(resp, &cookie_header("sid", &sid, 12 * 3600))
        }
        Ok(None) => {
            let ghost = User {
                id: "system".into(),
                name: name.trim().to_string(),
                role: "unknown".into(),
            };
            let _ = storage::audit(
                &conn,
                &ghost,
                "login_fail",
                "user",
                "unknown",
                "Mot de passe incorrect",
                None,
                None,
            );
            // Same card as the normal sign-in: one field, and the reason why.
            let body = html! {
                (ui::auth_brand(&shop_name(&conn)))
                (ui::auth_bismillah())

                h1 { (ui::t(lang, "login.title")) }
                div.form-error role="alert" { (ui::t(lang, "login.error")) }

                form method="post" action="/login" {
                    input type="hidden" name="name" value=(name);
                    label for="password" { (ui::t(lang, "login.password")) }
                    input type="password" id="password" name="password"
                          required autofocus autocomplete="current-password"
                          placeholder="••••••••";
                    button.btn.btn-primary.btn-block.btn-lg type="submit" {
                        (ui::t(lang, "login.enter"))
                    }
                }
            };
            (
                StatusCode::UNAUTHORIZED,
                Html(ui::auth_page(lang, theme_of(&headers), "Entrer", body).into_string()),
            )
                .into_response()
        }
        Err(err) => failure(lang, err),
    }
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let conn = state.db.lock().unwrap();
    if let Some(user) = user_of(&conn, &headers) {
        let _ = storage::audit(&conn, &user, "logout", "user", &user.id, "Déconnexion", None, None);
    }
    if let Some(sid) = cookie(&headers, "sid") {
        let _ = storage::end_session(&conn, &sid);
    }
    let resp = Redirect::to("/login").into_response();
    append_cookie(resp, "sid=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
}

// ---------------------------------------------------------------------------
// Preferences: theme + language (no settings screen needed)
// ---------------------------------------------------------------------------

async fn toggle_sidebar(State(_state): State<AppState>, headers: HeaderMap) -> Response {
    let next = if sidebar_collapsed(&headers) { "open" } else { "collapsed" };
    let resp = Redirect::to(&back(&headers)).into_response();
    append_cookie(resp, &cookie_header("side", next, 365 * 86_400))
}

async fn toggle_theme(State(_state): State<AppState>, headers: HeaderMap) -> Response {
    let next = theme_of(&headers).toggled();
    let resp = Redirect::to(&back(&headers)).into_response();
    append_cookie(resp, &cookie_header("theme", next.as_str(), 365 * 86_400))
}

async fn set_lang(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Response {
    let lang = Lang::from_code(&code);
    let resp = Redirect::to(&back(&headers)).into_response();
    append_cookie(resp, &cookie_header("lang", lang.code(), 365 * 86_400))
}

// ---------------------------------------------------------------------------
// Settings (6 plain items — DESIGN-003 §6)
// ---------------------------------------------------------------------------

async fn settings_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DoneQuery>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let shop = shop_name(&conn);
    let copies = storage::backups(&state.data_dir);
    let notice = toast_from(&query, lang);

    // The till stays visible behind the dialog: settings never feels like another app.
    let result = (|| -> Result<Markup> {
        let behind = pos_body(&conn, &headers, None, None)?;
        let body = html! {
            (behind)
            @if let Some(toast) = notice {
                (toast)
            }

            div.overlay role="dialog" aria-modal="true" aria-labelledby="settings-title" {
                div.modal {
                    div.modal-head {
                        h2 #settings-title { (ui::t(lang, "settings.title")) }
                        a.modal-close href="/" aria-label=(ui::t(lang, "common.close")) { "×" }
                    }
                    div.modal-body {
                        div.field {
                            h3 { (ui::t(lang, "settings.lang")) }
                            div.seg {
                                @for l in Lang::ALL {
                                    a href=(format!("/lang/{}", l.code()))
                                        class=(if l == lang { "seg-item active" } else { "seg-item" }) {
                                        (l.label())
                                    }
                                }
                            }
                        }

                        div.field {
                            h3 { (ui::t(lang, "setup.shop")) }
                            form method="post" action="/settings" {
                                input type="text" name="shop_name" value=(shop);
                                button.btn.btn-primary type="submit" style="margin-block-start:.6rem" {
                                    (ui::t(lang, "common.save"))
                                }
                            }
                        }

                        div.field {
                            h3 { "Historique" }
                            p.muted { "Toutes les actions : ventes, modifications, encaissements." }
                            div style="display:flex;gap:.5rem;flex-wrap:wrap" {
                                a.btn href="/journal" { "Qui a fait quoi" }
                                a.btn href="/session" { "Caisse / Z" }
                            }
                        }

                        div.field {
                            h3 { (ui::t(lang, "settings.backup")) }
                            p.muted { (ui::t(lang, "settings.security")) }
                            form method="post" action="/settings/backup" {
                                button.btn.btn-primary type="submit" {
                                    (ui::t(lang, "settings.backup_now"))
                                }
                            }
                            @if copies.is_empty() {
                                p.muted { (ui::t(lang, "settings.backup_none")) }
                            } @else {
                                p.muted {
                                    (ui::t(lang, "settings.backup_last")) " "
                                    span dir="ltr" { (copies[0].at.replace('_', " ")) }
                                }
                                p.muted { (ui::t(lang, "settings.backup_count")) " " (copies.len()) }
                            }
                            p.muted { (ui::t(lang, "settings.backup_where")) }
                        }
                    }
                    div.modal-foot {
                        a.btn.btn-ghost href="/" { (ui::t(lang, "common.close")) }
                        button.btn.btn-primary type="button" onclick="location.reload()" {
                            (ui::t(lang, "common.done"))
                        }
                    }
                }
            }
        };
        Ok(ui::page(
            &chrome(&conn, &headers, &user, Nav::Settings, 0, ui::t(lang, "settings.title")),
            body,
        ))
    })();

    match result {
        Ok(markup) => page_response(markup),
        Err(err) => failure(lang, err),
    }
}

#[derive(Deserialize)]
struct SettingsForm {
    shop_name: Option<String>,
}

async fn settings_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    match storage::backup_into(&conn, &state.data_dir) {
        Ok(_) => {
            let _ = storage::audit(
                &conn,
                &user,
                "create",
                "settings",
                "backup",
                "Sauvegarde de la base effectuée",
                None,
                None,
            );
            Redirect::to("/settings?done=backup").into_response()
        }
        Err(err) => failure(lang, err),
    }
}

async fn settings_save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SettingsForm>,
) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let name = form.shop_name.unwrap_or_default().trim().to_string();
    let result = (|| -> Result<()> {
        storage::set_setting(&conn, "shop_name", &name)?;
        storage::audit(
            &conn,
            &user,
            "update",
            "settings",
            "shop_name",
            "Nom du magasin modifié",
            None,
            Some(format!("{{\"shop_name\":{name:?}}}").as_str()),
        )?;
        Ok(())
    })();

    match result {
        Ok(()) => Redirect::to("/settings?saved=1").into_response(),
        Err(err) => failure(lang, err),
    }
}

// ---------------------------------------------------------------------------
// Assets & health
// ---------------------------------------------------------------------------

async fn stylesheet() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")],
        ui::CSS,
    )
        .into_response()
}

async fn health(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    let schema = storage::schema_version(&conn).unwrap_or(-1);
    let users = storage::has_users(&conn).unwrap_or(false);
    let problems = storage::doctor(&conn, std::path::Path::new("data/store.db")).unwrap_or_default();
    let body = format!(
        "ok\ndb=ok\nschema={schema}\nsetup={}\nwarnings={}\n{}",
        if users { "done" } else { "pending" },
        problems.len(),
        problems.join("\n")
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}
