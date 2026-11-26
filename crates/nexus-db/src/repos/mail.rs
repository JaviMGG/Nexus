use nexus_core::{NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

pub async fn get_last_uid(db: &Db, account_id: &str) -> Result<i64> {
    let row = sqlx::query("SELECT last_uid FROM mail_state WHERE account_id = ?")
        .bind(account_id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    match row {
        Some(r) => r.try_get::<i64, _>("last_uid").db(),
        None => Ok(0),
    }
}

pub async fn set_last_uid(db: &Db, account_id: &str, last_uid: i64) -> Result<()> {
    if account_id.trim().is_empty() {
        return Err(NexusError::validation("falta el id de la cuenta de correo"));
    }
    sqlx::query(
        "INSERT INTO mail_state (account_id, last_uid) VALUES (?, ?)
         ON CONFLICT(account_id) DO UPDATE SET last_uid = MAX(last_uid, excluded.last_uid)",
    )
    .bind(account_id)
    .bind(last_uid)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn delete(db: &Db, account_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM mail_state WHERE account_id = ?")
        .bind(account_id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}
