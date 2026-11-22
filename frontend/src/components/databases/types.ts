import type { Id } from "@/lib/types";

export type DbFieldKind =
  | "text"
  | "long_text"
  | "integer"
  | "decimal"
  | "date"
  | "datetime"
  | "checkbox"
  | "select"
  | "multiselect"
  | "url"
  | "email"
  | "phone"
  | "image"
  | "file"
  | "rating"
  | "relation"
  | "formula";

export type DbViewKind = "table" | "kanban" | "gallery" | "calendar" | "list";
export type DbCardinality = "one_to_one" | "one_to_many" | "many_to_many";
export type DbFilterOp =
  | "contains"
  | "not_contains"
  | "eq"
  | "ne"
  | "gt"
  | "gte"
  | "lt"
  | "lte"
  | "is_empty"
  | "not_empty";
export type DbSortDir = "asc" | "desc";

export interface DbSelectOption {
  id: Id;
  label: string;
  color: string;
}

export interface DbFieldConfig {
  options?: DbSelectOption[];
  target_database_id?: Id | null;
  cardinality?: DbCardinality | null;
  expression?: string | null;
  max_rating?: number | null;
  decimals?: number | null;
}

export interface Database {
  id: Id;
  name: string;
  description: string;
  icon: string;
  color: string;
  created_at: string;
  updated_at: string;
}

export interface DatabaseSummary extends Database {
  record_count: number;
  field_count: number;
  view_count: number;
}

export interface DbField {
  id: Id;
  database_id: Id;
  name: string;
  kind: DbFieldKind;
  config: DbFieldConfig;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface RelationChip {
  id: Id;
  title: string;
}

export interface DbRecord {
  id: Id;
  database_id: Id;
  values: Record<Id, unknown>;
  computed: Record<Id, unknown>;
  formula_errors: Record<Id, string>;
  relation_titles: Record<Id, RelationChip[]>;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface DbFilter {
  field_id: Id;
  op: DbFilterOp;
  value?: unknown;
}

export interface DbFilterGroup {
  conjunction: "and" | "or";
  filters: DbFilter[];
}

export interface DbSort {
  field_id: Id;
  dir: DbSortDir;
}

export interface DbViewConfig {
  visible_field_ids: Id[];
  group_field_id?: Id | null;
  date_field_id?: Id | null;
  image_field_id?: Id | null;
  card_field_ids: Id[];
  filters: DbFilterGroup;
  sorts: DbSort[];
}

export interface DbView {
  id: Id;
  database_id: Id;
  name: string;
  kind: DbViewKind;
  config: DbViewConfig;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface DatabaseDetail extends DatabaseSummary {
  fields: DbField[];
  views: DbView[];
}

export interface RecordQuery {
  search?: string | null;
  filters?: DbFilterGroup;
  sorts?: DbSort[];
  highlight_id?: Id | null;
  limit?: number | null;
  offset?: number | null;
}

export interface RecordPage {
  records: DbRecord[];
  total: number;
}

export interface CreateDbField {
  name: string;
  kind: DbFieldKind;
  config?: DbFieldConfig;
  position?: number | null;
}

export interface ImportPreview {
  columns: string[];
  rows: string[][];
  total_rows: number;
  format: string;
}

export interface ImportMapping {
  column: string;
  field_id?: Id | null;
  new_field?: CreateDbField | null;
}

export interface ImportResult {
  imported: number;
  skipped: number;
  errors: { row: number; message: string }[];
  created_fields: DbField[];
}

export interface ExportResult {
  filename: string;
  mime: string;
  content: string;
  row_count: number;
}

export interface DbTemplateField {
  name: string;
  kind: DbFieldKind;
  config: DbFieldConfig;
}

export interface DbTemplate {
  id: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  fields: DbTemplateField[];
  sample_records: Record<string, unknown>[];
  views: { name: string; kind: DbViewKind; config: DbViewConfig }[];
}

export interface AiFieldProposal {
  name: string;
  kind: DbFieldKind;
  config: DbFieldConfig;
  reason: string;
}

export interface AiStructureProposal {
  name: string;
  description: string;
  icon: string;
  color: string;
  fields: AiFieldProposal[];
}

export interface DuplicatePair {
  record_a: Id;
  record_b: Id;
  title_a: string;
  title_b: string;
  score: number;
  reason: string;
}

export interface RecordPreview {
  record_id: Id;
  database_id: Id;
  database_name: string;
  title: string;
  icon: string;
  color: string;
}
