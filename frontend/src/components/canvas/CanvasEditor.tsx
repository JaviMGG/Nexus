import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ConnectionMode,
  MarkerType,
  SelectionMode,
  useNodesState,
  useEdgesState,
  useReactFlow,
  type Connection,
  type EdgeChangeset,
  type NodeChangeset,
  type Viewport,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  ArrowDownToLine,
  ArrowUpToLine,
  CheckSquare,
  Copy,
  ExternalLink,
  Sparkles,
  Trash2,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Task } from "@/lib/types";
import { cn } from "@/lib/utils";
import { CanvasNodeView, type CanvasNode, type CanvasNodeData } from "./CanvasNode";
import { CanvasEdgeView, type CanvasFlowEdge } from "./CanvasEdge";
import { CanvasToolbar, ShortcutsDialog, LinkDialog, AiDescriptionDialog, type Tool } from "./CanvasToolbar";
import { PropertiesPanel } from "./PropertiesPanel";
import { InsertDialog, type RefPick } from "./InsertDialog";
import { MindmapProposalDialog, ExpandProposalDialog, SummaryDialog, OrganizeDialog, useAiMutation } from "./AiDialogs";
import type {
  AiExpandProposal,
  AiMindmapProposal,
  AiOrganizeProposal,
  Canvas,
  CanvasDocument,
  CanvasEdge,
  CanvasElement,
  CanvasElementType,
} from "./types";
import {
  cloneDocument,
  cloneElements,
  defaultContent,
  defaultSize,
  defaultStyle,
  downloadDataUrl,
  elementText,
  getFile,
  newId,
  nextZ,
  resolveColor,
  saveFile,
} from "./utils";

const nodeTypes = { canvas: CanvasNodeView };
const edgeTypes = { canvas: CanvasEdgeView };
const MAX_HISTORY = 100;
const AUTOSAVE_MS = 800;

interface CtxMenu {
  x: number;
  y: number;
  elementId?: string;
  edgeId?: string;
}

function toRfNode(el: CanvasElement, data: CanvasNodeData): CanvasNode {
  return {
    id: el.id,
    type: "canvas",
    position: { x: el.x, y: el.y },
    width: el.w,
    height: el.h,
    zIndex: el.z,
    style: { width: el.w, height: el.h },
    data,
    dragHandle: undefined,
  };
}

function toRfEdge(e: CanvasEdge, editing: boolean, handlers: { onStartEdit: (id: string) => void; onCommitLabel: (id: string, l: string) => void }): CanvasFlowEdge {
  return {
    id: e.id,
    type: "canvas",
    source: e.source,
    target: e.target,
    sourceHandle: e.source_handle ?? undefined,
    targetHandle: e.target_handle ?? undefined,
    markerEnd: e.arrow ? { type: MarkerType.ArrowClosed, width: 16, height: 16, color: resolveColor(e.color, "var(--muted-foreground)") } : undefined,
    data: { edge: e, editing, ...handlers },
  };
}

function CanvasEditorInner({ canvas }: { canvas: Canvas }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const rf = useReactFlow();
  const wrapperRef = useRef<HTMLDivElement | null>(null);

  const [doc, setDocState] = useState<CanvasDocument>(() => cloneDocument(canvas.document));
  const docRef = useRef(doc);
  const [saveState, setSaveState] = useState<"saved" | "saving" | "error">("saved");
  const [tool, setTool] = useState<Tool>("select");
  const [snap, setSnap] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingEdgeId, setEditingEdgeId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [selectedEdgeIds, setSelectedEdgeIds] = useState<string[]>([]);
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [insertOpen, setInsertOpen] = useState(false);
  const [linkOpen, setLinkOpen] = useState(false);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [mindmapAskOpen, setMindmapAskOpen] = useState(false);
  const [mindmapProposal, setMindmapProposal] = useState<AiMindmapProposal | null>(null);
  const [expandProposal, setExpandProposal] = useState<AiExpandProposal | null>(null);
  const [organizeProposal, setOrganizeProposal] = useState<AiOrganizeProposal | null>(null);
  const selectionEchoRef = useRef(false);
  const clipboardRef = useRef<{ elements: CanvasElement[]; edges: CanvasEdge[] } | null>(null);
  const historyRef = useRef<{ past: CanvasDocument[]; future: CanvasDocument[] }>({ past: [], future: [] });
  const [historyTick, setHistoryTick] = useState(0);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const viewportRef = useRef<Viewport>(canvas.document.viewport);
  const initialViewport = useRef(canvas.document.viewport);
  // Viewport por defecto (0,0,z1) = recién creado: encuadrar el contenido al abrir.
  const needsInitialFit = useRef(
    canvas.document.viewport.x === 0 &&
      canvas.document.viewport.y === 0 &&
      canvas.document.viewport.zoom === 1 &&
      canvas.document.elements.length > 0,
  );
  const onInit = useCallback(() => {
    if (needsInitialFit.current) {
      needsInitialFit.current = false;
      window.setTimeout(() => {
        void rf.fitView({ padding: 0.2, maxZoom: 1 });
      }, 50);
    }
  }, [rf]);

  // ------------------------------------------------------------------ persistencia
  const save = useMutation({
    mutationFn: (document: CanvasDocument) => invoke<Canvas>("canvas.save_document", { id: canvas.id, document }),
    onSuccess: (saved) => {
      qc.setQueryData<Canvas>(["canvas", canvas.id], saved);
      setSaveState("saved");
    },
    onError: (e) => {
      setSaveState("error");
      toast.error(e instanceof ApiError ? e.message : "No se pudo guardar el canvas");
    },
  });
  const saveRef = useRef(save.mutate);
  saveRef.current = save.mutate;

  const saveNow = useCallback(() => {
    if (saveTimer.current) {
      clearTimeout(saveTimer.current);
      saveTimer.current = null;
    }
    setSaveState("saving");
    saveRef.current({ ...docRef.current, viewport: { ...viewportRef.current } });
  }, []);

  const scheduleSave = useCallback((next: CanvasDocument) => {
    setSaveState("saving");
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      saveTimer.current = null;
      const withViewport = { ...next, viewport: { ...viewportRef.current } };
      saveRef.current(withViewport);
    }, AUTOSAVE_MS);
  }, []);

  const setDoc = useCallback(
    (updater: CanvasDocument | ((d: CanvasDocument) => CanvasDocument), recordHistory = true) => {
      const prev = docRef.current;
      const next = typeof updater === "function" ? updater(prev) : updater;
      if (next === prev) return;
      if (recordHistory) {
        const h = historyRef.current;
        h.past.push(cloneDocument(prev));
        if (h.past.length > MAX_HISTORY) h.past.shift();
        h.future = [];
        setHistoryTick((t) => t + 1);
      }
      docRef.current = next;
      setDocState(next);
      scheduleSave(next);
    },
    [scheduleSave],
  );

  const undo = useCallback(() => {
    const h = historyRef.current;
    const prev = h.past.pop();
    if (!prev) return;
    h.future.push(cloneDocument(docRef.current));
    setHistoryTick((t) => t + 1);
    docRef.current = prev;
    setDocState(prev);
    scheduleSave(prev);
  }, [scheduleSave]);

  const redo = useCallback(() => {
    const h = historyRef.current;
    const next = h.future.pop();
    if (!next) return;
    h.past.push(cloneDocument(docRef.current));
    setHistoryTick((t) => t + 1);
    docRef.current = next;
    setDocState(next);
    scheduleSave(next);
  }, [scheduleSave]);

  // Guardar al desmontar si hay cambios pendientes.
  useEffect(() => {
    return () => {
      if (saveTimer.current) {
        clearTimeout(saveTimer.current);
        saveRef.current({ ...docRef.current, viewport: { ...viewportRef.current } });
      }
    };
  }, []);

  // ------------------------------------------------------------------ edición de elementos
  const patchElements = useCallback(
    (ids: string[], patch: Partial<CanvasElement>) => {
      const set = new Set(ids);
      setDoc((d) => ({ ...d, elements: d.elements.map((e) => (set.has(e.id) ? { ...e, ...patch } : e)) }));
    },
    [setDoc],
  );

  const patchStyle = useCallback(
    (ids: string[], patch: Record<string, unknown>) => {
      const set = new Set(ids);
      setDoc((d) => ({
        ...d,
        elements: d.elements.map((e) => {
          if (!set.has(e.id)) return e;
          const style = { ...e.style };
          for (const [k, v] of Object.entries(patch)) {
            if (v === null) delete style[k];
            else style[k] = v;
          }
          return { ...e, style };
        }),
      }));
    },
    [setDoc],
  );

  const patchContent = useCallback(
    (ids: string[], patch: Record<string, unknown>) => {
      const set = new Set(ids);
      setDoc((d) => ({ ...d, elements: d.elements.map((e) => (set.has(e.id) ? { ...e, content: { ...e.content, ...patch } } : e)) }));
    },
    [setDoc],
  );

  const commitText = useCallback(
    (id: string, value: string) => {
      setEditingId((cur) => (cur === id ? null : cur));
      const el = docRef.current.elements.find((e) => e.id === id);
      if (!el) return;
      if (typeof el.content.text === "string" && el.content.text === value) return;
      patchContent([id], { text: value });
    },
    [patchContent],
  );

  const commitCard = useCallback(
    (id: string, title: string, body: string) => {
      setEditingId((cur) => (cur === id ? null : cur));
      patchContent([id], { title, body });
    },
    [patchContent],
  );

  const startEdit = useCallback((id: string) => {
    setEditingEdgeId(null);
    setEditingId(id);
  }, []);

  const addElement = useCallback(
    (type: CanvasElementType, position: { x: number; y: number }, content?: Record<string, unknown>, style?: Record<string, unknown>, size?: { w: number; h: number }) => {
      const { w, h } = size ?? defaultSize(type);
      const el: CanvasElement = {
        id: newId(),
        type,
        x: position.x - w / 2,
        y: position.y - h / 2,
        w,
        h,
        rotation: null,
        z: nextZ(docRef.current),
        group_id: null,
        style: { ...defaultStyle(type), ...(style ?? {}) },
        content: content ?? defaultContent(type),
      };
      setDoc((d) => ({ ...d, elements: [...d.elements, el] }));
      return el;
    },
    [setDoc],
  );

  const centerPosition = useCallback(() => {
    const rect = wrapperRef.current?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    return rf.screenToFlowPosition({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
  }, [rf]);

  // ------------------------------------------------------------------ selección / grupos
  const expandToGroups = useCallback((ids: string[]): string[] => {
    const d = docRef.current;
    const groups = new Set(d.elements.filter((e) => ids.includes(e.id) && e.group_id).map((e) => e.group_id!));
    if (groups.size === 0) return ids;
    const all = d.elements.filter((e) => ids.includes(e.id) || (e.group_id && groups.has(e.group_id))).map((e) => e.id);
    return Array.from(new Set(all));
  }, []);

  const deleteSelection = useCallback(
    (onlyIds?: string[]) => {
      const ids = new Set(expandToGroups(onlyIds ?? selectedIds));
      const edgeIds = new Set(onlyIds ? [] : selectedEdgeIds);
      if (ids.size === 0 && edgeIds.size === 0) return;
      setDoc((d) => ({
        ...d,
        elements: d.elements.filter((e) => !ids.has(e.id)),
        edges: d.edges.filter((e) => !edgeIds.has(e.id) && !ids.has(e.source) && !ids.has(e.target)),
      }));
      setSelectedIds((cur) => cur.filter((id) => !ids.has(id)));
      setSelectedEdgeIds([]);
      setEditingId(null);
      setEditingEdgeId(null);
    },
    [selectedIds, selectedEdgeIds, expandToGroups, setDoc],
  );

  const duplicateSelection = useCallback(
    (onlyIds?: string[], offset = { x: 32, y: 32 }) => {
      const ids = new Set(expandToGroups(onlyIds ?? selectedIds));
      if (ids.size === 0) return;
      const d = docRef.current;
      const els = d.elements.filter((e) => ids.has(e.id));
      const { elements, edges } = cloneElements(els, d.edges, offset, nextZ(d));
      setDoc((prev) => ({ ...prev, elements: [...prev.elements, ...elements], edges: [...prev.edges, ...edges] }));
      setSelectedIds(elements.map((e) => e.id));
      setSelectedEdgeIds([]);
    },
    [selectedIds, expandToGroups, setDoc],
  );

  const copySelection = useCallback(() => {
    const ids = new Set(expandToGroups(selectedIds));
    if (ids.size === 0) return;
    const d = docRef.current;
    clipboardRef.current = {
      elements: d.elements.filter((e) => ids.has(e.id)).map((e) => JSON.parse(JSON.stringify(e)) as CanvasElement),
      edges: d.edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ ...e })),
    };
    toast.success(`${ids.size} elemento(s) copiado(s)`);
  }, [selectedIds, expandToGroups]);

  const pasteClipboard = useCallback(() => {
    const clip = clipboardRef.current;
    if (!clip || clip.elements.length === 0) return;
    const { elements, edges } = cloneElements(clip.elements, clip.edges, { x: 48, y: 48 }, nextZ(docRef.current));
    setDoc((d) => ({ ...d, elements: [...d.elements, ...elements], edges: [...d.edges, ...edges] }));
    setSelectedIds(elements.map((e) => e.id));
    setSelectedEdgeIds([]);
    clipboardRef.current = { elements, edges };
  }, [setDoc]);

  const groupSelection = useCallback(() => {
    if (selectedIds.length < 2) return;
    const gid = newId();
    const ids = expandToGroups(selectedIds);
    patchElements(ids, { group_id: gid });
    setSelectedIds(ids);
    toast.success("Elementos agrupados");
  }, [selectedIds, patchElements, expandToGroups]);

  const ungroupSelection = useCallback(() => {
    patchElements(expandToGroups(selectedIds), { group_id: null });
  }, [selectedIds, patchElements, expandToGroups]);

  const bringFront = useCallback(
    (ids: string[]) => {
      const z = nextZ(docRef.current);
      const set = new Set(ids);
      setDoc((d) => ({ ...d, elements: d.elements.map((e) => (set.has(e.id) ? { ...e, z: z + ids.indexOf(e.id) } : e)) }));
    },
    [setDoc],
  );

  const sendBack = useCallback(
    (ids: string[]) => {
      const minZ = docRef.current.elements.reduce((m, e) => Math.min(m, e.z), 0);
      const set = new Set(ids);
      setDoc((d) => ({ ...d, elements: d.elements.map((e) => (set.has(e.id) ? { ...e, z: minZ - 1 - ids.indexOf(e.id) } : e)) }));
    },
    [setDoc],
  );

  const alignSelection = useCallback(
    (mode: "left" | "center-x" | "right" | "top" | "center-y" | "bottom") => {
      const ids = new Set(expandToGroups(selectedIds));
      const els = docRef.current.elements.filter((e) => ids.has(e.id));
      if (els.length < 2) return;
      const minX = Math.min(...els.map((e) => e.x));
      const maxX = Math.max(...els.map((e) => e.x + e.w));
      const minY = Math.min(...els.map((e) => e.y));
      const maxY = Math.max(...els.map((e) => e.y + e.h));
      setDoc((d) => ({
        ...d,
        elements: d.elements.map((e) => {
          if (!ids.has(e.id)) return e;
          switch (mode) {
            case "left": return { ...e, x: minX };
            case "center-x": return { ...e, x: (minX + maxX) / 2 - e.w / 2 };
            case "right": return { ...e, x: maxX - e.w };
            case "top": return { ...e, y: minY };
            case "center-y": return { ...e, y: (minY + maxY) / 2 - e.h / 2 };
            case "bottom": return { ...e, y: maxY - e.h };
          }
        }),
      }));
    },
    [selectedIds, expandToGroups, setDoc],
  );

  const distributeSelection = useCallback(
    (axis: "h" | "v") => {
      const ids = new Set(expandToGroups(selectedIds));
      const els = docRef.current.elements.filter((e) => ids.has(e.id));
      if (els.length < 3) {
        toast.info("Selecciona al menos 3 elementos para distribuir");
        return;
      }
      const key = axis === "h" ? "x" : "y";
      const dim = axis === "h" ? "w" : "h";
      const sorted = [...els].sort((a, b) => a[key] - b[key]);
      const min = sorted[0][key];
      const max = sorted[sorted.length - 1][key] + sorted[sorted.length - 1][dim];
      const totalDim = sorted.reduce((s, e) => s + e[dim], 0);
      const gap = (max - min - totalDim) / (sorted.length - 1);
      let cursor = min;
      const positions = new Map<string, number>();
      for (const e of sorted) {
        positions.set(e.id, cursor);
        cursor += e[dim] + gap;
      }
      setDoc((d) => ({ ...d, elements: d.elements.map((e) => (positions.has(e.id) ? { ...e, [key]: positions.get(e.id)! } : e)) }));
    },
    [selectedIds, expandToGroups, setDoc],
  );

  // ------------------------------------------------------------------ conexiones
  const onConnect = useCallback(
    (conn: Connection) => {
      if (!conn.source || !conn.target || conn.source === conn.target) return;
      const edge: CanvasEdge = {
        id: newId(),
        source: conn.source,
        target: conn.target,
        source_handle: conn.sourceHandle,
        target_handle: conn.targetHandle,
        arrow: tool !== "line",
        style: "solid",
        color: null,
        label: "",
      };
      setDoc((d) => ({ ...d, edges: [...d.edges, edge] }));
    },
    [setDoc, tool],
  );

  const commitEdgeLabel = useCallback(
    (id: string, label: string) => {
      setEditingEdgeId((cur) => (cur === id ? null : cur));
      setDoc((d) => ({ ...d, edges: d.edges.map((e) => (e.id === id ? { ...e, label } : e)) }));
    },
    [setDoc],
  );

  const startEdgeEdit = useCallback((id: string) => {
    setEditingId(null);
    setEditingEdgeId(id);
  }, []);

  const deleteEdge = useCallback(
    (id: string) => {
      setDoc((d) => ({ ...d, edges: d.edges.filter((e) => e.id !== id) }));
    },
    [setDoc],
  );

  // ------------------------------------------------------------------ integración NEXUS
  const toggleTask = useCallback(
    async (elementId: string, taskId: string, done: boolean) => {
      try {
        const t = await invoke<Task>("tasks.update", { id: taskId, data: { status: done ? "todo" : "done" } });
        patchContent([elementId], { status: t.status, title: t.title });
        qc.invalidateQueries({ queryKey: ["tasks"] });
        toast.success(done ? "Tarea marcada como pendiente" : "Tarea completada");
      } catch (e) {
        toast.error(e instanceof ApiError ? e.message : "No se pudo actualizar la tarea");
      }
    },
    [patchContent, qc],
  );

  const convertToTask = useCallback(
    async (elementId: string) => {
      const el = docRef.current.elements.find((e) => e.id === elementId);
      if (!el) return;
      const title = elementText(el).trim();
      if (!title) {
        toast.error("El elemento no tiene texto para convertir en tarea");
        return;
      }
      try {
        const task = await invoke<Task>("tasks.create", { data: { title: title.slice(0, 300), description: "" } });
        qc.invalidateQueries({ queryKey: ["tasks"] });
        setDoc((d) => ({
          ...d,
          elements: d.elements.map((e) =>
            e.id === elementId
              ? { ...e, type: "task_ref", w: Math.max(e.w, 240), h: Math.max(e.h, 110), style: {}, content: { ref_id: task.id, title: task.title, status: task.status } }
              : e,
          ),
        }));
        toast.success(`Tarea «${task.title}» creada y vinculada`);
      } catch (e) {
        toast.error(e instanceof ApiError ? e.message : "No se pudo crear la tarea");
      }
    },
    [setDoc, qc],
  );

  const insertRef = useCallback(
    (pick: RefPick) => {
      addElement(pick.type, centerPosition(), { ref_id: pick.refId, title: pick.title, ...pick.extra });
      toast.success(`${pick.title} insertado`);
    },
    [addElement, centerPosition],
  );

  const attachFiles = useCallback(
    async (files: FileList | File[], forceImage: boolean) => {
      const pos = centerPosition();
      let i = 0;
      for (const file of Array.from(files)) {
        try {
          const meta = await saveFile(file);
          const isImage = forceImage || meta.mime.startsWith("image/");
          addElement(
            isImage ? "image" : "document",
            { x: pos.x + i * 40, y: pos.y + i * 40 },
            { file_id: meta.id, name: meta.name, mime: meta.mime },
          );
          i += 1;
        } catch (e) {
          toast.error(e instanceof ApiError ? e.message : `No se pudo adjuntar ${file.name}`);
        }
      }
    },
    [addElement, centerPosition],
  );

  const openElement = useCallback(
    async (el: CanvasElement) => {
      switch (el.type) {
        case "task_ref":
          navigate("/tasks");
          break;
        case "project_ref":
          navigate(`/projects/${String(el.content.ref_id ?? "")}`);
          break;
        case "note_ref":
          navigate("/knowledge");
          break;
        case "database_ref":
          navigate(`/databases/${String(el.content.ref_id ?? "")}`);
          break;
        case "recipe_ref":
          navigate("/meals");
          break;
        case "link": {
          const url = typeof el.content.url === "string" ? el.content.url : "";
          if (/^https?:\/\//.test(url)) window.open(url, "_blank", "noopener");
          break;
        }
        case "document":
        case "image": {
          const fileId = typeof el.content.file_id === "string" ? el.content.file_id : "";
          if (!fileId) return;
          try {
            const { meta, dataUrl } = await getFile(fileId);
            if (el.type === "image") window.open(dataUrl, "_blank", "noopener");
            else downloadDataUrl(meta.name, dataUrl);
          } catch {
            toast.error("No se pudo abrir el archivo");
          }
          break;
        }
        default:
          startEdit(el.id);
      }
    },
    [navigate, startEdit],
  );

  // ------------------------------------------------------------------ plantillas / IA
  const insertTemplateDoc = useCallback(
    (templateDoc: CanvasDocument) => {
      const pos = centerPosition();
      const els = templateDoc.elements;
      if (els.length === 0) return;
      const minX = Math.min(...els.map((e) => e.x));
      const minY = Math.min(...els.map((e) => e.y));
      const maxX = Math.max(...els.map((e) => e.x + e.w));
      const maxY = Math.max(...els.map((e) => e.y + e.h));
      const offset = { x: pos.x - (minX + maxX) / 2, y: pos.y - (minY + maxY) / 2 };
      const { elements, edges } = cloneElements(els, templateDoc.edges, offset, nextZ(docRef.current));
      setDoc((d) => ({ ...d, elements: [...d.elements, ...elements], edges: [...d.edges, ...edges] }));
      toast.success("Plantilla insertada");
    },
    [centerPosition, setDoc],
  );

  const aiMindmap = useAiMutation<{ description: string }, AiMindmapProposal>("canvas.ai_mindmap");
  const aiExpand = useAiMutation<{ canvas_id: string; element_id: string }, AiExpandProposal>("canvas.ai_expand");
  const aiOrganize = useAiMutation<{ canvas_id: string }, AiOrganizeProposal>("canvas.ai_organize");

  const addMindmapProposal = useCallback(
    (p: AiMindmapProposal) => {
      const pos = centerPosition();
      const n = p.elements.length;
      const created: CanvasElement[] = p.elements.map((item, i) => {
        const angle = (2 * Math.PI * (i - 1)) / Math.max(n - 1, 1);
        const radius = i === 0 ? 0 : 320;
        const type = (["sticky", "text", "rect", "ellipse", "card"].includes(item.type) ? item.type : "sticky") as CanvasElementType;
        const { w, h } = i === 0 ? { w: 240, h: 120 } : defaultSize(type);
        return {
          id: newId(),
          type: i === 0 ? "ellipse" : type,
          x: pos.x + Math.cos(angle) * radius - w / 2,
          y: pos.y + Math.sin(angle) * radius - h / 2,
          w,
          h,
          rotation: null,
          z: nextZ(docRef.current) + i,
          group_id: null,
          style: i === 0 ? { background: "primary" } : { ...defaultStyle(type), ...(item.color ? { background: item.color, text: "#1f2937" } : {}) },
          content: { text: item.text },
        };
      });
      const edges: CanvasEdge[] = p.edges
        .filter((e) => e.source >= 0 && e.target >= 0 && e.source < created.length && e.target < created.length)
        .map((e) => ({
          id: newId(),
          source: created[e.source].id,
          target: created[e.target].id,
          source_handle: null,
          target_handle: null,
          arrow: false,
          style: "solid",
          color: null,
          label: e.label ?? "",
        }));
      setDoc((d) => ({ ...d, elements: [...d.elements, ...created], edges: [...d.edges, ...edges] }));
      toast.success(`${created.length} nodos añadidos al canvas`);
    },
    [centerPosition, setDoc],
  );

  const addExpandProposal = useCallback(
    (p: AiExpandProposal) => {
      const parent = docRef.current.elements.find((e) => e.id === p.parent_id);
      const baseX = parent ? parent.x + parent.w : p.parent_x + p.parent_w;
      const baseY = parent ? parent.y : p.parent_y;
      const created: CanvasElement[] = p.elements.map((item, i) => {
        const { w, h } = defaultSize("sticky");
        return {
          id: newId(),
          type: "sticky",
          x: baseX + 120,
          y: baseY + i * (h + 30) - ((p.elements.length - 1) * (h + 30)) / 2,
          w,
          h,
          rotation: null,
          z: nextZ(docRef.current) + i,
          group_id: null,
          style: { background: item.color ?? "#dbeafe", text: "#1f2937" },
          content: { text: item.text },
        };
      });
      const edges: CanvasEdge[] = parent
        ? created.map((c) => ({ id: newId(), source: parent.id, target: c.id, source_handle: "right", target_handle: "left", arrow: false, style: "solid", color: null, label: "" }))
        : [];
      setDoc((d) => ({ ...d, elements: [...d.elements, ...created], edges: [...d.edges, ...edges] }));
      toast.success(`${created.length} ramas añadidas`);
    },
    [setDoc],
  );

  const acceptConnections = useCallback(
    (connections: AiOrganizeProposal["connections"]) => {
      const edges: CanvasEdge[] = connections.map((c) => ({
        id: newId(),
        source: c.source,
        target: c.target,
        source_handle: null,
        target_handle: null,
        arrow: true,
        style: "solid",
        color: null,
        label: c.label ?? "",
      }));
      setDoc((d) => ({ ...d, edges: [...d.edges, ...edges] }));
      toast.success(`${edges.length} conexión(es) añadida(s)`);
    },
    [setDoc],
  );

  // ------------------------------------------------------------------ atajos de teclado
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && key === "s") { e.preventDefault(); saveNow(); return; }
      const target = e.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) return;
      if (mod && key === "z" && !e.shiftKey) { e.preventDefault(); undo(); }
      else if ((mod && key === "y") || (mod && e.shiftKey && key === "z")) { e.preventDefault(); redo(); }
      else if (mod && key === "c") { e.preventDefault(); copySelection(); }
      else if (mod && key === "v") { e.preventDefault(); pasteClipboard(); }
      else if (mod && key === "d") { e.preventDefault(); duplicateSelection(); }
      else if (mod && key === "g" && !e.shiftKey) { e.preventDefault(); groupSelection(); }
      else if (mod && key === "g" && e.shiftKey) { e.preventDefault(); ungroupSelection(); }
      else if (mod && key === "a") {
        e.preventDefault();
        setSelectedIds(docRef.current.elements.map((el) => el.id));
      } else if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); deleteSelection(); }
      else if (e.key === "Escape") {
        setCtxMenu(null);
        setEditingId(null);
        setEditingEdgeId(null);
        setSelectedIds([]);
        setSelectedEdgeIds([]);
        setTool("select");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveNow, undo, redo, copySelection, pasteClipboard, duplicateSelection, groupSelection, ungroupSelection, deleteSelection]);

  // ------------------------------------------------------------------ sincronización doc → React Flow
  const nodeDataHandlers = useMemo(
    () => ({
      editing: false,
      onStartEdit: startEdit,
      onCommitText: commitText,
      onCommitCard: commitCard,
      onToggleTask: (id: string, taskId: string, done: boolean) => void toggleTask(id, taskId, done),
    }),
    [startEdit, commitText, commitCard, toggleTask],
  );

  const edgeHandlers = useMemo(
    () => ({ onStartEdit: startEdgeEdit, onCommitLabel: commitEdgeLabel }),
    [startEdgeEdit, commitEdgeLabel],
  );

  const initialNodes = useMemo(
    () => doc.elements.map((el) => toRfNode(el, { ...nodeDataHandlers, editing: false, element: el })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  const initialEdges = useMemo(
    () => doc.edges.map((e) => toRfEdge(e, false, edgeHandlers)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  const [nodes, setNodes, onNodesChange] = useNodesState<CanvasNode>(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<CanvasFlowEdge>(initialEdges);
  const [canvasName, setCanvasName] = useState(canvas.name);

  useEffect(() => {
    setNodes((prev) => {
      const prevById = new Map(prev.map((n) => [n.id, n]));
      return doc.elements.map((el) => {
        const old = prevById.get(el.id);
        const base = toRfNode(el, { ...nodeDataHandlers, editing: editingId === el.id, element: el });
        if (old) {
          return {
            ...base,
            selected: old.selected,
            dragging: old.dragging,
            position: old.dragging ? old.position : { x: el.x, y: el.y },
            width: el.w,
            height: el.h,
            measured: old.measured,
          };
        }
        return base;
      });
    });
    setEdges((prev) => {
      const prevById = new Map(prev.map((e) => [e.id, e]));
      return doc.edges.map((e) => {
        const old = prevById.get(e.id);
        const next = toRfEdge(e, editingEdgeId === e.id, edgeHandlers);
        return old ? { ...next, selected: old.selected } : next;
      });
    });
  }, [doc, editingId, editingEdgeId, nodeDataHandlers, edgeHandlers, setNodes, setEdges]);

  // La selección lógica incluye el grupo completo: reflejarla en React Flow.
  // onSelectionChange solo actualiza si el conjunto difiere, así el eco es inocuo.
  useEffect(() => {
    if (selectionEchoRef.current) {
      selectionEchoRef.current = false;
      return;
    }
    const selSet = new Set(expandToGroups(selectedIds));
    const edgeSelSet = new Set(selectedEdgeIds);
    setNodes((prev) =>
      prev.some((n) => !!n.selected !== selSet.has(n.id))
        ? prev.map((n) => (n.selected === selSet.has(n.id) ? n : { ...n, selected: selSet.has(n.id) }))
        : prev,
    );
    setEdges((prev) =>
      prev.some((e) => !!e.selected !== edgeSelSet.has(e.id))
        ? prev.map((e) => (e.selected === edgeSelSet.has(e.id) ? e : { ...e, selected: edgeSelSet.has(e.id) }))
        : prev,
    );
  }, [selectedIds, selectedEdgeIds, expandToGroups, setNodes, setEdges]);

  // Cambios de React Flow → documento (fin de arrastre / redimensionado).
  const handleNodesChange = useCallback(
    (changes: NodeChangeset<CanvasNode>) => {
      onNodesChange(changes);
      const moved = changes.getByType("position").filter((c) => c.dragging === false && c.position);
      const resized = changes.getByType("dimensions").filter((c) => c.resizing === false && c.dimensions);
      if (moved.length === 0 && resized.length === 0) return;
      const posMap = new Map(moved.map((c) => [c.id, c.position!]));
      const dimMap = new Map(resized.map((c) => [c.id, c.dimensions!]));
      setDoc((d) => ({
        ...d,
        elements: d.elements.map((el) => {
          const p = posMap.get(el.id);
          const dim = dimMap.get(el.id);
          if (!p && !dim) return el;
          return {
            ...el,
            x: p ? p.x : el.x,
            y: p ? p.y : el.y,
            w: dim ? dim.width : el.w,
            h: dim ? dim.height : el.h,
          };
        }),
      }));
    },
    [onNodesChange, setDoc],
  );

  const handleEdgesChange = useCallback(
    (changes: EdgeChangeset<CanvasFlowEdge>) => {
      onEdgesChange(changes);
      const removed = changes.getByType("remove");
      if (removed.length > 0) {
        const ids = new Set(removed.map((c) => c.id));
        setDoc((d) => ({ ...d, edges: d.edges.filter((e) => !ids.has(e.id)) }));
      }
    },
    [onEdgesChange, setDoc],
  );

  const onSelectionChange = useCallback(
    ({ nodes: selNodes, edges: selEdges }: { nodes: CanvasNode[]; edges: CanvasFlowEdge[] }) => {
      const ids = expandToGroups(Array.from(new Set(selNodes.map((n) => n.id))).sort());
      const edgeIds = Array.from(new Set(selEdges.map((e) => e.id))).sort();
      let changed = false;
      setSelectedIds((cur) => {
        const c = [...cur].sort();
        if (c.length === ids.length && c.every((id, i) => id === ids[i])) return cur;
        changed = true;
        return ids;
      });
      setSelectedEdgeIds((cur) => {
        const c = [...cur].sort();
        if (c.length === edgeIds.length && c.every((id, i) => id === edgeIds[i])) return cur;
        changed = true;
        return edgeIds;
      });
      if (changed) selectionEchoRef.current = true;
    },
    [expandToGroups],
  );

  const onMoveEnd = useCallback((_: MouseEvent | TouchEvent | null, viewport: Viewport) => {
    viewportRef.current = viewport;
    scheduleSave(docRef.current);
  }, [scheduleSave]);

  const placeAt = useCallback(
    (clientX: number, clientY: number) => {
      if (tool === "select" || tool === "hand") return;
      if (!Number.isFinite(clientX) || !Number.isFinite(clientY)) return;
      const pos = rf.screenToFlowPosition({ x: clientX, y: clientY });
      const el = addElement(tool, pos);
      setTool("select");
      if (["sticky", "text", "rect", "ellipse"].includes(el.type)) setEditingId(el.id);
    },
    [tool, rf, addElement],
  );

  const onPaneClick = useCallback(
    (e: React.MouseEvent) => {
      setCtxMenu(null);
      setEditingId(null);
      setEditingEdgeId(null);
      placeAt(e.clientX, e.clientY);
    },
    [placeAt],
  );

  // Colocar al soltar tras arrastrar solo si hay herramienta activa (no en selección).
  const onSelectionEnd = useCallback(
    (e: React.MouseEvent) => {
      if (tool === "select" || tool === "hand") return;
      placeAt(e.clientX, e.clientY);
    },
    [tool, placeAt],
  );

  const onNodeContextMenu = useCallback((e: React.MouseEvent, node: CanvasNode) => {
    e.preventDefault();
    const rect = wrapperRef.current?.getBoundingClientRect();
    setCtxMenu({ x: e.clientX - (rect?.left ?? 0), y: e.clientY - (rect?.top ?? 0), elementId: node.id });
  }, []);

  const onEdgeContextMenu = useCallback((e: React.MouseEvent, edge: CanvasFlowEdge) => {
    e.preventDefault();
    const rect = wrapperRef.current?.getBoundingClientRect();
    setCtxMenu({ x: e.clientX - (rect?.left ?? 0), y: e.clientY - (rect?.top ?? 0), edgeId: edge.id });
  }, []);

  const onNodeDoubleClick = useCallback(
    (e: React.MouseEvent, node: CanvasNode) => {
      // El editor inline gestiona su propio doble click.
      const target = e.target as HTMLElement;
      if (target.closest("textarea, input, button")) return;
      const el = docRef.current.elements.find((x) => x.id === node.id);
      if (!el) return;
      if (editingId === el.id) return;
      void openElement(el);
    },
    [openElement, editingId],
  );

  const onEdgeDoubleClick = useCallback((_: React.MouseEvent, edge: CanvasFlowEdge) => {
    startEdgeEdit(edge.id);
  }, [startEdgeEdit]);

  const selectedElements = useMemo(
    () => doc.elements.filter((e) => selectedIds.includes(e.id)),
    [doc, selectedIds],
  );

  const attachInputRef = useRef<HTMLInputElement | null>(null);
  const attachAccept = useRef<"image" | "any">("any");
  void historyTick;

  const ctxElement = ctxMenu?.elementId ? doc.elements.find((e) => e.id === ctxMenu.elementId) : null;
  const canConvert = ctxElement && ["sticky", "text", "card"].includes(ctxElement.type);

  return (
    <div ref={wrapperRef} className="relative h-full w-full overflow-hidden bg-background">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={onConnect}
        connectionMode={ConnectionMode.Loose}
        onSelectionChange={onSelectionChange}
        onSelectionEnd={onSelectionEnd}
        onMoveEnd={onMoveEnd}
        onPaneClick={onPaneClick}
        onNodeContextMenu={onNodeContextMenu}
        onEdgeContextMenu={onEdgeContextMenu}
        onNodeDoubleClick={onNodeDoubleClick}
        onEdgeDoubleClick={onEdgeDoubleClick}
        defaultViewport={initialViewport.current}
        onInit={onInit}
        minZoom={0.1}
        maxZoom={2.5}
        snapToGrid={snap}
        snapGrid={[20, 20]}
        selectionOnDrag={tool !== "hand"}
        selectionMode={SelectionMode.Partial}
        panOnDrag={tool === "hand" ? true : [1, 2]}
        nodesDraggable={tool !== "hand"}
        selectionKeyCode="Shift"
        multiSelectionKeyCode={["Shift", "Control", "Meta"]}
        deleteKeyCode={null}
        zoomOnDoubleClick={false}
        zIndexMode="manual"
        elevateNodesOnSelect={false}
        elevateEdgesOnSelect={false}
        proOptions={{ hideAttribution: true }}
        className={cn(
          tool === "hand" && "cursor-grab active:cursor-grabbing",
          tool !== "select" && tool !== "hand" && "cursor-crosshair",
        )}
      >
        <Background variant={BackgroundVariant.Dots} gap={20} size={1} color="var(--border)" />
        <Controls position="bottom-left" showInteractive={false} className="!shadow-lg" />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          className="!bg-card"
          nodeColor={(n) => {
            const el = (n.data as CanvasNodeData | undefined)?.element;
            const bg = el?.style.background;
            return typeof bg === "string" ? resolveColor(bg, "var(--muted-foreground)") : "var(--muted-foreground)";
          }}
          maskColor="color-mix(in srgb, var(--background) 70%, transparent)"
        />
      </ReactFlow>

      <div className="pointer-events-none absolute left-1/2 top-3 z-20 -translate-x-1/2">
        <CanvasToolbar
          canvas={{ ...canvas, name: canvasName }}
          tool={tool}
          onToolChange={setTool}
          snap={snap}
          onToggleSnap={() => setSnap((s) => !s)}
          saveState={saveState}
          canUndo={historyRef.current.past.length > 0}
          canRedo={historyRef.current.future.length > 0}
          onUndo={undo}
          onRedo={redo}
          onSave={saveNow}
          selectionCount={selectedIds.length}
          onAlign={alignSelection}
          onDistribute={distributeSelection}
          onGroup={groupSelection}
          onUngroup={ungroupSelection}
          onDuplicate={() => duplicateSelection()}
          onDelete={deleteSelection}
          onInsertRef={() => setInsertOpen(true)}
          onAttach={(accept) => {
            attachAccept.current = accept;
            if (attachInputRef.current) {
              attachInputRef.current.accept = accept === "image" ? "image/*" : "*/*";
              attachInputRef.current.click();
            }
          }}
          onAddLink={() => setLinkOpen(true)}
          onShortcuts={() => setShortcutsOpen(true)}
          onAi={(kind) => {
            if (kind === "mindmap") setMindmapAskOpen(true);
            else if (kind === "summarize") setSummaryOpen(true);
            else {
              setOrganizeProposal(null);
              aiOrganize.mutate({ canvas_id: canvas.id }, { onSuccess: setOrganizeProposal });
            }
          }}
          getViewportElement={() => wrapperRef.current?.querySelector<HTMLElement>(".react-flow__viewport") ?? null}
          onRename={(name) => {
            invoke("canvas.rename", { id: canvas.id, name })
              .then(() => {
                setCanvasName(name);
                qc.invalidateQueries({ queryKey: ["canvases"] });
              })
              .catch((e) => toast.error(e instanceof ApiError ? e.message : "No se pudo renombrar"));
          }}
          onApplyTemplate={(id) => {
            invoke<CanvasDocument>("canvas.template_document", { id })
              .then(insertTemplateDoc)
              .catch((e) => toast.error(e instanceof ApiError ? e.message : "No se pudo insertar la plantilla"));
          }}
        />
      </div>

      <input
        ref={attachInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => {
          if (e.target.files?.length) void attachFiles(e.target.files, attachAccept.current === "image");
          e.target.value = "";
        }}
      />

      {selectedElements.length > 0 && !ctxMenu && (
        <PropertiesPanel
          elements={selectedElements}
          onPatch={patchElements}
          onPatchStyle={patchStyle}
          onPatchContent={patchContent}
          onBringFront={bringFront}
          onSendBack={sendBack}
          onClose={() => setSelectedIds([])}
        />
      )}

      {ctxMenu && (
        <>
          <div className="fixed inset-0 z-30" onClick={() => setCtxMenu(null)} onContextMenu={(e) => { e.preventDefault(); setCtxMenu(null); }} />
          <div
            className="absolute z-40 w-52 rounded-lg border border-border bg-popover p-1 shadow-xl animate-fade-in"
            style={{ left: Math.min(ctxMenu.x, (wrapperRef.current?.clientWidth ?? 800) - 220), top: Math.min(ctxMenu.y, (wrapperRef.current?.clientHeight ?? 600) - 260) }}
          >
            {ctxElement && (
              <>
                <CtxItem icon={Copy} label="Duplicar" onClick={() => { duplicateSelection([ctxElement.id]); setCtxMenu(null); }} />
                {canConvert && (
                  <CtxItem icon={CheckSquare} label="Convertir en tarea" onClick={() => { void convertToTask(ctxElement.id); setCtxMenu(null); }} />
                )}
                <CtxItem icon={Sparkles} label="Expandir con IA" onClick={() => {
                  setExpandProposal(null);
                  aiExpand.mutate({ canvas_id: canvas.id, element_id: ctxElement.id }, { onSuccess: setExpandProposal });
                  setCtxMenu(null);
                }} />
                <CtxItem icon={ExternalLink} label="Abrir / editar" onClick={() => { void openElement(ctxElement); setCtxMenu(null); }} />
                <CtxItem icon={ArrowUpToLine} label="Traer al frente" onClick={() => { bringFront([ctxElement.id]); setCtxMenu(null); }} />
                <CtxItem icon={ArrowDownToLine} label="Enviar al fondo" onClick={() => { sendBack([ctxElement.id]); setCtxMenu(null); }} />
                <div className="my-1 h-px bg-border" />
                <CtxItem icon={Trash2} label="Eliminar" danger onClick={() => { deleteSelection([ctxElement.id]); setCtxMenu(null); }} />
              </>
            )}
            {ctxMenu.edgeId && (
              <>
                <CtxItem icon={ExternalLink} label="Editar etiqueta" onClick={() => { startEdgeEdit(ctxMenu.edgeId!); setCtxMenu(null); }} />
                <CtxItem icon={Trash2} label="Eliminar conexión" danger onClick={() => { deleteEdge(ctxMenu.edgeId!); setCtxMenu(null); }} />
              </>
            )}
          </div>
        </>
      )}

      <ShortcutsDialog open={shortcutsOpen} onClose={() => setShortcutsOpen(false)} />
      <InsertDialog open={insertOpen} onClose={() => setInsertOpen(false)} onPick={insertRef} />
      <LinkDialog
        open={linkOpen}
        onClose={() => setLinkOpen(false)}
        onSubmit={(url, title) => addElement("link", centerPosition(), { url, title })}
      />
      <AiDescriptionDialog
        open={mindmapAskOpen}
        pending={aiMindmap.isPending}
        onClose={() => setMindmapAskOpen(false)}
        onSubmit={(description) => {
          aiMindmap.mutate({ description }, {
            onSuccess: (p) => {
              setMindmapAskOpen(false);
              setMindmapProposal(p);
            },
          });
        }}
      />
      <MindmapProposalDialog proposal={mindmapProposal} pending={false} onClose={() => setMindmapProposal(null)} onAdd={addMindmapProposal} />
      <ExpandProposalDialog proposal={expandProposal} pending={aiExpand.isPending && !expandProposal} onClose={() => setExpandProposal(null)} onAdd={addExpandProposal} />
      <SummaryDialog canvasId={canvas.id} open={summaryOpen} onClose={() => setSummaryOpen(false)} />
      <OrganizeDialog
        proposal={organizeProposal}
        pending={aiOrganize.isPending && !organizeProposal}
        titleOf={(id) => {
          const el = doc.elements.find((e) => e.id === id);
          return el ? elementText(el) || "Elemento" : id;
        }}
        onClose={() => setOrganizeProposal(null)}
        onAccept={acceptConnections}
      />
    </div>
  );
}

function CtxItem({ icon: Icon, label, onClick, danger }: { icon: typeof Copy; label: string; onClick: () => void; danger?: boolean }) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors hover:bg-accent cursor-pointer",
        danger && "text-destructive",
      )}
      onClick={onClick}
    >
      <Icon className="h-3.5 w-3.5" /> {label}
    </button>
  );
}

export function CanvasEditor({ canvas }: { canvas: Canvas }) {
  return (
    <ReactFlowProvider>
      <CanvasEditorInner key={canvas.id} canvas={canvas} />
    </ReactFlowProvider>
  );
}
