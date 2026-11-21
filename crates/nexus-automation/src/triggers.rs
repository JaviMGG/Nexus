//! TriggerRegistry: catálogo de disparadores de evento y cálculo de la
//! próxima ejecución de los disparadores temporales (con zona horaria).

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use nexus_core::{NexusError, Result, ScheduleSpec, Trigger, TriggerKind, TriggerSpec};

/// Eventos de dominio que el motor sabe escuchar. Se emiten como
/// `AppEvent::DomainEvent` desde el dispatch y desde los propios agentes.
pub const EVENT_CATALOG: &[(&str, &str, &str)] = &[
    (
        "task.created",
        "Tarea creada",
        "Cuando se crea una tarea nueva",
    ),
    (
        "task.completed",
        "Tarea completada",
        "Cuando una tarea pasa a estado completada",
    ),
    (
        "task.updated",
        "Tarea modificada",
        "Cuando se modifica cualquier campo de una tarea",
    ),
    (
        "task.overdue",
        "Tarea vencida",
        "Cuando una tarea vence sin completarse (comprobación periódica)",
    ),
    (
        "project.created",
        "Proyecto creado",
        "Cuando se crea un proyecto nuevo",
    ),
    (
        "project.status_changed",
        "Estado de proyecto modificado",
        "Cuando un proyecto cambia de estado",
    ),
    (
        "note.created",
        "Nota creada",
        "Cuando se crea una nota o documento",
    ),
    (
        "idea.created",
        "Idea guardada",
        "Cuando se guarda una idea en la base de conocimiento",
    ),
    (
        "goal.created",
        "Objetivo creado",
        "Cuando se crea un proyecto con objetivo definido",
    ),
    (
        "agent.finished",
        "Ejecución de agente terminada",
        "Cuando un run de agente finaliza",
    ),
];

pub fn catalog() -> Vec<TriggerSpec> {
    let mut specs: Vec<TriggerSpec> = EVENT_CATALOG
        .iter()
        .map(|(id, name, desc)| TriggerSpec {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            kind: TriggerKind::Event,
        })
        .collect();
    specs.insert(
        0,
        TriggerSpec {
            id: "schedule".into(),
            name: "Programación temporal".into(),
            description: "A una hora, a diario, semanal, mensual, por intervalo o una vez".into(),
            kind: TriggerKind::Schedule,
        },
    );
    specs
}

pub fn is_known_event(event: &str) -> bool {
    EVENT_CATALOG.iter().any(|(id, _, _)| *id == event)
}

/// Resuelve la zona horaria efectiva: explícita > ajuste global > local > UTC.
pub fn resolve_tz(explicit: Option<&str>, global: Option<&str>) -> Tz {
    explicit
        .and_then(|s| s.parse::<Tz>().ok())
        .or_else(|| global.and_then(|s| s.parse::<Tz>().ok()))
        .or_else(|| {
            iana_time_zone::get_timezone()
                .ok()
                .and_then(|s| s.parse::<Tz>().ok())
        })
        .unwrap_or(Tz::UTC)
}

pub fn validate_schedule(spec: &ScheduleSpec) -> Result<()> {
    match spec.kind.as_str() {
        "daily" | "weekly" | "monthly" | "once" => {
            let time = spec
                .time
                .as_deref()
                .ok_or_else(|| NexusError::validation("falta la hora (HH:MM)"))?;
            parse_time(time)?;
        }
        "interval" => {
            let mins = spec
                .interval_minutes
                .ok_or_else(|| NexusError::validation("falta interval_minutes"))?;
            if mins < 5 {
                return Err(NexusError::validation(
                    "el intervalo mínimo es de 5 minutos",
                ));
            }
        }
        other => {
            return Err(NexusError::validation(format!(
                "tipo de programación desconocido: {other}"
            )))
        }
    }
    if spec.kind == "weekly" {
        if spec.weekdays.is_empty() {
            return Err(NexusError::validation(
                "selecciona al menos un día de la semana",
            ));
        }
        for d in &spec.weekdays {
            if weekday_index(d).is_none() {
                return Err(NexusError::validation(format!(
                    "día de la semana inválido: {d}"
                )));
            }
        }
    }
    if spec.kind == "monthly" {
        let day = spec
            .day_of_month
            .ok_or_else(|| NexusError::validation("falta day_of_month"))?;
        if !(1..=31).contains(&day) {
            return Err(NexusError::validation(
                "day_of_month debe estar entre 1 y 31",
            ));
        }
    }
    if spec.kind == "once" {
        let date = spec
            .date
            .as_deref()
            .ok_or_else(|| NexusError::validation("falta la fecha (YYYY-MM-DD)"))?;
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| NexusError::validation("fecha inválida, usa YYYY-MM-DD"))?;
    }
    if let Some(tz) = &spec.timezone {
        tz.parse::<Tz>()
            .map_err(|_| NexusError::validation(format!("zona horaria inválida: {tz}")))?;
    }
    Ok(())
}

fn parse_time(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|_| NexusError::validation("hora inválida, usa HH:MM"))
}

fn weekday_index(s: &str) -> Option<u32> {
    match s {
        "mon" => Some(0),
        "tue" => Some(1),
        "wed" => Some(2),
        "thu" => Some(3),
        "fri" => Some(4),
        "sat" => Some(5),
        "sun" => Some(6),
        _ => None,
    }
}

/// Siguiente disparo estrictamente posterior a `after`. `last_fired` evita
/// repetir el mismo intervalo tras un reinicio.
pub fn next_occurrence(
    spec: &ScheduleSpec,
    after: DateTime<Utc>,
    last_fired: Option<DateTime<Utc>>,
    global_tz: Option<&str>,
) -> Result<Option<DateTime<Utc>>> {
    let tz = resolve_tz(spec.timezone.as_deref(), global_tz);
    let next = match spec.kind.as_str() {
        "interval" => {
            let mins = spec.interval_minutes.unwrap_or(60).max(5);
            let base = last_fired.unwrap_or(after);
            let candidate = base + Duration::minutes(mins);
            Some(candidate.max(after + Duration::seconds(1)))
        }
        "once" => {
            let date = NaiveDate::parse_from_str(spec.date.as_deref().unwrap_or(""), "%Y-%m-%d")
                .map_err(|_| NexusError::validation("fecha inválida"))?;
            let time = parse_time(spec.time.as_deref().unwrap_or("00:00"))?;
            let local = date.and_time(time);
            let dt = resolve_local(&tz, local);
            if dt > after && last_fired.map(|l| l < dt).unwrap_or(true) {
                Some(dt)
            } else {
                None
            }
        }
        "daily" => {
            let time = parse_time(spec.time.as_deref().unwrap_or("08:00"))?;
            next_daily(&tz, after, time, |_| true)
        }
        "weekly" => {
            let time = parse_time(spec.time.as_deref().unwrap_or("08:00"))?;
            let days: Vec<u32> = spec
                .weekdays
                .iter()
                .filter_map(|d| weekday_index(d))
                .collect();
            next_daily(&tz, after, time, |wd| days.contains(&wd))
        }
        "monthly" => {
            let time = parse_time(spec.time.as_deref().unwrap_or("08:00"))?;
            let day = spec.day_of_month.unwrap_or(1).clamp(1, 31) as u32;
            next_monthly(&tz, after, time, day)
        }
        other => {
            return Err(NexusError::validation(format!(
                "tipo de programación desconocido: {other}"
            )))
        }
    };
    Ok(next)
}

/// Convierte una fecha/hora local a UTC resolviendo ambigüedades de DST
/// (usa el primer instante válido; si la hora no existe, el siguiente válido).
fn resolve_local(tz: &Tz, local: chrono::NaiveDateTime) -> DateTime<Utc> {
    match tz.from_local_datetime(&local) {
        chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
        chrono::LocalResult::Ambiguous(dt, _) => dt.with_timezone(&Utc),
        chrono::LocalResult::None => {
            for mins in 1..=180 {
                let shifted = local + Duration::minutes(mins);
                if let chrono::LocalResult::Single(dt) = tz.from_local_datetime(&shifted) {
                    return dt.with_timezone(&Utc);
                }
            }
            tz.from_utc_datetime(&local).with_timezone(&Utc)
        }
    }
}

fn next_daily(
    tz: &Tz,
    after: DateTime<Utc>,
    time: NaiveTime,
    accept: impl Fn(u32) -> bool,
) -> Option<DateTime<Utc>> {
    let local_after = after.with_timezone(tz);
    for offset in 0..=8 {
        let date = local_after.date_naive() + Duration::days(offset);
        if !accept(date.weekday().num_days_from_monday()) {
            continue;
        }
        let dt = resolve_local(tz, date.and_time(time));
        if dt > after {
            return Some(dt);
        }
    }
    None
}

fn next_monthly(tz: &Tz, after: DateTime<Utc>, time: NaiveTime, day: u32) -> Option<DateTime<Utc>> {
    let local_after = after.with_timezone(tz);
    let mut year = local_after.year();
    let mut month = local_after.month();
    for _ in 0..14 {
        let last = last_day_of_month(year, month);
        let date = NaiveDate::from_ymd_opt(year, month, day.min(last))?;
        let dt = resolve_local(tz, date.and_time(time));
        if dt > after {
            return Some(dt);
        }
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    None
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .map(|d| (d - Duration::days(1)).day())
        .unwrap_or(28)
}

/// Clave de deduplicación de un disparo temporal: el instante programado.
pub fn schedule_mark(when: DateTime<Utc>) -> String {
    when.format("%Y-%m-%dT%H:%M").to_string()
}

/// Coincidencia de un evento de dominio con el trigger (incluye filtro plano).
pub fn event_matches(trigger: &Trigger, event: &str, data: &serde_json::Value) -> bool {
    match trigger {
        Trigger::Event {
            event: want,
            filter,
        } => {
            if want != event {
                return false;
            }
            match filter {
                serde_json::Value::Object(map) => map.iter().all(|(k, v)| data.get(k) == Some(v)),
                _ => true,
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn daily_next_respects_time() {
        let spec = ScheduleSpec {
            kind: "daily".into(),
            time: Some("08:00".into()),
            timezone: Some("UTC".into()),
            ..Default::default()
        };
        let after = utc("2026-01-10T09:00:00Z");
        let next = next_occurrence(&spec, after, None, None).unwrap().unwrap();
        assert_eq!(next, utc("2026-01-11T08:00:00Z"));

        let after = utc("2026-01-10T07:00:00Z");
        let next = next_occurrence(&spec, after, None, None).unwrap().unwrap();
        assert_eq!(next, utc("2026-01-10T08:00:00Z"));
    }

    #[test]
    fn weekly_only_selected_days() {
        let spec = ScheduleSpec {
            kind: "weekly".into(),
            time: Some("09:00".into()),
            weekdays: vec!["mon".into()],
            timezone: Some("UTC".into()),
            ..Default::default()
        };
        // 2026-01-10 es sábado → siguiente lunes 12.
        let next = next_occurrence(&spec, utc("2026-01-10T10:00:00Z"), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(next, utc("2026-01-12T09:00:00Z"));
    }

    #[test]
    fn monthly_clamps_short_months() {
        let spec = ScheduleSpec {
            kind: "monthly".into(),
            time: Some("08:00".into()),
            day_of_month: Some(31),
            timezone: Some("UTC".into()),
            ..Default::default()
        };
        let next = next_occurrence(&spec, utc("2026-02-01T00:00:00Z"), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(next, utc("2026-02-28T08:00:00Z"));
    }

    #[test]
    fn interval_uses_last_fired() {
        let spec = ScheduleSpec {
            kind: "interval".into(),
            interval_minutes: Some(30),
            ..Default::default()
        };
        let last = utc("2026-01-10T08:00:00Z");
        let after = utc("2026-01-10T08:10:00Z");
        let next = next_occurrence(&spec, after, Some(last), None)
            .unwrap()
            .unwrap();
        assert_eq!(next, utc("2026-01-10T08:30:00Z"));
    }

    #[test]
    fn once_expires() {
        let spec = ScheduleSpec {
            kind: "once".into(),
            date: Some("2026-01-10".into()),
            time: Some("08:00".into()),
            timezone: Some("UTC".into()),
            ..Default::default()
        };
        assert!(
            next_occurrence(&spec, utc("2026-01-10T09:00:00Z"), None, None)
                .unwrap()
                .is_none()
        );
        assert!(
            next_occurrence(&spec, utc("2026-01-09T09:00:00Z"), None, None)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn timezone_madrid_winter_and_summer() {
        let spec = ScheduleSpec {
            kind: "daily".into(),
            time: Some("08:00".into()),
            timezone: Some("Europe/Madrid".into()),
            ..Default::default()
        };
        let winter = next_occurrence(&spec, utc("2026-01-10T06:00:00Z"), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(winter, utc("2026-01-10T07:00:00Z")); // UTC+1
        let summer = next_occurrence(&spec, utc("2026-07-10T05:00:00Z"), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(summer, utc("2026-07-10T06:00:00Z")); // UTC+2
    }

    #[test]
    fn event_filter_matching() {
        let t = Trigger::Event {
            event: "project.status_changed".into(),
            filter: serde_json::json!({"status": "completed"}),
        };
        assert!(event_matches(
            &t,
            "project.status_changed",
            &serde_json::json!({"status": "completed"})
        ));
        assert!(!event_matches(
            &t,
            "project.status_changed",
            &serde_json::json!({"status": "active"})
        ));
        assert!(!event_matches(&t, "task.created", &serde_json::json!({})));
    }

    #[test]
    fn validation_rejects_bad_specs() {
        assert!(validate_schedule(&ScheduleSpec {
            kind: "daily".into(),
            ..Default::default()
        })
        .is_err());
        assert!(validate_schedule(&ScheduleSpec {
            kind: "interval".into(),
            interval_minutes: Some(1),
            ..Default::default()
        })
        .is_err());
        assert!(validate_schedule(&ScheduleSpec {
            kind: "weekly".into(),
            time: Some("08:00".into()),
            weekdays: vec!["funday".into()],
            ..Default::default()
        })
        .is_err());
    }
}
