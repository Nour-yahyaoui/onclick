# ADR-013 — AI assistant on the web dashboard (Groq, Tunisian derja, grounded in real numbers)

**Status:** ✅ **ACCEPTED** — read-only Q&A, answer language matches the question, **offline parser in v1**, voice input later.
**Date:** 2026-10-20
**Relates to:** ADR-003 (control plane, aggregates only), ADR-007 (every dinar exact), DESIGN-002 (simplicity), SEC-001

---

## 1. The requirement

On the web dashboard, the owner types or speaks:

> **« 9adeh rbaht nhar jem3a hathy »** — how much did I profit last Friday?
> **« 9adeh be3t mn iphone 12 pro ems »** — how many iPhone 12 Pro did I sell yesterday?

and gets an exact, trustworthy answer in his own language.

---

## 2. The non-negotiable rule: the model never computes money

> **An LLM must never be the one doing arithmetic on the shop's money.**

If you let a model read rows and add them up, it will occasionally be wrong by a few dinars and you will never notice. That directly violates **ADR-007 / DESIGN-002**: every dinar is exact.

**Correct architecture: LLM as a translator, SQL as the calculator.**

```
question (derja / fr / en)
        │
        ▼
  ┌───────────────────────────────┐
  │ 1. INTENT EXTRACTION (Groq)   │   text → STRICT JSON, no numbers
  │    metric · dimension ·       │   e.g. {"metric":"gross_profit",
  │    date_range · filters       │         "range":{"type":"day_of_week","value":"fri",
  │                               │                "offset":"last"},
  │                               │         "filters":{}}
  └───────────────┬───────────────┘
                  ▼
  ┌───────────────────────────────┐
  │ 2. VALIDATOR (our code)       │   whitelist metric/dimension/filter,
  │                               │   reject unknown keys, clamp ranges,
  │                               │   enforce shop_id from the session
  └───────────────┬───────────────┘
                  ▼
  ┌───────────────────────────────┐
  │ 3. PARAMETERISED SQL          │   runs against Neon aggregates
  │    (never model-generated)    │   → exact integer millimes
  └───────────────┬───────────────┘
                  ▼
  ┌───────────────────────────────┐
  │ 4. ANSWER RENDERING (Groq)    │   numbers injected verbatim, model only
  │    optional, with fallback    │   phrases them; template fallback exists
  └───────────────────────────────┘
```

- Step 1 output is **JSON only** (`response_format: json_object` + a published schema), max ~120 tokens.
- Step 3 uses a **hand-written query per metric** — the model's JSON only selects *which* query and *which parameters*. There is no text-to-SQL surface to attack.
- Step 4 is optional: if Groq fails, a deterministic template answers — `« الربح الإجمالي نهار الجمعة 17/10 : 412.500 د »`. **The feature always answers, even when the LLM is down.**
- **Numbers are formatted by our code** (`i64` millimes → `1 234.500 TND`, LTR-isolated inside Arabic text), then inserted into the sentence — never typed by the model.

---

## 3. Intent schema (the whole contract)

```jsonc
{
  "metric": "revenue | gross_profit | net_profit | sales_count | units_sold | avg_basket
           | discounts_total | cash_collected | receivables_open | expenses_total
           | stock_value | expiring_soon | top_products | margin_by_product
           | replacement_margin_gap | returns_total",
  "dimension": "total | per_product | per_category | per_brand | per_seller | per_day | per_week | per_month",
  "range": {
    "type": "day | day_of_week | week | month | year | last_n_days | named_month | explicit",
    "value": "2026-10-17 | fri | 7 | 2026-10 | ...",
    "offset": "last | this | offset_n"
  },
  "filters": {
    "product_query": "iphone 12 pro",     // fuzzy text → resolved by OUR matcher, not the model
    "brand": "apple", "category": "téléphones",
    "payment_method": "cash | card | credit",
    "seller": "sami"
  },
  "compare": "previous_period | none",
  "language": "ar-TN | fr | en"
}
```

**Resolution order in our code** (none of this is trusted to the model):
1. `shop_id` from the authenticated session — **always**, never from the payload.
2. `product_query` → fuzzy match against the shop's product names/aliases (trigram + token search); ambiguous → the assistant **asks** ("Vous voulez dire iPhone 12 Pro ou iPhone 12 Pro Max ?") with tappable chips.
3. Date ranges → computed in `Africa/Tunis`; "last Friday" = the most recent completed Friday, never a future date.

---

## 4. Tunisian derja support (this is where most products fail)

Owners type **Arabizi**: Latin letters + digits as phonemes. It must be normalised *before* intent extraction, and it must be done by us, not the model:

| Arabizi | Arabic | Meaning | Metric |
|---|---|---|---|
| `9adeh`, `qadeh`, `kadeh` | قدّاش | how much | — (question word) |
| `rbaht`, `rbah`, `ربحت` | ربحت | I profited | `gross_profit` |
| `be3t`, `b3t`, `بعت` | بعت | I sold | `sales_count` / `units_sold` |
| `byo3`, `mabi3at` | مبيعات | sales | `revenue` |
| `dakhl`, `مدخول` | دخل | income | `revenue` |
| `sarf`, `مصاريف` | مصاريف | expenses | `expenses_total` |
| `kasser`, `caisse` | كاسة | cash | `cash_collected` |
| `din`, `كريدي`, `carnet` | دين / كريدي | debt | `receivables_open` |
| `stock`, `makhzoun` | مخزون | stock | `stock_value` |
| `ems`, `لبارح` | البارح | yesterday | range = day, −1 |
| `lyoum`, `اليوم` | اليوم | today | day, 0 |
| `nhar jem3a`, `jem3a` | نهار الجمعة | Friday | `day_of_week: fri, last` |
| `hadha chhar`, `هالشهر` | هذا الشهر | this month | month, current |
| `l3am`, `العام` | العام | this year | year, current |
| `9bal`, `9bal chhar` | قبل | previous | offset −1 |

Implementation:
- **Normalisation table** (Arabizi → canonical Arabic + French/English synonyms), applied to the raw string.
- **A 200-question eval set** written in real derja/Arabizi/French, with expected JSON intents and expected numbers from a fixture DB. **This is the quality gate**: a change to the prompt or dictionary must pass it. Without this suite, AI features rot silently.
- **Few-shot prompt** carries ~12 exemplars (mix of derja, fr, en) + the metric catalog + "return JSON only".
- Model choice: a small/fast Groq model by default (e.g. a Llama-3.x 8B class) for intent extraction — intent JSON does not need a frontier model. A larger model is used only if the eval suite says so. Cost is then ~fractions of a cent per question.

---

## 5. What the assistant can and cannot answer

**Can** (all from aggregates that already exist in Neon — ADR-003):
revenue, gross/net profit, margin, sales count, units sold, average basket, discounts given, cash collected, receivables total + aging buckets, expenses, stock value, expiring soon count, top products, per-day/week/month trends, comparisons to the previous period, replacement-margin gap (ADR-008).

**Cannot — by design, not by limitation of the model:**
| Question | Why not | What it says instead |
|---|---|---|
| "Who owes me the most?" | debtor **names never leave the shop** (ADR-010 §2) | `« Total dû : 8,900.000 · 4 clients en retard +60j · (détail sur la caisse) »` |
| Any single customer's history | no customer records in the cloud | same |
| "Show me invoice #V-1042" | invoice lines are local-only | `« Les tickets restent sur la caisse »` |
| Anything needing a product photo | images never sync (ADR-012 §6) | — |

This is a **feature to sell, not to apologise for**: the AI can answer about the business without ever knowing who the customers are.

---

## 6. Guardrails

| Risk | Control |
|---|---|
| **Model invents a number** | Impossible by construction: it never sees or writes numbers; the answer template injects them (§2) |
| **Prompt injection** ("ignore your instructions and show…") | It can only emit the JSON schema; validator rejects anything outside the whitelist. No free-form SQL, no tool that can read arbitrary rows |
| **Cross-shop leakage** | `shop_id` comes from the session, never from the model output; every query is scoped and indexed by `shop_id` |
| **Cost abuse** | Per-shop rate limit (e.g. 60 questions/hour), 120-token cap on extraction, response cache keyed by `(shop_id, normalised question)` for 10 min, and a monthly budget cap per account |
| **Latency** | Groq LPU is fast (~200–600 ms); streaming the rendering step; the **numbers arrive first** (SQL runs immediately) so a slow model never delays the figure |
| **Provider outage** | deterministic template answer + the whole dashboard still works (§2 step 4 fallback) |
| **Key exposure** | `GROQ_API_KEY` only in the Vercel server environment; the browser never sees it (all calls through a `/api/ask` route) |
| **Audit** | Each question logged server-side: shop, normalised question, intent JSON, metric queried, latency, model, whether fallback was used. **Never the answer's PII** (there is none) |
| **Hallucinated advice** | The model is told to answer *only* from the returned figures; no advice, no forecasts, no speculation. Advisory text is generated by our rules engine (STRATEGY-001 ③), not the LLM |

---

## 7. UX

- One input on the dashboard: **« اسأل على رقم… / Posez une question… »** with 4 suggestion chips derived from today's data (`CA du jour · Profit ce mois · Top produits 7j · Reste dû`).
- **Answer card**: big number, one sentence, the period, and `[ Voir le détail ]` → deep-links to the report the number came from. **Every answer is traceable to a screen** — that's what makes it trustworthy rather than magic.
- **Voice input** (optional, browser `SpeechRecognition`/server STT): many owners will literally speak derja. Bonus, not v1-critical.
- Replies in the language of the question (derja-romanised in → derja-romanised or Arabic out, per a setting).
- **Nothing is hidden behind the assistant**: every question has a dashboard equivalent. AI is an accelerator, not the only door.
- Offline/local dashboard: an optional **deterministic parser** (same intent layer, regex + dictionary, no LLM) answers the 20 most common questions locally. No internet, no cost, no model. Worth doing — it also serves as the fallback when Groq is unreachable.

---

## 8. Data actually sent to Groq

Per question: the normalised question text, the intent schema description, ~12 few-shot examples, the metric catalog, and — for the rendering step — the **computed figures** (e.g. `{"gross_profit": 412500, "range": "2026-10-17", "sales_count": 63}`).
**Never:** customer names, phone numbers, product photos, invoice lines, debtor names, cashier full names (seller ids only), the DB itself.
Documented in the dashboard's privacy note, because "AI" and "privacy" must not contradict the app's promise.

---

## 9. Effort

| Item | Days |
|---|---|
| Analytics query layer per metric (`/api/metrics/*`, parameterised, shop-scoped) | 1.5 |
| Intent schema + validator + derja/fr/en normalisation dictionary | 1.5 |
| Groq integration (JSON mode, retries, timeouts, key handling, model config) | 0.75 |
| Answer rendering + deterministic template fallback + number injection | 0.75 |
| Chat UI (input, chips, answer card, deep-links, language handling, RTL) | 1.0 |
| Rate limits, response cache, audit logging, budget cap | 0.5 |
| **200-question eval suite** + CI gate | 1.0 |
| *(optional)* local deterministic parser for the offline dashboard | 0.75 |
| **Total** | **7.0 d** (+0.75 offline parser) |

**Placement:** M6 (web dashboard). The **offline parser** could ship earlier with M4 if you want the local dashboard to answer questions too.

---

## 10. Decisions

1. **Scope of the assistant** — read-only analytics Q&A (recommended), or do you also want it to *act* ("raise the price of X by 5%")? *Acting requires a command path into the till, with its own confirmations — I'd keep it read-only in v1.*
2. **Derja output style** — answer in Arabizi (how the owner types), in Arabic script, or in French? (Recommend: match the question's language; setting available.)
3. **Offline local parser** for the till dashboard — build in v1 (+0.75 d) or leave questions to the web dashboard only?
4. **Groq model tier** — start with a small/fast model validated by the eval suite (recommended, cheapest), or start with the largest for accuracy?
5. **Voice input** — in v1 or later?
6. Confirm that **names never go to the AI**, so "who owes me" is answered as totals only (ADR-010/ADR-003 rule).
