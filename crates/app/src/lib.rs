//! The till — axum handlers serving **server-rendered maud pages**.
//!
//! No `.html` files, no client framework, no JS bundle (ADR-014).
//! Everything the cashier does is a plain form POST + a re-render (~1–3 ms).

#![forbid(unsafe_code)]

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
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

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Db>>,
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

    let state = AppState { db: Arc::new(Mutex::new(conn)) };
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
        .route("/lang/{code}", get(set_lang))
        .route("/products", get(products_page))
        .route("/products/new", get(product_new_page).post(product_create))
        .route("/products/{id}/edit", get(product_edit_page).post(product_update))
        .route("/products/{id}/archive", post(product_archive))
        .route("/journal", get(journal_page))
        .route("/session", get(session_page))
        .route("/session/open", post(session_open))
        .route("/session/close", post(session_close))
        .route("/receipt/{id}", get(receipt_page))
        .route("/cart/add", post(cart_add))
        .route("/cart/qty", post(cart_qty))
        .route("/cart/price", post(cart_price))
        .route("/checkout", post(checkout))
        .route("/carnet", get(carnet_page))
        .route("/carnet/collect", post(carnet_collect))
        .route("/carnet/new", post(carnet_new))
        .route("/carnet/writeoff", post(carnet_writeoff))
        .route("/settings", get(settings_page).post(settings_save))
        .route("/assets/store.css", get(stylesheet))
        .route("/health", get(health))
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
    Html(markup.into_string()).into_response()
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
        h1 { "Une erreur est survenue" }
        p { "Rien n'est perdu : vos ventes sont enregistrées. Réessayez." }
        p.muted { "Si cela continue, appelez l'assistance." }
        a.btn.btn-primary.btn-block href="/" { "Retour" }
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

fn render_index(
    conn: &Db,
    headers: &HeaderMap,
    user: &User,
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

    let body = html! {
        @if let Some(done) = done {
            @if done == "credit" {
                (ui::banner("ok", ui::t(lang, "done.credit"), ""))
            } @else {
                (ui::banner("ok", ui::t(lang, "done.sale"), ""))
            }
        }

        div.page-head {
            h1 { (ui::t(lang, "nav.sell")) }
            span.muted {
                (ui::t(lang, "pos.today")) " " (ui::money_tnd(revenue))
                " · " (tickets) " " (ui::t(lang, "pos.tickets"))
            }
            span.spacer {}
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
                form.scan method="get" action="/" {
                    input type="search" name="q" dir="ltr"
                          placeholder=(ui::t(lang, "pos.search"))
                          value=(query.unwrap_or(""))
                          autofocus;
                    a.btn href="/products/new" { (ui::t(lang, "pos.new_product")) }
                }

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

    Ok(ui::page(&chrome(conn, headers, user, Nav::Sell, drafts.len(), ui::t(lang, "nav.sell")), body))
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

    let result = (|| -> Result<Markup> {
        let products = storage::list_products(&conn, params.q.as_deref())?;
        let body = html! {
            div style="display:flex;justify-content:space-between;align-items:center;margin-block-end:.75rem" {
                h1 { (ui::t(lang, "products.title")) }
                a.btn.btn-primary href="/products/new" { (ui::t(lang, "products.new")) }
            }
            form.scan method="get" action="/products" {
                input type="search" name="q" placeholder=(ui::t(lang, "pos.search"))
                      value=(params.q.clone().unwrap_or_default());
                button.btn type="submit" { "🔍" }
            }
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
                                td { (p.name) }
                                td.muted { (p.barcode.clone().unwrap_or_default()) }
                                td.money-cell { (ui::money(p.price)) }
                                td.money-cell { (p.stock) }
                                td {
                                    a.btn.btn-ghost href=(format!("/products/{}/edit", p.id)) {
                                        "Modifier"
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
        Ok(_) => Redirect::to("/products").into_response(),
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
        let body = html! {
            p { a href="/products" { "← " (ui::t(lang, "products.title")) } }
            h1 { (product.name) }

            form method="post" action=(format!("/products/{}/edit", product.id)) {
                label for="name" { (ui::t(lang, "common.name")) }
                input type="text" id="name" name="name" value=(product.name) required autofocus;

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
                    (ui::t(lang, "common.stock")) ": " (product.stock)
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
        Ok(()) => Redirect::to("/products").into_response(),
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
        Ok(()) => Redirect::to("/products").into_response(),
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

async fn session_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

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
        Ok(_) => Redirect::to("/session").into_response(),
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
        Ok(_) => Redirect::to("/session").into_response(),
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
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let result = (|| -> Result<Markup> {
        let receipt = storage::receipt(&conn, &path.id)?;
        let shop = shop_name(&conn);
        let body = html! {
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
                    button.btn.btn-primary type="button" onclick="window.print()" { "Imprimer" }
                    a.btn.btn-ghost href="/" { (ui::t(lang, "common.back")) }
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

    let result = (|| -> Result<Markup> {
        let debts = storage::open_debts(&conn)?;
        let total: i64 = debts.iter().map(|d| d.balance).sum();
        let body = html! {
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
        Ok(()) => Redirect::to("/carnet").into_response(),
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
        Ok(_) => Redirect::to("/carnet").into_response(),
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
        Ok(()) => Redirect::to("/carnet").into_response(),
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
        h1 { (ui::t(lang, "setup.title")) }
        p { (ui::t(lang, "setup.intro")) }

        div style="display:flex;gap:.5rem;margin-block-end:1rem" {
            @for l in Lang::ALL {
                a.btn.btn-ghost href=(format!("/lang/{}", l.code())) { (l.label()) }
            }
        }

        form method="post" action="/setup" {
            label for="shop_name" { (ui::t(lang, "setup.shop")) }
            input type="text" id="shop_name" name="shop_name" autofocus;

            label { (ui::t(lang, "setup.vat")) }
            div style="display:grid;gap:.35rem" {
                label style="font-weight:400" {
                    input type="radio" name="vat" value="no" checked; " " (ui::t(lang, "setup.vat_no"))
                }
                label style="font-weight:400" {
                    input type="radio" name="vat" value="single"; " " (ui::t(lang, "setup.vat_single"))
                }
                label style="font-weight:400" {
                    input type="radio" name="vat" value="per_product"; " " (ui::t(lang, "setup.vat_per_product"))
                }
            }

            label for="password" { (ui::t(lang, "setup.password")) }
            input type="password" id="password" name="password" required minlength="6";

            button.btn.btn-primary.btn-block.btn-lg type="submit" {
                (ui::t(lang, "setup.start"))
            }
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
    drop(conn);

    let body = html! {
        h1 { (ui::t(lang, "login.title")) }
        form method="post" action="/login" {
            label for="name" { (ui::t(lang, "login.user")) }
            input type="text" id="name" name="name" value="Patron" autofocus;

            label for="password" { (ui::t(lang, "login.password")) }
            input type="password" id="password" name="password" required;

            button.btn.btn-primary.btn-block.btn-lg type="submit" {
                (ui::t(lang, "login.enter"))
            }
        }
        div.muted style="margin-block-start:1rem" {
            @for l in Lang::ALL {
                a.btn.btn-ghost href=(format!("/lang/{}", l.code())) { (l.label()) }
            }
        }
    };

    page_response(ui::auth_page(lang, theme_of(&headers), "Entrer", body))
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
            let body = html! {
                (ui::banner("warn", ui::t(lang, "login.error"), ""))
                h1 { (ui::t(lang, "login.title")) }
                form method="post" action="/login" {
                    label for="name" { (ui::t(lang, "login.user")) }
                    input type="text" id="name" name="name" value="Patron";
                    label for="password" { (ui::t(lang, "login.password")) }
                    input type="password" id="password" name="password" required autofocus;
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

async fn settings_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let lang = lang_of(&headers);
    let conn = state.db.lock().unwrap();
    let Some(user) = user_of(&conn, &headers) else {
        return Redirect::to("/login").into_response();
    };

    let shop = shop_name(&conn);
    let body = html! {
        h1 { (ui::t(lang, "settings.title")) }

        div.card.card-pad style="margin-block-end:1rem" {
            h2 { (ui::t(lang, "settings.lang")) }
            div style="display:flex;gap:.5rem;flex-wrap:wrap" {
                @for l in Lang::ALL {
                    a.btn.btn-ghost href=(format!("/lang/{}", l.code())) { (l.label()) }
                }
            }
        }

        div.card.card-pad style="margin-block-end:1rem" {
            h2 { (ui::t(lang, "setup.shop")) }
            form method="post" action="/settings" {
                input type="text" name="shop_name" value=(shop);
                button.btn.btn-primary type="submit" style="margin-block-start:.5rem" {
                    (ui::t(lang, "common.save"))
                }
            }
        }

        div.card.card-pad style="margin-block-end:1rem" {
            h2 { "Qui a fait quoi" }
            p.muted { "Voir toutes les actions : ventes, modifications, encaissements." }
            a.btn.btn-ghost href="/journal" { "Ouvrir le journal" }
            a.btn.btn-ghost href="/session" { "Caisse / Z" }
        }

        div.card.card-pad {
            h2 { "Protection" }
            p.muted { (ui::t(lang, "settings.security")) }
        }
    };

    page_response(ui::page(
        &chrome(&conn, &headers, &user, Nav::Settings, 0, ui::t(lang, "settings.title")),
        body,
    ))
}

#[derive(Deserialize)]
struct SettingsForm {
    shop_name: Option<String>,
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
    let result = (|| -> Result<()> {
        let name = form.shop_name.unwrap_or_default();
        storage::set_setting(&conn, "shop_name", name.trim())?;
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
        Ok(()) => Redirect::to("/settings").into_response(),
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
