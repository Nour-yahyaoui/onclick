# STRATEGY-001 — How to win a place next to the existing SaaS

**Status:** recommendation — needs 4 decisions (§8)
**Date:** 2026-10-20
**Relates to:** ADR-002/003 (licensing, control plane), ADR-005 (barcodes/batches), DESIGN-001 (theme/i18n)

---

## 1. The strategic finding: compliance is the wedge, not features

Two Tunisian regulations dominate this market, and both are **integration work, not feature work** — which is exactly why incumbents are slow and you can win.

| Regulation | Status today (Oct 2026) | What it means for you |
|---|---|---|
| **E-invoicing TEIF via El Fatoora / TTN** — Art. 53, Loi de Finances 2026 (Loi n°17-2025, enacted 12 Dec 2025) | **In force since 1 Jan 2026** for VAT-liable businesses, extended to services. Format = TEIF XML, signature = XAdES-B, certificate from **ANCE/TunTrust** (~350 TND, 2 yrs), transmitted via TTN | Every B2B invoice your shops issue must eventually be electronic. **A till that generates + queues + transmits TEIF invoices is worth more than a prettier dashboard.** Penalties: 100–500 TND per paper invoice, capped 50,000 TND/year |
| **Fiscal cash register (NACEF homologation)** — arrêté of 14 Oct 2025, JORT n°125 | Phased: 1 Nov 2025 (tourism restaurants, cafés 2e/3e cat., tea rooms) · **1 Jul 2026 (other legal entities, on-premise consumption)** · 1 Jul 2027 (individuals, real regime) · 1 Jul 2028 (others) | Applies to **"consommation sur place"** activities — cafés/restaurants/fast-food — not general retail. If you sell to those segments, the software must bridge to a **homologated fiscal device** or be certified itself |

Source material: [TEIF/TTN obligations and penalties](https://elfatoora.digital/facture-electronique-tunisie.php?lang=en), [Finance Law 2026 analysis](https://hesabi.tn/actualites/facturation-electronique-obligatoire-tunisie-2026), [register rollout calendar](https://caisses.tn/caisses-obligatoires-en-tunisie/), [DGI phase dates](https://africanmanager.com/tunisie-lobligation-des-caisses-enregistreuses-entre-en-vigueur-le-1er-novembre/).

> **The strategic read:** incumbents in Tunisia split into two camps — *accounting/compliance tools* (strong on TEIF, weak on the till, back-office, slow) and *POS tools* (fast front-end, no real compliance, cloud-dependent). **Almost nobody owns "fast local till + real compliance + offline-first".** That intersection is your seat at the table, and it is defensible because compliance is tedious, documented, and regulated.

**Caveat to respect:** homologation is a certification process. Do not promise "NACEF homologated" until it is. The standard, honest path is **bridge to a homologated fiscal printer** (most Tunisian fiscal printers produce the fiscal journal and signature themselves) rather than certifying your binary — and target general retail first, cafés/restaurants later.

---

## 2. What actually differentiates you (architectural, not copyable in a sprint)

| Advantage | Why incumbents can't match it quickly |
|---|---|
| **The till cannot go down** — one exe, SQLite, no network on the sale path | Theirs is cloud-first. Offline mode is an afterthought ("cached read-only"), not the architecture |
| **15 ms sale on a 20-year-old PC over 3G** | A web POS pays 300–800 ms per action + page loads in a shop with intermittent 3G |
| **~0 infra cost per shop** → you can price at 25–60 TND/mo profitably | Cloud SaaS at that price is unprofitable in this market; they must charge more or ignore the segment |
| **Single file, no install ceremony, no new hardware** | They require accounts, onboarding calls, hardware bundles |
| **Data never leaves the shop** (aggregates only) | Their whole model requires uploading everything; many shopkeepers refuse or distrust that |
| **Arabic-first, RTL-correct, TND/millimes/TVA-native** | Most are French-first with poor Arabic or a `$`/generic tax model |
| **Per-lot margin, batch expiry** | Skipped because it's hard and unglamorous |

**Do not compete on:** number of features, integrations marketplace, AI, design awards. You will lose.

---

## 3. The five signature features (build these, ignore the other 40)

Ranked by *(shop pain × how badly competitors cover it) / effort*. Each is a reason to buy on its own.

### ① Carnet de crédit digital + WhatsApp statement ⭐ *highest ROI*
Tunisian neighbourhood retail runs on **customer credit (crédit / carnet)**. It's tracked in a paper notebook, it's the single biggest source of disputes and unpaid debt, and it is *the* reason a shopkeeper distrusts cloud tools (their notebook never goes offline).

- Per-customer running balance, opened from the till in 2 taps ("3al credit" button on the payment screen).
- Aging buckets (0–30/31–60/60+), credit limit with a soft warn, settle partial payments.
- **Statement to WhatsApp in one tap**: `wa.me/<phone>?text=…` deep link — free, no API, no cloud upload of the ledger. The customer gets a clean, dated, Arabic/French statement — which is exactly the social pressure that gets debts paid.
- Print the same statement for the shop door / disputes.
- **Effort ~3 days** (schema and ledger already planned). **Highest pain × lowest cost in this document.**

### ② Compliance engine: TEIF e-invoices, queued offline ⭐ *the moat*
- Generate **TEIF XML** for B2B invoices, sign with the shop's **ANCE certificate** (USB token via local PKCS#11 → works **offline**, no cloud dependency), queue and transmit to **El Fatoora/TTN** when connectivity exists.
- **You win the offline angle here too:** a web SaaS cannot issue a compliant invoice when the line is down. Your exe can produce and sign locally, queue the transmission, and retry — legally safer for the shop.
- Archive every issued/transmitted invoice with its status (`draft → signed → queued → accepted → rejected`) and a retry log.
- Also: a **fiscal-Z / grand-total journal** that is append-only and tamper-evident (hash chain), which is precisely what auditors want and what makes your records trustworthy.
- **Effort ~12–15 days** (TEIF schema, signing, token access, queue, error handling, TTN test environment). **This is what converts a "nice app" into a "must-have", and it's the hardest thing for a competitor to clone.**
- **Phase it:** v1.1 = TEIF for B2B invoices + fiscal journal + status tracking. v2 = fiscal-printer bridge for cafés/restaurants.

### ③ "It never went down" + the 21:00 WhatsApp summary ⭐ *retention machine*
- Marketing claim backed by a **visible sync/uptime log** in the app: "0 sales lost, 0 minutes down, 4 days queued and backfilled" — provable, unlike a promise.
- **Daily 21:00 summary** to the owner's phone: `CA 842.500 TND · 63 tickets · panier moyen 13.4 · cash 812 vs expected 800 (-12) · 4 produits périment cette semaine · meilleur: Yaourt 500g (38)`. Sent from your Next.js cron reading aggregates (no PII), delivered via **WhatsApp deep link, Telegram bot (free), or email**.
- Weekly: P&L one-pager + "3 actions to increase margin this week" (rules-based, e.g. reorder alerts, price-below-cost, dead stock).
- **Effort ~3 days** (cron + template + Telegram bot). Enormous stickiness: the owner opens it every single day, and cancelling means losing that message.

### ④ Batch/expiry + "shrinkage detective" — insight, not dashboards
- Expiry aging report (30/60/90 d), write-offs with reasons, returns-to-lot — already in ADR-005.
- Then the differentiator everyone skips: **tell them what it cost**. *"Ce mois: 240 TND perdus. 62% viennent de 4 produits. 2 fournisseurs en cause."*
- Auto-discount suggestions on near-expiry stock (v1.1) so loss becomes revenue.
- **Effort ~2 days on top of ADR-005** (rules, not ML).

### ⑤ Margin truth per supplier lot
- `sale_lines.unit_cost_millimes` already freezes cost per lot → **margin per delivery, per supplier**.
- Report: *"Livraison du 12/10 (Fournisseur B): marge 6.2% — pas 18%. Vous avez payé 0.180 de trop."*
- Nobody in this market shows it. It's the report that makes an owner feel the software **earns money**, which is what turns a 40 TND subscription into an obvious yes.
- **Effort ~1.5 days** (data already exists post-ADR-005).

### Runner-ups (v1.1 → v2, in order)
6. **Stock count from a phone on the shop WiFi** (the local server serves the LAN): scan shelves with the phone camera, reconcile — replaces a 4-hour manual inventory. ~3 days, very visible value.
7. **Multi-till / multi-shop** with aggregated reporting (architecture already ready with `device_id`).
8. **Zero-training onboarding**: Excel import + scan-to-create catalog + a 20-minute "first sale" path. Reduces your sales cycle, not just their effort. ~3 days.
9. **Arabic-first UI + Arabic receipts** (raster print path, +3 days) — decisive for many shops.
10. **Returns/credit notes in 2 taps**, credit note printed with the original.
11. **Open data**: one-click full export (Excel + raw `.db`). Anti-lock-in is a *selling point* in a market burned by cloud vendors. ~0.5 day.
12. **Loyalty / bundles / price lists per customer type** — only after 20 paying shops.

---

## 4. Anti-features — explicitly decide *not* to build

| Don't build | Why | Do this instead |
|---|---|---|
| Full accounting / GL / payroll | You'd compete with Hesabi/Sage and lose; huge surface, low differentiation | **Export to the accountant**: a clean journal + TEIF archive + CSV/XLSX pack, and a one-page "give this to your comptable" |
| E-commerce storefront / marketplace sync | Different product, different buyer | Postpone to v3 at the earliest |
| AI chat assistant | Demos well, nobody pays | Rules-based insights (④⑤) deliver the same "smart" feeling for 1/20 of the cost |
| Loyalty cards / points | Low urgency, high complexity | Only when 20+ shops ask |
| Card terminal / payment gateway integration | Hardware + banking partnerships in Tunisia are slow | None in v1 |
| Multi-currency, multi-warehouse, transfers | Enterprise concerns | Backlog |
| Feature parity with Odoo | Unwinnable, and their users aren't your users | Win on the 5 signature features |

---

## 5. Positioning & pricing

**Positioning statement (use it everywhere):**
> **« La caisse qui ne tombe jamais. »** Fonctionne sans internet, conforme TEIF, et vous dit où vous perdez de l'argent.
> *The till that never goes down. Works without internet, TEIF-compliant, and shows you where you're losing money.*

**Who to target first (pick one, then repeat):**
- **Best fit:** cosmetics / parapharmacie / grocery / quincaillerie with 2–5 staff, 100–500 sales/day, one PC, a scanner, batch or credit-heavy operation.
- **Avoid at first:** cafés/restaurants (highest regulatory urgency but fiscal-register homologation barrier — revisit at v2 with a printer bridge), large chains, anything needing payroll.

**Pricing shape — charge for the obligation, not the nice-to-have:**
| Tier | Price (TND) | Contents | Rationale |
|---|---|---|---|
| **Local POS** | **free forever** | selling, stock, batches, credit book, local reports, local dashboard | Removes all DRM risk from the part that must never fail; becomes your distribution engine (see ADR-002) |
| **Cloud** | 39–59 / month | remote dashboard, 21:00 summaries, multi-device access, cloud backup of aggregates | The genuinely cloud-shaped value |
| **Compliance** | +29–49 / month | TEIF generation, ANCE cert management, TTN transmission, audit archive | **Non-optional spend** — the easiest upsell you will ever have; priced against a 100–500 TND/invoice fine |
| **Install/setup** | 150–250 one-time | import, scanner config, receipt printer, training | Funds your first months; qualifies serious buyers |

**Distribution:** local computer shops / IT resellers as installers with 20–30% recurring commission. One exe + a USB stick + a 30-minute script is the whole sales kit — nobody else can offer that.

---

## 6. Scorecard (build order)

| # | Feature | Shop pain | Competitor coverage | Effort | Moat | Priority |
|---|---|---|---|---|---|---|
| ① | Credit book + WhatsApp statement | ★★★★★ | partial / cloud-only | 3 d | medium | **v1** |
| ② | TEIF e-invoicing + fiscal journal | ★★★★★ (legal) | weak in POS tools | 12–15 d | **high** | **v1.1** |
| ③ | 21:00 summary + uptime proof | ★★★★ | rare | 3 d | medium | **v1.1** |
| ④ | Shrinkage detective | ★★★★ | rare | 2 d | medium | **v1.1** |
| ⑤ | Margin per supplier lot | ★★★★ | almost none | 1.5 d | **high** | **v1.1** |
| ⑥ | Phone stock count | ★★★ | rare | 3 d | medium | v2 |
| ⑦ | Multi-till | ★★★ | expensive elsewhere | 4 d | low | v2 |
| ⑧ | 20-min onboarding | ★★★★ | slow everywhere | 3 d | low | v1 |
| ⑨ | Arabic receipts | ★★★ | rare | 3 d | medium | v2 |
| ⑩ | Returns in 2 taps | ★★★ | common | 1 d | none | v1 |

**Rule: ship ①②③ before anything else in this table.** Two signature features beat ten half-features. If you only get one right, make it ① (it sells the product locally by word of mouth) and if you only get one *moat*, make it ②.

---

## 7. Honest risks

| Risk | Reality | Mitigation |
|---|---|---|
| **Homologation requirement expands to your segment** | The arrêté currently covers on-premise consumption; phases run to 2028 | Track DGI notes quarterly; design the **fiscal-printer bridge** interface now (one trait, one driver) so a certified device can be attached without redesign |
| TEIF enforcement is inconsistent ("soft landing") | Officials signalled relaxed enforcement for SMEs in early 2026 | Sell compliance as *protection*, not fear; ship before enforcement tightens |
| ANCE certificate friction (350 TND, USB token) | Per-shop cost and a physical token | Make it a guided 3-step flow with screenshots; support Vault/soft cert later |
| Big player enters with a free POS | Possible (bank, telecom) | Your moat is support + local compliance + installer network, not price |
| Scope explosion from this document | The real risk | **Nothing here enters v1 except ①⑧⑩**; everything else is v1.1+ and gated on paying shops |
| You build ② before talking to an accountant | 15 days could be misaimed | Validate the TEIF output with one accountant and TTN's test environment **before** writing the signing layer |

---

## 8. Decisions needed

1. **Target segment for the first 5 shops** — retail (recommended) or cafés/restaurants (higher urgency, homologation barrier)?
2. **Compliance scope** — start with TEIF B2B e-invoicing, or also pursue fiscal-register/printer homologation in v2?
3. **Pricing** — free POS + paid cloud/compliance as in §5, or paid POS from day one (changes ADR-002/003)?
4. **Owner channel for the daily summary** — WhatsApp deep link (manual send, free), Telegram bot (automatic, free, needs the owner to install an app), or SMS (costs money, works for everyone)?
5. **Do you have access to a real shop + an accountant** for pilot validation? (Non-negotiable before building ②: 15 days of work must be verified against TTN's test flow, not guessed.)

---

## 9. One-paragraph summary

You will not win by having more features than Odoo or a cloud POS. You win by owning the intersection nobody occupies: **a till that is provably faster and never goes offline, with the two things Tunisian shops actually lose money on — customer credit (①) and non-compliance (②) — solved locally, at a price a cloud vendor cannot match, sold on a USB stick.** Everything else is decoration.
