# ADR-012 — Local image upload (product photos, ID photos, scans)

**Status:** ✅ **ACCEPTED** — automatic centre-crop in v1; **HEIC supported** (iPhone photos are the common case); 2 GB quota default.
**Date:** 2026-10-20
**Relates to:** SEC-001 (encryption at rest), ADR-006 (phones, trade-in), DESIGN-001 §5 (component inventory)

---

## 1. What is being added

A shop can **pick an image file from the PC** (or drag & drop, or paste) and attach it to:
- a **product** (catalog thumb, grid card)
- a **variant** (colour/storage)
- a **unit** (IMEI-specific photo: condition evidence, scratches)
- a **trade-in / dealer purchase** (device photo + ID document photo — ADR-006 §4.5)
- a **SAV ticket** (damage photo)
- a **delivery** (supplier invoice scan, if the shop has one)

Everything stays **on the shop's machine**. Images never sync (ADR-003), never touch Groq, never leave the PC.

---

## 2. The upload path

```
Browser (local app)
  [ Choisir une image ]  or  drag&drop  or  Ctrl+V
        │  POST /api/media (multipart, ≤ 10 MB)
        ▼
store.exe  ── validation ──► processing ──► encrypted storage ──► product row
              magic-bytes     resize ×2       AES-GCM (SEC-001)
              type check      strip EXIF      data/images/<hash>.bin
              size limit      re-encode WebP  index in `media` table
```

**Rules that prevent real problems:**

| Risk | Handling |
|---|---|
| Fake extension (`.jpg` that's a .exe/HTML) | **Never trust the MIME type or extension.** Sniff **magic bytes**; accept only JPEG/PNG/WebP/HEIC-by-decoder; anything else → `415` |
| Huge files / decompression bombs | Hard cap 10 MB upload; decode with a **pixel-count limit** (e.g. 40 MP) before allocating buffers |
| Path traversal | Never use the client filename for storage. Name = `sha256(content)` → `<hash>.bin`; original name kept only as metadata (sanitized, display-only) |
| **Privacy leak in EXIF** | **Strip all metadata**, including GPS, camera serial, and owner name. Trade-in ID photos must never carry the *photographer's* location. This is non-negotiable |
| Colours shifting / huge storage | Re-encode to **WebP**: `thumb 240 px` (grid) + `full 1024 px` (detail), quality 80. A 4 MB phone photo becomes ~40–90 KB + ~250 KB |
| Duplicate uploads | Content hash → same image reused, zero extra disk |
| Slow disk on import of 500 products | Background queue with progress; batch import zips supported |
| Backups ballooning | Images are included in the encrypted nightly backup, but **excluded from the DB file** (separate `data/images/`) so the DB stays small and fast |

---

## 3. Storage & encryption

```sql
media(
  id TEXT PRIMARY KEY,              -- uuidv7
  hash TEXT NOT NULL UNIQUE,        -- sha256 of original bytes
  bytes_full INTEGER NOT NULL,
  bytes_thumb INTEGER NOT NULL,
  width, height INTEGER NOT NULL,
  mime_in TEXT NOT NULL,            -- what arrived
  original_name TEXT NULL,          -- sanitized, display only
  created_at TEXT NOT NULL,
  created_by TEXT NOT NULL,         -- attribution (ADR-011)
  encrypted INTEGER NOT NULL DEFAULT 1
)
-- polymorphic attach, so one image can serve product/variant/unit/ticket
media_links(media_id, entity, entity_id, role, sort_order, PRIMARY KEY(media_id, entity, entity_id, role))
```

- **Encrypted with AES-GCM** using a key derived from `K_db` (SEC-001 §4.3). Filenames are opaque hashes → an attacker with the folder learns *nothing*, not even "this is an ID card".
- Thumb and full are separate files → the grid never decodes a 1024 px image.
- Deletion = soft (ADR-011): row marked deleted, blob kept until a **purge job** runs (weekly, owner-triggered, audited) and only if no other `media_links` reference it.
- Quota: default **2 GB** per shop with a warning at 80%; `--doctor` reports usage. Typical shop: 3 000 products × 300 KB ≈ 900 MB.

---

## 4. Processing

- Server-side with the Rust `image` crate (+ `webp` encoder): decode → **auto-orient by EXIF** (then drop EXIF) → resize (Lanczos) → encode WebP.
- Two derivatives only (thumb + full). No on-the-fly resizing on request → predictable latency.
- **HEIC/HEIF** (iPhone photos) is common: support via `libheif` if the static build allows; otherwise reject with a clear message *"convertissez en JPEG"* and a one-line PowerToys/Photos tip. Decide in M1 — must not be a silent failure.
- Transparency (PNG) flattened onto `--surface` white so dark-mode cards don't look broken.
- Animated GIF: first frame only (predictable size).
- Failure is graceful: upload fails → product keeps working without a photo, error toast, no partial DB state.

---

## 5. UI (DESIGN-002 rules apply)

- **Product form:** one image well, 1 primary action. `[ Choisir ]` · drag & drop · `Ctrl+V` paste · `[ Supprimer ]`.
- **Grid card:** 1:1 fixed-aspect box (no layout shift), placeholder = category-coloured initial tile, `loading="lazy"`, `decoding="async"`.
- **Unit photo (phones):** up to 4 photos with a small "condition evidence" label; useful for trade-in disputes and SAV.
- **Trade-in:** device photos + ID photo, clearly labelled **"reste sur cet ordinateur"** — so nobody assumes it went to the cloud.
- Cropping: a simple square crop/zoom for the catalog thumb only (v1.1 if time is short); v1 = centre-crop automatic.
- **No compression questions, no format pickers, no advanced options** — the cashier picks a file, it works.

---

## 6. Where images are allowed to go (privacy contract)

| Destination | Allowed |
|---|---|
| Local `data/images/` (encrypted) | ✅ always |
| Local backup file (encrypted) | ✅ |
| Excel/PDF export | ⚠️ only if the user includes images explicitly (off by default) |
| Cloud dashboard / Neon | ❌ **never** |
| Groq / any AI provider | ❌ **never** |
| Sync payload | ❌ **never** |

This is worth stating in the app: *« Les photos ne quittent jamais cet ordinateur. »*

---

## 7. Effort

| Item | Days |
|---|---|
| `media` + `media_links` tables, AES-GCM blob store, hash dedupe, purge job | 1.0 |
| Upload endpoint (magic-byte validation, limits, background queue, progress) | 1.0 |
| Processing pipeline (auto-orient, EXIF strip, WebP thumbs/full, HEIC decision) | 0.75 |
| UI: product well, drag&drop, paste, unit gallery, trade-in/SAV attach, placeholders | 0.75 |
| Quota, `--doctor` usage report, backup inclusion + restore test | 0.5 |
| **Total** | **4.0 d** |

**Placement:** M1 (catalog) for products; M3 for trade-in/SAV attachments.

---

## 8. Decisions

1. **HEIC support** — add `libheif` and accept iPhone photos natively (heavier static build, +0.5 d), or reject with a clear message?
2. **Quota default** — 2 GB, or unlimited with only a warning?
3. **Crop/zoom UI** in v1, or automatic centre-crop only (recommended)?
4. Do you want photos on the **printed receipt** (thermal) at all? (Requires raster path — same as Arabic receipts.)

## Implémentation (v1.0.4)

- `products.photo` contient une référence courte, jamais un chemin :
  `d:<clé>` (dessin embarqué) ou `f:<fichier>` (photo du magasin).
- Les fichiers vont dans `<data>/photos/p_<id>.<ext>` : déplacer le dossier de
  données ne casse aucune photo, et la base reste légère.
- Le format est reconnu par les **premiers octets** (JPEG/PNG/WebP), pas par le
  nom du fichier : une photo venue d'un téléphone s'appelle `IMG_0042`.
- Rien n'est envoyé sur le réseau. La route `/photos/{nom}` refuse tout nom qui
  sort du dossier (pas de `..`, pas de séparateur).
- Les huit dessins d'accessoires (téléphone, chargeur, câble, coque, écouteurs,
  casque, batterie externe, verre trempé) pèsent 41 Ko au total et sont gravés
  dans `store.exe` : le magasin a une image correcte dès le premier jour.
