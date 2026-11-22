-- NEXUS Database: bases de datos personalizadas con campos dinámicos,
-- registros con valores JSON, vistas guardadas y relaciones entre bases.
--
-- Decisiones de esquema:
-- * Soft-delete (deleted_at) en databases, db_fields y db_records: borrar un
--   campo NO destruye los valores guardados en values_json (se ignoran al leer
--   y vuelven si el campo se restaura).
-- * Los valores de cada registro viven en un único objeto JSON
--   field_id -> valor; los campos fórmula NO se almacenan (se calculan al leer).
-- * Las relaciones guardan arrays de ids de registro; al borrar un registro se
--   limpian las referencias y, además, los ids huérfanos se filtran al leer.
-- * Las vistas se borran físicamente al eliminar su base (no contienen datos).

CREATE TABLE IF NOT EXISTS databases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    icon TEXT NOT NULL DEFAULT 'database',
    color TEXT NOT NULL DEFAULT 'blue',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS db_fields (
    id TEXT PRIMARY KEY,
    database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_db_fields_database ON db_fields(database_id, position);

CREATE TABLE IF NOT EXISTS db_records (
    id TEXT PRIMARY KEY,
    database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    values_json TEXT NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_db_records_database ON db_records(database_id, position);

CREATE TABLE IF NOT EXISTS db_views (
    id TEXT PRIMARY KEY,
    database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_db_views_database ON db_views(database_id, position);
