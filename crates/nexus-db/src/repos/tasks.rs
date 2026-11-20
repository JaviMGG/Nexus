use nexus_core::{
    now_str, CreateTask, Id, NexusError, Priority, Result, Task, TaskFilter, TaskStatus, UpdateTask,
};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{Db, DbResultExt};

use super::{tags_from_json, tags_to_json};

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    due_date: Option<String>,
    project_id: Option<String>,
    parent_id: Option<String>,
    category: Option<String>,
    tags: String,
    position: i64,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

impl From<TaskRow> for Task {
    fn from(r: TaskRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            description: r.description,
            status: TaskStatus::parse(&r.status).unwrap_or_default(),
            priority: Priority::parse(&r.priority).unwrap_or_default(),
            due_date: r.due_date,
            project_id: r.project_id,
            parent_id: r.parent_id,
            category: r.category,
            tags: tags_from_json(&r.tags),
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
            completed_at: r.completed_at,
        }
    }
}

pub async fn list(db: &Db, f: &TaskFilter) -> Result<Vec<Task>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM tasks WHERE 1=1");
    if !f.include_deleted.unwrap_or(false) {
        qb.push(" AND deleted_at IS NULL");
    }
    if let Some(s) = &f.status {
        qb.push(" AND status = ").push_bind(s.as_str());
    }
    if let Some(p) = &f.project_id {
        qb.push(" AND project_id = ").push_bind(p);
    }
    if let Some(p) = &f.priority {
        qb.push(" AND priority = ").push_bind(p.as_str());
    }
    if let Some(s) = &f.search {
        let like = format!("%{s}%");
        qb.push(" AND (title LIKE ")
            .push_bind(like.clone())
            .push(" OR description LIKE ")
            .push_bind(like)
            .push(")");
    }
    if let Some(t) = &f.tag {
        qb.push(" AND tags LIKE ").push_bind(format!("%\"{t}\"%"));
    }
    if let Some(p) = &f.parent_id {
        qb.push(" AND parent_id = ").push_bind(p);
    }
    if f.root_only.unwrap_or(false) {
        qb.push(" AND parent_id IS NULL");
    }
    if let Some(d) = &f.due_before {
        qb.push(" AND due_date IS NOT NULL AND substr(due_date, 1, 10) <= ")
            .push_bind(d);
    }
    qb.push(" ORDER BY position ASC, created_at DESC");
    if let Some(l) = f.limit {
        qb.push(" LIMIT ").push_bind(l);
    }
    let rows: Vec<TaskRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(Task::from).collect())
}

pub async fn get(db: &Db, id: &str) -> Result<Task> {
    let row: Option<TaskRow> =
        sqlx::query_as("SELECT * FROM tasks WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Task::from)
        .ok_or_else(|| NexusError::not_found("tarea", id))
}

pub async fn create(db: &Db, data: CreateTask, id: Id) -> Result<Task> {
    let now = now_str();
    let status = data.status.unwrap_or_default();
    let completed_at = if status == TaskStatus::Done {
        Some(now.clone())
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO tasks (id, title, description, status, priority, due_date, project_id, parent_id, category, tags, position, created_at, updated_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&data.title)
    .bind(&data.description)
    .bind(status.as_str())
    .bind(data.priority.unwrap_or_default().as_str())
    .bind(&data.due_date)
    .bind(&data.project_id)
    .bind(&data.parent_id)
    .bind(&data.category)
    .bind(tags_to_json(&data.tags))
    .bind(next_position(db, data.parent_id.as_deref()).await?)
    .bind(&now)
    .bind(&now)
    .bind(&completed_at)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &id).await
}

async fn next_position(db: &Db, parent_id: Option<&str>) -> Result<i64> {
    let row = match parent_id {
        Some(p) => sqlx::query(
            "SELECT COALESCE(MAX(position), -1) + 1 AS pos FROM tasks WHERE parent_id = ?",
        )
        .bind(p),
        None => sqlx::query(
            "SELECT COALESCE(MAX(position), -1) + 1 AS pos FROM tasks WHERE parent_id IS NULL",
        ),
    }
    .fetch_one(&db.pool)
    .await
    .db()?;
    row.try_get::<i64, _>("pos").db()
}

pub async fn update(db: &Db, id: &str, patch: UpdateTask) -> Result<Task> {
    let mut task = get(db, id).await?;
    if let Some(v) = patch.title {
        task.title = v;
    }
    if let Some(v) = patch.description {
        task.description = v;
    }
    if let Some(v) = patch.status {
        if v == TaskStatus::Done && task.status != TaskStatus::Done {
            task.completed_at = Some(now_str());
        }
        if v != TaskStatus::Done {
            task.completed_at = None;
        }
        task.status = v;
    }
    if let Some(v) = patch.priority {
        task.priority = v;
    }
    if let Some(v) = patch.due_date {
        task.due_date = v;
    }
    if let Some(v) = patch.project_id {
        task.project_id = v;
    }
    if let Some(v) = patch.parent_id {
        if v.as_deref() == Some(id) {
            return Err(NexusError::validation(
                "una tarea no puede ser su propia subtarea",
            ));
        }
        task.parent_id = v;
    }
    if let Some(v) = patch.category {
        task.category = v;
    }
    if let Some(v) = patch.tags {
        task.tags = v;
    }
    if let Some(v) = patch.position {
        task.position = v;
    }
    task.updated_at = now_str();
    write(db, &task).await?;
    Ok(task)
}

pub async fn write(db: &Db, t: &Task) -> Result<()> {
    sqlx::query(
        "UPDATE tasks SET title=?, description=?, status=?, priority=?, due_date=?, project_id=?, parent_id=?, category=?, tags=?, position=?, updated_at=?, completed_at=? WHERE id=?",
    )
    .bind(&t.title)
    .bind(&t.description)
    .bind(t.status.as_str())
    .bind(t.priority.as_str())
    .bind(&t.due_date)
    .bind(&t.project_id)
    .bind(&t.parent_id)
    .bind(&t.category)
    .bind(tags_to_json(&t.tags))
    .bind(t.position)
    .bind(&t.updated_at)
    .bind(&t.completed_at)
    .bind(&t.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE tasks SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn restore(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE tasks SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn get_including_deleted(db: &Db, id: &str) -> Result<Task> {
    let row: Option<TaskRow> = sqlx::query_as("SELECT * FROM tasks WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(Task::from)
        .ok_or_else(|| NexusError::not_found("tarea", id))
}

pub async fn count_open(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM tasks WHERE deleted_at IS NULL AND status IN ('todo','in_progress')")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}

pub async fn count_completed_since(db: &Db, since: &str) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM tasks WHERE deleted_at IS NULL AND status='done' AND completed_at >= ?")
        .bind(since)
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}
