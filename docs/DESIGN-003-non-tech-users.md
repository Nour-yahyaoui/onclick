# DESIGN-003 — For people who are not tech-friendly (the binding UX law)

**Status:** ✅ **ACCEPTED — highest-priority design constraint** ("my audience is not tech friendly; think twice for every button and every text")
**Date:** 2026-10-20
**Relates to:** DESIGN-002 (simplicity doctrine), PLAN §6, every ADR with a UI surface

---

## 1. Who we are designing for

A shop owner in his 40s–60s. Uses WhatsApp and Facebook daily. Types with two fingers, often in Arabizi. Reads Arabic more comfortably than French. Has never heard the words "sync", "license", "cache", "profile" or "encryption". Wears glasses, works in poor light, on a 15-year-old 1366×768 monitor. **Has no patience for software and no time to learn anything.** If the app confuses him once, his son-in-law is told it's "complicated", and that's the end of your product in that neighbourhood.

**Consequence:** usability is not a polish phase. It is a **build gate**, and it outranks feature richness every single time.

---

## 2. The seven laws

| # | Law | Means |
|---|---|---|
| **1** | **No jargon, ever** | If a shopkeeper wouldn't say the word out loud, it doesn't appear on screen. (§3 has the blacklist) |
| **2** | **One action per screen** | One big button. Everything else is small, secondary, or absent. Max **3 visible buttons**, 1 primary |
| **3** | **Nothing to configure** | Setup = max 3 questions. Every setting has a working default. 90% of shops never open Settings |
| **4** | **Forgive, don't ask** | Undo instead of confirmation. Warn instead of block. Never a "are you sure?" for reversible things |
| **5** | **Explain the failure, then give the next step** | Never an error code. Never blame. Always a button that does something useful |
| **6** | **Big, and readable from 60 cm** | 16 px minimum text (18–20 in the till), 44 px targets, high contrast, no thin fonts, no colour-only signals |
| **7** | **A human is one tap away** | "Appeler l'assistance" inside the app. Support is a feature, not a form |

---

## 3. Vocabulary blacklist → what the user actually sees

**Your interfaces must never contain any word in the left column.**

| Never show | Show instead (fr) | Arabic guidance |
|---|---|---|
| sync, synchronisation, outbox, backfill, pending | « Sauvegarde : hier 21:04 ✓ » · « En attente d'internet (12 ventes) » | نسخة احتياطية |
| licence, lease, entitlement, grace period, expiration | « Abonnement actif jusqu'au 30/11 » · « Il reste 8 jours pour renouveler » | الاشتراك |
| device pairing, device limit, token | « Ce computer » → **« Cet ordinateur »** · « Code à 6 chiffres » | رمز مكوّن من 6 أرقام |
| compliance profile A/B/C/D | **never visible.** Setup asks: *« Est-ce que vous facturez la TVA ? »* → 3 big options | هل تُصرّح بالتوريد على القيمة المضافة؟ |
| database, SQLite, encryption, AES, hash, backup file | « Vos données sont protégées par un mot de passe » · « Sauvegarde automatique chaque soir » | النسخ الاحتياطي اليومي |
| batch, lot, FEFO, landed cost, FX, allocation | « Livraison » · « Péremption » · « Frais (transport, douane) » · « Prix d'achat en dollars » | التكلفة، الصلاحية |
| variant, SKU, matrix | « Modèle » · « Couleur » · « Capacité » | الموديل |
| margin, COGS, WAC, dispersion, deviation | « Bénéfice » · « Prix d'achat » · « Vous avez vendu moins cher que d'habitude » | الربح |
| audit log, event, entity, actor | « Qui a fait quoi » | من فعل ماذا |
| journal, report, KPI, dashboard | « Mes chiffres » · « Aujourd'hui » · « Ce mois » | أرقامي |
| owner/cashier/manager roles | « Patron » · « Vendeur » | صاحب المحل، بائع |
| crash, exception, timeout, 500 | « L'imprimante ne répond pas » + [Réessayer] [Imprimer en PDF] [Appeler] | — |
| login, session, credentials | « Entrez votre mot de passe » (and nothing else) | كلمة السر |
| "Oups !", "Une erreur est survenue" | **banned.** Say what happened and what to do next | — |

**Shop words are correct words:** *kassa, carnet, din, ticket, bon, livraison, caisse*. If the shop says "din", the button says "Din". Not "créance client".

---

## 4. The button budget

| Screen | Primary | Secondary | Everything else |
|---|---|---|---|
| **Vente (till)** | **[ Encaisser ]** (or [ Save ]) | [ Annuler ] | hidden |
| **Carnet** | **[ Encaisser ]** on a debt | [ Nouveau din ] | detail below the fold |
| **Livraison reçue** | **[ Enregistrer la livraison ]** | [ Ajouter une ligne ] | charges/comments collapsed |
| **Produit** | **[ Enregistrer ]** | [ Photo ] [ Supprimer (patron only) ] | everything else behind « Plus » |
| **Mes chiffres** | — (read-only) | [ Aujourd'hui ] [ Ce mois ] | filters behind « Plus » |

Rules: **exactly one green button per screen** · secondary buttons are outline only · destructive actions are never adjacent to the primary · no icon-only buttons anywhere (icon + word, always) · buttons ≥ 44 px, labels ≥ 16 px, and **the label must fit in Arabic without shrinking** (DESIGN-001 §4.3).

---

## 5. My own designs that were too technical — audited and fixed

Being honest, several ADRs I wrote would have failed this law. Corrected:

| Where I over-engineered | Before | **Now** |
|---|---|---|
| ADR-007 | User picks "compliance profile A/B/C/D" | **One question with 3 plain answers:** *Est-ce que vous facturez la TVA ?* → « Non » / « Oui, toujours le même taux » / « Oui, ça dépend du produit ». No profile letters anywhere |
| ADR-003 / ADR-002 | "Pairing code", "device limit reached", "lease lapsed" | « **Code à 6 chiffres** » · « Cet abonnement est déjà utilisé sur une autre caisse » · « **Votre abonnement attend un paiement** » |
| ADR-003 | Offline chip: `Offline — 3 days queued` | « **Pas d'internet. 12 ventes en attente — tout est enregistré ici.** » (reassurance first) |
| ADR-011 | "Journal d'activité", "post-close variance" | « **Qui a fait quoi** » · « Ce qui a changé après la clôture » |
| ADR-008 | "replacement margin gap", "dispersion" | « **Vous vendez à 1 500, mais le nouveau prix d'achat est 1 400 : votre bénéfice a baissé.** » |
| ADR-008 | "FX rate micros", "landed cost allocation" | « Prix d'achat en dollars » → « Frais de transport et douane » → **app shows one final cost per phone**. No rates on screen unless the owner asks |
| ADR-013 | Question box with a JSON schema behind it | **A box that says « Posez une question… »** + 4 suggestion buttons. Nothing about models, tokens, or intents |
| SEC-001 | "SQLCipher", "Argon2id", "DPAPI" in Settings | One line: « Vos données sont protégées par un mot de passe. Sauvegarde chaque soir à 21:00. » Technical detail only in a hidden "À propos" screen |
| ADR-010 | Debt aging buckets 0-30/31-60/61-90/90+ | « En retard : 3 clients · ensemble 4 200.000 · le plus ancien 68 jours » — buckets exist, but only if the owner opens the detail |
| ADR-005 | "FEFO / expired lot override" | « **Périmé** » badge + « Vendre quand même ? » (warn, not block) |
| ADR-009 | "price_source", "override_reason" fields | Never shown. The cashier types a price. Full stop |
| ADR-012 | format/crop/quota settings | One button: **[ Photo ]**. Everything automatic |

**Rule derived from this audit:** *if a feature needs a screen to explain it, the feature is wrong, not the screen.*

---

## 6. Settings: 40 internal keys → **6 things a shopkeeper understands**

```
Réglages
├── 🖨️  Imprimante            (choisir l'imprimante, tester)
├── 🧾  Ticket                (nom du magasin, téléphone, remerciement)
├── 👥  Vendeurs              (ajouter, retirer, code à 4–6 chiffres)
├── 🔒  Mot de passe          (protéger l'ordinateur)
├── 💾  Sauvegarde            (voir la dernière, restaurer, exporter)
└── ❓  Aide / Appeler Nour    (numéro, envoyer un problème)
```
Everything else — tax mode, valuation method, FEFO, currency, consumption policy, retention, decimals — is **set once by you during the pilot install**, stored, and hidden behind `Paramètres avancés` (owner password + a warning line: « Ces réglages changent vos calculs. »).

**Anti-pattern to avoid:** a settings page with 40 toggles is a support-ticket generator. Every toggle is a future phone call.

---

## 7. First run: **3 questions, then a sale**

```
1.  اللغة / Langue        [ العربية ]  [ Français ]
2.  Nom du magasin        [____________]        (optional, can be skipped)
3.  Vous facturez la TVA ?   ( ) Non   ( ) Oui, même taux   ( ) Oui, selon le produit
                                     [ Commencer ]
```
Then **immediately** a working till with 6 sample products and one highlighted hint: **« Scannez un produit ou tapez son nom »**.

No account. No email. No tour. No feature list. No "step 2 of 5". Nothing to sign up for. **The first thing that happens in this software is a successful sale.**

---

## 8. Errors, empty states, and the three-question test

Every message must pass: **(1) What happened? (2) What do I do now? (3) Who do I call?**

| Situation | ❌ Never | ✅ Yes |
|---|---|---|
| Printer offline | `ERR_PRINT_0x2F` | « **L'imprimante ne répond pas.** Vérifiez qu'elle est allumée. [ Réessayer ] [ Imprimer en PDF ] » |
| No internet | `sync failed: connection refused` | « **Pas d'internet.** Vos ventes sont enregistrées sur cet ordinateur. Elles partiront tout seules. » |
| Empty product list | `No results` | « Aucun produit. [ Ajouter le premier produit ] ou [ Importer un fichier Excel ] » |
| Wrong password | `Authentication failed` | « **Mot de passe incorrect.** [ Réessayer ] · Mot de passe oublié ? (code de secours) » |
| Unpaid subscription | `402 license_expired` | « **Votre abonnement doit être renouvelé.** Le vendeur peut continuer à travailler. [ J'ai payé, réactiver ] » |
| Scanner double-typing | silent | « Le code a été lu deux fois. Vérifiez la quantité. » |

Rules: no error codes visible to the shop (they go to the health bundle, not the screen) · never say "vous avez fait une erreur" · always offer an escape action · never a dead-end modal.

---

## 9. Touch, sight, and the physical shop

- **Default UI zoom 110–125 %**, font ≥ 16 px (POS labels 18 px, money 22–28 px, totals 32 px).
- Tap targets **≥ 44 px**, gaps ≥ 8 px (fat fingers, gloves, cracked screens).
- **Never colour alone** for meaning (red/green blindness, washed-out cheap panels): always + icon **and** word.
- **Never hover-only** and **never long-press-only** — tablets have neither.
- High contrast by default (DESIGN-001 measured ratios ≥ 4.5:1, money ≥ 7:1); dark mode for evening shops.
- No animation on the sale path; no sound except a short beep on a successful scan (that beep is how cashiers trust the scanner).
- Works on a 1366×768 screen with no scrolling for the 5 core screens (DESIGN-002 §2.1).

---

## 10. Support is part of the UI (biggest trust lever)

| Element | Why |
|---|---|
| **« Appeler Nour »** button in Aide, with your real phone number and a WhatsApp link | A shopkeeper who knows he can call a person will try the software. One who can't, won't |
| **« Envoyer un problème »** → attaches the health bundle (consented, PII-free, STRATEGY-002 S5) | Turns "it doesn't work" into an actionable report you can fix remotely |
| **Laminated A4 card** (Arabic + French), 6 pictures: *vendre, annuler une vente, prendre un din, recevoir une livraison, imprimer, appeler* | Many owners will never read a manual, but they will keep a card next to the till — and it's a **sales asset** in the demo |
| **"Erreur ? Annuler la dernière action"** always in the header for 5 minutes | Removes the fear of breaking something, which is the #1 reason people hesitate |

---

## 11. Testing protocol (non-negotiable before each install)

1. **The grandmother test:** a person who has never seen the app must complete *sell one product and take a debt* **without any instruction**. If they hesitate > 5 s, that's a bug.
2. **Measure:** time-to-first-sale (target **< 60 s** from first launch), hesitation points, errors per 20 sales (target **0**), and how many times they ask "what does this mean?" (target **0**).
3. **Watch, don't coach.** No pointing, no explaining. Write down every pause.
4. **5 shops minimum** before M5 (cloud). Their words become the labels.
5. Every complaint is categorised: *wording*, *missing action*, *layout*, *performance*, or *new feature* — and only *new feature* may be deferred.

---

## 12. The gate (added to DESIGN-002 §7)

A screen is not done until **all** of these are true:

- [ ] **Zero** words from the blacklist (§3)
- [ ] One primary action; ≤ 3 visible buttons; no icon-only button
- [ ] Every message passes the 3-question test (§8)
- [ ] Every field either has a default or is genuinely required
- [ ] Readable at 60 cm on a 1366×768 screen, no scrolling on the 5 core screens
- [ ] Works with a finger, a mouse, and a keyboard — no action requires two of them
- [ ] The Arabic version fits without shrinking the text
- [ ] **A non-technical person completed the task unaided** (§11)

---

## 13. Cost

| Item | Days |
|---|---|
| Copy pass in fr + ar (short labels, glossary, every message rewritten) | 2.0 |
| Settings consolidation (40 keys → 6 screens + hidden advanced) | 0.5 |
| Error/empty-state catalogue (~40 messages, 3-question test) | 1.0 |
| "Appeler / Envoyer un problème" wiring + health bundle attachment | 0.5 |
| Laminated support card (design + print files, ar/fr) | 0.5 |
| Usability test rounds (5 shops, 2 rounds) + fixes | 2.0 |
| **Total** | **6.5 d** |

**This is the best-spent 6.5 days in the entire plan.** Cheaper than one bad review in a small market.
