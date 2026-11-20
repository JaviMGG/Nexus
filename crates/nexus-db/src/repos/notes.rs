use nexus_core::{
    now_str, CreateNote, Id, NexusError, Note, NoteFilter, NoteKind, Result, UpdateNote,
};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{Db, DbResultExt};

use super::{tags_from_json, tags_to_json};

#[derive(sqlx::FromRow)]
struct NoteRow {
    id: String,
    title: String,
    content: String,
    kind: String,
    url: Option<String>,
    project_id: Option<String>,
    tags: String,
    pinned: i64,
    created_at: String,
    updated_at: String,
}

impl From<NoteRow> for Note {
    fn from(r: NoteRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            content: r.content,
            kind: NoteKind::parse(&r.kind).unwrap_or_default(),
            url: r.url,
            project_id: r.project_id,
            tags: tags_from_json(&r.tags),
            pinned: r.pinned != 0,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list(db: &Db, f: &NoteFilter) -> Result<Vec<Note>> {
    if let Some(q) = f.search.as_deref().filter(|q| !q.trim().is_empty()) {
        let ids = fts_search(db, q, f.limit.unwrap_or(50)).await?;
        let mut notes = Vec::new();
        for id in ids {
            if let Ok(n) = get(db, &id).await {
                if let Some(k) = &f.kind {
                    if &n.kind != k {
                        continue;
                    }
                }
                if let Some(p) = &f.project_id {
                    if n.project_id.as_ref() != Some(p) {
                        continue;
                    }
                }
                if let Some(t) = &f.tag {
                    if !n.tags.contains(t) {
                        continue;
                    }
                }
                notes.push(n);
            }
        }
        return Ok(notes);
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM notes WHERE deleted_at IS NULL");
    if let Some(k) = &f.kind {
        qb.push(" AND kind = ").push_bind(k.as_str());
    }
    if let Some(p) = &f.project_id {
        qb.push(" AND project_id = ").push_bind(p);
    }
    if let Some(t) = &f.tag {
        qb.push(" AND tags LIKE ").push_bind(format!("%\"{t}\"%"));
    }
    qb.push(" ORDER BY pinned DESC, updated_at DESC");
    if let Some(l) = f.limit {
        qb.push(" LIMIT ").push_bind(l);
    }
    let rows: Vec<NoteRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(Note::from).collect())
}

/// Convierte texto libre en una consulta FTS5 segura (tokens entrecomillados con prefijo).
pub fn sanitize_fts_query(q: &str) -> String {
    q.split_whitespace()
        .map(|tok| {
            let clean: String = tok
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            format!("\"{clean}\"*")
        })
        .filter(|t| t != "\"\"*")
        .collect::<Vec<_>>()
        .join(" AND ")
}

async fn fts_search(db: &Db, q: &str, limit: i64) -> Result<Vec<String>> {
    let match_expr = sanitize_fts_query(q);
    if match_expr.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query(
        "SELECT note_id FROM notes_fts WHERE notes_fts MATCH ? ORDER BY bm25(notes_fts) LIMIT ?",
    )
    .bind(&match_expr)
    .bind(limit)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.try_get::<String, _>("note_id").ok())
        .collect())
}

pub async fn get(db: &Db, id: &str) -> Result<Note> {
    let row: Option<NoteRow> =
        sqlx::query_as("SELECT * FROM notes WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Note::from)
        .ok_or_else(|| NexusError::not_found("nota", id))
}

pub async fn create(db: &Db, data: CreateNote, id: Id) -> Result<Note> {
    let now = now_str();
    let mut tx = db.pool.begin().await.db()?;
    sqlx::query(
        "INSERT INTO notes (id, title, content, kind, url, project_id, tags, pinned, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&data.title)
    .bind(&data.content)
    .bind(data.kind.unwrap_or_default().as_str())
    .bind(&data.url)
    .bind(&data.project_id)
    .bind(tags_to_json(&data.tags))
    .bind(data.pinned.unwrap_or(false) as i64)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .db()?;
    fts_upsert(&mut tx, &id, &data.title, &data.content).await?;
    tx.commit().await.db()?;
    get(db, &id).await
}

pub async fn update(db: &Db, id: &str, patch: UpdateNote) -> Result<Note> {
    let mut n = get(db, id).await?;
    if let Some(v) = patch.title {
        n.title = v;
    }
    if let Some(v) = patch.content {
        n.content = v;
    }
    if let Some(v) = patch.kind {
        n.kind = v;
    }
    if let Some(v) = patch.url {
        n.url = v;
    }
    if let Some(v) = patch.project_id {
        n.project_id = v;
    }
    if let Some(v) = patch.tags {
        n.tags = v;
    }
    if let Some(v) = patch.pinned {
        n.pinned = v;
    }
    n.updated_at = now_str();
    let mut tx = db.pool.begin().await.db()?;
    sqlx::query("UPDATE notes SET title=?, content=?, kind=?, url=?, project_id=?, tags=?, pinned=?, updated_at=? WHERE id=?")
        .bind(&n.title)
        .bind(&n.content)
        .bind(n.kind.as_str())
        .bind(&n.url)
        .bind(&n.project_id)
        .bind(tags_to_json(&n.tags))
        .bind(n.pinned as i64)
        .bind(&n.updated_at)
        .bind(&n.id)
        .execute(&mut *tx)
        .await
        .db()?;
    fts_upsert(&mut tx, &n.id, &n.title, &n.content).await?;
    tx.commit().await.db()?;
    Ok(n)
}

pub async fn write(db: &Db, n: &Note) -> Result<()> {
    let mut tx = db.pool.begin().await.db()?;
    sqlx::query("UPDATE notes SET title=?, content=?, kind=?, url=?, project_id=?, tags=?, pinned=?, updated_at=? WHERE id=?")
        .bind(&n.title)
        .bind(&n.content)
        .bind(n.kind.as_str())
        .bind(&n.url)
        .bind(&n.project_id)
        .bind(tags_to_json(&n.tags))
        .bind(n.pinned as i64)
        .bind(&n.updated_at)
        .bind(&n.id)
        .execute(&mut *tx)
        .await
        .db()?;
    fts_upsert(&mut tx, &n.id, &n.title, &n.content).await?;
    tx.commit().await.db()?;
    Ok(())
}

async fn fts_upsert(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
    title: &str,
    content: &str,
) -> Result<()> {
    sqlx::query("DELETE FROM notes_fts WHERE note_id = ?")
        .bind(id)
        .execute(&mut **tx)
        .await
        .db()?;
    sqlx::query("INSERT INTO notes_fts (note_id, title, content) VALUES (?, ?, ?)")
        .bind(id)
        .bind(title)
        .bind(content)
        .execute(&mut **tx)
        .await
        .db()?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    let mut tx = db.pool.begin().await.db()?;
    sqlx::query("UPDATE notes SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&mut *tx)
        .await
        .db()?;
    sqlx::query("DELETE FROM notes_fts WHERE note_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .db()?;
    tx.commit().await.db()?;
    Ok(())
}

pub async fn restore(db: &Db, id: &str) -> Result<()> {
    let n = get_including_deleted(db, id).await?;
    let mut tx = db.pool.begin().await.db()?;
    sqlx::query("UPDATE notes SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .db()?;
    fts_upsert(&mut tx, &n.id, &n.title, &n.content).await?;
    tx.commit().await.db()?;
    Ok(())
}

pub async fn get_including_deleted(db: &Db, id: &str) -> Result<Note> {
    let row: Option<NoteRow> = sqlx::query_as("SELECT * FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(Note::from)
        .ok_or_else(|| NexusError::not_found("nota", id))
}

pub async fn count(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM notes WHERE deleted_at IS NULL")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}

pub async fn all_for_export(db: &Db) -> Result<Vec<Note>> {
    let rows: Vec<NoteRow> = sqlx::query_as("SELECT * FROM notes ORDER BY created_at")
        .fetch_all(&db.pool)
        .await
        .db()?;
    Ok(rows.into_iter().map(Note::from).collect())
}
