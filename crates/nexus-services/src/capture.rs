//! Captura rápida sin IA: heurística local para detectar tareas/notas,
//! fechas límite en español e inglés y prioridad. La IA mejora este análisis
//! cuando está disponible, pero nunca es obligatoria.

use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use nexus_core::{CaptureResult, CreateNote, CreateTask, Priority};

pub fn heuristic_capture(content: &str) -> CaptureResult {
    let text = content.trim();
    let lower = text.to_lowercase();

    let is_note = lower.starts_with("idea:")
        || lower.starts_with("idea ")
        || lower.starts_with("nota:")
        || lower.starts_with("nota ")
        || lower.starts_with("apuntes")
        || lower.starts_with("documento")
        || lower.starts_with("http://")
        || lower.starts_with("https://");

    if is_note {
        let title = text
            .trim_start_matches(|c: char| c.is_alphabetic())
            .trim_start_matches([':', ' '])
            .trim();
        let title = if title.is_empty() { text } else { title };
        let title: String = title.chars().take(120).collect();
        return CaptureResult {
            kind: "note".into(),
            task: None,
            note: Some(CreateNote {
                title: capitalize(&title),
                content: text.to_string(),
                kind: Some(if lower.starts_with("idea") {
                    nexus_core::NoteKind::Idea
                } else {
                    nexus_core::NoteKind::Note
                }),
                ..Default::default()
            }),
            explanation: "Detectado como nota/idea (análisis local, sin IA).".into(),
            ai_used: false,
        };
    }

    let due = extract_due_date(&lower);
    let priority = if lower.contains("urgente")
        || lower.contains("urgent")
        || lower.contains("cuanto antes")
        || lower.contains("asap")
    {
        Priority::Urgent
    } else if lower.contains("importante") || lower.contains("important") {
        Priority::High
    } else {
        Priority::Medium
    };

    let mut title = text.to_string();
    for prefix in [
        "tengo que ",
        "debo ",
        "hay que ",
        "recordar ",
        "recuérdame ",
        "tengo pendiente ",
        "necesito ",
        "quiero ",
        "i need to ",
        "i have to ",
        "remind me to ",
        "todo: ",
        "task: ",
    ] {
        if lower.starts_with(prefix) {
            title = text[prefix.len()..].to_string();
            break;
        }
    }
    for phrase in [
        " antes del viernes",
        " antes del sábado",
        " antes del domingo",
        " antes del lunes",
        " antes del martes",
        " antes del miércoles",
        " antes del jueves",
        " antes de mañana",
        " para mañana",
        " para hoy",
        " hoy",
        " mañana",
        " pasado mañana",
        " esta semana",
        " la semana que viene",
        " la próxima semana",
        " before friday",
        " before monday",
        " tomorrow",
        " today",
        " next week",
        " this week",
    ] {
        let idx = title.to_lowercase().find(phrase);
        if let Some(i) = idx {
            title = format!("{}{}", &title[..i], &title[i + phrase.len()..]);
        }
    }
    let title = capitalize(title.trim().trim_end_matches(['.', ',', ';']));
    let title = if title.is_empty() {
        text.to_string()
    } else {
        title
    };

    let mut explanation = String::from("Detectado como tarea (análisis local, sin IA).");
    if let Some(d) = &due {
        explanation.push_str(&format!(" Fecha límite: {d}."));
    }
    if priority != Priority::Medium {
        explanation.push_str(&format!(" Prioridad: {}.", priority.as_str()));
    }

    CaptureResult {
        kind: "task".into(),
        task: Some(CreateTask {
            title,
            due_date: due,
            priority: Some(priority),
            ..Default::default()
        }),
        note: None,
        explanation,
        ai_used: false,
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn extract_due_date(lower: &str) -> Option<String> {
    let today = Utc::now().date_naive();
    if lower.contains("pasado mañana") || lower.contains("day after tomorrow") {
        return Some((today + Duration::days(2)).format("%Y-%m-%d").to_string());
    }
    if lower.contains("mañana") || lower.contains("tomorrow") {
        return Some((today + Duration::days(1)).format("%Y-%m-%d").to_string());
    }
    if lower.contains("hoy") || lower.contains("today") {
        return Some(today.format("%Y-%m-%d").to_string());
    }
    if lower.contains("la semana que viene")
        || lower.contains("la próxima semana")
        || lower.contains("next week")
    {
        let days_to_monday = (7 - today.weekday().num_days_from_monday()) % 7;
        let next_monday = today + Duration::days(days_to_monday as i64 + 7);
        return Some(next_monday.format("%Y-%m-%d").to_string());
    }
    if lower.contains("esta semana") || lower.contains("this week") {
        let days_to_friday = (4_i64 - today.weekday().num_days_from_monday() as i64).max(0);
        return Some(
            (today + Duration::days(days_to_friday))
                .format("%Y-%m-%d")
                .to_string(),
        );
    }
    let weekdays: [(&str, Weekday); 14] = [
        ("lunes", Weekday::Mon),
        ("martes", Weekday::Tue),
        ("miércoles", Weekday::Wed),
        ("miercoles", Weekday::Wed),
        ("jueves", Weekday::Thu),
        ("viernes", Weekday::Fri),
        ("sábado", Weekday::Sat),
        ("sabado", Weekday::Sat),
        ("domingo", Weekday::Sun),
        ("monday", Weekday::Mon),
        ("tuesday", Weekday::Tue),
        ("wednesday", Weekday::Wed),
        ("thursday", Weekday::Thu),
        ("friday", Weekday::Fri),
    ];
    // Fecha explícita: "el 25 de diciembre", "2026-12-25", "25/12"
    if let Some(d) = extract_explicit_date(lower, today) {
        return Some(d);
    }
    for (name, wd) in weekdays {
        if lower.contains(name) {
            let current = today.weekday().num_days_from_monday();
            let target = wd.num_days_from_monday();
            let mut delta = (target + 7 - current) % 7;
            if delta == 0 {
                delta = 7;
            }
            return Some(
                (today + Duration::days(delta as i64))
                    .format("%Y-%m-%d")
                    .to_string(),
            );
        }
    }
    None
}

fn extract_explicit_date(lower: &str, today: NaiveDate) -> Option<String> {
    let re_iso = regex::Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").ok()?;
    if let Some(c) = re_iso.captures(lower) {
        let d = NaiveDate::from_ymd_opt(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?);
        if let Some(d) = d {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    let re_dm = regex::Regex::new(r"\b(\d{1,2})/(\d{1,2})(?:/(\d{2,4}))?\b").ok()?;
    if let Some(c) = re_dm.captures(lower) {
        let day: u32 = c[1].parse().ok()?;
        let month: u32 = c[2].parse().ok()?;
        let year: i32 = match c.get(3) {
            Some(y) => {
                let y: i32 = y.as_str().parse().ok()?;
                if y < 100 {
                    2000 + y
                } else {
                    y
                }
            }
            None => today.year(),
        };
        if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_task_with_deadline() {
        let r = heuristic_capture("Tengo que preparar el portfolio antes del viernes");
        assert_eq!(r.kind, "task");
        let t = r.task.unwrap();
        assert!(t.title.to_lowercase().contains("portfolio"));
        assert!(t.due_date.is_some());
        assert!(!r.ai_used);
    }

    #[test]
    fn detects_idea_as_note() {
        let r = heuristic_capture("idea: app para organizar recetas con IA");
        assert_eq!(r.kind, "note");
        assert!(r.note.is_some());
    }

    #[test]
    fn detects_urgency() {
        let r = heuristic_capture("URGENTE: llamar al banco mañana");
        assert_eq!(r.task.unwrap().priority, Some(Priority::Urgent));
    }

    #[test]
    fn tomorrow_date() {
        let r = heuristic_capture("comprar pan mañana");
        let expected = (Utc::now().date_naive() + Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(r.task.unwrap().due_date, Some(expected));
    }

    #[test]
    fn explicit_iso_date() {
        let r = heuristic_capture("entregar informe 2027-03-15");
        assert_eq!(r.task.unwrap().due_date, Some("2027-03-15".into()));
    }
}
