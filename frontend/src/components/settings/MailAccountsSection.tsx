import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Loader2, Lock, Mail, Pencil, Plug, Plus, Server, ShieldCheck, Trash2 } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";

export interface MailAccount {
  id: string;
  name: string;
  imap_host: string;
  imap_port: number;
  smtp_host: string;
  smtp_port: number;
  username: string;
  use_tls: boolean;
}

interface AccountForm {
  account: MailAccount;
  password: string;
}

const ACCOUNTS_KEY = ["mail-accounts"];

const PASSWORD_NOTE = "La contraseña se guarda solo en el backend local y nunca se envía al frontend de nuevo";

function newAccount(): MailAccount {
  return {
    id: "",
    name: "",
    imap_host: "imap.gmail.com",
    imap_port: 993,
    smtp_host: "smtp.gmail.com",
    smtp_port: 465,
    username: "",
    use_tls: true,
  };
}

export function MailAccountsSection() {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<AccountForm | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<MailAccount | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);

  const accounts = useQuery({
    queryKey: ACCOUNTS_KEY,
    queryFn: () => invoke<MailAccount[]>("mail.accounts.list"),
  });

  const save = useMutation({
    mutationFn: (form: AccountForm) =>
      invoke<MailAccount[]>("mail.accounts.save", { account: form.account, password: form.password }),
    onSuccess: (updated, form) => {
      qc.setQueryData<MailAccount[]>(ACCOUNTS_KEY, updated);
      toast.success(form.account.id ? "Cuenta actualizada" : "Cuenta añadida");
      setEditing(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar la cuenta"),
  });

  const remove = useMutation({
    mutationFn: (id: string) => invoke("mail.accounts.delete", { id }),
    onSuccess: (_, id) => {
      qc.setQueryData<MailAccount[]>(ACCOUNTS_KEY, (prev) => prev?.filter((a) => a.id !== id) ?? []);
      toast.success("Cuenta eliminada");
      setDeleteTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar la cuenta"),
  });

  const test = useMutation({
    mutationFn: (id: string) => invoke("mail.accounts.test", { id }),
    onMutate: (id) => setTestingId(id),
    onSuccess: () => toast.success("Conexión correcta", { description: "IMAP y SMTP responden correctamente." }),
    onError: (e) => {
      toast.error("La prueba de conexión falló", {
        description: e instanceof ApiError ? e.message : "Error desconocido",
      });
    },
    onSettled: () => setTestingId(null),
  });

  const isEditingExisting = editing != null && editing.account.id !== "";
  const c = editing?.account;
  const formValid =
    c != null &&
    c.name.trim() !== "" &&
    c.imap_host.trim() !== "" &&
    c.smtp_host.trim() !== "" &&
    c.username.trim() !== "" &&
    c.imap_port > 0 &&
    c.imap_port <= 65535 &&
    c.smtp_port > 0 &&
    c.smtp_port <= 65535 &&
    (isEditingExisting || editing!.password.trim() !== "");

  return (
    <div className="space-y-4">
      {accounts.isLoading ? (
        <div className="space-y-2">
          <Skeleton className="h-20 w-full rounded-lg" />
          <Skeleton className="h-20 w-full rounded-lg" />
        </div>
      ) : accounts.isError ? (
        <div className="space-y-2 rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          <p>{accounts.error instanceof ApiError ? accounts.error.message : "No se pudieron cargar las cuentas."}</p>
          <Button size="sm" variant="outline" onClick={() => accounts.refetch()}>Reintentar</Button>
        </div>
      ) : (accounts.data ?? []).length === 0 ? (
        <div className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
          No hay cuentas de correo configuradas. Añade una para conectar tu buzón.
        </div>
      ) : (
        <ul className="space-y-2">
          {(accounts.data ?? []).map((account) => (
            <li
              key={account.id}
              className="rounded-lg border border-border bg-card p-4 shadow-sm transition-colors hover:border-ring/30"
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium">{account.name}</span>
                <Badge variant="secondary"><Mail /> IMAP {account.imap_host}:{account.imap_port}</Badge>
                <Badge variant="outline"><Server /> SMTP {account.smtp_host}:{account.smtp_port}</Badge>
                {account.use_tls ? (
                  <Badge variant="success"><ShieldCheck /> TLS</Badge>
                ) : (
                  <Badge variant="warning"><Lock /> Sin TLS</Badge>
                )}
              </div>
              <p className="mt-1 font-mono text-[11px] text-muted-foreground">{account.username}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setEditing({ account: { ...account }, password: "" })}
                >
                  <Pencil /> Editar
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => test.mutate(account.id)}
                  disabled={testingId != null}
                >
                  {testingId === account.id ? <Loader2 className="animate-spin" /> : <Plug />}
                  Probar conexión
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto text-muted-foreground hover:text-destructive"
                  onClick={() => setDeleteTarget(account)}
                >
                  <Trash2 /> Eliminar
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button size="sm" onClick={() => setEditing({ account: newAccount(), password: "" })}>
          <Plus /> Añadir cuenta
        </Button>
        <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          <Lock className="h-3 w-3 shrink-0" />
          {PASSWORD_NOTE}
        </p>
      </div>

      <Dialog open={editing != null} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{isEditingExisting ? "Editar cuenta de correo" : "Añadir cuenta de correo"}</DialogTitle>
            <DialogDescription>
              IMAP/SMTP con contraseña de aplicación (Gmail, Outlook…). Puertos habituales: IMAP 993 y SMTP 465 o 587.
            </DialogDescription>
          </DialogHeader>
          {editing && (
            <div className="space-y-3">
              <div className="space-y-1.5">
                <Label>Nombre</Label>
                <Input
                  value={editing.account.name}
                  onChange={(e) =>
                    setEditing({ ...editing, account: { ...editing.account, name: e.target.value } })
                  }
                  placeholder="Correo principal"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label>Servidor IMAP</Label>
                  <Input
                    value={editing.account.imap_host}
                    onChange={(e) =>
                      setEditing({ ...editing, account: { ...editing.account, imap_host: e.target.value } })
                    }
                    placeholder="imap.gmail.com"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>Puerto IMAP</Label>
                  <Input
                    type="number"
                    min={1}
                    max={65535}
                    value={editing.account.imap_port}
                    onChange={(e) =>
                      setEditing({
                        ...editing,
                        account: { ...editing.account, imap_port: parseInt(e.target.value, 10) || 0 },
                      })
                    }
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label>Servidor SMTP</Label>
                  <Input
                    value={editing.account.smtp_host}
                    onChange={(e) =>
                      setEditing({ ...editing, account: { ...editing.account, smtp_host: e.target.value } })
                    }
                    placeholder="smtp.gmail.com"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>Puerto SMTP</Label>
                  <Input
                    type="number"
                    min={1}
                    max={65535}
                    value={editing.account.smtp_port}
                    onChange={(e) =>
                      setEditing({
                        ...editing,
                        account: { ...editing.account, smtp_port: parseInt(e.target.value, 10) || 0 },
                      })
                    }
                  />
                </div>
              </div>
              <div className="space-y-1.5">
                <Label>Usuario</Label>
                <Input
                  value={editing.account.username}
                  onChange={(e) =>
                    setEditing({ ...editing, account: { ...editing.account, username: e.target.value } })
                  }
                  placeholder="nombre@dominio.com"
                  autoComplete="username"
                />
              </div>
              <div className="space-y-1.5">
                <Label>Contraseña de aplicación</Label>
                <Input
                  type="password"
                  value={editing.password}
                  onChange={(e) => setEditing({ ...editing, password: e.target.value })}
                  placeholder={isEditingExisting ? "•••••••• (sin cambios)" : "Contraseña de aplicación"}
                  autoComplete="new-password"
                />
              </div>
              <div className="flex items-center gap-2">
                <Switch
                  id="mail-account-tls"
                  checked={editing.account.use_tls}
                  onCheckedChange={(v) =>
                    setEditing({ ...editing, account: { ...editing.account, use_tls: v } })
                  }
                />
                <Label htmlFor="mail-account-tls" className="cursor-pointer">Usar TLS</Label>
              </div>
              <p className="flex items-start gap-1.5 text-[11px] text-muted-foreground">
                <Lock className="mt-0.5 h-3 w-3 shrink-0" />
                {PASSWORD_NOTE}
              </p>
            </div>
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
            <DialogTitle>Eliminar cuenta de correo</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.name}» y su contraseña se eliminarán del backend local.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => deleteTarget && remove.mutate(deleteTarget.id)} disabled={remove.isPending}>
              {remove.isPending && <Loader2 className="animate-spin" />}
              Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
