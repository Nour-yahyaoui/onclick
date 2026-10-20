-- ============================================================================
--  OneClick — schema v4
--  Photos produits. Aucun octet ne quitte la machine : le fichier est copié
--  dans <data>/photos et la base ne garde que son nom.
-- ============================================================================

PRAGMA foreign_keys = ON;

-- "d:charger"  -> one of the built-in drawings shipped inside store.exe
-- "f:p_ab12.jpg" -> a file uploaded by the shop, in <data>/photos
ALTER TABLE products ADD COLUMN photo TEXT;
ALTER TABLE products ADD COLUMN photo_at TEXT;

UPDATE schema_version SET version = 4;
