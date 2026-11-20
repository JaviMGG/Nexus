use nexus_core::{
    new_id, CalendarEvent, CreateEvent, EventFilter, NexusError, Result, UpdateEvent,
};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct EventService {
    pub db: Db,
}

impl EventService {
    pub async fn list(&self, f: EventFilter) -> Result<Vec<CalendarEvent>> {
        repos::events::list(&self.db, &f).await
    }

    pub async fn get(&self, id: &str) -> Result<CalendarEvent> {
        repos::events::get(&self.db, id).await
    }

    pub async fn create(&self, mut data: CreateEvent, actor: &str) -> Result<CalendarEvent> {
        data.title = validate::title(&data.title, "el título")?;
        data.date = event_date(&data.date)?;
        data.description = validate::long_text(&data.description, "la descripción", 20_000)?;
        data.location = validate::long_text(data.location.trim(), "la ubicación", 300)?;
        normalize_times(&mut data.start_time, &mut data.end_time, data.all_day)?;
        if let Some(p) = &data.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        if let Some(t) = &data.task_id {
            repos::tasks::get(&self.db, t).await?;
        }
        let event = repos::events::create(&self.db, data, new_id()).await?;
        repos::activity::record(
            &self.db,
            "created",
            "event",
            Some(&event.id),
            &format!("Evento creado: {}", event.title),
            actor,
            Some(serde_json::json!({"op": "delete", "entity": "event", "id": event.id})),
        )
        .await?;
        Ok(event)
    }

    pub async fn update(
        &self,
        id: &str,
        mut patch: UpdateEvent,
        actor: &str,
    ) -> Result<CalendarEvent> {
        let before = repos::events::get(&self.db, id).await?;
        if let Some(t) = &patch.title {
            patch.title = Some(validate::title(t, "el título")?);
        }
        if let Some(d) = &patch.date {
            patch.date = Some(event_date(d)?);
        }
        if let Some(d) = &patch.description {
            validate::long_text(d, "la descripción", 20_000)?;
        }
        if let Some(Some(l)) = &patch.location {
            validate::long_text(l.trim(), "la ubicación", 300)?;
            patch.location = Some(Some(l.trim().to_string()));
        }
        if let Some(Some(p)) = &patch.project_id {
            repos::projects::get(&self.db, p).await?;
        }
        if let Some(Some(t)) = &patch.task_id {
            repos::tasks::get(&self.db, t).await?;
        }
        let mut start = match &patch.start_time {
            Some(v) => v.clone(),
            None => before.start_time.clone(),
        };
        let mut end = match &patch.end_time {
            Some(v) => v.clone(),
            None => before.end_time.clone(),
        };
        let all_day = patch.all_day.unwrap_or(before.all_day);
        normalize_times(&mut start, &mut end, all_day)?;
        patch.start_time = Some(start);
        patch.end_time = Some(end);
        patch.all_day = Some(all_day);
        let event = repos::events::update(&self.db, id, patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "event",
            Some(id),
            &format!("Evento actualizado: {}", event.title),
            actor,
            Some(
                serde_json::json!({"op": "update", "entity": "event", "id": id, "before": before}),
            ),
        )
        .await?;
        Ok(event)
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let event = repos::events::get(&self.db, id).await?;
        repos::events::soft_delete(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "event",
            Some(id),
            &format!("Evento eliminado: {}", event.title),
            actor,
            Some(serde_json::json!({"op": "restore", "entity": "event", "id": id})),
        )
        .await?;
        Ok(())
    }

    pub async fn restore(&self, id: &str, actor: &str) -> Result<CalendarEvent> {
        repos::events::restore(&self.db, id).await?;
        let event = repos::events::get(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "restored",
            "event",
            Some(id),
            &format!("Evento restaurado: {}", event.title),
            actor,
            None,
        )
        .await?;
        Ok(event)
    }
}

fn event_date(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(NexusError::validation("la fecha no puede estar vacía"));
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| NexusError::validation(format!("fecha inválida: {s} (usa YYYY-MM-DD)")))
}

fn event_time(s: &str) -> Result<String> {
    let s = s.trim();
    chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .map(|t| t.format("%H:%M").to_string())
        .map_err(|_| NexusError::validation(format!("hora inválida: {s} (usa HH:MM)")))
}

/// Valida y normaliza horas; en eventos de todo el día se descartan y si hay
/// inicio y fin, el fin debe ser posterior al inicio.
fn normalize_times(
    start: &mut Option<String>,
    end: &mut Option<String>,
    all_day: bool,
) -> Result<()> {
    if all_day {
        *start = None;
        *end = None;
        return Ok(());
    }
    if let Some(s) = start.as_deref() {
        *start = Some(event_time(s)?);
    }
    if let Some(e) = end.as_deref() {
        *end = Some(event_time(e)?);
    }
    if let (Some(s), Some(e)) = (start.as_deref(), end.as_deref()) {
        if e <= s {
            return Err(NexusError::validation(
                "la hora de fin debe ser posterior a la de inicio",
            ));
        }
    }
    Ok(())
}
