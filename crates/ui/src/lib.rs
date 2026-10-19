//! UI layer — **all markup is Rust** (`maud!`), there are no `.html` files
//! (ADR-014). Components are plain functions returning `Markup`.
//!
//! Copy rules (DESIGN-003): no jargon, one primary action per screen,
//! every message says what happened and what to do next.

#![forbid(unsafe_code)]

use domain::format_money;
use maud::{html, Markup, DOCTYPE};

pub const APP_NAME: &str = "OneClick";
/// Rendered only through the Amiri/Noto Naskh stack (DESIGN-002 §6).
pub const BISMILLAH: &str = "بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ";

pub const CSS: &str = include_str!("../assets/store.css");

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Fr,
    Ar,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::Ar => "ar",
            Lang::En => "en",
        }
    }

    pub fn dir(self) -> &'static str {
        if self == Lang::Ar {
            "rtl"
        } else {
            "ltr"
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::Ar => "العربية",
            Lang::En => "English",
        }
    }

    pub fn from_code(code: &str) -> Lang {
        match code.trim().to_ascii_lowercase().as_str() {
            "ar" | "ar-tn" => Lang::Ar,
            "en" => Lang::En,
            _ => Lang::Fr,
        }
    }

    pub const ALL: [Lang; 3] = [Lang::Fr, Lang::Ar, Lang::En];
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn toggled(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    pub fn from_str(value: &str) -> Theme {
        if value.eq_ignore_ascii_case("dark") {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Sell,
    Products,
    Carnet,
    Settings,
}

impl Nav {
    fn href(self) -> &'static str {
        match self {
            Nav::Sell => "/",
            Nav::Products => "/products",
            Nav::Carnet => "/carnet",
            Nav::Settings => "/settings",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Nav::Sell => "nav.sell",
            Nav::Products => "nav.products",
            Nav::Carnet => "nav.carnet",
            Nav::Settings => "nav.settings",
        }
    }

    const ALL: [Nav; 4] = [Nav::Sell, Nav::Products, Nav::Carnet, Nav::Settings];
}

// ---------------------------------------------------------------------------
// Translations — French is the reference; Arabic/English fall back to it.
// ---------------------------------------------------------------------------

fn fr(key: &str) -> &'static str {
    match key {
        "nav.sell" => "Vendre",
        "nav.products" => "Produits",
        "nav.carnet" => "Carnet",
        "nav.settings" => "Réglages",
        "nav.logout" => "Sortir",
        "common.save" => "Enregistrer",
        "common.cancel" => "Annuler",
        "common.total" => "Total",
        "common.name" => "Nom",
        "common.price" => "Prix",
        "common.cost" => "Prix d'achat",
        "common.stock" => "En stock",
        "common.delete" => "Retirer",
        "common.back" => "Retour",
        "common.ok" => "OK",
        "pos.search" => "Scannez un produit ou tapez son nom",
        "pos.charge" => "Encaisser",
        "pos.cash" => "Espèces",
        "pos.card" => "Carte",
        "pos.credit" => "À crédit",
        "pos.debtor" => "Nom du client",
        "pos.phone" => "Téléphone (optionnel)",
        "pos.empty_cart" => "Le panier est vide. Scannez un produit pour commencer.",
        "pos.new_product" => "Nouveau produit",
        "pos.today" => "Aujourd'hui",
        "pos.tickets" => "tickets",
        "products.title" => "Produits",
        "products.new" => "Ajouter un produit",
        "products.barcode" => "Code-barres",
        "products.qty" => "Quantité de départ",
        "products.create" => "Créer le produit",
        "products.empty" => "Aucun produit pour l'instant.",
        "carnet.title" => "Carnet — qui me doit de l'argent",
        "carnet.debtor" => "Client",
        "carnet.opened" => "Depuis",
        "carnet.balance" => "Reste à payer",
        "carnet.collect" => "Encaisser",
        "carnet.empty" => "Personne ne vous doit d'argent.",
        "carnet.paid" => "Encaissé",
        "login.title" => "Entrer votre mot de passe",
        "login.user" => "Votre nom",
        "login.password" => "Mot de passe",
        "login.enter" => "Entrer",
        "login.error" => "Mot de passe incorrect. Réessayez.",
        "setup.title" => "Bienvenue",
        "setup.intro" => "Trois questions et vous pouvez vendre.",
        "setup.shop" => "Nom du magasin",
        "setup.password" => "Choisissez un mot de passe (pour protéger vos données)",
        "setup.start" => "Commencer",
        "setup.vat" => "Facturez-vous la TVA ?",
        "setup.vat_no" => "Non",
        "setup.vat_single" => "Oui, toujours le même taux",
        "setup.vat_per_product" => "Oui, ça dépend du produit",
        "done.sale" => "Vente enregistrée",
        "done.credit" => "Vente à crédit enregistrée — elle est dans le carnet",
        "theme.toggle" => "Jour / Nuit",
        "settings.title" => "Réglages",
        "settings.lang" => "Langue",
        "settings.security" => "Vos données sont protégées par votre mot de passe. Sauvegarde automatique chaque soir.",
        _ => "?",
    }
}

fn ar(key: &str) -> Option<&'static str> {
    Some(match key {
        "nav.sell" => "بيع",
        "nav.products" => "المنتجات",
        "nav.carnet" => "الديون",
        "nav.settings" => "الإعدادات",
        "nav.logout" => "خروج",
        "common.save" => "حفظ",
        "common.cancel" => "إلغاء",
        "common.total" => "المجموع",
        "common.name" => "الاسم",
        "common.price" => "السعر",
        "common.cost" => "سعر الشراء",
        "common.stock" => "في المخزون",
        "common.delete" => "حذف",
        "common.back" => "رجوع",
        "common.ok" => "حسناً",
        "pos.search" => "امسح منتجاً أو اكتب اسمه",
        "pos.charge" => "تسجيل البيع",
        "pos.cash" => "نقداً",
        "pos.card" => "بطاقة",
        "pos.credit" => "بالدين",
        "pos.debtor" => "اسم الحريف",
        "pos.phone" => "الهاتف (اختياري)",
        "pos.empty_cart" => "السلة فارغة. امسح منتجاً للبدء.",
        "pos.new_product" => "منتج جديد",
        "pos.today" => "اليوم",
        "pos.tickets" => "فاتورة",
        "products.title" => "المنتجات",
        "products.new" => "إضافة منتج",
        "products.barcode" => "الباركود",
        "products.qty" => "الكمية الحالية",
        "products.create" => "إنشاء المنتج",
        "products.empty" => "لا توجد منتجات بعد.",
        "carnet.title" => "الديون — من عليه فلوس",
        "carnet.debtor" => "الحريف",
        "carnet.opened" => "منذ",
        "carnet.balance" => "الباقي",
        "carnet.collect" => "تحصيل",
        "carnet.empty" => "لا أحد مدين لك.",
        "carnet.paid" => "تم التحصيل",
        "login.title" => "أدخل كلمة السر",
        "login.user" => "اسمك",
        "login.password" => "كلمة السر",
        "login.enter" => "دخول",
        "login.error" => "كلمة السر غير صحيحة. حاول مرة أخرى.",
        "setup.title" => "مرحباً",
        "setup.intro" => "ثلاثة أسئلة ويمكنك البيع.",
        "setup.shop" => "اسم المحل",
        "setup.password" => "اختر كلمة سر (لحماية بياناتك)",
        "setup.start" => "ابدأ",
        "setup.vat" => "هل تصرّح بالتوريد على القيمة المضافة؟",
        "setup.vat_no" => "لا",
        "setup.vat_single" => "نعم، دائماً نفس النسبة",
        "setup.vat_per_product" => "نعم، حسب المنتج",
        "done.sale" => "تم تسجيل البيع",
        "done.credit" => "تم تسجيل البيع بالدين — موجود في الديون",
        "theme.toggle" => "نهار / ليل",
        "settings.title" => "الإعدادات",
        "settings.lang" => "اللغة",
        "settings.security" => "بياناتك محمية بكلمة السر. نسخ احتياطي كل ليلة.",
        _ => return None,
    })
}

fn en(key: &str) -> Option<&'static str> {
    Some(match key {
        "nav.sell" => "Sell",
        "nav.products" => "Products",
        "nav.carnet" => "Debts",
        "nav.settings" => "Settings",
        "nav.logout" => "Sign out",
        "common.save" => "Save",
        "common.cancel" => "Cancel",
        "common.total" => "Total",
        "common.name" => "Name",
        "common.price" => "Price",
        "common.cost" => "Cost",
        "common.stock" => "In stock",
        "common.delete" => "Remove",
        "common.back" => "Back",
        "common.ok" => "OK",
        "pos.search" => "Scan a product or type its name",
        "pos.charge" => "Charge",
        "pos.cash" => "Cash",
        "pos.card" => "Card",
        "pos.credit" => "On credit",
        "pos.debtor" => "Customer name",
        "pos.phone" => "Phone (optional)",
        "pos.empty_cart" => "Cart is empty. Scan a product to start.",
        "pos.new_product" => "New product",
        "pos.today" => "Today",
        "pos.tickets" => "tickets",
        "products.title" => "Products",
        "products.new" => "Add product",
        "products.barcode" => "Barcode",
        "products.qty" => "Opening quantity",
        "products.create" => "Create product",
        "products.empty" => "No products yet.",
        "carnet.title" => "Debts — who owes me money",
        "carnet.debtor" => "Customer",
        "carnet.opened" => "Since",
        "carnet.balance" => "Balance",
        "carnet.collect" => "Collect",
        "carnet.empty" => "Nobody owes you money.",
        "carnet.paid" => "Collected",
        "login.title" => "Enter your password",
        "login.user" => "Your name",
        "login.password" => "Password",
        "login.enter" => "Enter",
        "login.error" => "Wrong password. Try again.",
        "setup.title" => "Welcome",
        "setup.intro" => "Three questions and you can sell.",
        "setup.shop" => "Shop name",
        "setup.password" => "Choose a password (to protect your data)",
        "setup.start" => "Start",
        "setup.vat" => "Do you charge VAT?",
        "setup.vat_no" => "No",
        "setup.vat_single" => "Yes, always the same rate",
        "setup.vat_per_product" => "Yes, it depends on the product",
        "done.sale" => "Sale saved",
        "done.credit" => "Credit sale saved — it is in the debts list",
        "theme.toggle" => "Day / Night",
        "settings.title" => "Settings",
        "settings.lang" => "Language",
        "settings.security" => "Your data is protected by your password. Automatic backup every evening.",
        _ => return None,
    })
}

pub fn t(lang: Lang, key: &str) -> &'static str {
    match lang {
        Lang::Fr => fr(key),
        Lang::Ar => ar(key).unwrap_or_else(|| fr(key)),
        Lang::En => en(key).unwrap_or_else(|| fr(key)),
    }
}

// ---------------------------------------------------------------------------
// Page shell
// ---------------------------------------------------------------------------

pub struct Chrome {
    pub lang: Lang,
    pub theme: Theme,
    pub shop: String,
    pub user: Option<String>,
    pub active: Option<Nav>,
    pub cart_count: usize,
    pub title: String,
}

pub fn page(chrome: &Chrome, body: Markup) -> Markup {
    let lang = chrome.lang;
    html! {
        (DOCTYPE)
        html lang=(lang.code()) dir=(lang.dir()) data-theme=(chrome.theme.as_str()) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { (chrome.title) " — " (APP_NAME) }
                link rel="stylesheet" href="/assets/store.css";
            }
            body {
                div.bismillah dir="rtl" lang="ar" { (BISMILLAH) }

                header.header {
                    span.brand { (APP_NAME) }
                    @if !chrome.shop.is_empty() {
                        span.shop { (chrome.shop) }
                    }
                    span.spacer {}
                    a.btn.btn-ghost href="/theme" title=(t(lang, "theme.toggle")) {
                        @if chrome.theme == Theme::Dark { "☀" } @else { "☾" }
                    }
                    @if let Some(name) = &chrome.user {
                        span.who { (name.as_str()) }
                        form method="post" action="/logout" {
                            button.btn.btn-ghost type="submit" { (t(lang, "nav.logout")) }
                        }
                    }
                }

                @if let Some(active) = chrome.active {
                    nav.nav {
                        @for item in Nav::ALL {
                            a href=(item.href()) class=(if item == active { "active" } else { "" }) {
                                (t(lang, item.key()))
                                @if item == Nav::Sell && chrome.cart_count > 0 {
                                    " · " (chrome.cart_count)
                                }
                            }
                        }
                    }
                }

                main.wrap { (body) }
            }
        }
    }
}

/// Minimal shell for login / setup (no navigation, nothing to click wrongly).
pub fn auth_page(lang: Lang, theme: Theme, title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(lang.code()) dir=(lang.dir()) data-theme=(theme.as_str()) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="light dark";
                title { (title) " — " (APP_NAME) }
                link rel="stylesheet" href="/assets/store.css";
            }
            body {
                div.bismillah dir="rtl" lang="ar" { (BISMILLAH) }
                div.auth.card.card-pad { (body) }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Small components
// ---------------------------------------------------------------------------

/// Money is always 3 decimals and stays LTR inside Arabic (DESIGN-002 §5).
pub fn money(millimes: i64) -> Markup {
    html! { span.money dir="ltr" { (format_money(millimes)) } }
}

pub fn money_tnd(millimes: i64) -> Markup {
    html! { span.money dir="ltr" { (format_money(millimes)) " TND" } }
}

pub fn banner(kind: &str, title: &str, text: &str) -> Markup {
    html! {
        div class=(format!("banner {kind}")) {
            strong { (title) }
            span { (text) }
        }
    }
}

pub fn empty_state(text: &str) -> Markup {
    html! { p.cart-empty { (text) } }
}

pub struct ProductView<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub price: i64,
    pub stock: i64,
}

/// The whole card is the button: one tap adds the product (DESIGN-002 R2).
pub fn product_card(product: &ProductView<'_>) -> Markup {
    let initial = product
        .name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string());
    let low = product.stock <= 0;
    html! {
        form method="post" action="/cart/add" {
            input type="hidden" name="product_id" value=(product.id);
            button.product type="submit" {
                div.thumb { (initial) }
                div.name { (product.name) }
                div.price { (money(product.price)) }
                div class=(if low { "stock low" } else { "stock" }) {
                    @if low { "Stock : 0" } @else { "Stock : " (product.stock) }
                }
            }
        }
    }
}

pub struct DraftView<'a> {
    pub product_id: &'a str,
    pub name: &'a str,
    pub qty: i64,
    pub unit_price: i64,
    pub line_total: i64,
}

pub fn draft_row(line: &DraftView<'_>, lang: Lang) -> Markup {
    html! {
        div.cart-line {
            div {
                div.title { (line.name) }
                form method="post" action="/cart/qty" {
                    input type="hidden" name="product_id" value=(line.product_id);
                    div.stepper {
                        button type="submit" name="qty" value=(line.qty - 1) aria-label="-" { "−" }
                        span.qty { (line.qty) }
                        button type="submit" name="qty" value=(line.qty + 1) aria-label="+" { "+" }
                    }
                }
                form method="post" action="/cart/price" {
                    input type="hidden" name="product_id" value=(line.product_id);
                    div.stepper {
                        input type="text" name="unit_price" dir="ltr" value=(format_money(line.unit_price));
                        button type="submit" { (t(lang, "common.ok")) }
                    }
                }
            }
            div.money-cell {
                div.money { (format_money(line.line_total)) }
            }
        }
    }
}
