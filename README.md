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
- **Actividad**: ejecuciones con trazabilidad de herramientas, aprobaciones pendientes,
  auditoría completa con **deshacer** operaciones.
- **Ajustes**: perfil, **14 paletas de colores intercambiables** + dark/light, proveedores de IA
  (API key, base URL, modelo, prueba de conexión), permisos de agentes, límites de uso,
  exportación JSON y copias de seguridad con restauración.

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

## Generar el ejecutable de escritorio

```bash
cd frontend && npm install && npm run build && cd ..
cargo build --release -p nexus-desktop
# Binario: target/release/nexus-desktop
```

Paquetes instalables (.deb / .rpm) con la CLI de Tauri:

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

**14 paletas** × modos **dark/light**:

- **Básicas**: Violet, Graphite, Ocean, Forest, Sunset, Rose.
- **Curadas** (temas completos con superficies, bordes y acentos propios, inspirados en esquemas
  clásicos): **Nord** (azul ártico), **Dracula** (púrpura nocturno), **Tokyo Night** (neón azul),
  **Catppuccin** (pasteles Mocha/Latte), **Gruvbox** (retro cálido), **Everforest** (bosque
  brumoso), **Kanagawa** (ola japonesa) y **Cyberpunk** (magenta/cian eléctrico).

Cámbialas desde el selector del sidebar o desde **Ajustes → Apariencia**, donde cada paleta
incluye una mini-previsualización real de la interfaz. La elección se persiste en el backend y
se aplica al instante mediante variables CSS.

## Tests y verificación

```bash
cargo test --workspace          # 55 tests: dominio, persistencia, servicios, IA, herramientas, agentes, eventos
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
- **Multiusuario**: NEXUS es monousuario y local; el servicio HTTP solo escucha en loopback.
- Los iconos del bundle son generados (placeholder profesional); puedes regenerarlos con
  `cargo tauri icon <png>` desde `crates/nexus-desktop`.

## Licencia

MIT.
