//! Repositorio de NEXUS Canvas: metadatos en columnas, documento JSON versionado.

use nexus_core::{now_str, Canvas, CanvasDocument, CanvasSummary, Id, NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct CanvasRow {
    id: String,
    name: String,
    description: String,
    document_json: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<CanvasRow> for Canvas {
    type Error = NexusError;

    fn try_from(r: CanvasRow) -> Result<Self> {
        let document: CanvasDocument = serde_json::from_str(&r.document_json)
            .map_err(|e| NexusError::Internal(format!("documento corrupto en {}: {e}", r.id)))?;
        Ok(Self {
            id: r.id,
            name: r.name,
            description: r.description,
            document,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

fn json_string<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|e| NexusError::Internal(e.to_string()))
}

pub async fn create(db: &Db, c: &Canvas) -> Result<Canvas> {
    sqlx::query(
        "INSERT INTO canvases (id, name, description, document_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&c.id)
    .bind(&c.name)
    .bind(&c.description)
    .bind(json_string(&c.document)?)
    .bind(&c.created_at)
    .bind(&c.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &c.id).await
}

pub async fn get(db: &Db, id: &str) -> Result<Canvas> {
    let row: Option<CanvasRow> =
        sqlx::query_as("SELECT * FROM canvases WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Canvas::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("canvas", id))
}

pub async fn list(db: &Db) -> Result<Vec<CanvasSummary>> {
    let rows = sqlx::query(
        "SELECT id, name, description, document_json, created_at, updated_at
         FROM canvases WHERE deleted_at IS NULL ORDER BY updated_at DESC",
    )
    .fetch_all(&db.pool)
    .await
    .db()?;
    rows.into_iter()
        .map(|r| {
            let doc: CanvasDocument =
                serde_json::from_str(&r.try_get::<String, _>("document_json").db()?)
                    .map_err(|e| NexusError::Internal(format!("documento corrupto: {e}")))?;
            Ok(CanvasSummary {
                id: r.try_get("id").db()?,
                name: r.try_get("name").db()?,
                description: r.try_get("description").db()?,
                element_count: doc.elements.len() as i64,
                edge_count: doc.edges.len() as i64,
                created_at: r.try_get("created_at").db()?,
                updated_at: r.try_get("updated_at").db()?,
            })
        })
        .collect()
}

pub async fn update_meta(
    db: &Db,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<Canvas> {
    let mut c = get(db, id).await?;
    if let Some(n) = name {
        c.name = n.to_string();
    }
    if let Some(d) = description {
        c.description = d.to_string();
    }
    c.updated_at = now_str();
    sqlx::query("UPDATE canvases SET name=?, description=?, updated_at=? WHERE id=?")
        .bind(&c.name)
        .bind(&c.description)
        .bind(&c.updated_at)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(c)
}

pub async fn save_document(db: &Db, id: &str, doc: &CanvasDocument) -> Result<Canvas> {
    get(db, id).await?;
    let now = now_str();
    sqlx::query(
        "UPDATE canvases SET document_json=?, updated_at=? WHERE id=? AND deleted_at IS NULL",
    )
    .bind(json_string(doc)?)
    .bind(&now)
    .bind(id)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, id).await
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("UPDATE canvases SET deleted_at=? WHERE id=? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("canvas", id));
    }
    Ok(())
}

pub async fn restore(db: &Db, id: &str) -> Result<()> {
    let res =
        sqlx::query("UPDATE canvases SET deleted_at=NULL WHERE id=? AND deleted_at IS NOT NULL")
            .bind(id)
            .execute(&db.pool)
            .await
            .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("canvas borrado", id));
    }
    Ok(())
}

pub async fn count(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM canvases WHERE deleted_at IS NULL")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}

/// Id nuevo garantizando unicidad frente a un conjunto existente.
pub fn unique_id(used: &mut std::collections::HashSet<Id>) -> Id {
    loop {
        let id = nexus_core::new_id();
        if used.insert(id.clone()) {
            return id;
        }
    }
}
