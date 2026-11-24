import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  BookOpen,
  Bot,
  CheckCircle2,
  ChevronDown,
  CookingPot,
  Database,
  HelpCircle,
  Info,
  Keyboard,
  PenTool,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Table2,
  Wifi,
  WifiOff,
  Workflow,
  XCircle,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError, apiUrl } from "@/lib/api";
import type { AiStatus, SettingsPayload } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";

const APP_VERSION = "0.1.4";

// ---------------------------------------------------------------------------
// Piezas reutilizables
// ---------------------------------------------------------------------------

function Section({ icon: Icon, title, children, id }: { icon: LucideIcon; title: string; children: React.ReactNode; id?: string }) {
  return (
    <section id={id} className="scroll-mt-6">
      <h2 className="mb-3 flex items-center gap-2 text-base font-semibold tracking-tight">
        <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/15">
          <Icon className="h-4 w-4 text-primary" />
        </span>
        {title}
      </h2>
      <div className="space-y-3 pl-9 text-sm leading-relaxed text-muted-foreground">{children}</div>
    </section>
  );
}

function Faq({ q, children }: { q: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="rounded-lg border border-border bg-card">
      <button
        className="flex w-full items-center gap-2 px-3 py-2.5 text-left text-sm font-medium text-foreground cursor-pointer"
        onClick={() => setOpen((v) => !v)}
      >
        <HelpCircle className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="flex-1">{q}</span>
        <ChevronDown className={cn("h-4 w-4 shrink-0 text-muted-foreground transition-transform", open && "rotate-180")} />
      </button>
      {open && <div className="border-t border-border/60 px-3 py-2.5 text-sm leading-relaxed text-muted-foreground animate-fade-in">{children}</div>}
    </div>
  );
}

function Feature({ icon: Icon, title, children, to }: { icon: LucideIcon; title: string; children: React.ReactNode; to: string }) {
  return (
    <Link to={to} className="block rounded-xl border border-border bg-card p-3 shadow-sm transition-colors hover:bg-muted/50">
      <p className="flex items-center gap-2 text-sm font-semibold text-foreground">
        <Icon className="h-4 w-4 text-primary" /> {title}
      </p>
      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{children}</p>
    </Link>
  );
}

// ---------------------------------------------------------------------------
// Diagnóstico en vivo
// ---------------------------------------------------------------------------

function Diagnostics() {
  const settings = useQuery({ queryKey: ["settings"], queryFn: () => invoke<SettingsPayload>("settings.get") });
  const health = useQuery({
    queryKey: ["about-health"],
    queryFn: async () => {
      const url = await apiUrl("/api/health");
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return res.json() as Promise<{ service: string; status: string; version: string }>;
    },
    refetchInterval: 30000,
    retry: 1,
  });

  const ai: AiStatus | undefined = settings.data?.ai_status;
  const providers = settings.data?.providers ?? [];
  const enabledProviders = providers.filter((p) => p.enabled);

  const Row = ({ ok, warn, label, detail }: { ok?: boolean; warn?: boolean; label: string; detail: string }) => (
    <li className="flex items-start gap-2.5 rounded-lg border border-border bg-card px-3 py-2.5">
      {ok ? (
        <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-success" />
      ) : warn ? (
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warning" />
      ) : (
        <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
      )}
      <div className="min-w-0">
        <p className="text-sm font-medium text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground">{detail}</p>
      </div>
    </li>
  );

  return (
    <div className="space-y-2">
      {settings.isLoading || health.isLoading ? (
        <>
          <Skeleton className="h-14 rounded-lg" />
          <Skeleton className="h-14 rounded-lg" />
          <Skeleton className="h-14 rounded-lg" />
        </>
      ) : (
        <ul className="space-y-2">
          <Row
            ok={health.isSuccess}
            label="Servicio local de NEXUS"
            detail={
              health.isSuccess
                ? `Funcionando correctamente (versión ${health.data.version}). Tus datos se guardan en este equipo.`
                : "No responde. Cierra y vuelve a abrir NEXUS; si usas el modo web, comprueba que el servicio esté arrancado."
            }
          />
          <Row
            ok={ai?.state === "connected"}
            warn={ai?.state !== "connected"}
            label="Inteligencia artificial"
            detail={
              ai?.state === "connected"
                ? `Conectada a ${ai.provider ?? "proveedor"} (${ai.model ?? "modelo"}). Los agentes y el modo IA están disponibles.`
                : ai?.state === "error"
                  ? `El proveedor respondió con error: ${ai.message ?? "sin detalle"}. Revisa la API key y el modelo en Ajustes → Inteligencia Artificial.`
                  : "Sin proveedor conectado. No pasa nada: todo lo manual — incluidas automatizaciones locales, Canvas, Bases de datos y Comidas — sigue funcionando. Para activar la IA, ve a Ajustes → Inteligencia Artificial."
            }
          />
          <Row
            ok={enabledProviders.length > 0 && enabledProviders.every((p) => p.api_key_set)}
            warn={enabledProviders.length === 0}
            label={`Proveedores configurados (${providers.length})`}
            detail={
              enabledProviders.length === 0
                ? "No hay ningún proveedor activo. Puedes añadir uno compatible OpenAI (Qwen, OpenAI, etc.) con su API key."
                : enabledProviders.map((p) => `${p.name}${p.api_key_set ? "" : " (falta API key)"}`).join(", ")
            }
          />
          <Row
            ok={settings.data?.app.automations.enabled !== false}
            warn={settings.data?.app.automations.enabled === false}
            label="Automatizaciones"
            detail={
              settings.data?.app.automations.enabled === false
                ? "Pausadas: ninguna automatización se ejecutará hasta que se reactiven."
                  : `En marcha. Límite diario de IA: ${(settings.data?.app.automations.daily_token_limit ?? 0).toLocaleString("es-ES")} tokens. ${settings.data?.app.automations.run_in_background ? "Aunque cierres la ventana, NEXUS sigue trabajando desde el iconito junto al reloj y sus avisos llegan como notificaciones del sistema." : "Solo trabajan con la ventana abierta. Si quieres que sigan al cerrar, activa «Que NEXUS siga trabajando aunque cierres la ventana» en Ajustes → Automatizaciones."}`
            }
          />
          <Row
            ok
            label="Herramientas (Canvas · Bases de datos · Comidas)"
            detail="Disponibles y 100% locales: no dependen de Internet ni de la IA. Sus datos e imágenes se guardan en este equipo y se incluyen en las copias de seguridad."
          />
        </ul>
      )}
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            settings.refetch();
            health.refetch();
          }}
        >
          <RefreshCw /> Volver a comprobar
        </Button>
        <Button size="sm" variant="outline" asChild>
          <Link to="/settings"><Zap /> Ir a Ajustes</Link>
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Página
// ---------------------------------------------------------------------------

export function AboutPage() {
  return (
    <div className="mx-auto max-w-[900px] space-y-8 p-6 animate-fade-in">
      <header>
        <div className="flex items-center gap-3">
          <img src="/logo.png" alt="NEXUS" className="h-11 w-11 rounded-xl object-cover shadow-sm" />
          <div>
            <h1 className="text-xl font-semibold tracking-tight">NEXUS</h1>
            <p className="text-xs text-muted-foreground">
              Centro de control personal · versión {APP_VERSION} ·{" "}
              <Badge variant="secondary">100% local</Badge>{" "}
              <Badge variant="success">Funciona sin IA</Badge>
            </p>
          </div>
        </div>
        <p className="mt-3 text-sm leading-relaxed text-muted-foreground">
          NEXUS es tu <b className="text-foreground">segundo cerebro</b>: reúne tareas, proyectos, calendario y notas
          en un solo sitio, añade herramientas potentes — <b className="text-foreground">Canvas</b> (pizarra infinita),{" "}
          <b className="text-foreground">Bases de datos</b> personalizadas y <b className="text-foreground">Comidas</b>{" "}
          (recetas, menús y compra) — y puede trabajar por ti gracias a agentes de IA y automatizaciones.
          Todo vive en <b className="text-foreground">tu equipo</b>: no hay servidores externos, no hay cuentas y tus
          datos no salen de tu ordenador. La IA es opcional: sin ella, NEXUS sigue funcionando al 100% en modo manual.
        </p>
      </header>

      <Section icon={Sparkles} title="Qué puedes hacer" id="funciones">
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <Feature icon={CheckCircle2} title="Tareas y proyectos" to="/tasks">
            Organiza pendientes con prioridades, fechas, etiquetas y subtareas; agrúpalas en proyectos con progreso
            automático. Vista lista o tablero Kanban.
          </Feature>
          <Feature icon={BookOpen} title="Conocimiento" to="/knowledge">
            Notas, ideas, documentos y enlaces con editor Markdown y búsqueda instantánea de texto completo.
          </Feature>
          <Feature icon={Database} title="Calendario" to="/calendar">
            Vista mensual de eventos con horas, colores y ubicación; superposición opcional de tareas con fecha límite.
          </Feature>
          <Feature icon={Keyboard} title="Captura rápida (Ctrl/⌘ + K)" to="/inbox">
            Escribe cualquier cosa desde cualquier pantalla: NEXUS detecta si es una tarea o una nota y la guarda.
            Con IA, extrae fecha y prioridad; sin IA, usa reglas locales en español.
          </Feature>
          <Feature icon={Bot} title="Chat y agentes de IA" to="/agents">
            Conversa con un asistente o lanza agentes especializados (planificador, gestor de proyectos, conocimiento,
            investigador, ejecutor) que usan herramientas reales sobre tus datos.
          </Feature>
          <Feature icon={Workflow} title="Automatizaciones" to="/automations">
            Configura una vez y olvídate: «cada mañana prepárame el resumen del día», «avísame si una tarea vence»,
            «al crear un proyecto, propón fases». Constructor visual de nodos, plantillas y modos Local / IA / Híbrido.
          </Feature>
          <Feature icon={ShieldCheck} title="Control total" to="/activity">
            Cada cambio queda auditado y se puede deshacer. Las acciones sensibles piden tu autorización explícita.
            Copias de seguridad y exportación completas desde Ajustes.
          </Feature>
          <Feature icon={PenTool} title="Canvas" to="/canvas">
            Pizarra infinita para organizar ideas: notas adhesivas, formas, flechas e imágenes, con referencias
            vivas a tus tareas, proyectos, notas, bases de datos y recetas. Grupos, capas, deshacer/rehacer,
            autoguardado, 6 plantillas y exportación PNG/SVG/JSON.
          </Feature>
          <Feature icon={Table2} title="Bases de datos" to="/databases">
            Crea tus propias bases tipo Airtable sin programar: 17 tipos de campo, relaciones, fórmulas seguras,
            5 vistas (tabla, kanban, galería, calendario, lista), vista <b>Esquema</b> con tablas y relaciones,
            filtros y import/export CSV/JSON. 8 plantillas.
          </Feature>
          <Feature icon={CookingPot} title="Comidas" to="/meals">
            Recetario con fotos y raciones ajustables, menú semanal con drag&drop, lista de la compra automática
            que suma ingredientes y agrupa por categorías, y despensa con caducidades y «¿qué puedo cocinar?».
          </Feature>
        </div>
      </Section>

      <Separator />

      <Section icon={Wifi} title="Diagnóstico: por qué puede fallar algo" id="diagnostico">
        <p>
          Este panel comprueba en vivo el estado de tu instalación. Si algo no funciona como esperas, empieza por aquí:
        </p>
        <Diagnostics />
      </Section>

      <Separator />

      <Section icon={HelpCircle} title="Preguntas frecuentes y problemas típicos" id="faq">
        <div className="space-y-2">
          <Faq q="Pone «Sin IA». ¿La app está rota?">
            No. Significa que no hay ningún proveedor de IA conectado, y es un estado perfectamente normal:
            tareas, proyectos, notas, calendario, búsqueda, copias de seguridad y automatizaciones en modo{" "}
            <b>Local</b> funcionan igual. Para activar la IA: <Link to="/settings" className="text-primary underline-offset-2 hover:underline">Ajustes → Inteligencia Artificial</Link>,
            añade un proveedor compatible OpenAI (por ejemplo Qwen vía DashScope) con su API key y pulsa{" "}
            <b>Probar conexión</b>.
          </Faq>
          <Faq q="El chat no responde o un agente falla con error de proveedor">
            Casi siempre es una de estas causas: API key incorrecta o caducada, nombre de modelo mal escrito
            (debe ser el identificador exacto que te da tu proveedor), o falta de conexión a Internet. Ve a{" "}
            <Link to="/settings" className="text-primary underline-offset-2 hover:underline">Ajustes</Link> y usa{" "}
            <b>Probar conexión</b>: te dirá el error real (por ejemplo, 401 = clave inválida). Tus datos nunca se
            pierden por un fallo de IA: la operación simplemente no se ejecuta.
          </Faq>
          <Faq q="Una automatización no se ejecuta">
            Comprueba en este orden: 1) que esté <b>activada</b> (interruptor en Mis automatizaciones); 2) que NEXUS
            esté <b>en marcha</b> — con la ventana abierta, o en la bandeja del sistema si activaste
            «Que NEXUS siga trabajando aunque cierres la ventana» (Ajustes → Automatizaciones); 3) su <b>modo</b>: si es IA o
            Híbrido y no hay proveedor, los pasos inteligentes se omiten o quedan pendientes (el modo Local nunca
            necesita IA); 4) el <b>disparador</b>: los eventos ocurren al crear/completar cosas, y los horarios usan la
            zona horaria configurada; 5) el <b>historial</b>: cada intento queda registrado con su error exacto.
          </Faq>
          <Faq q="Una automatización se quedó «Esperando autorización»">
            Es una acción sensible (borrar, encadenar, o marcada como «pedir autorización») esperando tu decisión.
            Ve a <Link to="/automations" className="text-primary underline-offset-2 hover:underline">Automatizaciones → Historial</Link>{" "}
            y pulsa <b>Autorizar</b> o <b>Rechazar</b>. Si nadie decide en 48 horas, caduca sola y la ejecución continúa
            sin esa acción. Nada se ejecuta a tus espaldas.
          </Faq>
          <Faq q="Creé una tarea/nota y la automatización no saltó">
            Los disparadores de evento reaccionan a las operaciones hechas desde la app (crear tarea, completar,
            crear proyecto, guardar idea…). Si la automatización se activó <i>después</i> del evento, no se ejecuta
            retroactivamente. Puedes lanzarla manualmente con <b>Ejecutar</b> o <b>Probar</b> (la prueba no modifica datos).
          </Faq>
          <Faq q="¿Dónde están mis datos? ¿Se suben a algún sitio?">
            Todo se guarda en un archivo SQLite <b>en tu equipo</b> (con permisos restringidos). Nada se envía a
            servidores de NEXUS: no existen. Lo único que puede salir de tu ordenador son las peticiones al proveedor
            de IA que tú configures, y solo cuando usas funciones de IA. Las API keys se guardan únicamente en el
            backend local y nunca se muestran ni se exportan.
          </Faq>
          <Faq q="¿Cómo hago una copia de seguridad o muevo mis datos a otro equipo?">
            <Link to="/settings" className="text-primary underline-offset-2 hover:underline">Ajustes → Datos</Link>:
            crea copias consistentes en caliente, expórtalas o restaura una copia. También hay exportación JSON
            completa (sin credenciales). En modo escritorio los datos están en{" "}
            <code className="rounded bg-muted px-1 text-[11px]">~/.local/share/app.nexus.desktop/</code>; en modo web,
            en <code className="rounded bg-muted px-1 text-[11px]">~/.local/share/nexus/</code>.
          </Faq>
          <Faq q="La ventana se ve en blanco o no carga">
            Si usas la app de escritorio, ciérrala y vuelve a abrirla (el servicio local arranca con ella). Si usas el
            modo web, comprueba que el servicio esté corriendo (<code className="rounded bg-muted px-1 text-[11px]">nexus-server</code>)
            y abre <code className="rounded bg-muted px-1 text-[11px]">http://127.0.0.1:47800</code>. El panel de
            diagnóstico de arriba te dice si el servicio responde.
          </Faq>
          <Faq q="¿Puedo usar otro modelo que no sea Qwen?">
            Sí. NEXUS habla el protocolo compatible OpenAI: sirve Qwen (DashScope), OpenAI y cualquier endpoint
            compatible (incluidos modelos locales tipo Ollama/LM Studio con pasarela compatible). Solo cambia la
            base URL, el modelo y la clave en Ajustes.
          </Faq>
          <Faq q="¿Las automatizaciones funcionan con la ventana cerrada?">
            Sí, si activas <b>Ajustes → Automatizaciones → «Que NEXUS siga trabajando aunque cierres la ventana»</b>: al cerrar la
            ventana, NEXUS queda en la bandeja del sistema (junto al reloj) y el motor sigue ejecutando
            automatizaciones. Clic en el icono de la bandeja para reabrir; clic derecho → Salir para cerrar de
            verdad. En Linux la bandeja necesita el paquete <code className="rounded bg-muted px-1 text-[11px]">libayatana-appindicator3</code>.
            En modo web, el servicio <code className="rounded bg-muted px-1 text-[11px]">nexus-server</code> ya corre
            en segundo plano por naturaleza. Si NEXUS estuvo completamente cerrado, los disparos perdidos se
            descartan de forma segura y se reprograman (nunca se ejecutan en tromba al abrir).
          </Faq>
          <Faq q="¿Qué es el modo Híbrido de una automatización?">
            Ejecuta primero todo lo que se puede hacer con reglas locales (sin gastar tokens) y usa la IA solo en los
            pasos que la necesitan. Si el proveedor no está disponible, lo local se ejecuta igual y lo inteligente
            queda registrado como «pendiente de IA» en el historial. La falta de IA nunca bloquea el motor.
          </Faq>
          <Faq q="Un agente hizo algo que no quería. ¿Puedo deshacerlo?">
            Sí. Cada modificación de un agente o automatización queda en{" "}
            <Link to="/activity" className="text-primary underline-offset-2 hover:underline">Actividad → Auditoría</Link>{" "}
            con un botón <b>Deshacer</b> que restaura el estado anterior. Además, puedes restringir los permisos
            (Ajustes → Permisos) o poner las automatizaciones en «pedir autorización» para que nada se ejecute sin tu visto bueno.
          </Faq>
          <Faq q="¿Canvas, Bases de datos y Comidas necesitan IA?">
            No. Los tres módulos funcionan al 100% sin proveedor y sin conexión: crear, editar, organizar,
            importar/exportar y planificar es todo local. La IA solo añade extras opcionales (generar mapas
            mentales, proponer estructuras de base de datos, sugerir recetas o menús) y siempre como{" "}
            <b>propuestas que tú revisas y confirmas</b>: nunca modifica nada por su cuenta. Si no hay
            proveedor, verás un aviso claro y el resto sigue funcionando.
          </Faq>
          <Faq q="En la lista de la compra, ¿por qué no se suman gramos con kilos?">
            Es deliberado: NEXUS <b>nunca convierte ni mezcla unidades distintas</b> (g ≠ kg, ml ≠ l, ud ≠ g)
            para no darte cantidades erróneas. Los ingredientes con el mismo nombre y la misma unidad se suman;
            si las unidades difieren, aparecen como líneas separadas. Puedes editar las cantidades a mano.
            Lo mismo aplica al escalado de raciones: multiplica cantidades, no cambia unidades.
          </Faq>
          <Faq q="¿Dónde se guardan las imágenes que subo a recetas, canvas o bases de datos?">
            En tu equipo: los archivos viven en la carpeta de datos de NEXUS (<code className="rounded bg-muted px-1 text-[11px]">files/</code>,
            permisos restringidos) y la base de datos solo guarda la referencia. Máximo 25 MB por archivo.
            Se incluyen en las copias de seguridad SQLite; la exportación JSON incluye los metadatos.
          </Faq>
          <Faq q="Cerré el canvas y al volver no puedo deshacer. ¿Se perdió algo?">
            No se perdió nada: el canvas se <b>autoguarda</b> mientras trabajas (verás «Guardado» arriba).
            Lo que no persiste entre sesiones es la <b>pila de deshacer/rehacer</b>: al reabrir, el contenido
            está tal cual lo dejaste, pero Ctrl+Z empieza de cero. Si exportaste el JSON, puedes reimportarlo
            como canvas nuevo en cualquier momento.
          </Faq>
          <Faq q="¿Qué es la vista «Esquema» de las bases de datos?">
            Un diagrama de tus tablas y cómo se conectan: cada base aparece con sus campos, la clave primaria
            (<b>PK</b>, el id único de cada registro) y las claves ajenas (<b>FK</b>, campos de relación que
            apuntan a otra base). Las flechas muestran la cardinalidad: 1:1 (un registro con uno), 1:N (uno con
            varios) y N:M (varios con varios). Clic en la cabecera de una tabla para abrir esa base. Si no ves
            flechas, es que aún no has creado campos de tipo Relación.
          </Faq>
          <Faq q="Las notificaciones de automatizaciones no aparecen con la ventana cerrada">
            Con «Que NEXUS siga trabajando aunque cierres la ventana» activo, los avisos de las automatizaciones
            se envían como <b>notificaciones del sistema</b> (las mismas que usan otras apps). Si no las ves:
            comprueba que tu sistema no tenga las notificaciones desactivadas o en «no molestar», y que NEXUS
            siga en la bandeja (iconito junto al reloj). Con la ventana abierta, los avisos aparecen dentro de
            la app, abajo a la derecha.
          </Faq>
          <Faq q="Borré un campo de mi base de datos. ¿Perdí los datos?">
            No: al eliminar un campo, sus valores <b>se conservan</b> internamente (borrado lógico). Si vuelves
            a crear un campo con el mismo nombre exacto, los datos antiguos no se recuperan automáticamente,
            pero siguen en la base y en tus copias de seguridad. Al borrar registros, las relaciones que
            apuntaban a ellos se limpian solas para no dejar enlaces rotos.
          </Faq>
          <Faq q="¿Qué fórmulas puedo usar en las bases de datos?">
            Aritmética (<code className="rounded bg-muted px-1 text-[11px]">+ - * / %</code>), comparaciones
            (<code className="rounded bg-muted px-1 text-[11px]">= != &lt; &gt; &lt;= &gt;=</code>), concatenación
            con <code className="rounded bg-muted px-1 text-[11px]">&amp;</code>, referencias a campos entre llaves
            (<code className="rounded bg-muted px-1 text-[11px]">{"{Precio}"} * {"{Cantidad}"}</code>) y funciones:
            IF, CONCAT, ROUND, ABS, LEN, UPPER, LOWER, TODAY y DAYS entre fechas. Se calculan al vuelo y es
            imposible que ejecuten código: el motor es un intérprete propio, cerrado y seguro.
          </Faq>
        </div>
      </Section>

      <Separator />

      <Section icon={Keyboard} title="Atajos y detalles útiles" id="atajos">
        <ul className="list-disc space-y-1 pl-4">
          <li><b className="text-foreground">Ctrl/⌘ + K</b>: captura rápida desde cualquier pantalla.</li>
          <li><b className="text-foreground">Supr</b> con una flecha seleccionada en el constructor: desconecta dos nodos.</li>
          <li>Arrastra bloques al lienzo de automatizaciones y conéctalos de izquierda a derecha; las condiciones tienen salida «sí» y «no».</li>
          <li><b className="text-foreground">Canvas</b>: Ctrl+Z / Ctrl+Y deshacer y rehacer · Ctrl+C/V copiar y pegar · Ctrl+D duplicar ·
            Ctrl+G agrupar (Ctrl+Shift+G desagrupar) · Supr eliminar · doble clic para escribir · Shift+clic o arrastrar para selección múltiple.</li>
          <li><b className="text-foreground">Comidas</b>: arrastra recetas de la tira lateral a cualquier día; el selector de raciones recalcula los ingredientes al momento.</li>
          <li>El selector de paleta del sidebar cambia el tema al instante (18 paletas × claro/oscuro, incluidas Vice City y Blackwall).</li>
          <li>Las búsquedas de Conocimiento usan el índice de texto completo de SQLite: encuentran palabras dentro de tus notas al momento.</li>
        </ul>
      </Section>

      <Separator />

      <Section icon={Database} title="Arquitectura y límites (para curiosos)" id="arquitectura">
        <p>
          NEXUS es un monolito modular en <b className="text-foreground">Rust</b> (Tokio + Axum + SQLx/SQLite) con una
          interfaz <b className="text-foreground">React + TypeScript</b> y shell de escritorio Tauri 2. El frontend
          habla con un servicio HTTP local protegido por token (solo escucha en 127.0.0.1) y recibe eventos en vivo
          por SSE. Los agentes usan un catálogo cerrado de herramientas tipadas: nunca ejecutan SQL libre, comandos
          de terminal ni código arbitrario.
        </p>
        <p>
          Límites conocidos de esta versión: el segundo plano usa la bandeja del sistema (no un servicio del
          sistema operativo que arranque solo con el equipo); la búsqueda web del agente investigador requiere un
          proveedor de búsqueda configurado (si no, lo dice explícitamente y no inventa fuentes); no hay sesiones
          de concentración todavía; la app es monousuario y local. En las herramientas: el deshacer del canvas no
          persiste entre sesiones; las fórmulas de bases de datos no se pueden usar como filtro (sí para ordenar);
          la importación CSV no crea relaciones ni adjuntos; las unidades de cocina nunca se convierten entre sí
          (g ≠ kg) y la nutrición de las recetas solo existe si tú la introduces — la IA jamás la estima.
        </p>
        <p className="flex items-center gap-1.5 text-xs">
          <Info className="h-3.5 w-3.5" />
          Documentación técnica completa en el repositorio: <code className="rounded bg-muted px-1">README.md</code> y{" "}
          <code className="rounded bg-muted px-1">docs/ARCHITECTURE.md</code>. Licencia MIT.
        </p>
      </Section>

      <footer className="flex items-center gap-2 pb-4 text-xs text-muted-foreground">
        <WifiOff className="h-3.5 w-3.5" />
        Sin telemetría, sin cuentas, sin nube: NEXUS es tuyo.
      </footer>
    </div>
  );
}
