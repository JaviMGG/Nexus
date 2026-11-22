use std::path::{Path, PathBuf};

use chrono::Utc;
use nexus_core::{NexusError, Result};
use nexus_db::{repos, Db, DbResultExt};
use sqlx::Row;

#[derive(Clone)]
pub struct BackupService {
    pub db: Db,
}

impl BackupService {
    /// Exportación JSON completa (sin credenciales).
    pub async fn export_json(&self) -> Result<serde_json::Value> {
        let tasks = repos::tasks::list(
            &self.db,
            &nexus_core::TaskFilter {
                include_deleted: Some(true),
                ..Default::default()
            },
        )
        .await?;
        let projects = repos::projects::list(&self.db, true).await?;
        let notes = repos::notes::all_for_export(&self.db).await?;
        let inbox = repos::inbox::list(&self.db, None).await?;
        let events = repos::events::list(
            &self.db,
            &nexus_core::EventFilter {
                include_deleted: Some(true),
                ..Default::default()
            },
        )
        .await?;
        let conversations = repos::chat::list_conversations(&self.db).await?;
        let mut convs = Vec::new();
        for c in conversations {
            let messages = repos::chat::list_messages(&self.db, &c.id).await?;
            convs.push(serde_json::json!({"conversation": c, "messages": messages}));
        }
        let runs = repos::runs::list_runs(&self.db, 1000).await?;
        let activity = repos::activity::list(&self.db, 500, None, None).await?;
        let app = repos::settings::get(&self.db, "app").await?;

        // Módulos nuevos: Canvas, Database y Meal Planner (solo metadatos;
        // los bytes de los adjuntos viven en <data_dir>/files y se incluyen
        // en los backups SQLite completos, no en el JSON).
        let mut canvases = Vec::new();
        for c in repos::canvas::list(&self.db).await? {
            canvases.push(repos::canvas::get(&self.db, &c.id).await?);
        }
        let mut databases = Vec::new();
        for d in repos::databases::list_databases(&self.db).await? {
            let detail = crate::databases::DatabaseService {
                db: self.db.clone(),
            }
            .get(&d.database.id)
            .await?;
            let records = repos::databases::all_records(&self.db, &d.database.id).await?;
            databases.push(serde_json::json!({"database": detail, "records": records}));
        }
        let recipes = repos::meals::list_recipes(&self.db, &Default::default()).await?;
        let pantry = repos::meals::list_pantry(&self.db, &Default::default()).await?;
        let menu_templates = repos::meals::list_menu_templates(&self.db).await?;
        let mut shopping = Vec::new();
        for l in repos::meals::list_shopping_lists(&self.db).await? {
            let items = repos::meals::list_shopping_items(&self.db, &l.id).await?;
            shopping.push(serde_json::json!({"list": l, "items": items}));
        }
        let files = repos::files::list(&self.db).await?;
        let automations = repos::automations::list(&self.db, false).await?;

        Ok(serde_json::json!({
            "version": 2,
            "exported_at": Utc::now().to_rfc3339(),
            "tasks": tasks,
            "projects": projects,
            "notes": notes,
            "inbox": inbox,
            "events": events,
            "conversations": convs,
            "runs": runs,
            "activity": activity,
            "settings": app,
            "canvases": canvases,
            "databases": databases,
            "recipes": recipes,
            "pantry": pantry,
            "menu_templates": menu_templates,
            "shopping_lists": shopping,
            "files": files,
            "automations": automations,
        }))
    }

    /// Copia de seguridad consistente mediante VACUUM INTO.
    pub async fn create_backup(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).map_err(|e| {
            NexusError::Internal(format!("no se pudo crear el directorio de backups: {e}"))
        })?;
        let name = format!("nexus-{}.sqlite", Utc::now().format("%Y%m%d-%H%M%S"));
        let path = dir.join(name);
        let sql = format!(
            "VACUUM INTO '{}'",
            path.display().to_string().replace('\'', "''")
        );
        sqlx::query(&sql).execute(&self.db.pool).await.db()?;
        Ok(path)
    }

    pub fn list_backups(dir: &Path) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| NexusError::Internal(e.to_string()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|e| e == "sqlite").unwrap_or(false))
            .collect();
        out.sort();
        out.reverse();
        Ok(out)
    }

    /// Restaura una copia: reemplaza los datos actuales en una transacción.
    /// Las credenciales actuales se conservan (no viajan en los backups exportados).
    pub async fn restore(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(NexusError::not_found("backup", &path.display().to_string()));
        }
        let mut conn = self.db.pool.acquire().await.db()?;
        let attach = format!(
            "ATTACH DATABASE '{}' AS backup",
            path.display().to_string().replace('\'', "''")
        );
        sqlx::query(&attach).execute(&mut *conn).await.db()?;

        let result = async {
            sqlx::query("BEGIN").execute(&mut *conn).await.db()?;
            // Verificación mínima de esquema compatible.
            let check: Option<String> =
                sqlx::query("SELECT name FROM backup.sqlite_master WHERE type='table' AND name='tasks'")
                    .fetch_optional(&mut *conn)
                    .await
                    .db()?
                    .map(|r| r.get::<String, _>("name"));
            if check.is_none() {
                return Err(NexusError::validation("el archivo no parece una base de datos NEXUS"));
            }
            for table in [
                "chat_messages", "conversations", "tool_calls", "approvals", "agent_runs", "activity_events",
                "automation_step_logs", "automation_approvals", "automation_executions", "automation_fired_marks", "automations",
                "shopping_items", "shopping_lists", "meal_plan_entries", "menu_templates", "pantry_items", "recipes",
                "db_views", "db_records", "db_fields", "databases",
                "canvases",
                "inbox_items", "events", "notes_fts", "notes", "tasks", "projects",
                "files",
            ] {
                sqlx::query(&format!("DELETE FROM main.{table}")).execute(&mut *conn).await.db()?;
                // Backups antiguos pueden no tener tablas añadidas en migraciones posteriores.
                let exists = sqlx::query("SELECT name FROM backup.sqlite_master WHERE type='table' AND name=?")
                    .bind(table)
                    .fetch_optional(&mut *conn)
                    .await
                    .db()?;
                if exists.is_some() {
                    sqlx::query(&format!("INSERT INTO main.{table} SELECT * FROM backup.{table}")).execute(&mut *conn).await.db()?;
                }
            }
            sqlx::query("DELETE FROM main.settings WHERE key NOT LIKE 'secret:%'").execute(&mut *conn).await.db()?;
            sqlx::query("INSERT INTO main.settings SELECT * FROM backup.settings WHERE key NOT LIKE 'secret:%'")
                .execute(&mut *conn)
                .await
                .db()?;
            sqlx::query("COMMIT").execute(&mut *conn).await.db()?;
            Ok::<(), NexusError>(())
        }
        .await;

        if result.is_err() {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
        }
        let _ = sqlx::query("DETACH DATABASE backup")
            .execute(&mut *conn)
            .await;
        result
    }
}
