//! Validación estructural de flujos: sin ciclos, sin referencias rotas,
//! acciones y agentes conocidos, límites de tamaño.

use std::collections::{HashMap, HashSet};

use nexus_core::{AutomationFlow, Condition, NexusError, Result, StepKind, TriggerEntry};

use crate::actions;

pub const MAX_STEPS: usize = 100;
pub const MAX_CONDITION_DEPTH: usize = 8;

pub fn validate_all(
    triggers: &[TriggerEntry],
    conditions: Option<&Condition>,
    flow: &AutomationFlow,
    known_agents: &[String],
) -> Result<()> {
    for entry in triggers {
        validate_trigger(&entry.trigger)?;
    }
    if let Some(c) = conditions {
        validate_condition(c, 0)?;
    }
    validate(flow, known_agents)
}

pub fn validate(flow: &AutomationFlow, known_agents: &[String]) -> Result<()> {
    if flow.steps.len() > MAX_STEPS {
        return Err(NexusError::validation(format!(
            "el flujo no puede tener más de {MAX_STEPS} pasos"
        )));
    }
    let ids: HashSet<&str> = flow.steps.iter().map(|s| s.id.as_str()).collect();
    if ids.len() != flow.steps.len() {
        return Err(NexusError::validation("hay pasos con id duplicado"));
    }
    if let Some(entry) = &flow.entry {
        if !ids.contains(entry.as_str()) {
            return Err(NexusError::validation("el paso de entrada no existe"));
        }
    } else if !flow.steps.is_empty() {
        return Err(NexusError::validation("el flujo no tiene paso de entrada"));
    }
    for step in &flow.steps {
        let targets: Vec<Option<&str>> = match &step.kind {
            StepKind::Condition {
                condition,
                on_true,
                on_false,
            } => {
                validate_condition(condition, 0)?;
                vec![
                    on_true.as_deref(),
                    on_false.as_deref(),
                    step.next.as_deref(),
                ]
            }
            StepKind::Action {
                action, params, ..
            } => {
                if !actions::is_known(action) {
                    return Err(NexusError::validation(format!(
                        "acción desconocida: {action}"
                    )));
                }
                if let Some(validation_error) =
                    crate::validation::validate_action_params(action, params)
                        .into_iter()
                        .next()
                {
                    return Err(NexusError::validation(format!(
                        "paso {}: {}",
                        step.id, validation_error.message
                    )));
                }
                vec![step.next.as_deref()]
            }
            StepKind::Agent { agent_id, goal, .. } => {
                if !known_agents.iter().any(|a| a == agent_id) {
                    return Err(NexusError::validation(format!(
                        "agente desconocido: {agent_id}"
                    )));
                }
                if goal.trim().is_empty() {
                    return Err(NexusError::validation(
                        "el paso de agente necesita un objetivo",
                    ));
                }
                vec![step.next.as_deref()]
            }
            StepKind::Delay { seconds } => {
                if *seconds < 0 || *seconds > 3600 {
                    return Err(NexusError::validation(
                        "el retraso debe estar entre 0 y 3600 segundos",
                    ));
                }
                vec![step.next.as_deref()]
            }
            StepKind::Notification { title, .. } => {
                if title.trim().is_empty() {
                    return Err(NexusError::validation("la notificación necesita un título"));
                }
                vec![step.next.as_deref()]
            }
            StepKind::End => vec![],
        };
        for t in targets.into_iter().flatten() {
            if !ids.contains(t) {
                return Err(NexusError::validation(format!(
                    "el paso {} apunta a un destino inexistente: {t}",
                    step.id
                )));
            }
        }
    }
    check_cycles(flow)?;
    Ok(())
}

fn check_cycles(flow: &AutomationFlow) -> Result<()> {
    let map: HashMap<&str, Vec<&str>> = flow
        .steps
        .iter()
        .map(|s| {
            let mut nexts = Vec::new();
            if let Some(n) = &s.next {
                nexts.push(n.as_str());
            }
            if let StepKind::Condition {
                on_true, on_false, ..
            } = &s.kind
            {
                if let Some(n) = on_true {
                    nexts.push(n.as_str());
                }
                if let Some(n) = on_false {
                    nexts.push(n.as_str());
                }
            }
            (s.id.as_str(), nexts)
        })
        .collect();

    // DFS con coloreado: gris = en curso, negro = terminado.
    let mut color: HashMap<&str, u8> = HashMap::new();
    fn visit<'a>(
        node: &'a str,
        map: &HashMap<&'a str, Vec<&'a str>>,
        color: &mut HashMap<&'a str, u8>,
    ) -> Result<()> {
        match color.get(node) {
            Some(1) => {
                return Err(NexusError::validation(
                    "el flujo contiene un ciclo; no se permiten bucles",
                ))
            }
            Some(2) => return Ok(()),
            _ => {}
        }
        color.insert(node, 1);
        if let Some(nexts) = map.get(node) {
            for n in nexts {
                visit(n, map, color)?;
            }
        }
        color.insert(node, 2);
        Ok(())
    }
    for id in map.keys() {
        visit(id, &map, &mut color)?;
    }
    Ok(())
}

pub fn validate_condition(cond: &Condition, depth: usize) -> Result<()> {
    if depth > MAX_CONDITION_DEPTH {
        return Err(NexusError::validation("condiciones demasiado anidadas"));
    }
    match cond {
        Condition::And { conditions } | Condition::Or { conditions } => {
            for c in conditions {
                validate_condition(c, depth + 1)?;
            }
        }
        Condition::Not { condition } => validate_condition(condition, depth + 1)?,
        Condition::Metric { metric, .. } => {
            if !crate::conditions::is_known_metric(metric) {
                return Err(NexusError::validation(format!(
                    "métrica desconocida: {metric}"
                )));
            }
        }
        Condition::EventField { field, .. } => {
            if field.trim().is_empty() {
                return Err(NexusError::validation(
                    "la condición de evento necesita un campo",
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_trigger(trigger: &nexus_core::Trigger) -> Result<()> {
    match crate::validation::validate_trigger(trigger).into_iter().next() {
        Some(error) => Err(NexusError::validation(error.message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::{AutomationFlow, CompareOp, Condition, FlowStep, ScheduleSpec, StepKind};

    fn agents() -> Vec<String> {
        vec!["planner".into(), "executor".into()]
    }

    fn step(id: &str, kind: StepKind, next: Option<&str>) -> FlowStep {
        FlowStep {
            id: id.into(),
            kind,
            next: next.map(Into::into),
        }
    }

    #[test]
    fn valid_linear_flow() {
        let flow = AutomationFlow {
            version: 1,
            entry: Some("a".into()),
            positions: Default::default(),
            steps: vec![
                step(
                    "a",
                    StepKind::Notification {
                        title: "hola".into(),
                        body: String::new(),
                    },
                    Some("b"),
                ),
                step("b", StepKind::End, None),
            ],
        };
        assert!(validate(&flow, &agents()).is_ok());
    }

    #[test]
    fn rejects_cycles() {
        let flow = AutomationFlow {
            version: 1,
            entry: Some("a".into()),
            positions: Default::default(),
            steps: vec![
                step("a", StepKind::Delay { seconds: 1 }, Some("b")),
                step("b", StepKind::Delay { seconds: 1 }, Some("a")),
            ],
        };
        assert!(validate(&flow, &agents()).is_err());
    }

    #[test]
    fn rejects_broken_reference() {
        let flow = AutomationFlow {
            version: 1,
            entry: Some("a".into()),
            positions: Default::default(),
            steps: vec![step("a", StepKind::Delay { seconds: 1 }, Some("ghost"))],
        };
        assert!(validate(&flow, &agents()).is_err());
    }

    #[test]
    fn rejects_unknown_action_and_agent() {
        let flow = AutomationFlow {
            version: 1,
            entry: Some("a".into()),
            positions: Default::default(),
            steps: vec![step(
                "a",
                StepKind::Action {
                    action: "sql.execute".into(),
                    params: serde_json::json!({}),
                    approval: None,
                },
                None,
            )],
        };
        assert!(validate(&flow, &agents()).is_err());
        let flow = AutomationFlow {
            version: 1,
            entry: Some("a".into()),
            positions: Default::default(),
            steps: vec![step(
                "a",
                StepKind::Agent {
                    agent_id: "hacker".into(),
                    goal: "x".into(),
                    approval: None,
                },
                None,
            )],
        };
        assert!(validate(&flow, &agents()).is_err());
    }

    #[test]
    fn branching_condition_ok() {
        let flow = AutomationFlow {
            version: 1,
            entry: Some("c".into()),
            positions: Default::default(),
            steps: vec![
                step(
                    "c",
                    StepKind::Condition {
                        condition: Condition::Metric {
                            metric: "open_tasks".into(),
                            op: CompareOp::Gt,
                            value: serde_json::json!(0),
                            params: serde_json::json!({}),
                        },
                        on_true: Some("t".into()),
                        on_false: Some("f".into()),
                    },
                    None,
                ),
                step(
                    "t",
                    StepKind::Notification {
                        title: "hay tareas".into(),
                        body: String::new(),
                    },
                    None,
                ),
                step("f", StepKind::End, None),
            ],
        };
        assert!(validate(&flow, &agents()).is_ok());
    }

    #[test]
    fn validates_schedule_formats() {
        let good = nexus_core::Trigger::Schedule {
            spec: ScheduleSpec {
                kind: "once".into(),
                time: Some("08:30".into()),
                date: Some("2026-12-31".into()),
                timezone: Some("Europe/Madrid".into()),
                ..ScheduleSpec::default()
            },
        };
        assert!(validate_trigger(&good).is_ok());

        let bad = nexus_core::Trigger::Schedule {
            spec: ScheduleSpec {
                kind: "once".into(),
                time: Some("8:30".into()),
                date: Some("31/12/2026".into()),
                timezone: Some("Madrid/Europe".into()),
                ..ScheduleSpec::default()
            },
        };
        let errors = crate::validation::validate_trigger(&bad);
        assert!(errors.iter().any(|e| e.field == "spec.time"));
        assert!(errors.iter().any(|e| e.field == "spec.date"));
        assert!(errors.iter().any(|e| e.field == "spec.timezone"));
    }

    #[test]
    fn validates_event_filter_formats() {
        use nexus_core::Trigger;

        let good = Trigger::Event {
            event: "task.created".into(),
            filter: serde_json::json!({
                "title": "Revisar",
                "status": "todo",
                "due_date": "2026-12-31",
                "position": 1,
                "tags": ["nexus"]
            }),
        };
        assert!(crate::validation::validate_trigger(&good).is_empty());

        let bad = Trigger::Event {
            event: "task.created".into(),
            filter: serde_json::json!({
                "due_date": "31/12/2026",
                "status": "unknown",
                "position": "1",
                "ghost": "x"
            }),
        };
        let errors = crate::validation::validate_trigger(&bad);
        assert!(errors.iter().any(|e| e.field == "filter.due_date"));
        assert!(errors.iter().any(|e| e.field == "filter.status"));
        assert!(errors.iter().any(|e| e.field == "filter.position"));
        assert!(errors.iter().any(|e| e.field == "filter.ghost"));
    }

    #[test]
    fn validates_action_due_date_format() {
        fn action_flow(params: serde_json::Value) -> AutomationFlow {
            AutomationFlow {
                version: 1,
                entry: Some("a".into()),
                positions: Default::default(),
                steps: vec![step(
                    "a",
                    StepKind::Action {
                        action: "tasks.create".into(),
                        params,
                        approval: None,
                    },
                    None,
                )],
            }
        }

        let valid = action_flow(serde_json::json!({
            "title": "Revisar",
            "due_date": "2026-12-31"
        }));
        assert!(validate(&valid, &agents()).is_ok());

        let template = action_flow(serde_json::json!({
            "title": "Revisar",
            "due_date": "{{due_date}}"
        }));
        assert!(validate(&template, &agents()).is_ok());

        let invalid = action_flow(serde_json::json!({
            "title": "Revisar",
            "due_date": "31/12/2026"
        }));
        let error = validate(&invalid, &agents()).unwrap_err();
        assert!(error.to_string().contains("fecha límite inválida"));
    }
}
