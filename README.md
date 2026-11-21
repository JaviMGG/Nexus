# NEXUS — Centro de control personal inteligente

NEXUS es tu segundo cerebro: tareas, proyectos, calendario, notas con búsqueda full-text, captura
rápida, chat con IA y un **sistema multiagente real** (orquestador + agentes especializados que
ejecutan herramientas sobre tus datos), todo en una aplicación de escritorio premium construida con
**Rust + Tauri 2 + React + TypeScript**.

La IA es el cerebro, pero **NEXUS funciona al 100% sin ella**: sin proveedor conectado, todas las
funciones manuales siguen disponibles y la app lo indica claramente (modo degradado).

---

## Características

- **Dashboard**: saludo, hora en vivo, resumen del día, tareas de hoy/atrasadas/prioritarias,
  próximas fechas límite, proyectos activos con progreso, actividad reciente y ejecuciones de agentes.
- **Tareas**: CRUD completo, prioridades, fechas límite, categorías, etiquetas, subtareas,
  filtros, búsqueda, vista **lista y Kanban con drag & drop**.
- **Proyectos**: estado, prioridad, objetivo, tareas y notas relacionadas, progreso calculado,
  historial de actividad.
- **Calendario**: vista mensual de eventos planificados con crear/editar/borrar, colores,
  eventos de todo el día o con hora, ubicación y proyecto asociado; superposición opcional de
  tareas con fecha límite. Los agentes (Planner/Executor) también pueden gestionar eventos.
- **Knowledge Base**: notas/ideas/documentos/enlaces, editor **Markdown** con vista previa,
  búsqueda **FTS5** de SQLite, fijado de notas, etiquetas.
- **Inbox / Captura rápida** (⌘K desde cualquier pantalla): la IA detecta si es tarea o nota y
  extrae título, fecha límite y prioridad; sin IA, una heurística local en español hace lo mismo.
- **Chat IA**: conversaciones con panel lateral minimizable, selección de agente, **streaming**
  de respuestas, visualización de herramientas usadas, aprobación/rechazo de acciones sensibles,
  cancelación de ejecuciones.
- **Agentes**: Orchestrator, Planner, Project Manager, Knowledge, Researcher y Executor, con
  prompts, herramientas y permisos propios; delegación real entre agentes.
- **Automatizaciones (Automation Center)**: motor local de reglas *trigger → condiciones →
  acciones → agentes* con scheduler persistente, constructor visual de nodos y flechas,
  **16 plantillas** preconfiguradas ordenadas por dificultad (1–5) y filtrables por modo,
  historial completo de ejecuciones, autorizaciones humanas y tres modos
  (Local / IA / Híbrido). Las automatizaciones locales funcionan **sin IA y offline**.
- **Actividad**: ejecuciones con trazabilidad de herramientas, aprobaciones pendientes,
  auditoría completa con **deshacer** operaciones.
- **Ajustes**: perfil, **16 paletas de colores intercambiables** + dark/light, proveedores de IA
  (API key, base URL, modelo, prueba de conexión), permisos de agentes, límites de uso,
  exportación JSON y copias de seguridad con restauración.
- **Acerca de**: documentación integrada para cualquier usuario — qué es NEXUS, qué hace cada
  sección, **diagnóstico en vivo** (servicio local, estado de IA, proveedores, motor de
  automatizaciones) y preguntas frecuentes con soluciones a los problemas típicos.

## Stack

| Capa | Tecnología |
|---|---|
| Escritorio | Tauri 2 |
| Backend | Rust (workspace de crates, monolito modular), Tokio, Axum, SQLx |
| Datos | SQLite (WAL) + migraciones versionadas + FTS5 |
| Frontend | React 18, TypeScript, Vite, Tailwind CSS 4, componentes estilo shadcn/ui, Radix, Lucide, TanStack Query, Zustand |
| IA | Proveedores intercambiables (compatible OpenAI). Primera integración: **Qwen** vía DashScope compatible-mode |

## Requisitos

- **Rust** stable (probado con 1.100).
- **Node.js** 20+ y npm.
- Linux: dependencias de Tauri 2 → `webkit2gtk-4.1`, `gtk3`, `libsoup3`, `librsvg`.
  - Arch: `sudo pacman -S webkit2gtk-4.1 gtk3 libsoup librsvg`
  - Debian/Ubuntu: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev`
- Opcional: `cargo install tauri-cli --version "^2"` para empaquetado (`cargo tauri build`).

## Instalación fácil (recomendada)

```bash
./install.sh
```

El instalador pregunta **¿nativa, web o ambas?** y se encarga de todo:

- **Nativa**: comprueba/instala dependencias de escritorio (con tu confirmación), compila en
  release, instala el binario en `~/.local/bin/nexus`, el icono y la entrada del lanzador de
  aplicaciones. Opcionalmente abre la app al terminar.
- **Web**: compila el servidor, lo instala en `~/.local/bin/nexus-server`, copia el frontend
  a `~/.local/share/nexus/web` (el propio binario lo sirve) y ofrece instalarlo como
  **servicio de usuario systemd** con arranque automático. URL: `http://127.0.0.1:47800`.
- Crea `.env` desde `.env.example` si no existe.
- Desinstalar: `./install.sh --uninstall` (elimina binarios/accesos; **conserva tus datos**).

## Puesta en marcha (desarrollo)

```bash
cd Nexus
cp .env.example .env        # opcional: rellena QWEN_API_KEY para usar IA
```

### Opción A — modo web (la más rápida)

```bash
# Terminal 1: servicio local Rust (API + agentes + SQLite)
cargo run -p nexus-api

# Terminal 2: frontend
cd frontend
npm install
npm run dev
```

Abre **http://localhost:5173**. El frontend usa el proxy de Vite hacia `127.0.0.1:47800`.

> Nota: el binario `nexus-server` también puede servir el frontend compilado directamente
> (sin Vite): si existe `index.html` en `$NEXUS_DATA_DIR/web` (o `NEXUS_WEB_DIR`), lo sirve
> en la raíz. Es lo que usa el modo web de `install.sh`.

### Opción B — aplicación de escritorio (Tauri)

```bash
# Terminal 1: frontend dev server
cd frontend && npm install && npm run dev

# Terminal 2: ventana nativa (arranca su propio servicio local embebido)
cargo run -p nexus-desktop
```

O con la CLI de Tauri (lanza el frontend automáticamente):

```bash
cargo install tauri-cli --version "^2"   # una vez
cd crates/nexus-desktop
cargo tauri dev
```

## Instaladores por plataforma

Cada release (tag `v*`) publica automáticamente en GitHub Releases:

| Plataforma | Archivo | Notas |
|---|---|---|
| Windows x64 | `NEXUS_*_x64-setup.exe` | Sin firmar: SmartScreen → **Más información → Ejecutar de todos modos** |
| macOS Intel | `NEXUS_*_x64.dmg` | Sin firmar: la primera vez, clic derecho → **Abrir** (Gatekeeper) |
| macOS Apple Silicon | `NEXUS_*_aarch64.dmg` | Igual que el anterior |
| Linux | `.deb`, `.rpm`, `.AppImage` | AppImage: `chmod +x` y ejecutar |

El workflow vive en `.github/workflows/release.yml` (matriz Windows/macOS/Linux + job de
publicación). También se puede lanzar a mano desde Actions → *Release* → *Run workflow*.

> **Móvil (hoja de ruta 0.2.0)**: Android requiere adaptar el entry point móvil de Tauri y el
> transporte del frontend (hoy escritorio: servidor HTTP embebido) + SDK/NDK de Android.
> iOS requiere además Mac, Xcode y cuenta Apple Developer; no existe IPA instalable libremente.

### Compilar localmente

```bash
cd frontend && npm install && npm run build && cd ..
cargo build --release -p nexus-desktop
# Binario: target/release/nexus-desktop
```

Paquetes instalables con la CLI de Tauri (en Linux genera .deb/.rpm/AppImage; en macOS, .dmg;
en Windows, NSIS):

```bash
cd crates/nexus-desktop
cargo tauri build
# Paquetes en target/release/bundle/
```

## Configurar Qwen (u otro proveedor)

Dos vías, equivalentes:

1. **Variables de entorno** (desarrollo): en `.env`
   ```
   QWEN_API_KEY=sk-...
   QWEN_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
   QWEN_MODEL=qwen-max        # pon aquí el identificador real de tu modelo (p. ej. el de Qwen 4 Max)
   ```
2. **Ajustes → Inteligencia Artificial** dentro de la app: añade un proveedor compatible OpenAI
   con su API key, base URL y modelo, y pulsa **Probar conexión**.

La integración usa el **modo compatible OpenAI de DashScope** (Alibaba Cloud Model Studio),
documentado en https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope.
El identificador del modelo **no se presupone**: se toma de tu configuración. Las claves solo se
guardan en el backend local (archivo SQLite con permisos `0600`); nunca viajan al frontend ni a los logs.

Para añadir otro proveedor (OpenAI, Anthropic, local…): implementa el trait `AiProvider`
en `nexus-ai` y regístralo; el sistema multiagente no cambia.

### Sin IA

Sin proveedor, NEXUS arranca y funciona con normalidad: CRUD, búsqueda, kanban, backups…
El indicador del sidebar muestra **Sin IA**, el chat responde en modo manual y la captura
rápida usa análisis heurístico local. Ningún error por falta de API key.

## Paletas de colores

**16 paletas** × modos **dark/light**:

- **Básicas**: Violet, Graphite, Ocean, Forest, Sunset, Rose.
- **Avanzados** (temas completos con superficies, bordes y acentos propios, inspirados en esquemas
  clásicos): **Nord** (azul ártico), **Dracula** (púrpura nocturno), **Tokyo Night** (neón azul),
  **Catppuccin** (pasteles Mocha/Latte), **Gruvbox** (retro cálido), **Everforest** (bosque
  brumoso), **Kanagawa** (ola japonesa) y **Cyberpunk** (magenta/cian eléctrico).
- **Otros**: **Odysseus** (carbón tecnológico y cian coral) y **Expedition 33** (oro cálido y rosa
  Belle Époque).

Cámbialas desde el selector del sidebar o desde **Ajustes → Apariencia**, donde cada paleta
incluye una mini-previsualización real de la interfaz. La elección se persiste en el backend y
se aplica al instante mediante variables CSS.

## Automation Center

Sección **Automatizaciones** del sidebar, con cinco apartados:

- **Resumen**: automatizaciones activas, ejecuciones totales/correctas/fallidas, consumo de IA
  del día frente al límite diario, agentes más usados, próximas ejecuciones programadas y
  últimas ejecuciones.
- **Mis automatizaciones**: listado con estado, modo, última/próxima ejecución, activar/desactivar,
  **Ejecutar ahora**, **prueba sin modificar datos**, editar y eliminar.
- **Constructor**: lienzo visual de **nodos y flechas** (React Flow) con disposición
  **de izquierda a derecha**: arrastra bloques (acción, agente IA, condición IF/ELSE, espera,
  notificación, fin), conéctalos arrastrando entre puntos, mueve los nodos libremente (las
  posiciones se guardan), ramas sí/no en las condiciones, minimapa, zoom y validación en vivo
  (sin ciclos, sin referencias rotas, acciones y agentes conocidos). El nodo «Disparador»
  apunta al paso de entrada. Crear o editar una automatización se hace **en la propia página**
  (vista completa con botón Volver), sin ventanas emergentes: una **barra de herramientas**
  superior concentra nombre, icono, modo, disparador y activación (con panel de opciones
  desplegable), y el lienzo ocupa todo el ancho; la configuración de cada nodo se abre como
  panel flotante dentro del propio lienzo.
- **Plantillas**: 16 automatizaciones preconfiguradas y funcionales, **ordenadas por dificultad
  (1 a 5) y filtrables por modo**, con ejemplos de cada tipo: locales sin IA (Recordatorio
  diario, Etiquetar tareas urgentes, Informe diario local), con IA (Resumen instantáneo de
  tarea, Smart Inbox, Project Kickoff, Goal Breakdown, Research to Knowledge…) e híbridas
  (Bienvenida de proyecto, Morning Briefing, Overdue Monitor, End of Day, Stale Project
  Detector, Weekly Review, Duplicate Notes Detector, Planificación semanal inteligente).
  Se instalan desactivadas para revisarlas y personalizarlas antes de activarlas.
- **Historial**: cada ejecución con pasos, duración, agentes implicados, tokens consumidos,
  errores, logs y autorizaciones pendientes (autorizar/rechazar desde la propia vista).

### Cómo funciona el motor

- **Triggers**: programación temporal (diaria, semanal, mensual, por intervalo ≥5 min, una vez)
  con **zona horaria IANA** y cambios de horario de verano, y eventos de dominio reales
  (`task.created/completed/updated/overdue`, `project.created/status_changed`, `note.created`,
  `idea.created`, `goal.created`, `agent.finished`) emitidos por las operaciones normales de la
  app — sin sondeos constantes a la base de datos. El vencimiento de tareas se detecta con una
  comprobación determinista periódica y marca por tarea/día (sin duplicados).
- **Condiciones**: árbol AND/OR/NOT con métricas deterministas sobre datos reales (tareas
  pendientes/vencidas, progreso y días de inactividad de proyectos, notas que contienen texto,
  resultado de la ejecución anterior…) y campos del evento. **Nunca SQL generado por IA**.
- **Acciones**: catálogo cerrado sobre los servicios existentes (crear/modificar/completar
  tareas y subtareas, prioridades, etiquetas, proyectos, notas, resúmenes, informes locales,
  notificaciones, encadenar otra automatización local). Sin ejecución de código arbitrario.
- **Agentes**: cualquier flujo puede invocar a los agentes reales con su objetivo; el resultado
  vuelve al flujo (`{{agent_result}}`) y puede guardarse o usarse en condiciones. Delegación
  entre agentes la gestiona el motor multiagente existente.
- **Tres modos por automatización**: **Local** (solo Rust, offline, sin tokens), **IA**
  (requiere proveedor) e **Híbrido** (local primero; sin proveedor, las acciones inteligentes
  quedan registradas como *pendientes* y lo local continúa). La falta de IA nunca bloquea el motor.
- **Seguridad**: lecturas automáticas; modificaciones locales según la política de cada
  automatización (auto o pedir autorización) y siempre auditadas con deshacer; operaciones
  sensibles (borrar, encadenar) **siempre requieren autorización explícita** con vista previa.
- **Ejecución fiable**: cola con concurrencia limitada, timeouts, cancelación, reintentos
  controlados, límite de pasos/tokens/llamadas, cooldowns, deduplicación y **recuperación tras
  reinicio** (las ejecuciones interrumpidas se marcan fallidas; los disparos perdidos mientras
  NEXUS estaba cerrado se descartan y reprograman, nunca se ejecutan en tromba).
- **Consumo**: presupuesto de tokens por ejecución y límite diario global configurable
  (Ajustes), registro real del consumo que reporta el proveedor.

Las automatizaciones se ejecutan **mientras NEXUS esté abierto** (escritorio o servicio web).
La arquitectura (scheduler + cola persistentes en SQLite) permite añadir más adelante un
proceso residente en segundo plano sin rediseño.

## Tests y verificación

```bash
cargo test --workspace          # 98 tests: dominio, persistencia, servicios, IA, herramientas,
                                # agentes, eventos y motor de automatizaciones (43)
cargo clippy --workspace --all-targets
cargo fmt --all --check
cd frontend && npm run build    # typecheck + build de producción
```

Prueba E2E contra el servidor real (con la app arrancada en modo web):

```bash
NEXUS_DATA_DIR=/tmp/nexus-e2e NEXUS_PORT=47899 cargo run -p nexus-api &
node scripts/e2e.mjs            # 28 comprobaciones: CRUD, FTS, captura sin IA, chat degradado,
                                # proveedores, secretos, undo, backups, SSE, auth…
```

## Datos y copias de seguridad

- Modo web/headless: `~/.local/share/nexus/nexus.sqlite` (o `NEXUS_DATA_DIR`).
- Escritorio (Tauri): `~/.local/share/app.nexus.desktop/nexus.sqlite`.
- Backups: subcarpeta `backups/` (creadas con `VACUUM INTO`, consistentes).
- Exportación JSON completa desde Ajustes (sin credenciales).

## Estructura del proyecto

```
Nexus/
├── install.sh            # Instalador interactivo (nativa / web / ambas) + --uninstall
├── .env.example          # Plantilla de configuración (QWEN_API_KEY, puerto, etc.)
├── crates/
│   ├── nexus-core/       # Dominio puro: entidades, DTOs, errores, eventos
│   ├── nexus-db/         # SQLite + SQLx: migraciones y repositorios
│   ├── nexus-services/   # Servicios de aplicación: validación, auditoría, undo, backups
│   ├── nexus-ai/         # Proveedores de IA intercambiables (Qwen/OpenAI-compatible)
│   ├── nexus-tools/      # Registro de herramientas con riesgos y validación JSON Schema
│   ├── nexus-agents/     # Registro de agentes + motor de ejecución (bucle, delegación, límites)
│   ├── nexus-automation/ # Automation Center: triggers, scheduler, condiciones, acciones,
│   │                     # ejecución de flujos, autorizaciones, plantillas y dispatcher de agentes
│   ├── nexus-app/        # Composition root + dispatch de comandos
│   ├── nexus-api/        # Servicio HTTP local (Axum) + binario headless nexus-server
│   └── nexus-desktop/    # Shell Tauri 2
├── frontend/             # React + TS + Vite + Tailwind 4
├── scripts/e2e.mjs       # Prueba E2E del API real
└── docs/ARCHITECTURE.md  # Arquitectura del monolito modular
```

## Actualizar una instalación existente

Tras cambiar el código:

```bash
./install.sh                        # vuelve a compilar e instala (elige el mismo modo)
# o manualmente:
cargo build --release -p nexus-desktop
cp target/release/nexus-desktop ~/.local/bin/nexus.new && mv -f ~/.local/bin/nexus.new ~/.local/bin/nexus
```

Las migraciones de base de datos se aplican automáticamente al arrancar; tus datos se conservan.

## Limitaciones conocidas

- **Qwen 4 Max**: el adaptador está implementado y verificado contra el endpoint compatible
  OpenAI de DashScope (respuesta 401 real con key inválida → error claro). La ejecución real
  con un modelo Qwen 4 Max requiere tu `QWEN_API_KEY` y el identificador de modelo exacto que
  te proporcione tu proveedor; no se ha presupuestado ningún ID no verificado.
- **Búsqueda web del Researcher**: el trait `WebSearchProvider` y la herramienta `web.search`
  están preparados, pero no hay proveedor de búsqueda externo integrado aún. La herramienta
  devuelve un error explícito y el agente nunca inventa fuentes.
- **Búsqueda vectorial/embeddings**: la arquitectura de conocimiento está preparada (FTS5 hoy);
  los embeddings se añadirán como nuevo módulo sin tocar el resto.
- **Credenciales**: se almacenan en el SQLite local con permisos `0600`. No se usa el llavero
  del sistema operativo (keyring); puede añadirse implementando el mismo interfaz de secretos.
- **Streaming**: depende de que el proveedor soporte SSE (`stream: true`). Si no, se usa la
  respuesta completa.
- **Kanban**: drag & drop nativo HTML5 (sin librería); en táctil funciona mediante los menús de
  estado de cada tarea.
- **Automatizaciones en segundo plano**: en esta versión el motor corre mientras NEXUS está
  abierto; no hay proceso residente con la ventana cerrada (la arquitectura lo permite añadir).
  Los disparos de tipo "sesión de concentración" no existen aún porque NEXUS todavía no tiene
  sesiones de concentración; el registro de triggers es extensible.
- **Constructor de automatizaciones**: el lienzo de nodos usa React Flow (@xyflow/react);
  las condiciones complejas AND/OR/NOT anidadas se editan como JSON en la condición raíz
  (el lienzo edita condiciones hoja: métrica/campo + operador + valor).
- **Multiusuario**: NEXUS es monousuario y local; el servicio HTTP solo escucha en loopback.
- El icono de la app es el logo oficial (`crates/nexus-desktop/icons/logo-master.png`): se usa
  en el bundle de escritorio (todos los tamaños + `.ico` de Windows), en el lanzador de
  `install.sh`, como favicon web y dentro de la propia interfaz. Para cambiarlo, sustituye el
  master y regenera los tamaños (o `cargo tauri icon <png>` desde `crates/nexus-desktop`,
  copiando además el resultado a `frontend/public/logo.png` y `favicon.png`).

## Licencia

MIT.
