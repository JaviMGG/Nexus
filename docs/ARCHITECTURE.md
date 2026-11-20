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
│                              │  nexus-tools                            │
│                              │  Tool trait + ToolRegistry (19 tools)   │
│                              │  riesgos Read/Write/Destructive         │
│                              │  validación JSON Schema                 │
├──────────────────────────────┴─────────────────────────────────────────┤
│  nexus-services — servicios de aplicación                              │
│  tasks · projects · notes · inbox · chat · activity(undo) · settings   │
│  dashboard · backup · capture heurística · validación                  │
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
- `nexus-app` ensambla todo y expone `dispatch(command, payload) -> Result<Value>`: un único
  punto de entrada usado por HTTP y por Tauri. Añadir un caso de uso = añadir un comando.
- `nexus-api` / `nexus-desktop` son capas de transporte intercambiables.

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
`agent_runs`, `tool_calls`, `approvals`, `activity_events` (con `undo_json`), `settings`.

- **Undo**: las mutaciones guardan el estado previo o la operación inversa; `activity.undo` la
  aplica y consume el registro (no doble undo).
- **Backups**: `VACUUM INTO` (copia consistente en caliente); restauración por `ATTACH` +
  reemplazo transaccional de tablas, conservando los secretos actuales. La ruta de restauración
  se valida contra el directorio de backups (path traversal bloqueado).

## Frontend

- **Estado servidor**: TanStack Query; las mutaciones invalidan keys y los eventos SSE
  (`useAppEvents`) mantienen todo sincronizado en vivo, incluido el streaming del chat.
- **Estado cliente**: Zustand (`theme`, `ui`).
- **Temas y paletas**: variables CSS (`--background`, `--primary`, `--accent`, …) con 6 paletas ×
  dark/light definidas en `index.css`; Tailwind 4 las mapea con `@theme inline`. El store aplica
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
- `nexus-app` (3): parsing de captura IA con fallback.
- `scripts/e2e.mjs` (28): API real end-to-end incluyendo SSE en vivo y modo sin IA.
