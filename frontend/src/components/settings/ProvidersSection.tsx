import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  KeyRound,
  Loader2,
  Lock,
  Pencil,
  Plug,
  Plus,
  Sparkles,
  Star,
  Trash2,
  WifiOff,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { AiStatus, ProviderConfig } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

function newProvider(): ProviderConfig {
  return {
    id: "",
    name: "Qwen",
    kind: "openai_compatible",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-max",
    api_key_set: false,
    temperature: 0.7,
    max_tokens: 4096,
    requests_per_minute: 30,
    enabled: true,
    is_default: false,
  };
}

function AiStatusBanner({ status }: { status: AiStatus | undefined }) {
  const state = status?.state ?? "disconnected";
  return (
    <div
      className={cn(
        "flex items-center gap-2.5 rounded-lg border px-4 py-3 text-sm",
        state === "connected" && "border-success/40 bg-success/10 text-success",
        state === "error" && "border-destructive/40 bg-destructive/10 text-destructive",
        state === "disconnected" && "border-border bg-muted/60 text-muted-foreground",
      )}
    >
      {state === "connected" ? (
        <Sparkles className="h-4 w-4 shrink-0" />
      ) : state === "error" ? (
        <Plug className="h-4 w-4 shrink-0" />
      ) : (
        <WifiOff className="h-4 w-4 shrink-0" />
      )}
      <div className="min-w-0">
        {state === "connected" && (
          <p className="font-medium">Conectada · {status?.provider} · {status?.model}</p>
        )}
        {state === "disconnected" && (
          <p>Desconectada — NEXUS funciona en modo manual. Añade un proveedor para activar la IA.</p>
        )}
        {state === "error" && (
          <p className="font-medium">Error de conexión{status?.message ? `: ${status.message}` : ""}</p>
        )}
      </div>
    </div>
  );
}

interface ProviderForm {
  config: ProviderConfig;
  apiKey: string;
}

export function ProvidersSection({ providers, aiStatus }: { providers: ProviderConfig[]; aiStatus: AiStatus | undefined }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<ProviderForm | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderConfig | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["settings"] });
    qc.invalidateQueries({ queryKey: ["ai-status"] });
  };

  const save = useMutation({
    mutationFn: (form: ProviderForm) =>
      invoke<ProviderConfig>("providers.save", {
        config: form.config,
        ...(form.apiKey.trim() ? { api_key: form.apiKey.trim() } : {}),
      }),
    onSuccess: (_, form) => {
      toast.success(form.config.id ? "Proveedor actualizado" : "Proveedor añadido");
      invalidate();
      setEditing(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar el proveedor"),
  });

  const test = useMutation({
    mutationFn: (id: string) => invoke<{ message: string }>("providers.test", { id }),
    onMutate: (id) => setTestingId(id),
    onSuccess: (r) => {
      toast.success("Conexión correcta", { description: r.message });
      invalidate();
    },
    onError: (e) => {
      toast.error("La prueba de conexión falló", {
        description: e instanceof ApiError ? e.message : "Error desconocido",
      });
      invalidate();
    },
    onSettled: () => setTestingId(null),
  });

  const setDefault = useMutation({
    mutationFn: (id: string) => invoke("providers.set_default", { id }),
    onSuccess: () => {
      toast.success("Proveedor predeterminado actualizado");
      invalidate();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cambiar el predeterminado"),
  });

  const remove = useMutation({
    mutationFn: () => invoke("providers.delete", { id: deleteTarget!.id }),
    onSuccess: () => {
      toast.success("Proveedor eliminado");
      invalidate();
      setDeleteTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar"),
  });

  const isEditingExisting = editing != null && editing.config.id !== "";
  const formValid =
    editing != null &&
    editing.config.name.trim() !== "" &&
    editing.config.base_url.trim() !== "" &&
    editing.config.model.trim() !== "" &&
    editing.config.temperature >= 0 &&
    editing.config.temperature <= 2 &&
    editing.config.max_tokens > 0 &&
    editing.config.requests_per_minute > 0;

  return (
    <div className="space-y-4">
      <AiStatusBanner status={aiStatus} />

      {providers.length === 0 ? (
        <div className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
          No hay proveedores configurados. NEXUS seguirá funcionando en modo manual.
        </div>
      ) : (
        <ul className="space-y-2">
          {providers.map((p) => (
            <li key={p.id} className="rounded-lg border border-border bg-card p-4 shadow-sm transition-colors hover:border-ring/30">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium">{p.name}</span>
                <span className="font-mono text-xs text-muted-foreground">{p.model}</span>
                {p.is_default && <Badge variant="default"><Star /> Predeterminado</Badge>}
                {p.api_key_set ? (
                  <Badge variant="success"><KeyRound /> Key configurada</Badge>
                ) : (
                  <Badge variant="warning"><KeyRound /> Sin key</Badge>
                )}
                {!p.enabled && <Badge variant="outline">Deshabilitado</Badge>}
              </div>
              <p className="mt-1 max-w-md truncate text-[11px] text-muted-foreground">{p.base_url}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button size="sm" variant="outline" onClick={() => setEditing({ config: { ...p }, apiKey: "" })}>
                  <Pencil /> Editar
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => test.mutate(p.id)}
                  disabled={testingId != null}
                >
                  {testingId === p.id ? <Loader2 className="animate-spin" /> : <Plug />}
                  Probar conexión
                </Button>
                {!p.is_default && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setDefault.mutate(p.id)}
                    disabled={setDefault.isPending}
                  >
                    <Star /> Hacer predeterminado
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto text-muted-foreground hover:text-destructive"
                  onClick={() => setDeleteTarget(p)}
                >
                  <Trash2 /> Eliminar
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button size="sm" onClick={() => setEditing({ config: newProvider(), apiKey: "" })}>
          <Plus /> Añadir proveedor
        </Button>
        <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <Lock className="h-3 w-3" />
          Las API keys se guardan solo en el backend local; nunca se almacenan en el frontend.
        </p>
      </div>

      <Dialog open={editing != null} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{isEditingExisting ? "Editar proveedor" : "Añadir proveedor"}</DialogTitle>
            <DialogDescription>
              Compatible con la API de OpenAI (Qwen/DashScope, OpenRouter, Ollama, LM Studio…).
            </DialogDescription>
          </DialogHeader>
          {editing && (
            <ProviderFields
              form={editing}
              onChange={setEditing}
              isExisting={isEditingExisting}
            />
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditing(null)}>Cancelar</Button>
            <Button onClick={() => editing && save.mutate(editing)} disabled={!formValid || save.isPending}>
              {save.isPending && <Loader2 className="animate-spin" />}
              Guardar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteTarget != null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar proveedor</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.name}» y su API key se eliminarán del backend. NEXUS volverá al modo manual si era el único proveedor.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending && <Loader2 className="animate-spin" />}
              Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ProviderFields({
  form,
  onChange,
  isExisting,
}: {
  form: ProviderForm;
  onChange: (f: ProviderForm) => void;
  isExisting: boolean;
}) {
  const c = form.config;
  const set = <K extends keyof ProviderConfig>(key: K, value: ProviderConfig[K]) =>
    onChange({ ...form, config: { ...c, [key]: value } });

  const [temp, setTemp] = useState(String(c.temperature));
  useEffect(() => setTemp(String(c.temperature)), [c.id]);

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label>Nombre</Label>
          <Input value={c.name} onChange={(e) => set("name", e.target.value)} placeholder="Qwen" />
        </div>
        <div className="space-y-1.5">
          <Label>Modelo</Label>
          <Input value={c.model} onChange={(e) => set("model", e.target.value)} placeholder="qwen-max" />
        </div>
      </div>
      <div className="space-y-1.5">
        <Label>Base URL</Label>
        <Input
          value={c.base_url}
          onChange={(e) => set("base_url", e.target.value)}
          placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
        />
      </div>
      <div className="space-y-1.5">
        <Label>API key</Label>
        <Input
          type="password"
          value={form.apiKey}
          onChange={(e) => onChange({ ...form, apiKey: e.target.value })}
          placeholder={isExisting ? "•••••••• (sin cambios)" : "sk-…"}
          autoComplete="new-password"
        />
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1.5">
          <Label>Temperatura</Label>
          <Input
            type="number"
            min={0}
            max={2}
            step={0.1}
            value={temp}
            onChange={(e) => {
              setTemp(e.target.value);
              const n = parseFloat(e.target.value);
              if (!Number.isNaN(n)) set("temperature", n);
            }}
          />
        </div>
        <div className="space-y-1.5">
          <Label>Máx. tokens</Label>
          <Input
            type="number"
            min={1}
            value={c.max_tokens}
            onChange={(e) => set("max_tokens", parseInt(e.target.value, 10) || 0)}
          />
        </div>
        <div className="space-y-1.5">
          <Label>Peticiones/min</Label>
          <Input
            type="number"
            min={1}
            value={c.requests_per_minute}
            onChange={(e) => set("requests_per_minute", parseInt(e.target.value, 10) || 0)}
          />
        </div>
      </div>
      <div className="flex items-center gap-6 pt-1">
        <div className="flex items-center gap-2">
          <Switch id="provider-enabled" checked={c.enabled} onCheckedChange={(v) => set("enabled", v)} />
          <Label htmlFor="provider-enabled" className="cursor-pointer">Habilitado</Label>
        </div>
        <div className="flex items-center gap-2">
          <Switch id="provider-default" checked={c.is_default} onCheckedChange={(v) => set("is_default", v)} />
          <Label htmlFor="provider-default" className="cursor-pointer">Predeterminado</Label>
        </div>
      </div>
    </div>
  );
}
