//! Validación estructural de flujos: sin ciclos, sin referencias rotas,
//! acciones y agentes conocidos, límites de tamaño.

use std::collections::{HashMap, HashSet};

use nexus_core::{AutomationFlow, Condition, NexusError, Result, StepKind};

use crate::actions;
use crate::triggers;

pub const MAX_STEPS: usize = 100;
pub const MAX_CONDITION_DEPTH: usize = 8;

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
            StepKind::Action { action, .. } => {
                if !actions::is_known(action) {
                    return Err(NexusError::validation(format!(
                        "acción desconocida: {action}"
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
    match trigger {
        nexus_core::Trigger::Schedule { spec } => triggers::validate_schedule(spec),
        nexus_core::Trigger::Event { event, .. } => {
            if !triggers::is_known_event(event) {
                return Err(NexusError::validation(format!(
                    "evento desconocido: {event}"
                )));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::{AutomationFlow, CompareOp, Condition, FlowStep, StepKind};

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
}
