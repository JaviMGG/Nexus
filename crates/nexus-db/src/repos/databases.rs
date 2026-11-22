//! Repositorio de NEXUS Database: bases, campos, registros y vistas.
//! Los filtros se traducen a SQL seguro con QueryBuilder + json_extract;
//! nunca se interpola entrada del usuario ni de la IA en el SQL.

use std::collections::HashMap;

use nexus_core::{
    now_str, CreateDatabase, CreateDbField, CreateDbView, Database, DatabaseSummary, DbField,
    DbFieldKind, DbFilter, DbFilterOp, DbRecord, DbSort, DbSortDir, DbView, DbViewConfig,
    DbViewKind, Id, NexusError, RecordQuery, Result, UpdateDatabase, UpdateDbField, UpdateDbView,
};
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{Db, DbResultExt};

fn json_string<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|e| NexusError::Internal(e.to_string()))
}

fn parse_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| NexusError::Internal(format!("json corrupto: {e}")))
}

// ---------------------------------------------------------------------------
// Databases
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct DatabaseRow {
    id: String,
    name: String,
    description: String,
    icon: String,
    color: String,
    created_at: String,
    updated_at: String,
}

impl From<DatabaseRow> for Database {
    fn from(r: DatabaseRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            icon: r.icon,
            color: r.color,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn create_database(db: &Db, id: &str, data: &CreateDatabase) -> Result<Database> {
    let now = now_str();
    sqlx::query(
        "INSERT INTO databases (id, name, description, icon, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(&data.name)
    .bind(&data.description)
    .bind(if data.icon.is_empty() { "database" } else { &data.icon })
    .bind(if data.color.is_empty() { "blue" } else { &data.color })
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    get_database(db, id).await
}

pub async fn get_database(db: &Db, id: &str) -> Result<Database> {
    let row: Option<DatabaseRow> =
        sqlx::query_as("SELECT * FROM databases WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Database::from)
        .ok_or_else(|| NexusError::not_found("base de datos", id))
}

pub async fn list_databases(db: &Db) -> Result<Vec<DatabaseSummary>> {
    let rows = sqlx::query(
        "SELECT d.*,
                (SELECT COUNT(*) FROM db_records r WHERE r.database_id = d.id AND r.deleted_at IS NULL) AS record_count,
                (SELECT COUNT(*) FROM db_fields f WHERE f.database_id = d.id AND f.deleted_at IS NULL) AS field_count,
                (SELECT COUNT(*) FROM db_views v WHERE v.database_id = d.id) AS view_count
         FROM databases d WHERE d.deleted_at IS NULL ORDER BY d.created_at ASC",
    )
    .fetch_all(&db.pool)
    .await
    .db()?;
    rows.into_iter()
        .map(|r| {
            Ok(DatabaseSummary {
                record_count: r.try_get::<i64, _>("record_count").db()?,
                field_count: r.try_get::<i64, _>("field_count").db()?,
                view_count: r.try_get::<i64, _>("view_count").db()?,
                database: Database {
                    id: r.try_get::<String, _>("id").db()?,
                    name: r.try_get::<String, _>("name").db()?,
                    description: r.try_get::<String, _>("description").db()?,
                    icon: r.try_get::<String, _>("icon").db()?,
                    color: r.try_get::<String, _>("color").db()?,
                    created_at: r.try_get::<String, _>("created_at").db()?,
                    updated_at: r.try_get::<String, _>("updated_at").db()?,
                },
            })
        })
        .collect()
}

pub async fn update_database(db: &Db, id: &str, patch: &UpdateDatabase) -> Result<Database> {
    let mut d = get_database(db, id).await?;
    if let Some(v) = &patch.name {
        d.name = v.clone();
    }
    if let Some(v) = &patch.description {
        d.description = v.clone();
    }
    if let Some(v) = &patch.icon {
        d.icon = v.clone();
    }
    if let Some(v) = &patch.color {
        d.color = v.clone();
    }
    d.updated_at = now_str();
    sqlx::query(
        "UPDATE databases SET name=?, description=?, icon=?, color=?, updated_at=? WHERE id=?",
    )
    .bind(&d.name)
    .bind(&d.description)
    .bind(&d.icon)
    .bind(&d.color)
    .bind(&d.updated_at)
    .bind(id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(d)
}

pub async fn soft_delete_database(db: &Db, id: &str) -> Result<()> {
    let mut tx = db.pool.begin().await.db()?;
    soft_delete_database_tx(&mut tx, id).await?;
    tx.commit().await.db()?;
    Ok(())
}

pub async fn soft_delete_database_tx(ex: &mut sqlx::SqliteConnection, id: &str) -> Result<()> {
    let now = now_str();
    sqlx::query("UPDATE databases SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(&now)
        .bind(id)
        .execute(&mut *ex)
        .await
        .db()?;
    sqlx::query("UPDATE db_fields SET deleted_at = ? WHERE database_id = ? AND deleted_at IS NULL")
        .bind(&now)
        .bind(id)
        .execute(&mut *ex)
        .await
        .db()?;
    sqlx::query(
        "UPDATE db_records SET deleted_at = ? WHERE database_id = ? AND deleted_at IS NULL",
    )
    .bind(&now)
    .bind(id)
    .execute(&mut *ex)
    .await
    .db()?;
    sqlx::query("DELETE FROM db_views WHERE database_id = ?")
        .bind(id)
        .execute(&mut *ex)
        .await
        .db()?;
    Ok(())
}

pub async fn count_records(db: &Db, database_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM db_records WHERE database_id = ? AND deleted_at IS NULL",
    )
    .bind(database_id)
    .fetch_one(&db.pool)
    .await
    .db()?;
    row.try_get::<i64, _>("n").db()
}

// ---------------------------------------------------------------------------
// Fields
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct FieldRow {
    id: String,
    database_id: String,
    name: String,
    kind: String,
    config_json: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<FieldRow> for DbField {
    type Error = NexusError;

    fn try_from(r: FieldRow) -> Result<Self> {
        Ok(Self {
            id: r.id,
            database_id: r.database_id,
            name: r.name,
            kind: DbFieldKind::parse(&r.kind).ok_or_else(|| {
                NexusError::Internal(format!("tipo de campo desconocido: {}", r.kind))
            })?,
            config: parse_json(&r.config_json)?,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

pub async fn create_field(
    db: &Db,
    id: &str,
    database_id: &str,
    data: &CreateDbField,
) -> Result<DbField> {
    let now = now_str();
    let position = match data.position {
        Some(p) => p,
        None => {
            let row = sqlx::query(
                "SELECT COALESCE(MAX(position), -1) + 1 AS p FROM db_fields WHERE database_id = ? AND deleted_at IS NULL",
            )
            .bind(database_id)
            .fetch_one(&db.pool)
            .await
            .db()?;
            row.try_get::<i64, _>("p").db()?
        }
    };
    sqlx::query(
        "INSERT INTO db_fields (id, database_id, name, kind, config_json, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(database_id)
    .bind(&data.name)
    .bind(data.kind.as_str())
    .bind(json_string(&data.config)?)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    get_field(db, id).await
}

pub async fn get_field(db: &Db, id: &str) -> Result<DbField> {
    let row: Option<FieldRow> =
        sqlx::query_as("SELECT * FROM db_fields WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(DbField::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("campo", id))
}

pub async fn list_fields(db: &Db, database_id: &str) -> Result<Vec<DbField>> {
    let rows: Vec<FieldRow> = sqlx::query_as(
        "SELECT * FROM db_fields WHERE database_id = ? AND deleted_at IS NULL ORDER BY position ASC, created_at ASC",
    )
    .bind(database_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    rows.into_iter().map(DbField::try_from).collect()
}

pub async fn update_field(db: &Db, id: &str, patch: &UpdateDbField) -> Result<DbField> {
    let mut f = get_field(db, id).await?;
    if let Some(v) = &patch.name {
        f.name = v.clone();
    }
    if let Some(v) = &patch.kind {
        f.kind = *v;
    }
    if let Some(v) = &patch.config {
        f.config = v.clone();
    }
    f.updated_at = now_str();
    sqlx::query("UPDATE db_fields SET name=?, kind=?, config_json=?, updated_at=? WHERE id=?")
        .bind(&f.name)
        .bind(f.kind.as_str())
        .bind(json_string(&f.config)?)
        .bind(&f.updated_at)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(f)
}

/// Soft-delete: los valores de los registros NO se tocan (se conservan por si
/// el campo se restaura; al leer solo se muestran campos activos).
pub async fn soft_delete_field(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE db_fields SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn reorder_fields(db: &Db, database_id: &str, ordered_ids: &[Id]) -> Result<()> {
    let mut tx = db.pool.begin().await.db()?;
    for (i, id) in ordered_ids.iter().enumerate() {
        sqlx::query("UPDATE db_fields SET position = ? WHERE id = ? AND database_id = ?")
            .bind(i as i64)
            .bind(id)
            .bind(database_id)
            .execute(&mut *tx)
            .await
            .db()?;
    }
    tx.commit().await.db()?;
    Ok(())
}

/// Campos relation activos que apuntan a `target` (para limpiar referencias).
pub async fn relation_fields_targeting(db: &Db, target: &str) -> Result<Vec<DbField>> {
    let rows: Vec<FieldRow> =
        sqlx::query_as("SELECT * FROM db_fields WHERE kind = 'relation' AND deleted_at IS NULL")
            .fetch_all(&db.pool)
            .await
            .db()?;
    let mut out = Vec::new();
    for r in rows {
        let f = DbField::try_from(r)?;
        if f.config.target_database_id.as_deref() == Some(target) {
            out.push(f);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
pub struct RecordRow {
    pub id: String,
    pub database_id: String,
    pub values_json: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub fn record_from_row(r: RecordRow) -> Result<DbRecord> {
    Ok(DbRecord {
        id: r.id,
        database_id: r.database_id,
        values: parse_json(&r.values_json)?,
        computed: serde_json::Map::new(),
        formula_errors: serde_json::Map::new(),
        relation_titles: serde_json::Map::new(),
        position: r.position,
        created_at: r.created_at,
        updated_at: r.updated_at,
    })
}

pub async fn insert_record(
    ex: &mut sqlx::SqliteConnection,
    id: &str,
    database_id: &str,
    values: &serde_json::Map<String, Value>,
) -> Result<()> {
    let now = now_str();
    let row = sqlx::query(
        "SELECT COALESCE(MAX(position), -1) + 1 AS p FROM db_records WHERE database_id = ?",
    )
    .bind(database_id)
    .fetch_one(&mut *ex)
    .await
    .db()?;
    let position: i64 = row.try_get("p").db()?;
    sqlx::query(
        "INSERT INTO db_records (id, database_id, values_json, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(database_id)
    .bind(json_string(values)?)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&mut *ex)
    .await
    .db()?;
    Ok(())
}

pub async fn create_record(
    db: &Db,
    id: &str,
    database_id: &str,
    values: &serde_json::Map<String, Value>,
) -> Result<DbRecord> {
    let mut conn = db.pool.acquire().await.db()?;
    insert_record(&mut conn, id, database_id, values).await?;
    drop(conn);
    get_record(db, id).await
}

pub async fn get_record(db: &Db, id: &str) -> Result<DbRecord> {
    let row: Option<RecordRow> =
        sqlx::query_as("SELECT * FROM db_records WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(record_from_row)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("registro", id))
}

pub async fn get_record_row(db: &Db, id: &str) -> Result<RecordRow> {
    let row: Option<RecordRow> =
        sqlx::query_as("SELECT * FROM db_records WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.ok_or_else(|| NexusError::not_found("registro", id))
}

/// Todos los registros vivos de una base (para fórmulas, export, IA…).
pub async fn all_records(db: &Db, database_id: &str) -> Result<Vec<DbRecord>> {
    let rows: Vec<RecordRow> = sqlx::query_as(
        "SELECT * FROM db_records WHERE database_id = ? AND deleted_at IS NULL ORDER BY position ASC, created_at ASC",
    )
    .bind(database_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    rows.into_iter().map(record_from_row).collect()
}

pub async fn write_values(
    ex: &mut sqlx::SqliteConnection,
    id: &str,
    values: &serde_json::Map<String, Value>,
) -> Result<()> {
    sqlx::query("UPDATE db_records SET values_json = ?, updated_at = ? WHERE id = ?")
        .bind(json_string(values)?)
        .bind(now_str())
        .bind(id)
        .execute(&mut *ex)
        .await
        .db()?;
    Ok(())
}

pub async fn soft_delete_records(db: &Db, ids: &[Id]) -> Result<()> {
    let now = now_str();
    let mut tx = db.pool.begin().await.db()?;
    for id in ids {
        sqlx::query("UPDATE db_records SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await
            .db()?;
    }
    tx.commit().await.db()?;
    Ok(())
}

/// Títulos (primer campo text) de los registros dados, para chips de relación.
pub async fn record_titles(db: &Db, ids: &[Id]) -> Result<HashMap<Id, String>> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, values_json FROM db_records WHERE deleted_at IS NULL AND id IN (",
    );
    {
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(id);
        }
    }
    qb.push(")");
    let rows = qb.build().fetch_all(&db.pool).await.db()?;
    for r in rows {
        let id: String = r.try_get("id").db()?;
        let values: serde_json::Map<String, Value> =
            parse_json(&r.try_get::<String, _>("values_json").db()?)?;
        out.insert(id, first_text_value(&values));
    }
    Ok(out)
}

fn first_text_value(values: &serde_json::Map<String, Value>) -> String {
    values
        .values()
        .find_map(|v| v.as_str().filter(|s| !s.trim().is_empty()))
        .unwrap_or("(sin título)")
        .to_string()
}

// ---------------------------------------------------------------------------
// Filtros → SQL seguro
// ---------------------------------------------------------------------------

/// Expresión SQL del valor crudo de un campo. El id se liga como parámetro;
/// json_extract devuelve el tipo JSON nativo (número, texto, booleano).
fn value_expr(qb: &mut QueryBuilder<Sqlite>, _kind: DbFieldKind, field_id: &str) {
    qb.push("json_extract(values_json, '$.\"' || ")
        .push_bind(field_id.to_string());
    qb.push(" || '\"')");
}

fn is_empty_expr(qb: &mut QueryBuilder<Sqlite>, kind: DbFieldKind, field_id: &str) {
    qb.push("(");
    value_expr(qb, kind, field_id);
    qb.push(" IS NULL");
    match kind {
        DbFieldKind::Relation | DbFieldKind::Multiselect => {
            qb.push(" OR json_array_length(values_json, '$.\"' || ")
                .push_bind(field_id.to_string())
                .push(" || '\"') = 0");
        }
        DbFieldKind::Checkbox => {}
        _ => {
            qb.push(" OR ");
            value_expr(qb, kind, field_id);
            qb.push(" = ''");
        }
    }
    qb.push(")");
}

fn bind_scalar(qb: &mut QueryBuilder<Sqlite>, kind: DbFieldKind, value: &Value) {
    match kind {
        DbFieldKind::Integer | DbFieldKind::Rating => {
            qb.push_bind(
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .unwrap_or(0),
            );
        }
        DbFieldKind::Decimal => {
            qb.push_bind(
                value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                    .unwrap_or(0.0),
            );
        }
        DbFieldKind::Checkbox => {
            let b = value
                .as_bool()
                .unwrap_or_else(|| value.as_str() == Some("true"));
            qb.push_bind(b as i64);
        }
        _ => {
            qb.push_bind(
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
            );
        }
    }
}

fn push_filter(
    qb: &mut QueryBuilder<Sqlite>,
    fields: &HashMap<Id, DbField>,
    f: &DbFilter,
) -> Result<()> {
    let field = fields.get(&f.field_id).ok_or_else(|| {
        NexusError::validation(format!("campo de filtro desconocido: {}", f.field_id))
    })?;
    if field.kind == DbFieldKind::Formula {
        return Err(NexusError::validation(
            "los campos fórmula no se pueden filtrar en el servidor (se calculan al leer)",
        ));
    }
    let kind = field.kind;
    let fid = field.id.as_str();
    match f.op {
        DbFilterOp::IsEmpty => is_empty_expr(qb, kind, fid),
        DbFilterOp::NotEmpty => {
            qb.push("NOT ");
            is_empty_expr(qb, kind, fid);
        }
        DbFilterOp::Contains | DbFilterOp::NotContains => {
            let needle = f.value.as_str().unwrap_or_default().to_lowercase();
            if f.op == DbFilterOp::NotContains {
                qb.push("NOT (");
            }
            match kind {
                DbFieldKind::Select | DbFieldKind::Multiselect | DbFieldKind::Relation => {
                    qb.push("EXISTS (SELECT 1 FROM json_each(values_json, '$.\"' || ")
                        .push_bind(fid.to_string())
                        .push(" || '\"') je WHERE lower(je.value) LIKE '%' || ")
                        .push_bind(needle)
                        .push(" || '%')");
                }
                _ => {
                    qb.push("lower(COALESCE(CAST(");
                    value_expr(qb, kind, fid);
                    qb.push(" AS TEXT), '')) LIKE '%' || ")
                        .push_bind(needle)
                        .push(" || '%'");
                }
            }
            if f.op == DbFilterOp::NotContains {
                qb.push(")");
            }
        }
        DbFilterOp::Eq | DbFilterOp::Ne
            if matches!(kind, DbFieldKind::Multiselect | DbFieldKind::Relation) =>
        {
            if f.op == DbFilterOp::Ne {
                qb.push("NOT ");
            }
            qb.push("EXISTS (SELECT 1 FROM json_each(values_json, '$.\"' || ")
                .push_bind(fid.to_string())
                .push(" || '\"') je WHERE je.value = ")
                .push_bind(f.value.as_str().unwrap_or_default().to_string())
                .push(")");
        }
        op => {
            if matches!(kind, DbFieldKind::Multiselect | DbFieldKind::Relation) {
                return Err(NexusError::validation(
                    "usa contains/eq/is_empty para campos multiselect o relation",
                ));
            }
            value_expr(qb, kind, fid);
            qb.push(match op {
                DbFilterOp::Eq => " = ",
                DbFilterOp::Ne => " IS NOT ",
                DbFilterOp::Gt => " > ",
                DbFilterOp::Gte => " >= ",
                DbFilterOp::Lt => " < ",
                DbFilterOp::Lte => " <= ",
                _ => " = ",
            });
            bind_scalar(qb, kind, &f.value);
        }
    }
    Ok(())
}

fn push_where<'a>(
    qb: &mut QueryBuilder<'a, Sqlite>,
    fields: &HashMap<Id, DbField>,
    q: &'a RecordQuery,
) -> Result<()> {
    if let Some(s) = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let needle = s.to_lowercase();
        qb.push(" AND EXISTS (SELECT 1 FROM json_each(values_json) je WHERE je.type = 'text' AND lower(je.value) LIKE '%' || ")
            .push_bind(needle)
            .push(" || '%')");
    }
    if let Some(h) = &q.highlight_id {
        qb.push(" AND id = ").push_bind(h);
    }
    let usable: Vec<&DbFilter> = q.filters.filters.iter().collect();
    if !usable.is_empty() {
        qb.push(" AND (");
        for (i, f) in usable.iter().enumerate() {
            if i > 0 {
                qb.push(if q.filters.is_or() { " OR " } else { " AND " });
            }
            push_filter(qb, fields, f)?;
        }
        qb.push(")");
    }
    Ok(())
}

fn push_order(qb: &mut QueryBuilder<Sqlite>, fields: &HashMap<Id, DbField>, sorts: &[DbSort]) {
    let mut terms = 0;
    for s in sorts {
        let Some(field) = fields.get(&s.field_id) else {
            continue;
        };
        if !field.kind.is_stored()
            || matches!(field.kind, DbFieldKind::Multiselect | DbFieldKind::Relation)
        {
            continue;
        }
        if terms == 0 {
            qb.push(" ORDER BY ");
        } else {
            qb.push(", ");
        }
        // Los vacíos siempre al final, independientemente de la dirección.
        qb.push("(");
        is_empty_expr(qb, field.kind, &field.id);
        qb.push(") ASC, ");
        value_expr(qb, field.kind, &field.id);
        qb.push(if s.dir == DbSortDir::Desc {
            " DESC"
        } else {
            " ASC"
        });
        terms += 1;
    }
    if terms == 0 {
        qb.push(" ORDER BY ");
    } else {
        qb.push(", ");
    }
    qb.push("position ASC, created_at ASC");
}

pub struct QueriedRecords {
    pub records: Vec<DbRecord>,
    pub total: i64,
}

/// Consulta paginada con filtros/orden en SQL. Si hay ordenación por fórmula,
/// el servicio pagina en memoria (las fórmulas no viven en SQL).
pub async fn query_records(
    db: &Db,
    database_id: &str,
    fields: &[DbField],
    q: &RecordQuery,
    in_memory_paging: bool,
) -> Result<QueriedRecords> {
    let map: HashMap<Id, DbField> = fields.iter().map(|f| (f.id.clone(), f.clone())).collect();

    let mut cq: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT COUNT(*) AS n FROM db_records WHERE database_id = ");
    cq.push_bind(database_id).push(" AND deleted_at IS NULL");
    push_where(&mut cq, &map, q)?;
    let row = cq.build().fetch_one(&db.pool).await.db()?;
    let total: i64 = row.try_get("n").db()?;

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM db_records WHERE database_id = ");
    qb.push_bind(database_id).push(" AND deleted_at IS NULL");
    push_where(&mut qb, &map, q)?;
    push_order(&mut qb, &map, &q.sorts);
    if !in_memory_paging {
        let limit = q.limit.unwrap_or(500).clamp(1, 5000);
        let offset = q.offset.unwrap_or(0).max(0);
        qb.push(" LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
    }
    let rows: Vec<RecordRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    let records = rows
        .into_iter()
        .map(record_from_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(QueriedRecords { records, total })
}

/// Ids de registros vivos cuyos valores contienen `record_id` en el campo dado.
pub async fn records_referencing(
    db: &Db,
    field: &DbField,
    record_id: &str,
) -> Result<Vec<(Id, serde_json::Map<String, Value>)>> {
    let mut conn = db.pool.acquire().await.db()?;
    records_referencing_ex(&mut conn, field, record_id).await
}

pub async fn records_referencing_ex(
    ex: &mut sqlx::SqliteConnection,
    field: &DbField,
    record_id: &str,
) -> Result<Vec<(Id, serde_json::Map<String, Value>)>> {
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT id, values_json FROM db_records WHERE database_id = ");
    qb.push_bind(&field.database_id)
        .push(" AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM json_each(values_json, '$.\"' || ")
        .push_bind(&field.id)
        .push(" || '\"') je WHERE je.value = ")
        .push_bind(record_id)
        .push(")");
    let rows = qb.build().fetch_all(&mut *ex).await.db()?;
    let mut out = Vec::new();
    for r in rows {
        let id: String = r.try_get("id").db()?;
        let values: serde_json::Map<String, Value> =
            parse_json(&r.try_get::<String, _>("values_json").db()?)?;
        out.push((id, values));
    }
    Ok(out)
}

/// Ids vivos de una base (validación de relaciones).
pub async fn live_record_ids(db: &Db, database_id: &str) -> Result<Vec<Id>> {
    let rows =
        sqlx::query("SELECT id FROM db_records WHERE database_id = ? AND deleted_at IS NULL")
            .bind(database_id)
            .fetch_all(&db.pool)
            .await
            .db()?;
    rows.into_iter()
        .map(|r| r.try_get::<String, _>("id").db())
        .collect()
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ViewRow {
    id: String,
    database_id: String,
    name: String,
    kind: String,
    config_json: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ViewRow> for DbView {
    type Error = NexusError;

    fn try_from(r: ViewRow) -> Result<Self> {
        Ok(Self {
            id: r.id,
            database_id: r.database_id,
            name: r.name,
            kind: DbViewKind::parse(&r.kind).ok_or_else(|| {
                NexusError::Internal(format!("tipo de vista desconocido: {}", r.kind))
            })?,
            config: parse_json(&r.config_json)?,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

pub async fn create_view(
    db: &Db,
    id: &str,
    database_id: &str,
    data: &CreateDbView,
) -> Result<DbView> {
    let now = now_str();
    let row = sqlx::query(
        "SELECT COALESCE(MAX(position), -1) + 1 AS p FROM db_views WHERE database_id = ?",
    )
    .bind(database_id)
    .fetch_one(&db.pool)
    .await
    .db()?;
    let position: i64 = row.try_get("p").db()?;
    sqlx::query(
        "INSERT INTO db_views (id, database_id, name, kind, config_json, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(database_id)
    .bind(&data.name)
    .bind(data.kind.as_str())
    .bind(json_string(&data.config)?)
    .bind(position)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    get_view(db, id).await
}

pub async fn get_view(db: &Db, id: &str) -> Result<DbView> {
    let row: Option<ViewRow> = sqlx::query_as("SELECT * FROM db_views WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(DbView::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("vista", id))
}

pub async fn list_views(db: &Db, database_id: &str) -> Result<Vec<DbView>> {
    let rows: Vec<ViewRow> = sqlx::query_as(
        "SELECT * FROM db_views WHERE database_id = ? ORDER BY position ASC, created_at ASC",
    )
    .bind(database_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    rows.into_iter().map(DbView::try_from).collect()
}

pub async fn update_view(db: &Db, id: &str, patch: &UpdateDbView) -> Result<DbView> {
    let mut v = get_view(db, id).await?;
    if let Some(n) = &patch.name {
        v.name = n.clone();
    }
    if let Some(k) = &patch.kind {
        v.kind = *k;
    }
    if let Some(c) = &patch.config {
        v.config = c.clone();
    }
    v.updated_at = now_str();
    sqlx::query("UPDATE db_views SET name=?, kind=?, config_json=?, updated_at=? WHERE id=?")
        .bind(&v.name)
        .bind(v.kind.as_str())
        .bind(json_string(&v.config)?)
        .bind(&v.updated_at)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(v)
}

pub async fn delete_view(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM db_views WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("vista", id));
    }
    Ok(())
}

/// Configuración de vista usada por el export (filtros/orden propios).
pub fn view_query(config: &DbViewConfig) -> RecordQuery {
    RecordQuery {
        filters: config.filters.clone(),
        sorts: config.sorts.clone(),
        limit: Some(5000),
        ..Default::default()
    }
}
