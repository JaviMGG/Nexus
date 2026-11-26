use chrono::{DateTime, NaiveDate, NaiveTime};
use chrono_tz::Tz;
use nexus_core::{ScheduleSpec, Trigger, TriggerEntry, TriggerFieldKind, TriggerValidationError};
use serde_json::Value;

use crate::{trigger_fields, triggers};

pub fn validate_trigger(trigger: &Trigger) -> Vec<TriggerValidationError> {
    match trigger {
        Trigger::Schedule { spec } => validate_schedule(spec),
        Trigger::Event { event, filter } => validate_event(event, filter),
    }
}

pub fn validate_entries(entries: &[TriggerEntry]) -> Vec<TriggerValidationError> {
    entries
        .iter()
        .flat_map(|entry| validate_trigger(&entry.trigger))
        .collect()
}

fn error(field: impl Into<String>, message: impl Into<String>) -> TriggerValidationError {
    TriggerValidationError {
        field: field.into(),
        message: message.into(),
    }
}

fn validate_schedule(spec: &ScheduleSpec) -> Vec<TriggerValidationError> {
    let mut errors = Vec::new();
    match spec.kind.as_str() {
        "daily" | "weekly" | "monthly" | "once" => match spec.time.as_deref() {
            None => errors.push(error("spec.time", "falta la hora (HH:MM)")),
            Some(time) if !is_hhmm(time) => {
                errors.push(error("spec.time", "hora inválida, usa HH:MM"));
            }
            Some(_) => {}
        },
        "interval" => match spec.interval_minutes {
            None => errors.push(error("spec.interval_minutes", "falta el intervalo en minutos")),
            Some(mins) if mins < 5 => errors.push(error(
                "spec.interval_minutes",
                "el intervalo mínimo es de 5 minutos",
            )),
            Some(_) => {}
        },
        other => errors.push(error(
            "spec.kind",
            format!("tipo de programación desconocido: {other}"),
        )),
    }

    if spec.kind == "weekly" {
        if spec.weekdays.is_empty() {
            errors.push(error(
                "spec.weekdays",
                "selecciona al menos un día de la semana",
            ));
        }
        for day in &spec.weekdays {
            if !is_weekday(day) {
                errors.push(error(
                    "spec.weekdays",
                    format!("día de la semana inválido: {day}"),
                ));
            }
        }
    }

    if spec.kind == "monthly" {
        match spec.day_of_month {
            None => errors.push(error("spec.day_of_month", "falta el día del mes")),
            Some(day) if !(1..=31).contains(&day) => errors.push(error(
                "spec.day_of_month",
                "el día del mes debe estar entre 1 y 31",
            )),
            Some(_) => {}
        }
    }

    if spec.kind == "once" {
        match spec.date.as_deref() {
            None => errors.push(error("spec.date", "falta la fecha (YYYY-MM-DD)")),
            Some(date) if !is_ymd(date) => {
                errors.push(error("spec.date", "fecha inválida, usa YYYY-MM-DD"));
            }
            Some(_) => {}
        }
    }

    if let Some(tz) = spec.timezone.as_deref() {
        if !tz.is_empty() && tz.parse::<Tz>().is_err() {
            errors.push(error("spec.timezone", format!("zona horaria inválida: {tz}")));
        }
    }

    errors
}

fn validate_event(event: &str, filter: &Value) -> Vec<TriggerValidationError> {
    let mut errors = Vec::new();
    if !triggers::is_known_event(event) {
        errors.push(error("event", format!("evento desconocido: {event}")));
        return errors;
    }

    let fields = trigger_fields::fields_for(event);
    let object = match filter {
        Value::Null => return errors,
        Value::Object(map) => map,
        _ => {
            errors.push(error("filter", "el filtro debe ser un objeto JSON"));
            return errors;
        }
    };

    for (key, value) in object {
        let field = format!("filter.{key}");
        let (base, contains) = match key.strip_suffix('~') {
            Some(base) => (base, true),
            None => (key.as_str(), false),
        };
        let Some(spec) = fields.iter().find(|field| field.key == base) else {
            errors.push(error(field, format!("filtro desconocido: {key}")));
            continue;
        };

        if contains {
            if !matches!(value, Value::String(_)) {
                errors.push(error(field, "la búsqueda 'contiene' debe ser texto"));
            }
            continue;
        }

        if value.is_null() {
            if !spec.nullable {
                errors.push(error(field, "este campo no admite null"));
            }
            continue;
        }

        let valid = match spec.kind {
            TriggerFieldKind::Text | TriggerFieldKind::Id => value
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()),
            TriggerFieldKind::Enum => value
                .as_str()
                .is_some_and(|text| spec.options.iter().any(|option| option == text)),
            TriggerFieldKind::Date => value.as_str().is_some_and(is_date),
            TriggerFieldKind::DateTime => value.as_str().is_some_and(is_datetime),
            TriggerFieldKind::Number => value.as_number().is_some(),
            TriggerFieldKind::Boolean => value.as_bool().is_some(),
            TriggerFieldKind::Tags => value
                .as_array()
                .is_some_and(|tags| tags.iter().all(|tag| tag.is_string())),
        };
        if !valid {
            errors.push(error(field, format!("{}: {}", spec.label, spec.hint)));
        }
    }

    errors
}

pub fn validate_action_params(action: &str, params: &Value) -> Vec<TriggerValidationError> {
    let mut errors = Vec::new();
    let accepts_due_date = matches!(
        action,
        "tasks.create"
            | "tasks.create_subtask"
            | "projects.add_task"
            | "tasks.update"
            | "tasks.complete"
            | "tasks.set_priority"
            | "tasks.add_tags"
            | "tasks.reorder"
    );
    if !accepts_due_date {
        return errors;
    }

    let Some(value) = params.get("due_date") else {
        return errors;
    };
    if value.is_null() {
        return errors;
    }
    let Some(text) = value.as_str() else {
        errors.push(error(
            "params.due_date",
            "la fecha límite debe ser texto",
        ));
        return errors;
    };

    let trimmed = text.trim();
    if trimmed.is_empty() || (trimmed.contains("{{") && trimmed.contains("}}")) {
        return errors;
    }
    if !is_date(trimmed) {
        errors.push(error(
            "params.due_date",
            format!("fecha límite inválida: {text} (usa YYYY-MM-DD o RFC3339)"),
        ));
    }
    errors
}

fn is_hhmm(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 5
        && bytes[2] == b':'
        && bytes[..2].iter().all(|b| b.is_ascii_digit())
        && bytes[3..].iter().all(|b| b.is_ascii_digit())
        && NaiveTime::parse_from_str(value, "%H:%M").is_ok()
}

fn is_ymd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

fn is_date(value: &str) -> bool {
    is_ymd(value) || DateTime::parse_from_rfc3339(value).is_ok()
}

fn is_datetime(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value).is_ok() || DateTime::parse_from_rfc2822(value).is_ok()
}

fn is_weekday(value: &str) -> bool {
    matches!(
        value,
        "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun"
    )
}
