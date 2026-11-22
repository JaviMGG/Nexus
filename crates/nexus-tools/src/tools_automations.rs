//! Herramientas del Automation Center: listar, consultar, crear y
//! activar/desactivar automatizaciones. Crear y activar piden siempre
//! autorización del usuario (always_confirm).

use std::sync::Arc;

use nexus_core::{NexusError, Result, Risk};
use serde_json::{json, Value};

use crate::builtins::{define_tool, dto, enum_prop, req_str, schema, val};
use crate::{Tool, ToolContext};

define_tool!(
    AutomationsList,
    "automations.list",
    Risk::Read,
    "Lista las automatizaciones reales del usuario; con enabled_only=true solo las activadas.",
    schema(json!({ "enabled_only": { "type": "boolean" } }), &[]),
    |args, ctx| {
        let enabled_only = args
            .get("enabled_only")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        val(ctx.automation_ops()?.list(enabled_only).await?)
    }
);

define_tool!(
    AutomationsGet,
    "automations.get",
    Risk::Read,
    "Obtiene el detalle de una automatización por id: trigger, flujo, límites y estado.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        val(ctx.automation_ops()?.get(&id).await?)
    }
);

define_tool!(
    AutomationsCreate,
    "automations.create",
    Risk::Write,
    confirm,
    "Crea una automatización real (trigger + flujo). Requiere autorización del usuario; por defecto se crea desactivada.",
    schema(
        json!({
            "name": { "type": "string" },
            "description": { "type": "string" },
            "trigger": {
                "type": "object",
                "description": "Forma: {\"type\":\"schedule\",\"spec\":{\"kind\":\"daily\",\"time\":\"08:00\"}} o {\"type\":\"event\",\"event\":\"task.created\",\"filter\":{}}. Kinds de schedule: daily/weekly/monthly/interval/once."
            },
            "flow": {
                "type": "object",
                "description": "Forma: {\"version\":1,\"entry\":\"p1\",\"steps\":[{\"id\":\"p1\",\"type\":\"action\",\"action\":\"tasks.complete\",\"params\":{\"task_id\":\"task\"},\"next\":null}]}. Tipos de paso: action/agent/condition/delay/notification/end."
            },
            "enabled": {
                "type": "boolean",
                "description": "por defecto false; crear activada requiere confirmación"
            },
            "mode": enum_prop(&["local", "ai", "hybrid"])
        }),
        &["name", "trigger"]
    ),
    |args, ctx| {
        let mut data = json!({
            "name": req_str(&args, "name")?,
            "trigger": args
                .get("trigger")
                .filter(|v| v.is_object())
                .ok_or_else(|| {
                    NexusError::validation("falta el campo obligatorio 'trigger' (object)")
                })?
                .clone(),
        });
        for key in ["description", "flow", "enabled", "mode"] {
            if let Some(v) = args.get(key) {
                data[key] = v.clone();
            }
        }
        let data = dto::<nexus_core::CreateAutomation>(data)?;
        val(ctx.automation_ops()?.create(data).await?)
    }
);

define_tool!(
    AutomationsSetEnabled,
    "automations.set_enabled",
    Risk::Write,
    confirm,
    "Activa o desactiva una automatización por id. Requiere autorización del usuario.",
    schema(
        json!({
            "id": { "type": "string" },
            "enabled": { "type": "boolean" }
        }),
        &["id", "enabled"]
    ),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let enabled = args
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or_else(|| NexusError::validation("falta 'enabled' (boolean)"))?;
        val(ctx.automation_ops()?.set_enabled(&id, enabled).await?)
    }
);

pub(crate) fn tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(AutomationsList),
        Arc::new(AutomationsGet),
        Arc::new(AutomationsCreate),
        Arc::new(AutomationsSetEnabled),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_args, AutomationOps};
    use nexus_core::{Automation, CreateAutomation};
    use tokio::sync::Mutex;

    struct MockOps {
        items: Mutex<Vec<Automation>>,
    }

    fn automation(id: &str, name: &str, enabled: bool) -> Automation {
        serde_json::from_value(json!({
            "id": id,
            "name": name,
            "description": "",
            "icon": "",
            "enabled": enabled,
            "mode": "local",
            "trigger": { "type": "schedule", "spec": { "kind": "daily", "time": "08:00" } },
            "conditions": null,
            "flow": { "version": 1, "steps": [], "entry": null },
            "permissions": { "write": "auto" },
            "limits": {},
            "policy": "skip_if_active",
            "template_id": null,
            "last_run_at": null,
            "next_run_at": null,
            "run_count": 0,
            "last_status": null,
            "created_at": "2026-11-21T00:00:00Z",
            "updated_at": "2026-11-21T00:00:00Z"
        }))
        .unwrap()
    }

    #[async_trait::async_trait]
    impl AutomationOps for MockOps {
        async fn list(&self, enabled_only: bool) -> Result<Vec<Automation>> {
            let items = self.items.lock().await;
            Ok(items
                .iter()
                .filter(|a| !enabled_only || a.enabled)
                .cloned()
                .collect())
        }

        async fn get(&self, id: &str) -> Result<Automation> {
            self.items
                .lock()
                .await
                .iter()
                .find(|a| a.id == id)
                .cloned()
                .ok_or_else(|| NexusError::NotFound(format!("automatización {id}")))
        }

        async fn create(&self, data: CreateAutomation) -> Result<Automation> {
            let mut a = automation("auto-nueva", &data.name, data.enabled.unwrap_or(false));
            a.description = data.description;
            a.trigger = data.trigger;
            if let Some(flow) = data.flow {
                a.flow = flow;
            }
            self.items.lock().await.push(a.clone());
            Ok(a)
        }

        async fn set_enabled(&self, id: &str, enabled: bool) -> Result<Automation> {
            let mut items = self.items.lock().await;
            let a = items
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(|| NexusError::NotFound(format!("automatización {id}")))?;
            a.enabled = enabled;
            Ok(a.clone())
        }
    }

    async fn ctx() -> ToolContext {
        ToolContext {
            services: nexus_services::Services::new(
                nexus_db::Db::connect_memory().await.unwrap(),
            ),
            run_id: "t".into(),
            agent_id: "executor".into(),
            automation: Some(Arc::new(MockOps {
                items: Mutex::new(vec![
                    automation("auto-1", "Resumen diario", true),
                    automation("auto-2", "Apagada", false),
                ]),
            })),
        }
    }

    fn tool(name: &str) -> Arc<dyn Tool> {
        tools().into_iter().find(|t| t.spec().name == name).unwrap()
    }

    #[tokio::test]
    async fn list_and_get_return_mock_data() {
        let ctx = ctx().await;
        let list = tool("automations.list");
        let all = list.execute(json!({}), &ctx).await.unwrap();
        assert_eq!(all.as_array().unwrap().len(), 2);
        let enabled = list
            .execute(json!({"enabled_only": true}), &ctx)
            .await
            .unwrap();
        let arr = enabled.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "auto-1");

        let get = tool("automations.get");
        let one = get.execute(json!({"id": "auto-2"}), &ctx).await.unwrap();
        assert_eq!(one["name"], "Apagada");
        assert_eq!(one["enabled"], false);
    }

    #[tokio::test]
    async fn create_without_name_fails_validation() {
        let ctx = ctx().await;
        let create = tool("automations.create");
        let args = json!({ "trigger": { "type": "schedule", "spec": { "kind": "daily", "time": "08:00" } } });
        let err = validate_args(&create.spec(), &args).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        let err = create.execute(args, &ctx).await.unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
    }

    #[tokio::test]
    async fn create_valid_reaches_mock() {
        let ctx = ctx().await;
        let create = tool("automations.create");
        let args = json!({
            "name": "Completar tarea al crearla",
            "description": "demo",
            "trigger": { "type": "event", "event": "task.created", "filter": {} },
            "flow": {
                "version": 1,
                "entry": "p1",
                "steps": [
                    { "id": "p1", "type": "action", "action": "tasks.complete", "params": { "task_id": "task" }, "next": null }
                ]
            },
            "mode": "local"
        });
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        assert_eq!(created["name"], "Completar tarea al crearla");
        assert_eq!(created["enabled"], false);
        assert_eq!(created["trigger"]["type"], "event");
        assert_eq!(created["flow"]["entry"], "p1");

        let got = tool("automations.get")
            .execute(json!({"id": created["id"]}), &ctx)
            .await
            .unwrap();
        assert_eq!(got["id"], created["id"]);

        let toggled = tool("automations.set_enabled")
            .execute(json!({"id": created["id"], "enabled": true}), &ctx)
            .await
            .unwrap();
        assert_eq!(toggled["enabled"], true);
    }

    #[test]
    fn risks_and_confirmation() {
        for name in ["automations.create", "automations.set_enabled"] {
            let t = tool(name);
            assert_eq!(t.risk(), Risk::Write);
            assert!(t.always_confirm());
        }
        for name in ["automations.list", "automations.get"] {
            let t = tool(name);
            assert_eq!(t.risk(), Risk::Read);
            assert!(!t.always_confirm());
        }
    }
}
