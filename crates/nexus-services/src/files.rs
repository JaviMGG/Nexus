//! Archivos adjuntos compartidos (Canvas, Database, Meals).
//! Bytes en <data_dir>/files/<id> (permisos 0600), metadatos en SQLite.

use std::path::PathBuf;

use nexus_core::{new_id, NexusError, Result};
use nexus_db::{repos, Db};

pub const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone)]
pub struct FileService {
    pub db: Db,
    pub dir: PathBuf,
}

impl FileService {
    pub fn new(db: Db, data_dir: &std::path::Path) -> Self {
        Self {
            db,
            dir: data_dir.join("files"),
        }
    }

    fn path_for(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(NexusError::validation("id de archivo inválido"));
        }
        Ok(self.dir.join(id))
    }

    pub async fn save(
        &self,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<repos::files::FileMeta> {
        if name.trim().is_empty() || name.len() > 200 {
            return Err(NexusError::validation("nombre de archivo inválido"));
        }
        if bytes.is_empty() {
            return Err(NexusError::validation("el archivo está vacío"));
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err(NexusError::validation(
                "el archivo supera el máximo de 25 MB",
            ));
        }
        let id = new_id();
        std::fs::create_dir_all(&self.dir).map_err(|e| {
            NexusError::Internal(format!("no se pudo crear el directorio de archivos: {e}"))
        })?;
        let path = self.path_for(&id)?;
        std::fs::write(&path, bytes)
            .map_err(|e| NexusError::Internal(format!("no se pudo guardar el archivo: {e}")))?;
        restrict(&path);
        let meta =
            repos::files::create(&self.db, &id, name.trim(), mime, bytes.len() as i64).await?;
        Ok(meta)
    }

    pub async fn read(&self, id: &str) -> Result<(repos::files::FileMeta, Vec<u8>)> {
        let meta = repos::files::get(&self.db, id).await?;
        let bytes = std::fs::read(self.path_for(id)?)
            .map_err(|e| NexusError::Internal(format!("no se pudo leer el archivo: {e}")))?;
        Ok((meta, bytes))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        repos::files::get(&self.db, id).await?;
        let path = self.path_for(id)?;
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| NexusError::Internal(format!("no se pudo borrar el archivo: {e}")))?;
        }
        repos::files::delete(&self.db, id).await
    }

    pub async fn list(&self) -> Result<Vec<repos::files::FileMeta>> {
        repos::files::list(&self.db).await
    }
}

fn restrict(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}
