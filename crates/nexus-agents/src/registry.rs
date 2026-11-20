//! Agentes por defecto de NEXUS: roles, allowlists de herramientas y
//! system prompts profesionales en español. La fecha actual se inyecta
//! en runtime desde el motor de ejecución.

use std::collections::HashMap;

use nexus_core::AgentDef;

pub struct AgentSpec {
    pub def: AgentDef,
    pub system_prompt: String,
}

pub struct AgentRegistry {
    agents: HashMap<String, AgentSpec>,
    order: Vec<String>,
}

const COMMON_RULES: &str = "\
Reglas comunes:
- Usa SIEMPRE las herramientas disponibles para obtener o modificar datos reales; nunca inventes tareas, notas, proyectos ni resultados.
- Si una herramienta falla o no está disponible, dilo claramente y trabaja con lo que tengas.
- Las fechas van en formato YYYY-MM-DD.
- Responde siempre en español, de forma concisa y profesional.
- Cuando crees o modifiques datos, confirma brevemente qué hiciste con los ids o títulos reales.";

impl AgentRegistry {
    pub fn with_defaults() -> Self {
        let read_only = [
            "tasks.list",
            "tasks.get",
            "projects.list",
            "projects.get",
            "notes.list",
            "notes.search",
            "notes.get",
            "inbox.list",
            "events.list",
            "dashboard.summary",
            "web.search",
        ];
        let mut orchestrator_tools: Vec<String> = read_only.iter().map(|s| s.to_string()).collect();
        orchestrator_tools.push("agents.delegate".into());

        let defs: Vec<(AgentDef, String)> = vec![
            (
                AgentDef {
                    id: "orchestrator".into(),
                    name: "Orquestador".into(),
                    description: "Coordina el equipo de agentes: analiza la petición y delega el trabajo real en el agente adecuado.".into(),
                    tools: orchestrator_tools,
                    max_iterations: 10,
                },
                format!(
                    "Eres el Orquestador de NEXUS, el coordinador del equipo de agentes personales del usuario.\n\
                     Tu rol: analizar la petición, consultar el estado real (tareas, proyectos, notas, dashboard) con herramientas de lectura \
                     y delegar el trabajo concreto con agents.delegate en el agente adecuado:\n\
                     - planner: planificación y desglose de tareas.\n\
                     - project_manager: gestión de proyectos y su seguimiento.\n\
                     - knowledge: base de conocimiento y notas.\n\
                     - researcher: investigación (web si está configurada, si no solo conocimiento interno).\n\
                     - executor: ejecución directa de cambios (crear, actualizar, eliminar, procesar inbox).\n\
                     NUNCA hagas mutaciones directas: no tienes herramientas de escritura; delega siempre.\n\
                     No delegues en ti mismo ni encadenes delegaciones innecesarias; una delegación bien definida por subobjetivo.\n\
                     Al recibir el resultado de un hijo, intégralo y responde al usuario con un resumen claro.\n\n{COMMON_RULES}"
                ),
            ),
            (
                AgentDef {
                    id: "planner".into(),
                    name: "Planificador".into(),
                    description: "Convierte objetivos en planes de tareas concretas y realistas.".into(),
                    tools: ["tasks.list", "tasks.get", "projects.list", "projects.get", "dashboard.summary", "notes.search", "tasks.create", "tasks.update", "events.list", "events.create", "events.update"]
                        .iter().map(|s| s.to_string()).collect(),
                    max_iterations: 8,
                },
                format!(
                    "Eres el Planificador de NEXUS. Tu rol: convertir objetivos del usuario en planes de acción concretos.\n\
                     Antes de planificar, consulta las tareas y proyectos existentes para no duplicar trabajo.\n\
                     Crea tareas accionables (título claro, descripción breve, prioridad y fecha límite realista cuando aplique) \
                     y usa subtareas (parent_id) para desglosar trabajos grandes.\n\
                     No elimines nada ni toques notas; céntrate en tareas y su estructura.\n\
                     Termina con un resumen del plan creado y sus próximos pasos.\n\n{COMMON_RULES}"
                ),
            ),
            (
                AgentDef {
                    id: "project_manager".into(),
                    name: "Gestor de proyectos".into(),
                    description: "Crea y mantiene proyectos, su progreso y la documentación asociada.".into(),
                    tools: ["projects.list", "projects.get", "projects.create", "projects.update", "tasks.list", "tasks.get", "tasks.create", "tasks.update", "notes.list", "notes.search", "notes.create", "dashboard.summary", "events.list", "events.create", "events.update"]
                        .iter().map(|s| s.to_string()).collect(),
                    max_iterations: 8,
                },
                format!(
                    "Eres el Gestor de Proyectos de NEXUS. Tu rol: mantener los proyectos del usuario organizados y al día.\n\
                     Revisa el progreso real con projects.get antes de proponer cambios; crea proyectos con objetivo medible, \
                     asocia tareas al proyecto correcto y documenta decisiones en notas vinculadas.\n\
                     Detecta tareas vencidas o bloqueadas y propón ajustes concretos de estado, prioridad o fechas.\n\
                     No elimines datos; archivar o pausar es preferible a borrar.\n\n{COMMON_RULES}"
                ),
            ),
            (
                AgentDef {
                    id: "knowledge".into(),
                    name: "Conocimiento".into(),
                    description: "Curador de la base de conocimiento: busca, resume y organiza notas.".into(),
                    tools: ["notes.list", "notes.search", "notes.get", "notes.create", "notes.update", "tasks.list", "tasks.get", "projects.list", "projects.get", "events.list"]
                        .iter().map(|s| s.to_string()).collect(),
                    max_iterations: 8,
                },
                format!(
                    "Eres el agente de Conocimiento de NEXUS, curador de la base de conocimiento personal del usuario.\n\
                     Busca primero con notes.search y notes.list antes de responder; cita los títulos reales de las notas que uses.\n\
                     Sintetiza la información en notas bien estructuradas (kind adecuado: note, idea, document o link), \
                     con etiquetas útiles y vínculo al proyecto cuando aplique.\n\
                     Actualiza notas existentes en lugar de duplicarlas. Nunca elimines notas.\n\
                     Si no hay información sobre el tema, dilo claramente y ofrece crear una nota inicial.\n\n{COMMON_RULES}"
                ),
            ),
            (
                AgentDef {
                    id: "researcher".into(),
                    name: "Investigador".into(),
                    description: "Investiga con búsqueda web (si está configurada) y la base de conocimiento; guarda hallazgos en notas.".into(),
                    tools: ["notes.list", "notes.search", "notes.get", "notes.create", "web.search", "tasks.list", "tasks.get"]
                        .iter().map(|s| s.to_string()).collect(),
                    max_iterations: 8,
                },
                format!(
                    "Eres el Investigador de NEXUS. Tu rol: investigar temas y guardar hallazgos verificables en notas.\n\
                     Usa web.search para fuentes externas. Si web.search no está disponible (sin proveedor configurado), \
                     dilo claramente al usuario y trabaja SOLO con la base de conocimiento interna (notes.search/notes.get); \
                     NUNCA inventes fuentes, URLs ni resultados de búsqueda.\n\
                     Guarda los hallazgos en una nota con kind document o link, citando las fuentes reales consultadas.\n\
                     Distingue siempre entre hechos verificados y tu propia síntesis.\n\n{COMMON_RULES}"
                ),
            ),
            (
                AgentDef {
                    id: "executor".into(),
                    name: "Ejecutor".into(),
                    description: "Ejecuta cambios concretos: tareas, proyectos, notas e inbox.".into(),
                    tools: ["tasks.list", "tasks.get", "tasks.create", "tasks.update", "tasks.delete", "projects.create", "projects.update", "projects.list", "projects.get", "notes.create", "notes.update", "notes.list", "notes.get", "notes.delete", "inbox.list", "inbox.process", "dashboard.summary", "events.list", "events.create", "events.update", "events.delete"]
                        .iter().map(|s| s.to_string()).collect(),
                    max_iterations: 8,
                },
                format!(
                    "Eres el Ejecutor de NEXUS. Tu rol: ejecutar cambios concretos y verificarlos.\n\
                     Antes de modificar, lee el estado real (get/list) para usar ids correctos; nunca supongas ids.\n\
                     Las eliminaciones son destructivas y pueden pedir confirmación al usuario: explica qué vas a borrar y por qué.\n\
                     Procesa el inbox convirtiéndolo en tareas o notas bien formadas.\n\
                     Después de cada cambio, verifica el resultado y resume exactamente qué quedó hecho.\n\n{COMMON_RULES}"
                ),
            ),
        ];

        let mut agents = HashMap::new();
        let mut order = Vec::new();
        for (def, prompt) in defs {
            order.push(def.id.clone());
            agents.insert(
                def.id.clone(),
                AgentSpec {
                    def,
                    system_prompt: prompt,
                },
            );
        }
        Self { agents, order }
    }

    pub fn get(&self, id: &str) -> Option<&AgentSpec> {
        self.agents.get(id)
    }

    pub fn list(&self) -> Vec<AgentDef> {
        self.order
            .iter()
            .filter_map(|id| self.agents.get(id).map(|a| a.def.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_default_agents_with_correct_allowlists() {
        let registry = AgentRegistry::with_defaults();
        let defs = registry.list();
        assert_eq!(defs.len(), 6);
        let ids: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        for expected in [
            "orchestrator",
            "planner",
            "project_manager",
            "knowledge",
            "researcher",
            "executor",
        ] {
            assert!(ids.contains(&expected), "falta {expected}");
        }

        let orch = registry.get("orchestrator").unwrap();
        assert!(orch.def.tools.contains(&"agents.delegate".to_string()));
        for forbidden in [
            "tasks.create",
            "tasks.update",
            "tasks.delete",
            "notes.create",
            "projects.create",
            "inbox.process",
        ] {
            assert!(
                !orch.def.tools.contains(&forbidden.to_string()),
                "orchestrator no debe tener {forbidden}"
            );
        }
        for read in [
            "tasks.list",
            "tasks.get",
            "projects.list",
            "projects.get",
            "notes.list",
            "notes.search",
            "notes.get",
            "dashboard.summary",
        ] {
            assert!(orch.def.tools.contains(&read.to_string()));
        }

        let executor = registry.get("executor").unwrap();
        assert!(executor.def.tools.contains(&"tasks.delete".to_string()));
        assert!(executor.def.tools.contains(&"inbox.process".to_string()));
        assert!(!executor.def.tools.contains(&"agents.delegate".to_string()));

        let researcher = registry.get("researcher").unwrap();
        assert!(researcher.def.tools.contains(&"web.search".to_string()));
        assert!(!researcher.def.tools.contains(&"notes.delete".to_string()));

        for def in &defs {
            assert!(!def.description.is_empty());
            let spec = registry.get(&def.id).unwrap();
            assert!(spec.system_prompt.contains("español"));
            assert!(spec.system_prompt.contains("YYYY-MM-DD"));
        }
        assert!(registry.get("inexistente").is_none());
    }
}
