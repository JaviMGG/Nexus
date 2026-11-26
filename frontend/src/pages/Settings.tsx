import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Database, Gauge, Loader2, Mail, Palette, Save, ShieldCheck, Sparkles, User, Workflow } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { AgentPermissions, AppSettings, Policy, SettingsPayload, UsageLimits } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { EmptyState } from "@/components/ui/empty-state";
import { ThemeToggle, PaletteGrid } from "@/components/settings/PaletteGrid";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { MailAccountsSection } from "@/components/settings/MailAccountsSection";
import { DataSection } from "@/components/settings/DataSection";

function Section({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: LucideIcon;
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-4">
      <div className="flex items-start gap-3">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/15">
          <Icon className="h-4 w-4 text-primary" />
        </div>
        <div>
          <h2 className="text-sm font-semibold">{title}</h2>
          {description && <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>}
        </div>
      </div>
      <div className="pl-11">{children}</div>
    </section>
  );
}

function ProfileSection({ app }: { app: AppSettings }) {
  const qc = useQueryClient();
  const [name, setName] = useState(app.user_name);
  useEffect(() => setName(app.user_name), [app.user_name]);

  const save = useMutation({
    mutationFn: () => invoke<AppSettings>("settings.update", { patch: { user_name: name.trim() } }),
    onSuccess: () => {
      toast.success("Perfil guardado");
      qc.invalidateQueries({ queryKey: ["settings"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar"),
  });

  return (
    <div className="flex max-w-md items-end gap-2">
      <div className="flex-1 space-y-1.5">
        <Label>Tu nombre</Label>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="¿Cómo te llamas?"
          onKeyDown={(e) => {
            if (e.key === "Enter" && name.trim() && name !== app.user_name) save.mutate();
          }}
        />
      </div>
      <Button
        size="sm"
        onClick={() => save.mutate()}
        disabled={!name.trim() || name === app.user_name || save.isPending}
      >
        {save.isPending ? <Loader2 className="animate-spin" /> : <Save />}
        Guardar
      </Button>
    </div>
  );
}

const PERMISSION_LABEL: Record<keyof AgentPermissions, string> = {
  read: "Lectura",
  write: "Escritura",
  destructive: "Destructivas",
};

function PermissionsSection({ app }: { app: AppSettings }) {
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: (permissions: AgentPermissions) =>
      invoke<AppSettings>("settings.update", { patch: { permissions } }),
    onSuccess: () => {
      toast.success("Permisos actualizados");
      qc.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo actualizar"),
  });

  return (
    <div className="space-y-3">
      <div className="grid max-w-2xl gap-3 sm:grid-cols-3">
        {(Object.keys(PERMISSION_LABEL) as (keyof AgentPermissions)[]).map((key) => (
          <div key={key} className="space-y-1.5">
            <Label>{PERMISSION_LABEL[key]}</Label>
            <Select
              value={app.permissions[key]}
              onValueChange={(v) => save.mutate({ ...app.permissions, [key]: v as Policy })}
              disabled={save.isPending}
            >
              <SelectTrigger className="text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">Automático</SelectItem>
                <SelectItem value="confirm">Pedir confirmación</SelectItem>
              </SelectContent>
            </Select>
          </div>
        ))}
      </div>
      <p className="text-[11px] text-muted-foreground">
        Las acciones destructivas (eliminar datos, sobrescribir copias…) deberían pedir siempre
        confirmación. Las aprobaciones pendientes aparecen en el chat y en Actividad → Aprobaciones.
      </p>
    </div>
  );
}

const LIMIT_LABEL: Record<keyof UsageLimits, string> = {
  max_iterations: "Iteraciones máx.",
  max_tokens_per_run: "Tokens por ejecución",
  max_delegation_depth: "Profundidad de delegación",
  requests_per_minute: "Peticiones/min",
};

function LimitsSection({ app }: { app: AppSettings }) {
  const qc = useQueryClient();
  const [draft, setDraft] = useState<Record<keyof UsageLimits, string>>({
    max_iterations: String(app.limits.max_iterations),
    max_tokens_per_run: String(app.limits.max_tokens_per_run),
    max_delegation_depth: String(app.limits.max_delegation_depth),
    requests_per_minute: String(app.limits.requests_per_minute),
  });

  useEffect(() => {
    setDraft({
      max_iterations: String(app.limits.max_iterations),
      max_tokens_per_run: String(app.limits.max_tokens_per_run),
      max_delegation_depth: String(app.limits.max_delegation_depth),
      requests_per_minute: String(app.limits.requests_per_minute),
    });
  }, [app.limits]);

  const parsed = Object.fromEntries(
    Object.entries(draft).map(([k, v]) => [k, parseInt(v, 10)]),
  ) as Record<keyof UsageLimits, number>;

  const valid = Object.values(parsed).every((n) => Number.isFinite(n) && n > 0);
  const dirty = (Object.keys(parsed) as (keyof UsageLimits)[]).some(
    (k) => parsed[k] !== app.limits[k],
  );

  const save = useMutation({
    mutationFn: () => invoke<AppSettings>("settings.update", { patch: { limits: parsed } }),
    onSuccess: () => {
      toast.success("Límites actualizados");
      qc.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo actualizar"),
  });

  return (
    <div className="space-y-3">
      <div className="grid max-w-2xl gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {(Object.keys(LIMIT_LABEL) as (keyof UsageLimits)[]).map((key) => (
          <div key={key} className="space-y-1.5">
            <Label>{LIMIT_LABEL[key]}</Label>
            <Input
              type="number"
              min={1}
              value={draft[key]}
              onChange={(e) => setDraft((d) => ({ ...d, [key]: e.target.value }))}
            />
          </div>
        ))}
      </div>
      <div className="flex items-center gap-3">
        <Button size="sm" onClick={() => save.mutate()} disabled={!valid || !dirty || save.isPending}>
          {save.isPending ? <Loader2 className="animate-spin" /> : <Save />}
          Guardar límites
        </Button>
        {!valid && <span className="text-[11px] text-destructive">Todos los valores deben ser mayores que 0.</span>}
      </div>
      <p className="text-[11px] text-muted-foreground">
        Los límites protegen tu consumo de tokens y evitan bucles: cada ejecución se detiene al alcanzarlos.
      </p>
    </div>
  );
}

function AutomationsSection({ app }: { app: AppSettings }) {
  const qc = useQueryClient();
  const [tokenDraft, setTokenDraft] = useState(String(app.automations.daily_token_limit));

  useEffect(() => {
    setTokenDraft(String(app.automations.daily_token_limit));
  }, [app.automations.daily_token_limit]);

  const update = useMutation({
    mutationFn: (patch: Record<string, unknown>) =>
      invoke<AppSettings>("settings.update", { patch: { automations: patch } }),
    onSuccess: () => {
      toast.success("Ajustes de automatizaciones guardados");
      qc.invalidateQueries({ queryKey: ["settings"] });
      qc.invalidateQueries({ queryKey: ["automation-overview"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo actualizar"),
  });

  const tokenValue = parseInt(tokenDraft, 10);
  const tokenValid = Number.isFinite(tokenValue) && tokenValue >= 0;
  const tokenDirty = tokenValid && tokenValue !== app.automations.daily_token_limit;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-4 rounded-lg border border-border px-3 py-2.5">
        <div>
          <p className="text-sm font-medium">Que NEXUS siga trabajando aunque cierres la ventana</p>
          <p className="text-[11px] text-muted-foreground">
            Al cerrar, NEXUS no se apaga: se queda como un iconito junto al reloj y tus
            automatizaciones siguen funcionando. Pulsa ese icono para volver a abrir la ventana;
            para apagarlo del todo, clic derecho sobre él → Salir.
          </p>
        </div>
        <Switch
          checked={app.automations.run_in_background}
          onCheckedChange={(v) => update.mutate({ run_in_background: v })}
          disabled={update.isPending}
        />
      </div>

      <div className="flex flex-wrap items-end gap-3">
        <div className="space-y-1.5">
          <Label>Límite diario de tokens (todas las automatizaciones)</Label>
          <Input
            type="number"
            min={0}
            className="w-44"
            value={tokenDraft}
            onChange={(e) => setTokenDraft(e.target.value)}
          />
        </div>
        <Button
          size="sm"
          onClick={() => update.mutate({ daily_token_limit: tokenValue })}
          disabled={!tokenValid || !tokenDirty || update.isPending}
        >
          {update.isPending ? <Loader2 className="animate-spin" /> : <Save />}
          Guardar límite
        </Button>
      </div>
      <p className="text-[11px] text-muted-foreground">
        0 = sin límite. El modo Local no consume tokens; este tope protege el consumo de IA.
      </p>
    </div>
  );
}

export function SettingsPage() {
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => invoke<SettingsPayload>("settings.get"),
  });

  if (settings.isLoading) {
    return (
      <div className="mx-auto max-w-[1200px] space-y-6 p-6">
        <Skeleton className="h-7 w-40" />
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="space-y-3">
            <Skeleton className="h-5 w-56" />
            <Skeleton className="h-24 w-full max-w-2xl rounded-xl" />
          </div>
        ))}
      </div>
    );
  }

  if (settings.isError || !settings.data) {
    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <EmptyState
          icon={Database}
          title="No se pudieron cargar los ajustes"
          description={settings.error instanceof ApiError ? settings.error.message : "Comprueba que el servicio local está activo."}
          action={<Button size="sm" variant="outline" onClick={() => settings.refetch()}>Reintentar</Button>}
        />
      </div>
    );
  }

  const { app, providers, ai_status } = settings.data;

  return (
    <div className="mx-auto max-w-[1200px] space-y-8 p-6 animate-fade-in">
      <div>
        <h1 className="text-lg font-semibold tracking-tight">Ajustes</h1>
        <p className="text-xs text-muted-foreground">Personaliza NEXUS a tu manera. Todo se guarda en local.</p>
      </div>

      <Section icon={User} title="Perfil" description="Cómo te saluda NEXUS en el dashboard.">
        <ProfileSection app={app} />
      </Section>

      <Separator />

      <Section
        icon={Palette}
        title="Apariencia"
        description="Tema y paleta de color. Se aplican al instante y se sincronizan con el backend."
      >
        <div className="space-y-4">
          <ThemeToggle />
          <PaletteGrid />
        </div>
      </Section>

      <Separator />

      <Section
        icon={Sparkles}
        title="Inteligencia Artificial"
        description="Opcional: sin proveedor, NEXUS funciona en modo manual con todas sus funciones."
      >
        <ProvidersSection providers={providers} aiStatus={ai_status} />
      </Section>

      <Separator />

      <Section
        icon={ShieldCheck}
        title="Permisos de agentes"
        description="Controla qué pueden hacer los agentes sin preguntarte."
      >
        <PermissionsSection app={app} />
      </Section>

      <Separator />

      <Section icon={Gauge} title="Límites de uso" description="Topes de seguridad para las ejecuciones de agentes.">
        <LimitsSection app={app} />
      </Section>

      <Separator />

      <Section
        icon={Workflow}
        title="Automatizaciones"
        description="Tus automatizaciones se activan y desactivan una a una en la sección Automatizaciones. Aquí controlas cómo trabajan."
      >
        <AutomationsSection app={app} />
      </Section>

      <Separator />

      <Section
        icon={Mail}
        title="Correo"
        description="Cuentas IMAP/SMTP con contraseña de aplicación para conectar tu buzón."
      >
        <MailAccountsSection />
      </Section>

      <Separator />

      <Section
        icon={Database}
        title="Datos y copias de seguridad"
        description="Exporta, guarda y restaura el estado completo de tu NEXUS."
      >
        <DataSection />
      </Section>
    </div>
  );
}
