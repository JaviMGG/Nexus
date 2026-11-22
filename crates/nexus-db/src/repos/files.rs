use nexus_core::{now_str, Id, NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct FileMeta {
    pub id: Id,
    pub name: String,
    pub mime: String,
    pub size: i64,
    pub created_at: String,
}

pub async fn create(db: &Db, id: &str, name: &str, mime: &str, size: i64) -> Result<FileMeta> {
    let now = now_str();
    sqlx::query("INSERT INTO files (id, name, mime, size, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(mime)
        .bind(size)
        .bind(&now)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(FileMeta {
        id: id.to_string(),
        name: name.to_string(),
        mime: mime.to_string(),
        size,
        created_at: now,
    })
}

pub async fn get(db: &Db, id: &str) -> Result<FileMeta> {
    let row: Option<FileMeta> = sqlx::query_as("SELECT * FROM files WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.ok_or_else(|| NexusError::not_found("archivo", id))
}

pub async fn list(db: &Db) -> Result<Vec<FileMeta>> {
    let rows: Vec<FileMeta> = sqlx::query_as("SELECT * FROM files ORDER BY created_at DESC")
        .fetch_all(&db.pool)
        .await
        .db()?;
    Ok(rows)
}

pub async fn delete(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("archivo", id));
    }
    Ok(())
}

pub async fn count(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM files")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}
