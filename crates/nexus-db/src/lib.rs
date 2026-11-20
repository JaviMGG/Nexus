//! nexus-db: capa de persistencia del monolito (SQLite + SQLx).
//! Migraciones versionadas y repositorios por entidad.

pub mod repos;

use std::path::Path;
use std::str::FromStr;

use nexus_core::{NexusError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                NexusError::Internal(format!("no se pudo crear el directorio de datos: {e}"))
            })?;
        }
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(db_err)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .map_err(db_err)?;
        let db = Self { pool };
        db.migrate().await?;
        restrict_permissions(path);
        Ok(db)
    }

    /// Base de datos en memoria para tests (una sola conexión para compartir estado).
    pub async fn connect_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(db_err)?
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .map_err(db_err)?;
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}

fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

pub fn db_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::Database(e.to_string())
}

/// Extensión para convertir cualquier error de infraestructura en error de dominio.
pub trait DbResultExt<T> {
    fn db(self) -> Result<T>;
}

impl<T, E: std::fmt::Display> DbResultExt<T> for std::result::Result<T, E> {
    fn db(self) -> Result<T> {
        self.map_err(db_err)
    }
}
