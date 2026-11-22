export type CanvasElementType =
  | "sticky"
  | "text"
  | "rect"
  | "ellipse"
  | "line"
  | "arrow"
  | "image"
  | "link"
  | "card"
  | "task_ref"
  | "project_ref"
  | "note_ref"
  | "database_ref"
  | "recipe_ref"
  | "document";

export interface CanvasElementStyle {
  /** Token del tema ("primary", "card", …) o hex elegido por el usuario. */
  background?: string;
  border?: string;
  text?: string;
  font_size?: number;
  align?: "left" | "center" | "right";
  [key: string]: unknown;
}

export interface CanvasElement {
  id: string;
  type: CanvasElementType;
  x: number;
  y: number;
  w: number;
  h: number;
  rotation?: number | null;
  z: number;
  group_id?: string | null;
  style: CanvasElementStyle;
  content: Record<string, unknown>;
}

export interface CanvasEdge {
  id: string;
  source: string;
  target: string;
  source_handle?: string | null;
  target_handle?: string | null;
  arrow: boolean;
  style: "solid" | "dashed";
  color?: string | null;
  label: string;
}

export interface CanvasViewport {
  x: number;
  y: number;
  zoom: number;
}

export interface CanvasDocument {
  version: number;
  elements: CanvasElement[];
  edges: CanvasEdge[];
  viewport: CanvasViewport;
}

export interface Canvas {
  id: string;
  name: string;
  description: string;
  document: CanvasDocument;
  created_at: string;
  updated_at: string;
}

export interface CanvasSummary {
  id: string;
  name: string;
  description: string;
  element_count: number;
  edge_count: number;
  created_at: string;
  updated_at: string;
}

export interface CanvasTemplateInfo {
  id: string;
  name: string;
  description: string;
  element_count: number;
}

export interface CanvasExport {
  format: "nexus.canvas";
  version: number;
  name: string;
  description: string;
  document: CanvasDocument;
}

export interface AiMindmapProposal {
  elements: { type: string; text: string; color?: string }[];
  edges: { source: number; target: number; label: string }[];
}

export interface AiExpandProposal {
  parent_id: string;
  parent_x: number;
  parent_y: number;
  parent_w: number;
  parent_h: number;
  elements: { type: string; text: string; color?: string }[];
}

export interface AiOrganizeProposal {
  connections: { source: string; target: string; label: string; reason: string }[];
}
