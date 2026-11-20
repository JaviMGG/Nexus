use chrono::Utc;
use nexus_core::{
    new_id, CreateProject, NoteFilter, Project, ProjectDetail, ProjectProgress, ProjectStatus,
    Result, TaskFilter, TaskStatus, UpdateProject,
};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct ProjectService {
    pub db: Db,
}

impl ProjectService {
    pub async fn list(&self, include_archived: bool) -> Result<Vec<Project>> {
        repos::projects::list(&self.db, include_archived).await
    }

    pub async fn progress(&self, project: Project) -> Result<ProjectProgress> {
        let tasks = repos::tasks::list(
            &self.db,
            &TaskFilter {
                project_id: Some(project.id.clone()),
                ..Default::default()
            },
        )
        .await?;
        let counted: Vec<_> = tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Cancelled)
            .collect();
        let total = counted.len() as i64;
        let done = counted
            .iter()
            .filter(|t| t.status == TaskStatus::Done)
            .count() as i64;
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let overdue = counted
            .iter()
            .filter(|t| {
                t.status != TaskStatus::Done
                    && t.due_date
                        .as_deref()
                        .map(|d| &d[..10.min(d.len())] < today.as_str())
                        .unwrap_or(false)
            })
            .count() as i64;
        let progress = if total == 0 {
            0.0
        } else {
            done as f64 / total as f64
        };
        Ok(ProjectProgress {
            project,
            total,
            done,
            progress,
            overdue,
        })
    }

    pub async fn get(&self, id: &str) -> Result<ProjectDetail> {
        let project = repos::projects::get(&self.db, id).await?;
        let tasks = repos::tasks::list(
            &self.db,
            &TaskFilter {
                project_id: Some(id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        let notes = repos::notes::list(
            &self.db,
            &NoteFilter {
                project_id: Some(id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        let progress = self.progress(project.clone()).await?;
        let activity = repos::activity::list(&self.db, 30, None, Some(id)).await?;
        Ok(ProjectDetail {
            project,
            tasks,
            notes,
            progress,
            activity,
        })
    }

    pub async fn create(&self, mut data: CreateProject, actor: &str) -> Result<Project> {
        data.name = validate::title(&data.name, "el nombre")?;
        data.description = validate::long_text(&data.description, "la descripción", 20_000)?;
        data.goal = validate::long_text(&data.goal, "el objetivo", 2_000)?;
        let project = repos::projects::create(&self.db, data, new_id()).await?;
        repos::activity::record(
            &self.db,
            "created",
            "project",
            Some(&project.id),
            &format!("Proyecto creado: {}", project.name),
            actor,
            Some(serde_json::json!({"op": "delete", "entity": "project", "id": project.id})),
        )
        .await?;
        Ok(project)
    }

    pub async fn update(&self, id: &str, mut patch: UpdateProject, actor: &str) -> Result<Project> {
        let before = repos::projects::get(&self.db, id).await?;
        if let Some(n) = &patch.name {
            patch.name = Some(validate::title(n, "el nombre")?);
        }
        if let Some(d) = &patch.description {
            validate::long_text(d, "la descripción", 20_000)?;
        }
        if let Some(g) = &patch.goal {
            validate::long_text(g, "el objetivo", 2_000)?;
        }
        let project = repos::projects::update(&self.db, id, patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "project",
            Some(id),
            &format!("Proyecto actualizado: {}", project.name),
            actor,
            Some(serde_json::json!({"op": "update", "entity": "project", "id": id, "before": before})),
        )
        .await?;
        Ok(project)
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let project = repos::projects::get(&self.db, id).await?;
        repos::projects::soft_delete(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "project",
            Some(id),
            &format!("Proyecto eliminado: {}", project.name),
            actor,
            Some(serde_json::json!({"op": "restore", "entity": "project", "id": id})),
        )
        .await?;
        Ok(())
    }

    pub async fn restore(&self, id: &str, actor: &str) -> Result<Project> {
        repos::projects::restore(&self.db, id).await?;
        let p = repos::projects::get(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "restored",
            "project",
            Some(id),
            &format!("Proyecto restaurado: {}", p.name),
            actor,
            None,
        )
        .await?;
        Ok(p)
    }

    pub async fn active_with_progress(&self) -> Result<Vec<ProjectProgress>> {
        let projects = repos::projects::list(&self.db, false).await?;
        let mut out = Vec::new();
        for p in projects
            .into_iter()
            .filter(|p| p.status == ProjectStatus::Active)
        {
            out.push(self.progress(p).await?);
        }
        Ok(out)
    }

    pub async fn list_with_progress(&self, include_archived: bool) -> Result<Vec<ProjectProgress>> {
        let projects = repos::projects::list(&self.db, include_archived).await?;
        let mut out = Vec::new();
        for p in projects {
            out.push(self.progress(p).await?);
        }
        Ok(out)
    }
}
