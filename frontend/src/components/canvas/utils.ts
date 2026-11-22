import { invoke } from "@/lib/api";
import type { FileMeta } from "@/lib/types";
import type { CanvasDocument, CanvasElement, CanvasElementType } from "./types";

export function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return crypto.randomUUID();
  return `el_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
}

export const ELEMENT_LABEL: Record<CanvasElementType, string> = {
  sticky: "Nota adhesiva",
  text: "Texto",
  rect: "Rectángulo",
  ellipse: "Elipse",
  line: "Línea",
  arrow: "Flecha",
  image: "Imagen",
  link: "Enlace",
  card: "Tarjeta",
  task_ref: "Tarea",
  project_ref: "Proyecto",
  note_ref: "Nota",
  database_ref: "Base de datos",
  recipe_ref: "Receta",
  document: "Documento",
};

export const STICKY_COLORS = [
  "#fef3c7",
  "#dcfce7",
  "#dbeafe",
  "#fce7f3",
  "#ede9fe",
  "#ffedd5",
  "#fee2e2",
  "#e2e8f0",
];

const THEME_TOKENS = new Set([
  "primary",
  "card",
  "muted",
  "accent",
  "secondary",
  "success",
  "warning",
  "destructive",
  "background",
]);

/** Convierte un color del modelo (token o hex) a valor CSS real del tema. */
export function resolveColor(value: string | undefined | null, fallback: string): string {
  if (!value) return fallback;
  if (THEME_TOKENS.has(value)) return `var(--${value})`;
  return value;
}

export function isTokenColor(value: string | undefined | null): boolean {
  return !!value && THEME_TOKENS.has(value);
}

export function textColorFor(background: string | undefined): string {
  if (!background) return "var(--foreground)";
  if (background.startsWith("#")) {
    const hex = background.slice(1);
    if (hex.length === 6) {
      const r = parseInt(hex.slice(0, 2), 16);
      const g = parseInt(hex.slice(2, 4), 16);
      const b = parseInt(hex.slice(4, 6), 16);
      const luma = 0.299 * r + 0.587 * g + 0.114 * b;
      return luma > 150 ? "#1f2937" : "#f9fafb";
    }
    return "var(--foreground)";
  }
  if (background === "primary" || background === "destructive") return `var(--${background}-foreground)`;
  if (background === "success" || background === "warning") return "#ffffff";
  return "var(--foreground)";
}

export function elementText(el: CanvasElement): string {
  const c = el.content;
  const str = (k: string) => (typeof c[k] === "string" ? (c[k] as string) : "");
  switch (el.type) {
    case "card":
      return [str("title"), str("body")].filter(Boolean).join(": ");
    case "link":
      return str("title") || str("url");
    case "image":
    case "document":
      return str("name") || "Archivo";
    case "task_ref":
    case "project_ref":
    case "note_ref":
    case "database_ref":
    case "recipe_ref":
      return str("title") || "Referencia";
    default:
      return str("text");
  }
}

export function defaultSize(type: CanvasElementType): { w: number; h: number } {
  switch (type) {
    case "sticky":
      return { w: 190, h: 170 };
    case "text":
      return { w: 240, h: 60 };
    case "rect":
    case "ellipse":
      return { w: 200, h: 110 };
    case "line":
    case "arrow":
      return { w: 220, h: 12 };
    case "image":
      return { w: 260, h: 200 };
    case "link":
      return { w: 240, h: 80 };
    case "card":
      return { w: 260, h: 150 };
    case "document":
      return { w: 240, h: 90 };
    default:
      return { w: 240, h: 110 };
  }
}

export function defaultContent(type: CanvasElementType): Record<string, unknown> {
  switch (type) {
    case "card":
      return { title: "Título", body: "Escribe aquí…" };
    case "link":
      return { url: "https://", title: "Enlace" };
    default:
      return { text: "" };
  }
}

export function defaultStyle(type: CanvasElementType): Record<string, unknown> {
  if (type === "sticky") return { background: STICKY_COLORS[0], text: "#1f2937" };
  return {};
}

export function nextZ(doc: CanvasDocument): number {
  return doc.elements.reduce((m, e) => Math.max(m, e.z), 0) + 1;
}

export function cloneDocument(doc: CanvasDocument): CanvasDocument {
  return JSON.parse(JSON.stringify(doc)) as CanvasDocument;
}

/** Copia elementos (y aristas internas) con ids nuevos y un desplazamiento. */
export function cloneElements(
  elements: CanvasElement[],
  edges: CanvasDocument["edges"],
  offset: { x: number; y: number },
  zStart: number,
): { elements: CanvasElement[]; edges: CanvasDocument["edges"] } {
  const idMap = new Map<string, string>();
  const groupMap = new Map<string, string>();
  const newEls = elements.map((el, i) => {
    const id = newId();
    idMap.set(el.id, id);
    let group = el.group_id ?? null;
    if (group) {
      if (!groupMap.has(group)) groupMap.set(group, newId());
      group = groupMap.get(group)!;
    }
    const copy: CanvasElement = JSON.parse(JSON.stringify(el)) as CanvasElement;
    return { ...copy, id, x: el.x + offset.x, y: el.y + offset.y, z: zStart + i, group_id: group };
  });
  const newEdges = edges
    .filter((e) => idMap.has(e.source) && idMap.has(e.target))
    .map((e) => ({ ...e, id: newId(), source: idMap.get(e.source)!, target: idMap.get(e.target)! }));
  return { elements: newEls, edges: newEdges };
}

// ---------------------------------------------------------------------------
// Archivos: imágenes y documentos como data URLs cacheadas en memoria
// ---------------------------------------------------------------------------

const fileCache = new Map<string, Promise<{ meta: FileMeta; dataUrl: string }>>();

export function getFile(fileId: string): Promise<{ meta: FileMeta; dataUrl: string }> {
  let p = fileCache.get(fileId);
  if (!p) {
    p = invoke<{ meta: FileMeta; data_base64: string }>("files.get", { id: fileId })
      .then(({ meta, data_base64 }) => ({
        meta,
        dataUrl: `data:${meta.mime || "application/octet-stream"};base64,${data_base64}`,
      }))
      .catch((e) => {
        fileCache.delete(fileId);
        throw e;
      });
    fileCache.set(fileId, p);
  }
  return p;
}

export async function saveFile(file: File): Promise<FileMeta> {
  const buf = await file.arrayBuffer();
  const bytes = new Uint8Array(buf);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return invoke<FileMeta>("files.save", {
    name: file.name,
    mime: file.type || "application/octet-stream",
    data_base64: btoa(binary),
  });
}

// ---------------------------------------------------------------------------
// Descargas reales vía Blob + a.download
// ---------------------------------------------------------------------------

export function downloadBlob(filename: string, blob: Blob) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}

export function downloadDataUrl(filename: string, dataUrl: string) {
  const a = document.createElement("a");
  a.href = dataUrl;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
}

export function safeFilename(name: string): string {
  return name.trim().replace(/[^\p{L}\p{N}\-_ ]/gu, "").replace(/\s+/g, "-").toLowerCase() || "canvas";
}
