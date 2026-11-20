use nexus_core::{now_str, ActivityEvent, Id, NexusError, Result};

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct ActivityRow {
    id: String,
    kind: String,
    entity_type: String,
    entity_id: Option<String>,
    summary: String,
    actor: String,
    undo_json: Option<String>,
    created_at: String,
}

impl From<ActivityRow> for ActivityEvent {
    fn from(r: ActivityRow) -> Self {
        Self {
            id: r.id,
            kind: r.kind,
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            summary: r.summary,
            actor: r.actor,
            undo_json: r.undo_json.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: r.created_at,
        }
    }
}

pub async fn record(
    db: &Db,
    kind: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    summary: &str,
    actor: &str,
    undo_json: Option<serde_json::Value>,
) -> Result<ActivityEvent> {
    let id = nexus_core::new_id();
    sqlx::query(
        "INSERT INTO activity_events (id, kind, entity_type, entity_id, summary, actor, undo_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(kind)
    .bind(entity_type)
    .bind(entity_id)
    .bind(summary)
    .bind(actor)
    .bind(undo_json.map(|v| v.to_string()))
    .bind(now_str())
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &id).await
}

pub async fn get(db: &Db, id: &str) -> Result<ActivityEvent> {
    let row: Option<ActivityRow> = sqlx::query_as("SELECT * FROM activity_events WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(ActivityEvent::from)
        .ok_or_else(|| NexusError::not_found("evento", id))
}

pub async fn list(
    db: &Db,
    limit: i64,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
) -> Result<Vec<ActivityEvent>> {
    let rows: Vec<ActivityRow> = match (entity_type, entity_id) {
        (Some(t), Some(id)) => {
            sqlx::query_as("SELECT * FROM activity_events WHERE entity_type = ? AND entity_id = ? ORDER BY created_at DESC LIMIT ?")
                .bind(t)
                .bind(id)
                .bind(limit)
        }
        (Some(t), None) => {
            sqlx::query_as("SELECT * FROM activity_events WHERE entity_type = ? ORDER BY created_at DESC LIMIT ?").bind(t).bind(limit)
        }
        _ => sqlx::query_as("SELECT * FROM activity_events ORDER BY created_at DESC LIMIT ?").bind(limit),
    }
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(ActivityEvent::from).collect())
}

pub async fn clear_undo(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE activity_events SET undo_json = NULL WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub fn new_activity_id() -> Id {
    nexus_core::new_id()
}
