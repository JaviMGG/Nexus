use nexus_core::{
    new_id, CreateTask, NexusError, Result, Task, TaskDetail, TaskFilter, TaskStatus, UpdateTask,
};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct TaskService {
    pub db: Db,
}

impl TaskService {
    pub async fn list(&self, f: TaskFilter) -> Result<Vec<Task>> {
        repos::tasks::list(&self.db, &f).await
    }

    pub async fn get(&self, id: &str) -> Result<TaskDetail> {
        let task = repos::tasks::get(&self.db, id).await?;
        let subtasks = repos::tasks::list(
            &self.db,
            &TaskFilter {
                parent_id: Some(id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        Ok(TaskDetail { task, subtasks })
    }

    pub async fn create(&self, mut data: CreateTask, actor: &str) -> Result<Task> {
        data.title = validate::title(&data.title, "el título")?;
        data.description = validate::long_text(&data.description, "la descripción", 20_000)?;
        data.tags = validate::tags(&data.tags)?;
        if let Some(d) = &data.due_date {
            data.due_date = Some(validate::due_date(d)?);
        }
        if let Some(p) = &data.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        if let Some(p) = &data.parent_id {
            repos::tasks::get(&self.db, p).await?;
        }
        let task = repos::tasks::create(&self.db, data, new_id()).await?;
        repos::activity::record(
            &self.db,
            "created",
            "task",
            Some(&task.id),
            &format!("Tarea creada: {}", task.title),
            actor,
            Some(serde_json::json!({"op": "delete", "entity": "task", "id": task.id})),
        )
        .await?;
        Ok(task)
    }

    pub async fn update(&self, id: &str, mut patch: UpdateTask, actor: &str) -> Result<Task> {
        let before = repos::tasks::get(&self.db, id).await?;
        if let Some(t) = &patch.title {
            patch.title = Some(validate::title(t, "el título")?);
        }
        if let Some(d) = &patch.description {
            validate::long_text(d, "la descripción", 20_000)?;
        }
        if let Some(tags) = &patch.tags {
            patch.tags = Some(validate::tags(tags)?);
        }
        if let Some(Some(d)) = &patch.due_date {
            patch.due_date = Some(Some(validate::due_date(d)?));
        }
        if let Some(Some(p)) = &patch.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        if let Some(Some(p)) = &patch.parent_id {
            repos::tasks::get(&self.db, p).await?;
            self.check_cycle(id, p).await?;
        }
        let task = repos::tasks::update(&self.db, id, patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "task",
            Some(id),
            &format!("Tarea actualizada: {}", task.title),
            actor,
            Some(serde_json::json!({"op": "update", "entity": "task", "id": id, "before": before})),
        )
        .await?;
        Ok(task)
    }

    async fn check_cycle(&self, id: &str, new_parent: &str) -> Result<()> {
        let mut current = new_parent.to_string();
        for _ in 0..64 {
            if current == id {
                return Err(NexusError::validation("jerarquía de subtareas cíclica"));
            }
            match repos::tasks::get(&self.db, &current).await {
                Ok(t) => match t.parent_id {
                    Some(p) => current = p,
                    None => return Ok(()),
                },
                Err(_) => return Ok(()),
            }
        }
        Err(NexusError::validation(
            "jerarquía de subtareas demasiado profunda",
        ))
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let task = repos::tasks::get(&self.db, id).await?;
        repos::tasks::soft_delete(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "task",
            Some(id),
            &format!("Tarea eliminada: {}", task.title),
            actor,
            Some(serde_json::json!({"op": "restore", "entity": "task", "id": id})),
        )
        .await?;
        Ok(())
    }

    pub async fn restore(&self, id: &str, actor: &str) -> Result<Task> {
        repos::tasks::restore(&self.db, id).await?;
        let task = repos::tasks::get(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "restored",
            "task",
            Some(id),
            &format!("Tarea restaurada: {}", task.title),
            actor,
            None,
        )
        .await?;
        Ok(task)
    }

    pub async fn reorder(
        &self,
        id: &str,
        status: TaskStatus,
        position: i64,
        actor: &str,
    ) -> Result<Task> {
        self.update(
            id,
            UpdateTask {
                status: Some(status),
                position: Some(position),
                ..Default::default()
            },
            actor,
        )
        .await
    }

    pub async fn subtasks(&self, id: &str) -> Result<Vec<Task>> {
        repos::tasks::list(
            &self.db,
            &TaskFilter {
                parent_id: Some(id.to_string()),
                ..Default::default()
            },
        )
        .await
    }
}
