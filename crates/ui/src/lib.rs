//! UI layer — **all markup is Rust** (`maud!`), there are no `.html` files
//! (ADR-014). Components are plain functions returning `Markup`.
//!
//! Copy rules (DESIGN-003): no jargon, one primary action per screen,
//! every message says what happened and what to do next.

#![forbid(unsafe_code)]

use domain::format_money;
use maud::{html, Markup, PreEscaped, DOCTYPE};

pub const APP_NAME: &str = "OneClick";
/// Rendered only through the Amiri/Noto Naskh stack (DESIGN-002 §6).
pub const BISMILLAH: &str = "بِسْمِ اللّٰهِ الرَّحْمٰنِ الرَّحِيمِ";

pub const CSS: &str = include_str!("../assets/store.css");

/// Remembers a dragged sidebar width. Progressive enhancement only.
const SIDEBAR_JS: &str = r#"
(function () {
  try {
    var shell = document.querySelector('.shell');
    var bar = document.querySelector('.sidebar');
    if (!shell || !bar) return;
    var saved = localStorage.getItem('oneclick.side');
    if (saved && !shell.classList.contains('collapsed')) {
      bar.style.setProperty('inline-size', saved + 'px');
    }
    bar.addEventListener('pointerup', function () {
      if (shell.classList.contains('collapsed')) return;
      var px = Math.round(bar.getBoundingClientRect().width);
      if (px > 90 && px < 440) localStorage.setItem('oneclick.side', px);
    });
  } catch (e) {}
})();
"#;

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
    Sales,
    Products,
    Carnet,
    Settings,
}

impl Nav {
    fn href(self) -> &'static str {
        match self {
            Nav::Sell => "/",
            Nav::Sales => "/sales",
            Nav::Products => "/products",
            Nav::Carnet => "/carnet",
            Nav::Settings => "/settings",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Nav::Sell => "nav.sell",
            Nav::Sales => "nav.sales",
            Nav::Products => "nav.products",
            Nav::Carnet => "nav.carnet",
            Nav::Settings => "nav.settings",
        }
    }

}

// ---------------------------------------------------------------------------
// Translations — French is the reference; Arabic/English fall back to it.
// ---------------------------------------------------------------------------

fn fr(key: &str) -> &'static str {
    match key {
        "nav.sell" => "Vendre",
        "nav.products" => "Produits",
        "nav.sales" => "Ventes",
        "nav.quick_sell" => "Vente rapide",
        "nav.search" => "Chercher ou scanner…",
        "nav.group_sell" => "Caisse",
        "nav.collapse" => "Réduire le menu",
        "nav.group_manage" => "Gestion",
        "nav.carnet" => "Carnet",
        "nav.settings" => "Réglages",
        "nav.logout" => "Sortir",
        "nav.menu" => "Menu",
        "nav.session" => "Caisse / Z",
        "nav.journal" => "Journal",
        "common.save" => "Enregistrer",
        "common.close" => "Fermer",
        "common.done" => "Terminé",
        "toast.sale" => "Vente enregistrée",
        "toast.credit" => "Vente à crédit enregistrée — ajoutée au carnet",
        "toast.saved" => "Enregistré",
        "toast.created" => "Produit ajouté",
        "toast.archived" => "Produit archivé",
        "toast.collected" => "Paiement encaissé",
        "toast.debt" => "Dette ajoutée au carnet",
        "toast.written_off" => "Dette passée en perte",
        "toast.opened" => "Caisse ouverte",
        "toast.closed" => "Caisse fermée — Z enregistré",
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
        "pos.articles" => "articles",
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
        "login.title" => "Bonjour",
        "theme.short" => "Jour",
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
        "nav.sales" => "المبيعات",
        "nav.quick_sell" => "بيع سريع",
        "nav.search" => "ابحث أو امسح…",
        "nav.group_sell" => "الصندوق",
        "nav.collapse" => "تصغير القائمة",
        "nav.group_manage" => "الإدارة",
        "nav.carnet" => "الديون",
        "nav.settings" => "الإعدادات",
        "nav.logout" => "خروج",
        "common.save" => "حفظ",
        "common.close" => "إغلاق",
        "common.done" => "تم",
        "toast.sale" => "تم تسجيل البيع",
        "toast.credit" => "بيع بالأجل — أضيف إلى الكرنيه",
        "toast.saved" => "تم الحفظ",
        "toast.created" => "تمت إضافة المنتج",
        "toast.archived" => "تم أرشفة المنتج",
        "toast.collected" => "تم استلام الدفعة",
        "toast.debt" => "أضيفت دين إلى الكرنيه",
        "toast.written_off" => "شُطب الدين كخسارة",
        "toast.opened" => "تم فتح الصندوق",
        "toast.closed" => "تم إغلاق الصندوق — Z محفوظ",
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
        "pos.articles" => "قطعة",
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
        "login.title" => "مرحباً",
        "theme.short" => "نهار",
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
        "nav.sales" => "Sales",
        "nav.quick_sell" => "Quick sale",
        "nav.search" => "Search or scan…",
        "nav.group_sell" => "Till",
        "nav.collapse" => "Collapse menu",
        "nav.group_manage" => "Manage",
        "nav.carnet" => "Debts",
        "nav.settings" => "Settings",
        "nav.logout" => "Sign out",
        "common.save" => "Save",
        "common.close" => "Close",
        "common.done" => "Done",
        "toast.sale" => "Sale saved",
        "toast.credit" => "Credit sale saved — added to the debt book",
        "toast.saved" => "Saved",
        "toast.created" => "Product added",
        "toast.archived" => "Product archived",
        "toast.collected" => "Payment collected",
        "toast.debt" => "Debt added to the book",
        "toast.written_off" => "Debt written off",
        "toast.opened" => "Drawer opened",
        "toast.closed" => "Drawer closed — Z saved",
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
        "pos.articles" => "items",
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
        "login.title" => "Welcome",
        "theme.short" => "Day",
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
    /// Sidebar reduced to its icon rail.
    pub collapsed: bool,
}


impl Nav {
    /// Line icons, drawn inline so the collapsed rail still says what each
    /// destination is without a single tooltip.
    pub fn icon(self) -> Markup {
        let paths: &[&str] = match self {
            Nav::Sell => &[
                "M4 8h16l-1.6 9.2a2 2 0 0 1-2 1.8H7.6a2 2 0 0 1-2-1.8L4 8Z",
                "M9 8V6a3 3 0 0 1 6 0v2",
            ],
            Nav::Sales => &[
                "M8 6h13", "M8 12h13", "M8 18h13",
                "M3.5 6h.01", "M3.5 12h.01", "M3.5 18h.01",
            ],
            Nav::Carnet => &[
                "M3 7.5A2.5 2.5 0 0 1 5.5 5H19a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5.5A2.5 2.5 0 0 1 3 16.5v-9Z",
                "M3 9h18", "M16.5 14.5h.01",
            ],
            Nav::Products => &[
                "M12 3.2 20 7.6v8.8L12 20.8 4 16.4V7.6L12 3.2Z",
                "M4 7.6 12 12l8-4.4", "M12 12v8.8",
            ],
            Nav::Settings => &[
                "M4 7h9", "M19 7h1", "M4 17h5", "M15 17h5",
                "M15.5 7a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z",
                "M11.5 17a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z",
            ],
        };

        html! {
            svg.nav-icon viewBox="0 0 24 24" fill="none" stroke="currentColor"
                "stroke-width"=(1.7) "stroke-linecap"="round" "stroke-linejoin"="round"
                aria-hidden="true" {
                @for d in paths {
                    path d=(d);
                }
            }
        }
    }
}

/// Extra glyphs for the sidebar destinations that are not in `Nav`.
pub fn icon_session() -> Markup {
    html! {
        svg.nav-icon viewBox="0 0 24 24" fill="none" stroke="currentColor"
            "stroke-width"=(1.7) "stroke-linecap"="round" "stroke-linejoin"="round"
            aria-hidden="true" {
            path d="M6.5 3h11A1.5 1.5 0 0 1 19 4.5v15A1.5 1.5 0 0 1 17.5 21h-11A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3Z";
            path d="M8.5 7h7";
            path d="M9 11h.01"; path d="M12 11h.01"; path d="M15 11h.01";
            path d="M9 14.5h.01"; path d="M12 14.5h.01"; path d="M15 14.5h.01";
            path d="M9 18h6";
        }
    }
}

pub fn icon_journal() -> Markup {
    html! {
        svg.nav-icon viewBox="0 0 24 24" fill="none" stroke="currentColor"
            "stroke-width"=(1.7) "stroke-linecap"="round" "stroke-linejoin"="round"
            aria-hidden="true" {
            circle cx="12" cy="12" r="8.5";
            path d="M12 7.5V12l3 1.8";
        }
    }
}

pub fn icon_panel() -> Markup {
    html! {
        svg.nav-icon viewBox="0 0 24 24" fill="none" stroke="currentColor"
            "stroke-width"=(1.7) "stroke-linecap"="round" "stroke-linejoin"="round"
            aria-hidden="true" {
            rect x="3.5" y="4.5" width="17" height="15" rx="2.5";
            path d="M9.5 4.5v15";
        }
    }
}

pub fn page(chrome: &Chrome, body: Markup) -> Markup {
    let lang = chrome.lang;
    let shop = if chrome.shop.is_empty() {
        APP_NAME.to_string()
    } else {
        chrome.shop.clone()
    };
    let active = chrome.active;

    let item = |nav: Nav, extra: Option<Markup>| {
        let is_active = active == Some(nav);
        html! {
            a href=(nav.href())
              class=(if is_active { "nav-item active" } else { "nav-item" })
              aria-current=(if is_active { "page" } else { "false" })
              title=(t(lang, nav.key())) {
                (nav.icon())
                span.nav-label { (t(lang, nav.key())) }
                @if nav == Nav::Sell && chrome.cart_count > 0 {
                    span.count { (chrome.cart_count) }
                }
                @if let Some(extra) = extra { (extra) }
            }
        }
    };

    let side_class = if chrome.collapsed { "shell collapsed" } else { "shell" };

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
                div class=(side_class) {
                    aside.sidebar {
                        div.side-top {
                            a.brand href="/" title=(shop) {
                                div.brand-tile { "1C" }
                                div.brand-text {
                                    div.brand-name { (shop) }
                                    @if let Some(name) = &chrome.user {
                                        div.brand-sub { (name.as_str()) }
                                    }
                                }
                            }
                            /* collapse / expand — the rail keeps every icon */
                            a.side-toggle href="/sidebar" title=(t(lang, "nav.collapse"))
                              aria-label=(t(lang, "nav.collapse")) {
                                (icon_panel())
                            }
                        }

                        /* Quick sell: always the first thing in reach */
                        a.btn.btn-primary.quick-sell href="/" title=(t(lang, "nav.quick_sell")) {
                            span.plus aria-hidden="true" { "+" }
                            span.qs-label { (t(lang, "nav.quick_sell")) }
                        }

                        form.side-search method="get" action="/" role="search" {
                            input type="search" name="q" dir="ltr"
                                  placeholder=(t(lang, "nav.search"))
                                  aria-label=(t(lang, "nav.search"))
                                  autofocus[active == Some(Nav::Sell)];
                        }

                        nav.side-nav aria-label=(t(lang, "nav.group_sell")) {
                            div.nav-group {
                                div.nav-title { (t(lang, "nav.group_sell")) }
                                (item(Nav::Sell, None))
                                (item(Nav::Sales, None))
                                (item(Nav::Carnet, None))
                                (item(Nav::Products, None))
                            }
                            div.nav-group {
                                div.nav-title { (t(lang, "nav.group_manage")) }
                                a.nav-item href="/session" title=(t(lang, "nav.session")) {
                                    (icon_session())
                                    span.nav-label { (t(lang, "nav.session")) }
                                }
                                a.nav-item href="/journal" title=(t(lang, "nav.journal")) {
                                    (icon_journal())
                                    span.nav-label { (t(lang, "nav.journal")) }
                                }
                                (item(Nav::Settings, None))
                            }
                        }

                        div.side-foot {
                            a.icon-btn href=(format!("/lang/{}", next_lang(lang).code()))
                                title=(lang.label()) { (lang.code().to_uppercase()) }

                            a.icon-btn href="/theme" title=(t(lang, "theme.toggle")) {
                                @if chrome.theme == Theme::Dark {
                                    span.icon-sun {}
                                } @else {
                                    span.icon-moon {}
                                }
                            }

                            details.menu.up {
                                summary.icon-btn aria-label=(t(lang, "nav.menu")) { "⋯" }
                                div.menu-panel {
                                    @if let Some(name) = &chrome.user {
                                        div.who { (name.as_str()) }
                                    }
                                    a href="/session" { (t(lang, "nav.session")) }
                                    a href="/journal" { (t(lang, "nav.journal")) }
                                    a href="/settings" { (t(lang, "nav.settings")) }
                                    @if chrome.user.is_some() {
                                        form method="post" action="/logout" {
                                            button type="submit" { (t(lang, "nav.logout")) }
                                        }
                                    }
                                    div.menu-version { (APP_NAME) " " (version()) }
                                }
                            }
                        }
                    }

                    div.main-col {
                        /* Bismillah sits under the header, never over the content. */
                        div.bismillah-bar {
                            div.bismillah dir="rtl" lang="ar" { (BISMILLAH) }
                        }
                        main.wrap { (body) }
                    }
                }

                /* Progressive enhancement only: remembers the dragged width.
                   Without JavaScript the sidebar is still resizable per page. */
                script { (PreEscaped(SIDEBAR_JS)) }
            }
        }
    }
}

fn next_lang(lang: Lang) -> Lang {
    match lang {
        Lang::Fr => Lang::Ar,
        Lang::Ar => Lang::En,
        Lang::En => Lang::Fr,
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
            body.auth-shell {
                div.auth { (body) }
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

/// Non-blocking confirmation: appears bottom-centre, fades out on its own.
/// Bismillah for the sign-in card — sits directly under the brand row.
pub fn auth_bismillah() -> Markup {
    html! {
        div.bismillah-bar {
            div.bismillah dir="rtl" lang="ar" { (BISMILLAH) }
        }
    }
}

/// Brand row for the sign-in / first-run card: tile + shop name.
/// Small build stamp: which version is running (bumped on every commit).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn auth_brand(shop: &str) -> Markup {
    let label = if shop.trim().is_empty() { APP_NAME } else { shop };
    html! {
        div.auth-brand {
            div.brand-tile { "1C" }
            div {
                div.brand-name { (label) }
                div.brand-sub { (APP_NAME) }
            }
        }
    }
}

pub fn toast(text: &str, kind: &str) -> Markup {
    html! {
        div class=(format!("toast {kind}")) role="status" aria-live="polite" {
            span.toast-dot aria-hidden="true" {}
            span.toast-text { (text) }
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
    let initial: String = product
        .name
        .split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .collect();
    let low = product.stock <= 0;
    html! {
        form method="post" action="/cart/add" {
            input type="hidden" name="product_id" value=(product.id);
            button.product type="submit" {
                div.thumb { (initial) }
                div.name { (product.name) }
                div.price { (money(product.price)) }
                div class=(if low { "stock low" } else { "stock" }) {
                    @if low { "Rupture" } @else { (product.stock) " en stock" }
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
            div.title { (line.name) }
            div.line-total.money { (format_money(line.line_total)) }

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
                    input type="text" name="unit_price" dir="ltr"
                          value=(format_money(line.unit_price)) aria-label=(t(lang, "common.price"));
                    button type="submit" aria-label=(t(lang, "common.ok")) { "✓" }
                }
            }
        }
    }
}
