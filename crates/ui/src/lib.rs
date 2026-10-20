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
    var grip = document.querySelector('.side-grip');
    if (!shell || !bar) return;

    var saved = parseInt(localStorage.getItem('oneclick.side'), 10);
    if (saved >= 90 && saved <= 440) shell.style.setProperty('--side-w', saved + 'px');

    if (!grip) return;
    var rtl = getComputedStyle(shell).direction === 'rtl';
    var dragging = false;

    function widthAt(clientX) {
      var r = bar.getBoundingClientRect();
      return rtl ? r.right - clientX : clientX - r.left;
    }

    grip.addEventListener('pointerdown', function (e) {
      if (shell.classList.contains('collapsed')) return;
      dragging = true;
      grip.setPointerCapture(e.pointerId);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      e.preventDefault();
    });

    grip.addEventListener('pointermove', function (e) {
      if (!dragging) return;
      var w = Math.max(90, Math.min(440, widthAt(e.clientX)));
      shell.style.setProperty('--side-w', w + 'px');
    });

    function stop() {
      if (!dragging) return;
      dragging = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      var w = Math.round(bar.getBoundingClientRect().width);
      try { localStorage.setItem('oneclick.side', w); } catch (err) {}
    }

    grip.addEventListener('pointerup', stop);
    grip.addEventListener('pointercancel', stop);
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
    Expenses,
    Settings,
}

impl Nav {
    fn href(self) -> &'static str {
        match self {
            Nav::Sell => "/",
            Nav::Sales => "/sales",
            Nav::Products => "/products",
            Nav::Carnet => "/carnet",
            Nav::Expenses => "/expenses",
            Nav::Settings => "/settings",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Nav::Sell => "nav.sell",
            Nav::Sales => "nav.sales",
            Nav::Products => "nav.products",
            Nav::Carnet => "nav.carnet",
            Nav::Expenses => "nav.expenses",
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
        "nav.expenses" => "Charges",
        "scan.placeholder" => "Scanner ou taper un code",
        "scan.add" => "Ajouter",
        "scan.hint" => "Produit inconnu ? On le crée en deux questions, sans quitter la vente.",
        "quick.title" => "Produit inconnu — on l'ajoute ?",
        "quick.intro" => "Ce code n'est pas encore dans le système. C'est normal : vous complétez quand vous avez le temps.",
        "quick.name" => "Nom du produit (facultatif)",
        "quick.name_hint" => "ex. Chargeur USB-C",
        "quick.price" => "Prix de vente",
        "quick.cost" => "Prix d'achat (facultatif)",
        "quick.qty" => "Quantité",
        "quick.add" => "Ajouter à la vente",
        "quick.note" => "Le prix suffit : tout le reste peut rester vide et être complété plus tard.",
        "quick.later" => "Plus tard",
        "products.fills_itself" => "Votre stock se remplit tout seul : chaque produit scanné s'ajoute ici. Complétez les fiches quand vous avez un moment.",
        "products.no_price" => "prix à définir",
        "product.never_sold" => "Pas encore vendu. Son prix se précisera à la première vente.",
        "product.sold" => "Quantité vendue",
        "product.revenue" => "Total vendu",
        "product.last_sold" => "Dernière vente",
        "product.not_counted" => "Stock non compté — vous vendez sans inventaire.",
        "expense.title" => "Charges du magasin",
        "expense.intro" => "Loyer, eau, électricité, entretien : ce qui sort chaque mois.",
        "expense.per_month" => "Par mois",
        "expense.per_day" => "Par jour",
        "expense.per_day_hint" => "Ce qu'il faut encaisser chaque jour pour les couvrir.",
        "expense.add" => "Ajouter une charge",
        "expense.label" => "Nom",
        "expense.label_hint" => "ex. Loyer du local",
        "expense.kind" => "Type",
        "expense.kind.rent" => "Loyer",
        "expense.kind.power" => "Électricité",
        "expense.kind.water" => "Eau",
        "expense.kind.maintenance" => "Entretien",
        "expense.kind.telecom" => "Téléphone / Internet",
        "expense.kind.other" => "Autre",
        "expense.amount" => "Montant",
        "expense.period" => "Périodicité",
        "expense.period.month" => "Par mois",
        "expense.period.quarter" => "Par trimestre",
        "expense.period.year" => "Par an",
        "expense.save" => "Enregistrer la charge",
        "expense.empty" => "Aucune charge enregistrée. Ajoutez le loyer pour connaître votre coût réel.",
        "expense.stop" => "Arrêter",
        "expense.stopped" => "Arrêtée",
        "toast.added" => "Ajouté à la vente",
        "toast.expense" => "Charge ajoutée",
        "toast.expense_updated" => "Charge modifiée",
        "toast.expense_stopped" => "Charge arrêtée",
        "common.edit" => "Modifier",
        "sales.one" => "Vente",
        "sales.back" => "Retour aux ventes",
        "sales.item" => "Article",
        "sales.qty" => "Qté",
        "sales.unit" => "Prix unitaire",
        "sales.seller" => "Qui a vendu",
        "sales.payment" => "Paiement",
        "sales.customer" => "Client",
        "sales.status" => "État",
        "sales.closed" => "Encaissée",
        "sales.cancelled" => "Annulée",
        "sales.edit" => "Modifier la vente",
        "sales.edit_title" => "Modifier la vente",
        "sales.cancel" => "Annuler la vente",
        "sales.cancel_hint" => "Annuler remet les articles en stock. La vente reste dans l'historique.",
        "sales.cancel_confirm" => "Annuler cette vente et remettre le stock ?",
        "sales.qty_hint" => "Mettez 0 pour retirer un article. Le prix peut être corrigé.",
        "sales.reason" => "Raison (facultatif)",
        "sales.ticket" => "Ticket",
        "sales.save" => "Enregistrer les changements",
        "toast.edited" => "Vente modifiée",
        "toast.cancelled" => "Vente annulée — stock remis",
        "nav.quick_sell" => "Vente rapide",
        "nav.search" => "Chercher ou scanner…",
        "nav.group_sell" => "Caisse",
        "nav.collapse" => "Réduire le menu",
        "nav.expand" => "Ouvrir le menu",
        "nav.resize" => "Redimensionner le menu",
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
        "pos.new_sale" => "Nouvelle vente",
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
        "settings.security" => "Vos données sont un seul fichier sur cet ordinateur. La protection par mot de passe existe ; le chiffrement du fichier n'est pas encore activé.",
        "settings.backup" => "Sauvegarde",
        "settings.backup_now" => "Sauvegarder maintenant",
        "settings.backup_none" => "Aucune sauvegarde pour le moment. Faites-en une avant de fermer le magasin.",
        "settings.backup_last" => "Dernière sauvegarde :",
        "settings.backup_count" => "Sauvegardes gardées :",
        "settings.backup_where" => "Elles vont dans le dossier data\\backups, à côté de la caisse. Copiez-le sur une clé USB de temps en temps.",
        "toast.backup" => "Sauvegarde faite",
        _ => "?",
    }
}

fn ar(key: &str) -> Option<&'static str> {
    Some(match key {
        "nav.sell" => "بيع",
        "nav.products" => "المنتجات",
        "nav.sales" => "المبيعات",
        "nav.expenses" => "المصاريف",
        "nav.session" => "الصندوق / Z",
        "nav.journal" => "السجل",
        "nav.menu" => "القائمة",
        "scan.placeholder" => "امسح أو اكتب الرمز",
        "scan.add" => "أضف",
        "scan.hint" => "منتج غير معروف؟ ننشئه بسؤالين دون مغادرة البيعة.",
        "quick.title" => "منتج غير معروف — نضيفوه؟",
        "quick.intro" => "هذا الرمز موش موجود بعد. عادي: تكمّل وقت ما تفضى.",
        "quick.name" => "اسم المنتج (اختياري)",
        "quick.name_hint" => "مثال: شارجور USB-C",
        "quick.price" => "سعر البيع",
        "quick.cost" => "سعر الشراء (اختياري)",
        "quick.qty" => "الكمية",
        "quick.add" => "أضف إلى البيعة",
        "quick.note" => "السعر يكفي: الباقي ينجم يبقى فارغ و يتكمّل من بعد.",
        "quick.later" => "من بعد",
        "products.fills_itself" => "مخزونك يتعبّى وحدو: كل منتج تمسحو يتزاد هنا. كمّل البطاقات وقت ما تلقى وقت.",
        "products.no_price" => "السعر موش محدد",
        "product.never_sold" => "ما تباعش بعد. السعر يتوضّح في أول بيعة.",
        "product.sold" => "الكمية المبيوعة",
        "product.revenue" => "مجموع المبيعات",
        "product.last_sold" => "آخر بيعة",
        "product.not_counted" => "المخزون موش محسوب — تبيع بلا جرد.",
        "expense.title" => "مصاريف المحل",
        "expense.intro" => "الكراء، الماء، الضو، الصيانة: اللي يخرج كل شهر.",
        "expense.per_month" => "في الشهر",
        "expense.per_day" => "في اليوم",
        "expense.per_day_hint" => "اللي لازم تدخّلو كل يوم باش تغطّيهم.",
        "expense.add" => "زيد مصروف",
        "expense.label" => "الاسم",
        "expense.label_hint" => "مثال: كراء المحل",
        "expense.kind" => "النوع",
        "expense.kind.rent" => "كراء",
        "expense.kind.power" => "ضو",
        "expense.kind.water" => "ماء",
        "expense.kind.maintenance" => "صيانة",
        "expense.kind.telecom" => "تلفون / أنترنت",
        "expense.kind.other" => "آخر",
        "expense.amount" => "المبلغ",
        "expense.period" => "الدورية",
        "expense.period.month" => "في الشهر",
        "expense.period.quarter" => "في الثلاثية",
        "expense.period.year" => "في السنة",
        "expense.save" => "سجّل المصروف",
        "expense.empty" => "ما فماش مصاريف مسجلة. زيد الكراء باش تعرف كلفتك الحقيقية.",
        "expense.stop" => "وقّف",
        "expense.stopped" => "متوقف",
        "toast.added" => "تزاد في البيعة",
        "toast.expense" => "تزاد مصروف",
        "toast.expense_updated" => "تبدّل المصروف",
        "toast.expense_stopped" => "توقف المصروف",
        "common.edit" => "تعديل",
        "sales.one" => "بيعة",
        "sales.back" => "رجوع إلى المبيعات",
        "sales.item" => "المادة",
        "sales.qty" => "الكمية",
        "sales.unit" => "سعر الوحدة",
        "sales.seller" => "البائع",
        "sales.payment" => "الدفع",
        "sales.customer" => "العميل",
        "sales.status" => "الحالة",
        "sales.closed" => "مُسجّلة",
        "sales.cancelled" => "ملغاة",
        "sales.edit" => "تعديل البيعة",
        "sales.edit_title" => "تعديل البيعة",
        "sales.cancel" => "إلغاء البيعة",
        "sales.cancel_hint" => "الإلغاء يرجع الكميات إلى المخزون. البيعة تبقى في السجل.",
        "sales.cancel_confirm" => "إلغاء هذه البيعة وإرجاع المخزون؟",
        "sales.qty_hint" => "ضع 0 لحذف مادة. يمكنك تصحيح السعر.",
        "sales.reason" => "السبب (اختياري)",
        "sales.ticket" => "التذكرة",
        "sales.save" => "حفظ التعديلات",
        "toast.edited" => "تم تعديل البيعة",
        "toast.cancelled" => "أُلغيت البيعة — رجع المخزون",
        "nav.quick_sell" => "بيع سريع",
        "nav.search" => "ابحث أو امسح…",
        "nav.group_sell" => "الصندوق",
        "nav.collapse" => "تصغير القائمة",
        "nav.expand" => "فتح القائمة",
        "nav.resize" => "تغيير عرض القائمة",
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
        "pos.new_sale" => "بيعة جديدة",
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
        "settings.security" => "بياناتك ملف واحد في هذا الحاسوب. الحماية بكلمة السر موجودة؛ أما تشفير الملف فمازال موش مفعّل.",
        "settings.backup" => "نسخة احتياطية",
        "settings.backup_now" => "خُذ نسخة الآن",
        "settings.backup_none" => "ما فماش نسخة للتو. خُذ وحدة قبل ما تسكّر المحل.",
        "settings.backup_last" => "آخر نسخة:",
        "settings.backup_count" => "النسخ المحفوظة:",
        "settings.backup_where" => "تتخزن في مجلد data\\backups حذا الصندوق. انقلها على clé USB من وقت لآخر.",
        "toast.backup" => "تم أخذ النسخة",
        _ => return None,
    })
}

fn en(key: &str) -> Option<&'static str> {
    Some(match key {
        "nav.sell" => "Sell",
        "nav.products" => "Products",
        "nav.sales" => "Sales",
        "nav.expenses" => "Costs",
        "nav.session" => "Till / Z",
        "nav.journal" => "Journal",
        "nav.menu" => "Menu",
        "scan.placeholder" => "Scan or type a code",
        "scan.add" => "Add",
        "scan.hint" => "Unknown item? We create it in two questions, without leaving the sale.",
        "quick.title" => "Unknown item — add it?",
        "quick.intro" => "This code is not in the system yet. That is normal: fill it in when you have time.",
        "quick.name" => "Item name (optional)",
        "quick.name_hint" => "e.g. USB-C charger",
        "quick.price" => "Selling price",
        "quick.cost" => "Purchase price (optional)",
        "quick.qty" => "Quantity",
        "quick.add" => "Add to the sale",
        "quick.note" => "The price is enough: everything else can stay empty and be completed later.",
        "quick.later" => "Later",
        "products.fills_itself" => "Your stock fills itself: every scanned item is added here. Complete the sheets whenever you have a moment.",
        "products.no_price" => "price to set",
        "product.never_sold" => "Not sold yet. Its price will settle on the first sale.",
        "product.sold" => "Quantity sold",
        "product.revenue" => "Total sold",
        "product.last_sold" => "Last sale",
        "product.not_counted" => "Stock not counted — you sell without an inventory.",
        "expense.title" => "Shop costs",
        "expense.intro" => "Rent, water, power, maintenance: what leaves every month.",
        "expense.per_month" => "Per month",
        "expense.per_day" => "Per day",
        "expense.per_day_hint" => "What you must take every day to cover them.",
        "expense.add" => "Add a cost",
        "expense.label" => "Name",
        "expense.label_hint" => "e.g. Shop rent",
        "expense.kind" => "Type",
        "expense.kind.rent" => "Rent",
        "expense.kind.power" => "Power",
        "expense.kind.water" => "Water",
        "expense.kind.maintenance" => "Maintenance",
        "expense.kind.telecom" => "Phone / Internet",
        "expense.kind.other" => "Other",
        "expense.amount" => "Amount",
        "expense.period" => "Frequency",
        "expense.period.month" => "Per month",
        "expense.period.quarter" => "Per quarter",
        "expense.period.year" => "Per year",
        "expense.save" => "Save the cost",
        "expense.empty" => "No cost recorded. Add the rent to know your real cost.",
        "expense.stop" => "Stop",
        "expense.stopped" => "Stopped",
        "toast.added" => "Added to the sale",
        "toast.expense" => "Cost added",
        "toast.expense_updated" => "Cost updated",
        "toast.expense_stopped" => "Cost stopped",
        "common.edit" => "Edit",
        "sales.one" => "Sale",
        "sales.back" => "Back to sales",
        "sales.item" => "Item",
        "sales.qty" => "Qty",
        "sales.unit" => "Unit price",
        "sales.seller" => "Sold by",
        "sales.payment" => "Payment",
        "sales.customer" => "Customer",
        "sales.status" => "Status",
        "sales.closed" => "Completed",
        "sales.cancelled" => "Cancelled",
        "sales.edit" => "Edit sale",
        "sales.edit_title" => "Edit sale",
        "sales.cancel" => "Cancel sale",
        "sales.cancel_hint" => "Cancelling returns the items to stock. The sale stays in the history.",
        "sales.cancel_confirm" => "Cancel this sale and return the stock?",
        "sales.qty_hint" => "Set 0 to remove an item. The price can be corrected.",
        "sales.reason" => "Reason (optional)",
        "sales.ticket" => "Receipt",
        "sales.save" => "Save changes",
        "toast.edited" => "Sale updated",
        "toast.cancelled" => "Sale cancelled — stock restored",
        "nav.quick_sell" => "Quick sale",
        "nav.search" => "Search or scan…",
        "nav.group_sell" => "Till",
        "nav.collapse" => "Collapse menu",
        "nav.expand" => "Expand menu",
        "nav.resize" => "Resize menu",
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
        "pos.new_sale" => "New sale",
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
        "settings.security" => "Your data is one file on this computer. Password protection exists; encrypting the file is not enabled yet.",
        "settings.backup" => "Backup",
        "settings.backup_now" => "Back up now",
        "settings.backup_none" => "No backup yet. Make one before you close the shop.",
        "settings.backup_last" => "Last backup:",
        "settings.backup_count" => "Backups kept:",
        "settings.backup_where" => "They go to the data\\backups folder, next to the till. Copy it to a USB stick now and then.",
        "toast.backup" => "Backup done",
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
            Nav::Expenses => &[
                "M3.5 7.5h17v9h-17z",
                "M12 14.4a2.4 2.4 0 1 0 0-4.8 2.4 2.4 0 0 0 0 4.8Z",
                "M6.6 10v4M17.4 10v4",
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

/// Fold / unfold arrow for the sidebar toggle.
pub fn icon_chevron(point_start: bool) -> Markup {
    let d = if point_start { "M14.5 6.5 9 12l5.5 5.5" } else { "M9.5 6.5 15 12l-5.5 5.5" };
    html! {
        svg.nav-icon viewBox="0 0 24 24" fill="none" stroke="currentColor"
            "stroke-width"=(2) "stroke-linecap"="round" "stroke-linejoin"="round"
            aria-hidden="true" {
            path d=(d);
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
    let toggle_key = if chrome.collapsed { "nav.expand" } else { "nav.collapse" };

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
                            /* collapse / expand — the arrow shows which way it will move */
                            a.side-toggle href="/sidebar" title=(t(lang, toggle_key))
                              aria-label=(t(lang, toggle_key))
                              aria-expanded=(if chrome.collapsed { "false" } else { "true" }) {
                                (icon_chevron((lang == Lang::Ar) != chrome.collapsed))
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
                                (item(Nav::Expenses, None))
                                (item(Nav::Settings, None))
                            }
                        }

                        /* drag handle for resizing — a visible line, not a hidden corner */
                        div.side-grip role="separator" aria-orientation="vertical"
                            aria-label=(t(lang, "nav.resize")) {}

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
                                    a href="/expenses" { (t(lang, "nav.expenses")) }
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
    /// false = the shop never counted this item: say nothing about stock.
    pub counted: bool,
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
    let low = product.counted && product.stock <= 0;
    let no_price = product.price <= 0;
    html! {
        form method="post" action="/cart/add" {
            input type="hidden" name="product_id" value=(product.id);
            button.product type="submit" {
                div.thumb { (initial) }
                div.name { (product.name) }
                @if no_price {
                    div.price.price-open { "prix à définir" }
                } @else {
                    div.price { (money(product.price)) }
                }
                @if product.counted {
                    div class=(if low { "stock low" } else { "stock" }) {
                        @if low { "Rupture" } @else { (product.stock) " en stock" }
                    }
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
