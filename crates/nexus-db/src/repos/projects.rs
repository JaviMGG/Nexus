use nexus_core::{
    now_str, CreateProject, Id, NexusError, Priority, Project, ProjectStatus, Result, UpdateProject,
};

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    description: String,
    status: String,
    priority: String,
    goal: String,
    created_at: String,
    updated_at: String,
}

impl From<ProjectRow> for Project {
    fn from(r: ProjectRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            status: ProjectStatus::parse(&r.status).unwrap_or_default(),
            priority: Priority::parse(&r.priority).unwrap_or_default(),
            goal: r.goal,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list(db: &Db, include_archived: bool) -> Result<Vec<Project>> {
    let sql = if include_archived {
        "SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY created_at DESC"
    } else {
        "SELECT * FROM projects WHERE deleted_at IS NULL AND status != 'archived' ORDER BY created_at DESC"
    };
    let rows: Vec<ProjectRow> = sqlx::query_as(sql).fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(Project::from).collect())
}

pub async fn get(db: &Db, id: &str) -> Result<Project> {
    let row: Option<ProjectRow> =
        sqlx::query_as("SELECT * FROM projects WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Project::from)
        .ok_or_else(|| NexusError::not_found("proyecto", id))
}

pub async fn create(db: &Db, data: CreateProject, id: Id) -> Result<Project> {
    let now = now_str();
    sqlx::query(
        "INSERT INTO projects (id, name, description, status, priority, goal, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&data.name)
    .bind(&data.description)
    .bind(data.status.unwrap_or_default().as_str())
    .bind(data.priority.unwrap_or_default().as_str())
    .bind(&data.goal)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &id).await
}

pub async fn update(db: &Db, id: &str, patch: UpdateProject) -> Result<Project> {
    let mut p = get(db, id).await?;
    if let Some(v) = patch.name {
        p.name = v;
    }
    if let Some(v) = patch.description {
        p.description = v;
    }
    if let Some(v) = patch.status {
        p.status = v;
    }
    if let Some(v) = patch.priority {
        p.priority = v;
    }
    if let Some(v) = patch.goal {
        p.goal = v;
    }
    p.updated_at = now_str();
    sqlx::query("UPDATE projects SET name=?, description=?, status=?, priority=?, goal=?, updated_at=? WHERE id=?")
        .bind(&p.name)
        .bind(&p.description)
        .bind(p.status.as_str())
        .bind(p.priority.as_str())
        .bind(&p.goal)
        .bind(&p.updated_at)
        .bind(&p.id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(p)
}

pub async fn write(db: &Db, p: &Project) -> Result<()> {
    sqlx::query("UPDATE projects SET name=?, description=?, status=?, priority=?, goal=?, updated_at=? WHERE id=?")
        .bind(&p.name)
        .bind(&p.description)
        .bind(p.status.as_str())
        .bind(p.priority.as_str())
        .bind(&p.goal)
        .bind(&p.updated_at)
        .bind(&p.id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE projects SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn restore(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE projects SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn get_including_deleted(db: &Db, id: &str) -> Result<Project> {
    let row: Option<ProjectRow> = sqlx::query_as("SELECT * FROM projects WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(Project::from)
        .ok_or_else(|| NexusError::not_found("proyecto", id))
}
