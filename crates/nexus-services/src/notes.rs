use nexus_core::{new_id, CreateNote, Note, NoteFilter, Result, UpdateNote};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct NoteService {
    pub db: Db,
}

impl NoteService {
    pub async fn list(&self, f: NoteFilter) -> Result<Vec<Note>> {
        repos::notes::list(&self.db, &f).await
    }

    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<Note>> {
        repos::notes::list(
            &self.db,
            &NoteFilter {
                search: Some(query.to_string()),
                limit: Some(limit),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn get(&self, id: &str) -> Result<Note> {
        repos::notes::get(&self.db, id).await
    }

    pub async fn create(&self, mut data: CreateNote, actor: &str) -> Result<Note> {
        data.title = validate::title(&data.title, "el título")?;
        data.content = validate::long_text(&data.content, "el contenido", 200_000)?;
        data.tags = validate::tags(&data.tags)?;
        if let Some(u) = &data.url {
            data.url = Some(validate::url(u)?);
        }
        if let Some(p) = &data.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        let note = repos::notes::create(&self.db, data, new_id()).await?;
        repos::activity::record(
            &self.db,
            "created",
            "note",
            Some(&note.id),
            &format!("Nota creada: {}", note.title),
            actor,
            Some(serde_json::json!({"op": "delete", "entity": "note", "id": note.id})),
        )
        .await?;
        Ok(note)
    }

    pub async fn update(&self, id: &str, mut patch: UpdateNote, actor: &str) -> Result<Note> {
        let before = repos::notes::get(&self.db, id).await?;
        if let Some(t) = &patch.title {
            patch.title = Some(validate::title(t, "el título")?);
        }
        if let Some(c) = &patch.content {
            validate::long_text(c, "el contenido", 200_000)?;
        }
        if let Some(tags) = &patch.tags {
            patch.tags = Some(validate::tags(tags)?);
        }
        if let Some(Some(u)) = &patch.url {
            patch.url = Some(Some(validate::url(u)?));
        }
        if let Some(Some(p)) = &patch.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        let note = repos::notes::update(&self.db, id, patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "note",
            Some(id),
            &format!("Nota actualizada: {}", note.title),
            actor,
            Some(serde_json::json!({"op": "update", "entity": "note", "id": id, "before": before})),
        )
        .await?;
        Ok(note)
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let note = repos::notes::get(&self.db, id).await?;
        repos::notes::soft_delete(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "note",
            Some(id),
            &format!("Nota eliminada: {}", note.title),
            actor,
            Some(serde_json::json!({"op": "restore", "entity": "note", "id": id})),
        )
        .await?;
        Ok(())
    }

    pub async fn restore(&self, id: &str, actor: &str) -> Result<Note> {
        repos::notes::restore(&self.db, id).await?;
        let n = repos::notes::get(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "restored",
            "note",
            Some(id),
            &format!("Nota restaurada: {}", n.title),
            actor,
            None,
        )
        .await?;
        Ok(n)
    }
}
