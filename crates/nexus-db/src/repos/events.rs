use nexus_core::{
    now_str, CalendarEvent, CreateEvent, EventColor, EventFilter, Id, NexusError, Result,
    UpdateEvent,
};
use sqlx::QueryBuilder;

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    title: String,
    description: String,
    date: String,
    start_time: Option<String>,
    end_time: Option<String>,
    all_day: i64,
    color: String,
    location: String,
    project_id: Option<String>,
    task_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<EventRow> for CalendarEvent {
    fn from(r: EventRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            description: r.description,
            date: r.date,
            start_time: r.start_time,
            end_time: r.end_time,
            all_day: r.all_day != 0,
            color: EventColor::parse(&r.color).unwrap_or_default(),
            location: r.location,
            project_id: r.project_id,
            task_id: r.task_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list(db: &Db, f: &EventFilter) -> Result<Vec<CalendarEvent>> {
    let mut qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new("SELECT * FROM events WHERE 1=1");
    if !f.include_deleted.unwrap_or(false) {
        qb.push(" AND deleted_at IS NULL");
    }
    if let Some(from) = &f.from {
        qb.push(" AND date >= ").push_bind(from);
    }
    if let Some(to) = &f.to {
        qb.push(" AND date <= ").push_bind(to);
    }
    if let Some(p) = &f.project_id {
        qb.push(" AND project_id = ").push_bind(p);
    }
    qb.push(" ORDER BY date ASC, start_time IS NULL DESC, start_time ASC");
    let rows: Vec<EventRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(CalendarEvent::from).collect())
}

pub async fn get(db: &Db, id: &str) -> Result<CalendarEvent> {
    let row: Option<EventRow> =
        sqlx::query_as("SELECT * FROM events WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(CalendarEvent::from)
        .ok_or_else(|| NexusError::not_found("evento", id))
}

pub async fn create(db: &Db, data: CreateEvent, id: Id) -> Result<CalendarEvent> {
    let now = now_str();
    sqlx::query(
        "INSERT INTO events (id, title, description, date, start_time, end_time, all_day, color, location, project_id, task_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&data.title)
    .bind(&data.description)
    .bind(&data.date)
    .bind(&data.start_time)
    .bind(&data.end_time)
    .bind(data.all_day)
    .bind(data.color.unwrap_or_default().as_str())
    .bind(&data.location)
    .bind(&data.project_id)
    .bind(&data.task_id)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &id).await
}

pub async fn update(db: &Db, id: &str, patch: UpdateEvent) -> Result<CalendarEvent> {
    let mut event = get(db, id).await?;
    if let Some(v) = patch.title {
        event.title = v;
    }
    if let Some(v) = patch.description {
        event.description = v;
    }
    if let Some(v) = patch.date {
        event.date = v;
    }
    if let Some(v) = patch.start_time {
        event.start_time = v;
    }
    if let Some(v) = patch.end_time {
        event.end_time = v;
    }
    if let Some(v) = patch.all_day {
        event.all_day = v;
        if v {
            event.start_time = None;
            event.end_time = None;
        }
    }
    if let Some(v) = patch.color {
        event.color = v;
    }
    if let Some(v) = patch.location {
        event.location = v.unwrap_or_default();
    }
    if let Some(v) = patch.project_id {
        event.project_id = v;
    }
    if let Some(v) = patch.task_id {
        event.task_id = v;
    }
    event.updated_at = now_str();
    write(db, &event).await?;
    Ok(event)
}

pub async fn write(db: &Db, e: &CalendarEvent) -> Result<()> {
    sqlx::query(
        "UPDATE events SET title=?, description=?, date=?, start_time=?, end_time=?, all_day=?, color=?, location=?, project_id=?, task_id=?, updated_at=? WHERE id=?",
    )
    .bind(&e.title)
    .bind(&e.description)
    .bind(&e.date)
    .bind(&e.start_time)
    .bind(&e.end_time)
    .bind(e.all_day)
    .bind(e.color.as_str())
    .bind(&e.location)
    .bind(&e.project_id)
    .bind(&e.task_id)
    .bind(&e.updated_at)
    .bind(&e.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE events SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn restore(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE events SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn get_including_deleted(db: &Db, id: &str) -> Result<CalendarEvent> {
    let row: Option<EventRow> = sqlx::query_as("SELECT * FROM events WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(CalendarEvent::from)
        .ok_or_else(|| NexusError::not_found("evento", id))
}
