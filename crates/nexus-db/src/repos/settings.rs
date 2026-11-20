use nexus_core::{NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

pub async fn get(db: &Db, key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    Ok(row.map(|r| r.get::<String, _>("value")))
}

pub async fn set(db: &Db, key: &str, value: &str) -> Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(value)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn delete(db: &Db, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn list_prefix(db: &Db, prefix: &str) -> Result<Vec<(String, String)>> {
    let rows = sqlx::query("SELECT key, value FROM settings WHERE key LIKE ? ORDER BY key")
        .bind(format!("{prefix}%"))
        .fetch_all(&db.pool)
        .await
        .db()?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
        .collect())
}

pub async fn get_json<T: serde::de::DeserializeOwned>(db: &Db, key: &str) -> Result<Option<T>> {
    match get(db, key).await? {
        Some(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| NexusError::Internal(format!("setting {key} corrupto: {e}"))),
        None => Ok(None),
    }
}

pub async fn set_json<T: serde::Serialize>(db: &Db, key: &str, value: &T) -> Result<()> {
    let s = serde_json::to_string(value).map_err(|e| NexusError::Internal(e.to_string()))?;
    set(db, key, &s).await
}
