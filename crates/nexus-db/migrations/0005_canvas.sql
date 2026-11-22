-- NEXUS Canvas: pizarras infinitas. El documento (elementos, aristas y
-- viewport) se persiste como JSON versionado; los metadatos en columnas.
CREATE TABLE IF NOT EXISTS canvases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    document_json TEXT NOT NULL DEFAULT '{"version":1,"elements":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_canvases_updated ON canvases (updated_at DESC);
