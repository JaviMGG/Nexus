use nexus_core::{new_id, CreateNote, CreateTask, Id, InboxItem, InboxStatus, NexusError, Result};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct InboxService {
    pub db: Db,
}

impl InboxService {
    pub async fn list(&self, status: Option<InboxStatus>) -> Result<Vec<InboxItem>> {
        repos::inbox::list(&self.db, status).await
    }

    pub async fn create(&self, content: &str, actor: &str) -> Result<InboxItem> {
        let content = validate::long_text(content.trim(), "el contenido", 20_000)?;
        if content.is_empty() {
            return Err(NexusError::validation("el contenido no puede estar vacío"));
        }
        let item = repos::inbox::create(&self.db, &content, new_id()).await?;
        repos::activity::record(
            &self.db,
            "captured",
            "inbox",
            Some(&item.id),
            "Captura rápida añadida al inbox",
            actor,
            None,
        )
        .await?;
        Ok(item)
    }

    /// Convierte un elemento del inbox en tarea o nota.
    /// `data` puede traer campos estructurados (p. ej. sugeridos por la IA).
    pub async fn process(
        &self,
        id: &str,
        kind: &str,
        data: Option<serde_json::Value>,
        services: &crate::Services,
        actor: &str,
    ) -> Result<Id> {
        let item = repos::inbox::get(&self.db, id).await?;
        if item.status == InboxStatus::Processed {
            return Err(NexusError::Conflict("el elemento ya fue procesado".into()));
        }
        let created_id = match kind {
            "task" => {
                let mut task: CreateTask = match data {
                    Some(v) => serde_json::from_value(v).map_err(|e| {
                        NexusError::validation(format!("datos de tarea inválidos: {e}"))
                    })?,
                    None => CreateTask {
                        title: item.content.clone(),
                        ..Default::default()
                    },
                };
                if task.title.trim().is_empty() {
                    task.title = item.content.clone();
                }
                services.tasks.create(task, actor).await?.id
            }
            "note" => {
                let mut note: CreateNote = match data {
                    Some(v) => serde_json::from_value(v).map_err(|e| {
                        NexusError::validation(format!("datos de nota inválidos: {e}"))
                    })?,
                    None => CreateNote {
                        title: first_line(&item.content),
                        content: item.content.clone(),
                        ..Default::default()
                    },
                };
                if note.title.trim().is_empty() {
                    note.title = first_line(&item.content);
                }
                services.notes.create(note, actor).await?.id
            }
            other => {
                return Err(NexusError::validation(format!(
                    "tipo desconocido: {other} (usa task o note)"
                )))
            }
        };
        repos::inbox::set_status(&self.db, id, InboxStatus::Processed, Some(&created_id)).await?;
        Ok(created_id)
    }

    pub async fn archive(&self, id: &str) -> Result<InboxItem> {
        repos::inbox::get(&self.db, id).await?;
        repos::inbox::set_status(&self.db, id, InboxStatus::Archived, None).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        repos::inbox::get(&self.db, id).await?;
        repos::inbox::delete(&self.db, id).await
    }
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("Nota").trim();
    let mut t: String = line.chars().take(120).collect();
    if t.is_empty() {
        t = "Nota".into();
    }
    t
}
