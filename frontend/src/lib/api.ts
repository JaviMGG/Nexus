/**
 * Cliente de API unificado:
 * - En Tauri: obtiene el puerto del servicio local vía comando `server_info`
 *   y llama a http://127.0.0.1:<port>/api/command con token de sesión.
 * - En desarrollo web: usa el proxy de Vite (/api → 127.0.0.1:47800).
 */

export class ApiError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

interface ServerInfo {
  port: number;
  token: string;
}

let serverInfoPromise: Promise<ServerInfo | null> | null = null;

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function getServerInfo(): Promise<ServerInfo | null> {
  if (!serverInfoPromise) {
    serverInfoPromise = (async () => {
      if (isTauri()) {
        const { invoke } = await import("@tauri-apps/api/core");
        // El servicio puede tardar unos instantes en arrancar tras abrir la app.
        for (let i = 0; i < 30; i++) {
          try {
            return await invoke<ServerInfo>("server_info");
          } catch {
            await new Promise((r) => setTimeout(r, 300));
          }
        }
        return null;
      }
      const res = await fetch("/api/session");
      if (!res.ok) return null;
      const data = await res.json();
      return { port: 0, token: data.token as string };
    })().catch(() => null);
  }
  return serverInfoPromise;
}

export async function apiBase(): Promise<string> {
  if (isTauri()) {
    const info = await getServerInfo();
    if (info) return `http://127.0.0.1:${info.port}`;
  }
  return "";
}

export async function apiUrl(path: string): Promise<string> {
  const base = await apiBase();
  const info = await getServerInfo();
  const url = `${base}${path}`;
  if (info?.token) {
    return url + (url.includes("?") ? "&" : "?") + `token=${encodeURIComponent(info.token)}`;
  }
  return url;
}

export async function invoke<T>(command: string, payload: unknown = {}): Promise<T> {
  const base = await apiBase();
  const info = await getServerInfo();
  let res: Response;
  try {
    res = await fetch(`${base}/api/command`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(info?.token ? { Authorization: `Bearer ${info.token}` } : {}),
      },
      body: JSON.stringify({ command, payload }),
    });
  } catch {
    throw new ApiError("network", "No se puede conectar con el servicio local de NEXUS.");
  }
  const body = await res.json().catch(() => null);
  if (!res.ok) {
    const err = body?.error ?? { code: "internal", message: `Error ${res.status}` };
    throw new ApiError(err.code ?? "internal", err.message ?? "Error desconocido");
  }
  return body?.result as T;
}
