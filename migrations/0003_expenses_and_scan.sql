-- ============================================================================
--  OneClick — schema v3
--  1. Charges fixes du magasin (loyer, eau, électricité, entretien…), chacune
--     avec SA périodicité : par mois, par trimestre ou par an.
--  2. Produits créés « au scan » : le magasin ne saisit pas son stock d'avance,
--     il vend d'abord et complète la fiche quand il a le temps.
-- ============================================================================

PRAGMA foreign_keys = ON;

-- ── Charges fixes ───────────────────────────────────────────────────────────
-- Un loyer se paie chaque mois, une assurance chaque trimestre, une patente
-- chaque année. On garde le montant TEL QU'IL EST PAYÉ + sa période, et on
-- calcule le « par mois » à l'affichage : aucun arrondi stocké, aucun mélange.
CREATE TABLE IF NOT EXISTS expenses (
    id               TEXT PRIMARY KEY,
    label            TEXT NOT NULL,                     -- « Loyer », « STEG »…
    kind             TEXT NOT NULL DEFAULT 'other',     -- rent|power|water|maintenance|telecom|other
    amount_millimes  INTEGER NOT NULL,                  -- ce qui sort de la caisse, une fois
    period           TEXT NOT NULL DEFAULT 'month',     -- month | quarter | year
    start_day        TEXT NOT NULL,                     -- YYYY-MM-DD
    note             TEXT,
    is_active        INTEGER NOT NULL DEFAULT 1,        -- 0 = arrêtée (jamais supprimée)
    stopped_day      TEXT,
    created_by       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    updated_by       TEXT
);
CREATE INDEX IF NOT EXISTS ix_expenses_active ON expenses(is_active);

-- ── Produits venus d'une vente, pas d'un inventaire ─────────────────────────
-- origin = 'scan' : la fiche est née d'un code-barres scanné en caisse.
-- Le stock n'est pas tenu (tracks_stock = 0) : on ne bloque jamais une vente.
ALTER TABLE products ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual';

CREATE INDEX IF NOT EXISTS ix_sale_lines_product ON sale_lines(product_id);

UPDATE schema_version SET version = 3;
