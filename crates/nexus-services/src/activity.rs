use nexus_core::{ActivityEvent, CalendarEvent, NexusError, Note, Project, Result, Task};
use nexus_db::{repos, Db};

#[derive(Clone)]
pub struct ActivityService {
    pub db: Db,
}

impl ActivityService {
    pub async fn list(
        &self,
        limit: i64,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
    ) -> Result<Vec<ActivityEvent>> {
        repos::activity::list(&self.db, limit.clamp(1, 500), entity_type, entity_id).await
    }

    /// Deshace una operación reversible registrada en la auditoría.
    pub async fn undo(&self, activity_id: &str) -> Result<ActivityEvent> {
        let event = repos::activity::get(&self.db, activity_id).await?;
        let undo = event
            .undo_json
            .clone()
            .ok_or_else(|| NexusError::Conflict("esta operación no se puede deshacer".into()))?;
        let op = undo.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let entity = undo.get("entity").and_then(|v| v.as_str()).unwrap_or("");
        let id = undo
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| NexusError::Internal("undo sin id".into()))?;

        match (op, entity) {
            ("restore", "task") => repos::tasks::restore(&self.db, id).await?,
            ("restore", "note") => repos::notes::restore(&self.db, id).await?,
            ("restore", "project") => repos::projects::restore(&self.db, id).await?,
            ("restore", "event") => repos::events::restore(&self.db, id).await?,
            ("delete", "task") => repos::tasks::soft_delete(&self.db, id).await?,
            ("delete", "note") => repos::notes::soft_delete(&self.db, id).await?,
            ("delete", "project") => repos::projects::soft_delete(&self.db, id).await?,
            ("delete", "event") => repos::events::soft_delete(&self.db, id).await?,
            ("update", "task") => {
                let before: Task = serde_json::from_value(undo["before"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                repos::tasks::write(&self.db, &before).await?;
            }
            ("update", "note") => {
                let before: Note = serde_json::from_value(undo["before"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                repos::notes::write(&self.db, &before).await?;
            }
            ("update", "project") => {
                let before: Project = serde_json::from_value(undo["before"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                repos::projects::write(&self.db, &before).await?;
            }
            ("update", "event") => {
                let before: CalendarEvent = serde_json::from_value(undo["before"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                repos::events::write(&self.db, &before).await?;
            }
            ("meals_delete_recipe", _) => repos::meals::soft_delete_recipe(&self.db, id).await?,
            ("meals_restore_recipe", _) => {
                let recipe: nexus_core::Recipe = serde_json::from_value(undo["recipe"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                crate::meals::MealService {
                    db: self.db.clone(),
                }
                .restore_recipe(recipe)
                .await?;
            }
            ("meals_delete_plan_entry", _) => repos::meals::delete_plan_entry(&self.db, id).await?,
            ("meals_restore_plan_entry", _) => {
                let entry: nexus_core::MealPlanEntry =
                    serde_json::from_value(undo["entry"].clone()).map_err(|e| {
                        NexusError::Internal(format!("estado previo inválido: {e}"))
                    })?;
                crate::meals::MealService {
                    db: self.db.clone(),
                }
                .restore_plan_entry(entry)
                .await?;
            }
            ("meals_delete_pantry_item", _) => {
                repos::meals::delete_pantry_item(&self.db, id).await?
            }
            ("meals_restore_pantry_item", _) => {
                let item: nexus_core::PantryItem = serde_json::from_value(undo["item"].clone())
                    .map_err(|e| NexusError::Internal(format!("estado previo inválido: {e}")))?;
                crate::meals::MealService {
                    db: self.db.clone(),
                }
                .restore_pantry_item(item)
                .await?;
            }
            ("meals_delete_menu_template", _) => {
                repos::meals::delete_menu_template(&self.db, id).await?
            }
            ("meals_delete_shopping_list", _) => {
                repos::meals::delete_shopping_list(&self.db, id).await?
            }
            ("delete", "canvas") => repos::canvas::soft_delete(&self.db, id).await?,
            ("restore", "canvas") => repos::canvas::restore(&self.db, id).await?,
            _ => {
                return Err(NexusError::Conflict(format!(
                    "operación no reversible: {op} {entity}"
                )))
            }
        }

        repos::activity::clear_undo(&self.db, activity_id).await?;
        repos::activity::record(
            &self.db,
            "undone",
            entity,
            Some(id),
            &format!("Deshecho: {}", event.summary),
            "user",
            None,
        )
        .await
    }
}
