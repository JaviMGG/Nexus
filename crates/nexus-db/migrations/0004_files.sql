-- NEXUS: archivos adjuntos (imágenes, documentos) para Canvas, Database y Meals.
-- Los bytes viven en <data_dir>/files/<id>; aquí solo los metadatos.
CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mime TEXT NOT NULL DEFAULT 'application/octet-stream',
    size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
