# Arquitectura de NEXUS — Monolito modular

NEXUS es un **monolito modular**: un único proceso y una única base de datos, pero con módulos
(crates Rust) de responsabilidad única y dependencias unidireccionales. Cada módulo podría
extraerse a un servicio independiente sin reescribir el dominio.

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Frontend (React + TS)                           │
│  páginas · componentes ui · stores (theme/ui) · TanStack Query · SSE   │
└───────────────┬────────────────────────────────────▲───────────────────┘
                │ POST /api/command                  │ GET /api/events (SSE)
┌───────────────▼────────────────────────────────────┴───────────────────┐
│  nexus-api (Axum, loopback + token)   ·   nexus-desktop (Tauri 2)      │
├────────────────────────────────────────────────────────────────────────┤
│  nexus-app — composition root + dispatch(command, payload)             │
├──────────────────────────────┬─────────────────────────────────────────┤
│  nexus-agents                │  nexus-ai                               │
│  AgentRegistry (6 agentes)   │  AiProvider trait                       │
│  ExecutionEngine:            │  OpenAiCompatibleProvider (Qwen)        │
│   bucle de herramientas      │  ProviderRegistry + rate limiting       │
│   delegación · límites       │  streaming SSE · health · errores       │
│   cancelación · aprobaciones ├─────────────────────────────────────────┤
├──────────────────────────────┤  nexus-tools                            │
│  nexus-automation            │  Tool trait + ToolRegistry (19 tools)   │
│  AutomationManager:          │  riesgos Read/Write/Destructive         │
│   TriggerRegistry · scheduler│  validación JSON Schema                 │
│   ConditionEngine · acciones │                                         │
│   ExecutionEngine de flujos  │                                         │
│   AgentDispatcher · history  │                                         │
│   autorizaciones · plantillas│                                         │
├──────────────────────────────┴─────────────────────────────────────────┤
│  nexus-services — servicios de aplicación                              │
│  tasks · projects · notes · inbox · chat · activity(undo) · settings   │
│  dashboard · backup · capture heurística · validación                  │
│  canvas · databases (campos/vistas/fórmulas) · meals · files           │
├────────────────────────────────────────────────────────────────────────┤
│  nexus-db — SQLite (WAL) + SQLx · migraciones versionadas · FTS5       │
├────────────────────────────────────────────────────────────────────────┤
│  nexus-core — dominio puro: entidades, DTOs, errores, AppEvent         │
└────────────────────────────────────────────────────────────────────────┘
```

## Reglas de dependencia

- `nexus-core` no depende de nada del workspace (dominio puro, sin infraestructura).
- `nexus-db` solo conoce `core`. Los errores de SQLx se convierten a `NexusError` en la frontera.
- `nexus-services` orquesta repositorios; aplica validación y reglas de negocio; registra
  auditoría con información de deshacer. No conoce IA.
- `nexus-ai` conoce `core` y `services` (para leer configuración/secretos). Ningún otro módulo
  depende de un proveedor concreto.
- `nexus-tools` conoce `services`: las herramientas son adaptadores finos sobre los servicios,
  con esquema JSON, nivel de riesgo y validación de argumentos.
- `nexus-agents` conoce `ai` + `tools` + `services`: es el único lugar donde existe el bucle
  agéntico.
- `nexus-automation` conoce `agents` + `ai` + `services`: orquesta automatizaciones locales y
  reutiliza el motor multiagente existente (no duplica el bucle agéntico).
- `nexus-app` ensambla todo y expone `dispatch(command, payload) -> Result<Value>`: un único
  punto de entrada usado por HTTP y por Tauri. Añadir un caso de uso = añadir un comando.
- `nexus-api` / `nexus-desktop` son capas de transporte intercambiables. `nexus-desktop`
  añade bandeja del sistema (feature `tray-icon`): con `settings.app.automations.run_in_background`,
  cerrar la ventana oculta la app en la bandeja y el scheduler sigue corriendo; el flag se
  refresca en vivo escuchando `DataChanged{settings}` del bus. En Linux la bandeja carga
  `libayatana-appindicator3` en runtime; si falta, se degrada sin bandeja (cierre normal).

## Flujo de una petición

1. El frontend llama `invoke("tasks.create", {data})` (`lib/api.ts`).
2. En Tauri obtiene puerto/token con el comando `server_info`; en web, con `GET /api/session`.
3. `POST /api/command` → validación de Host loopback + token → `AppContext::dispatch`.
4. El servicio valida, muta en una transacción SQLite y registra actividad (con `undo_json`).
5. `dispatch` emite `AppEvent::DataChanged` por el canal broadcast.
6. El frontend recibe el evento por SSE e invalida las queries afectadas (TanStack Query).

## Sistema multiagente

### Agentes

| Agente | Rol | Herramientas |
|---|---|---|
| `orchestrator` | Entiende, divide, delega y supervisa | lecturas + `agents.delegate` |
| `planner` | Organiza día/semana, prioriza, detecta sobrecarga | lecturas + `tasks.create/update` |
| `project_manager` | Estado de proyectos, atrasos, bloqueos | projects.*, tasks.*, notes.* |
| `knowledge` | Memoria: busca, lee, clasifica y relaciona notas | notes.*, lecturas |
| `researcher` | Investiga y guarda informes | notes.*, `web.search` (si hay proveedor) |
| `executor` | Ejecuta acciones y verifica resultados | mutaciones completas + inbox |

Cada agente tiene prompt propio (en español), allowlist de herramientas y límite de iteraciones.

### Bucle de ejecución (real, no simulado)

```
goal → contexto (prompt + fecha + historial de chat)
  └─► proveedor.chat_stream(contexto, tools)
        ├─ sin tool_calls → respuesta final → persistir mensaje + run Completed
        └─ con tool_calls:
             ¿permitida para el agente? ─ no → ToolCall Denied
             validate_args(JSON Schema) ─ falla → error al modelo
             ¿needs_approval(risk, permisos)? ─ sí → Approval + run WaitingApproval
                                                  (espera decisión o cancelación)
             ejecutar herramienta (timeout 60s) → ToolCall Ok/Error + auditoría
             resultado → mensaje tool → siguiente iteración
```

Controles implementados:

- **Límites**: iteraciones por run (`min(agente, settings.limits)`), tokens por run, profundidad
  de delegación, peticiones por minuto por proveedor (ventana deslizante).
- **Cancelación cooperativa**: `CancellationToken` por run, propagada a hijos; puntos de
  comprobación en cada iteración, durante esperas de aprobación y streaming.
- **Reintentos**: solo errores transitorios (red/429/5xx), máx. 2 con backoff 1s/3s.
- **Trazabilidad**: cada tool call se persiste (args, resultado, estado, duración) y cada
  mutación queda en `activity_events` con actor `agent:<id>`.
- **Recuperación**: al arrancar, los runs huérfanos de procesos anteriores se marcan `failed`.
- **Seguridad**: el modelo nunca ejecuta SQL, shell ni accede al filesystem. Solo herramientas
  tipadas y validadas. Lecturas automáticas; escrituras según permisos; destructivas requieren
  aprobación explícita por defecto.

### Modo sin IA

`ProviderRegistry::default_provider()` devuelve `AiUnavailable` si no hay proveedor usable.
`chat.send` guarda el mensaje del usuario y responde con un mensaje assistant de modo manual;
`agents.start` devuelve error claro sin crear runs; la captura rápida cae a la heurística local.
El estado (`connected/disconnected/error`) se emite por SSE y se muestra en el sidebar.

## Herramientas: Canvas, Database y Meal Planner

Tres módulos internos del monolito (migraciones 0004–0007), cada uno con su capa completa:
modelos en `nexus-core` (`canvas.rs`, `databases.rs`, `meals.rs`), repos en `nexus-db`,
servicios en `nexus-services` y comandos enrutados por prefijo desde `dispatch.rs` hacia
`dispatch_canvas.rs` / `dispatch_databases.rs` / `dispatch_meals.rs` en `nexus-app`.

- **Canvas**: documento JSON versionado `{elements, edges, viewport}` validado en backend
  (tipos, geometría finita, aristas a elementos existentes, límites 5000/10000). Undo/redo y
  autoguardado (debounce 800 ms) viven en el editor React Flow; `save_document` no audita.
  Referencias cruzadas por id (`task_ref`, `project_ref`, `note_ref`, `database_ref`,
  `recipe_ref`) sin duplicar datos. Export PNG/SVG en frontend (html-to-image), JSON en backend.
- **Database**: esquemas dinámicos (`db_fields.kind` + `config_json`), valores por registro en
  `values_json` validados por tipo; borrado de campos y registros con soft-delete (los datos
  no se pierden). Fórmulas con parser Pratt propio (sin eval, límites de profundidad);
  filtros/orden con QueryBuilder + `json_extract` (jamás SQL de usuario/IA). Relaciones como
  arrays de ids con limpieza transaccional al borrar. Import CSV/JSON con preview y mapeo.
- **Meals**: recetas con ingredientes estructurados; consolidación de la compra por
  (nombre normalizado, unidad) — nunca convierte unidades; planner anclado al lunes
  (`week_start`), menús plantilla con `day_offset` relativo; despensa con caducidad solo de
  fechas del usuario y `pantry_matches` por cobertura. Nutrición opcional, jamás estimada.
- **Files** (compartido): bytes en `<data_dir>/files/<uuid>` con permisos 0600, metadatos en
  la tabla `files`, máx. 25 MB; comandos `files.*` con base64. `Services::with_data_dir`
  inyecta el directorio real; los tests usan `Services::new` (dir por defecto).
- **IA opcional**: helper `nexus_app::ai_json::ask_json` (timeout 60 s, JSON estricto,
  `AiUnavailable` claro sin proveedor). Todas las salidas de IA son propuestas revisables.
- **Backups**: la restauración SQLite incluye las tablas nuevas (orden respetando FKs) y la
  exportación JSON (version 2) incluye canvases, databases+registros, recetas, despensa,
  menús, listas y metadatos de archivos.

## Automation Center (`nexus-automation`)

Motor local de automatizaciones con responsabilidades separadas:

| Módulo | Responsabilidad |
|---|---|
| `manager.rs` (AutomationManager) | CRUD validado, cola con semáforo (máx. 3 concurrentes), políticas de ejecución, recuperación tras reinicio, overview |
| `triggers.rs` (TriggerRegistry) | Catálogo de eventos de dominio + cálculo de próxima ocurrencia con `chrono-tz` (DST incluido) |
| `conditions.rs` (ConditionEngine) | Árbol AND/OR/NOT + métricas deterministas sobre repositorios reales; nunca SQL libre |
| `actions.rs` (ActionRegistry) | Catálogo cerrado de acciones tipadas sobre los servicios; riesgo read/write/sensitive; interpolación `{{campo}}` |
| `runner.rs` (ExecutionEngine de flujos) | Recorrido de pasos con presupuesto, timeouts, cancelación, IF/ELSE, reintentos e historial |
| `dispatcher.rs` (AgentDispatcher) | Puente al `ExecutionEngine` multiagente: espera del run, tokens, límites, modo híbrido sin proveedor |
| `templates.rs` | 16 plantillas funcionales (CreateAutomation completos) con dificultad 1-5 y ejemplos Local/IA/Híbrido |
| `flow.rs` | Validación estructural: sin ciclos (DFS), referencias válidas, acciones/agentes conocidos |

Persistencia (migración `0003_automations.sql`): `automations` (trigger/condiciones/flujo como
JSON versionado, incluidas las posiciones de los nodos del lienzo), `automation_executions`,
`automation_step_logs`, `automation_approvals`, `automation_fired_marks` (deduplicación por disparo).

Frontend del Automation Center: `pages/Automations.tsx` (5 pestañas) + `components/automations/`
(editor a página completa `AutomationEditor.tsx` —sin diálogo, con vista de edición in-place—,
lienzo **React Flow** `FlowCanvas.tsx` orientado de izquierda a derecha, nodos custom por tipo
de paso con salidas sí/no para IF/ELSE, aristas derivadas de `next`/`on_true`/`on_false`,
auto-layout para flujos sin posiciones, panel lateral de configuración `StepEditor.tsx`,
diálogo de ejecución con pasos/autorizaciones/runs de agente). El modelo de flujo es la única
fuente de verdad: el lienzo solo lo renderiza y edita; la validación estructural vive en el
backend (`flow.rs`).

Eventos: el dispatch emite `AppEvent::DomainEvent {event, entity, entity_id, data}` tras las
mutaciones de usuario; el manager los consume por broadcast (sin polling) y lanza las
automatizaciones cuyo trigger coincide. `task.overdue` se genera con una comprobación
determinista en el tick del scheduler (marca por tarea+día). El scheduler persiste
`next_run_at`; al reabrir NEXUS recalcula desde ahora y descarta disparos perdidos (>10 min)
en lugar de ejecutarlos en tromba. Las ejecuciones huérfanas se marcan `failed`.

Modos: `local` (sin IA), `ai` (requiere proveedor; sin él la ejecución falla con error claro)
e `hybrid` (los pasos de agente sin proveedor se registran como `deferred` y el flujo local
continúa). Presupuesto: tokens por ejecución, llamadas a agentes, pasos máximos, timeout,
reintentos, cooldown y límite diario global (`settings.app.automations.daily_token_limit`).

Autorizaciones: las acciones sensibles y las marcadas `confirm` crean un registro en
`automation_approvals`, ponen la ejecución en `waiting_approval` y esperan la decisión humana
(watch channel, sin condiciones de carrera), con vista previa del cambio y caducidad a las 48 h.

## Proveedores de IA

`AiProvider` (trait async) con `chat`, `chat_stream` y `health`. La implementación actual es
`OpenAiCompatibleProvider`, que sirve para Qwen (DashScope compatible-mode), OpenAI, y cualquier
endpoint compatible. Añadir Anthropic o modelos locales = nueva implementación del trait +
registro en `ProviderRegistry`; el motor de agentes no cambia.

Configuración: tabla `settings` (providers JSON + secretos bajo claves `secret:provider:<id>`),
más fallback de entorno `QWEN_API_KEY/QWEN_BASE_URL/QWEN_MODEL`. Los secretos nunca se serializan
hacia el frontend ni se loguean.

## Datos

SQLite en modo WAL, `foreign_keys=ON`, migraciones SQLx versionadas (`crates/nexus-db/migrations`).
Tablas: `projects`, `tasks` (jerárquicas, soft-delete), `events` (calendario), `notes` + `notes_fts`
(FTS5 sincronizada transaccionalmente), `inbox_items`, `conversations`, `chat_messages`,
`agent_runs`, `tool_calls`, `approvals`, `activity_events` (con `undo_json`), `settings`,
`automations`, `automation_executions`, `automation_step_logs`, `automation_approvals`,
`automation_fired_marks`, `files`, `canvases`, `databases`, `db_fields`, `db_records`,
`db_views`, `recipes`, `meal_plan_entries`, `menu_templates`, `shopping_lists`,
`shopping_items`, `pantry_items`.

- **Undo**: las mutaciones guardan el estado previo o la operación inversa; `activity.undo` la
  aplica y consume el registro (no doble undo).
- **Backups**: `VACUUM INTO` (copia consistente en caliente); restauración por `ATTACH` +
  reemplazo transaccional de tablas, conservando los secretos actuales. La ruta de restauración
  se valida contra el directorio de backups (path traversal bloqueado).

## Frontend

- **Estado servidor**: TanStack Query; las mutaciones invalidan keys y los eventos SSE
  (`useAppEvents`) mantienen todo sincronizado en vivo, incluido el streaming del chat.
- **Estado cliente**: Zustand (`theme`, `ui`).
- **Temas y paletas**: variables CSS (`--background`, `--primary`, `--accent`, …) con 18 paletas ×
  dark/light definidas en `index.css` (6 básicas de acento + 8 avanzadas de tema completo: Nord,
  Dracula, Tokyo Night, Catppuccin, Gruvbox, Everforest, Kanagawa, Cyberpunk + 4 otras: Odysseus,
  Expedition 33, Vice City, Blackwall); Tailwind 4 las
  mapea con `@theme inline`. El store aplica
  `data-palette` + clase `dark/light` al instante, persiste en localStorage (anti-flash en
  `index.html`) y sincroniza con el backend (`settings.update`).
- **Transporte**: un único cliente `invoke()` funciona en Tauri (puerto dinámico + token) y en
  web (proxy de Vite + `/api/session`).

## Seguridad

- Servicio HTTP **solo en 127.0.0.1**, token de sesión por arranque, comprobación de `Host`
  loopback (mitiga DNS-rebinding) y CORS restringido a orígenes de desarrollo/Tauri.
- Credenciales exclusivamente en backend; archivo de datos con permisos `0600`.
- Validación de entrada en servicios (longitudes, enums, fechas, URLs, ciclos de subtareas).
- Herramientas de agentes con riesgo declarado, permisos configurables y aprobaciones auditadas.
- Errores centralizados (`NexusError` con código estable) y logs estructurados (`tracing`).

## Testing

- `nexus-services` (11): CRUD, validación, ciclos, progreso, FTS, inbox, undo, secretos,
  persistencia tras reinicio, backup/restore, dashboard, captura heurística.
- `nexus-ai` (9): parsing OpenAI/SSE, errores 401, rate limiter, registry sin proveedor.
- `nexus-tools` (9): validación de esquemas, matriz de permisos, ejecución real, web.search honesto.
- `nexus-agents` (14): bucle feliz con MockProvider, límites de iteraciones/tokens, delegación y
  profundidad, aprobaciones, cancelación, allowlists, recuperación de runs huérfanos.
- `nexus-automation` (43): triggers y zona horaria/DST, validación de flujos (ciclos, ramas),
  condiciones AND/OR/NOT con datos reales, acciones locales, eventos de dominio, scheduler y
  deduplicación, recuperación tras reinicio, autorizaciones (aprobar/rechazar), cancelación,
  límites de pasos/llamadas, reintentos, modos local/IA/híbrido con MockProvider, encadenado,
  plantillas instalables y overview.
- `nexus-services/tests/canvas.rs` (13): CRUD, duplicado con ids nuevos, documento idéntico
  tras guardar/recuperar, validación de tipos/aristas/ids, plantillas, export/import JSON,
  task_ref con tarea real, auditoría.
- `nexus-services/tests/databases.rs` (21 + 5 del parser): CRUD, validaciones por tipo,
  reorder y soft-delete de campos, fórmulas e2e, filtros AND/OR, relaciones y limpieza,
  vistas, import CSV con mapeo, export roundtrip, plantillas, adjuntos.
- `nexus-services/tests/meals.rs` (18): recetas, escalado de raciones, planner, menús,
  consolidación de compra (unidades incompatibles nunca se suman), despensa, caducidades,
  pantry_matches, nutrición IA descartada.
- `nexus-app` (3): parsing de captura IA con fallback.
- `scripts/e2e.mjs` (28): API real end-to-end incluyendo SSE en vivo y modo sin IA.
