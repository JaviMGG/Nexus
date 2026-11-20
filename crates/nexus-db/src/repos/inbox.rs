use nexus_core::{now_str, Id, InboxItem, InboxStatus, NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct InboxRow {
    id: String,
    content: String,
    status: String,
    processed_into: Option<String>,
    created_at: String,
}

impl From<InboxRow> for InboxItem {
    fn from(r: InboxRow) -> Self {
        Self {
            id: r.id,
            content: r.content,
            status: InboxStatus::parse(&r.status).unwrap_or(InboxStatus::Pending),
            processed_into: r.processed_into,
            created_at: r.created_at,
        }
    }
}

pub async fn list(db: &Db, status: Option<InboxStatus>) -> Result<Vec<InboxItem>> {
    let rows: Vec<InboxRow> = match status {
        Some(s) => {
            sqlx::query_as("SELECT * FROM inbox_items WHERE status = ? ORDER BY created_at DESC")
                .bind(s.as_str())
        }
        None => sqlx::query_as("SELECT * FROM inbox_items ORDER BY created_at DESC"),
    }
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(InboxItem::from).collect())
}

pub async fn get(db: &Db, id: &str) -> Result<InboxItem> {
    let row: Option<InboxRow> = sqlx::query_as("SELECT * FROM inbox_items WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(InboxItem::from)
        .ok_or_else(|| NexusError::not_found("elemento de inbox", id))
}

pub async fn create(db: &Db, content: &str, id: Id) -> Result<InboxItem> {
    sqlx::query(
        "INSERT INTO inbox_items (id, content, status, created_at) VALUES (?, ?, 'pending', ?)",
    )
    .bind(&id)
    .bind(content)
    .bind(now_str())
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &id).await
}

pub async fn set_status(
    db: &Db,
    id: &str,
    status: InboxStatus,
    processed_into: Option<&str>,
) -> Result<InboxItem> {
    sqlx::query("UPDATE inbox_items SET status = ?, processed_into = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(processed_into)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    get(db, id).await
}

pub async fn delete(db: &Db, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM inbox_items WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn count_pending(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM inbox_items WHERE status = 'pending'")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}
