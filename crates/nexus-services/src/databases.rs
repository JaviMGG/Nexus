//! NEXUS Database: servicio de bases de datos personalizadas.
//! Validación por tipo, filtros/orden en backend, motor de fórmulas seguro
//! (parser recursivo, sin eval), relaciones con limpieza de referencias,
//! import/export CSV+JSON, plantillas e IA opcional (solo propuestas).

use std::collections::{HashMap, HashSet};

use nexus_core::{
    new_id, AiFieldProposal, AiStructureProposal, CreateDatabase, CreateDbField, CreateDbRecord,
    CreateDbView, Database, DatabaseDetail, DatabaseSummary, DbCardinality, DbField, DbFieldConfig,
    DbFieldKind, DbRecord, DbSelectOption, DbSortDir, DbTemplate, DbTemplateField, DbView,
    DbViewConfig, DbViewKind, DuplicatePair, ExportResult, ImportPreview, ImportRequest,
    ImportResult, ImportRowError, NexusError, RecordPage, RecordPreview, RecordQuery, Result,
    UpdateDatabase, UpdateDbField, UpdateDbRecord, UpdateDbView, DATABASE_COLORS, OPTION_COLORS,
};
use nexus_db::{repos, Db};
use serde_json::{json, Map, Value};

use crate::validate;

pub const MAX_IMPORT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_IMPORT_ROWS: usize = 10_000;
const MAX_FIELDS: usize = 100;
const MAX_RECORDS: i64 = 100_000;

#[derive(Clone)]
pub struct DatabaseService {
    pub db: Db,
}

// ===========================================================================
// Motor de fórmulas (parser recursivo descendente, sin eval de código)
// ===========================================================================

pub mod formula {
    use std::collections::HashMap;

    use chrono::{NaiveDate, Utc};

    #[derive(Debug, Clone, PartialEq)]
    pub enum FVal {
        Null,
        Num(f64),
        Str(String),
        Bool(bool),
    }

    impl FVal {
        pub fn to_json(&self) -> serde_json::Value {
            match self {
                FVal::Null => serde_json::Value::Null,
                FVal::Num(n) => serde_json::Number::from_f64(*n)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                FVal::Str(s) => serde_json::Value::String(s.clone()),
                FVal::Bool(b) => serde_json::Value::Bool(*b),
            }
        }

        pub fn from_json(v: &serde_json::Value) -> Self {
            match v {
                serde_json::Value::Null => FVal::Null,
                serde_json::Value::Bool(b) => FVal::Bool(*b),
                serde_json::Value::Number(n) => FVal::Num(n.as_f64().unwrap_or(0.0)),
                serde_json::Value::String(s) => FVal::Str(s.clone()),
                serde_json::Value::Array(a) => FVal::Str(
                    a.iter()
                        .map(|x| FVal::from_json(x).as_text())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                other => FVal::Str(other.to_string()),
            }
        }

        pub fn truthy(&self) -> bool {
            match self {
                FVal::Null => false,
                FVal::Bool(b) => *b,
                FVal::Num(n) => *n != 0.0,
                FVal::Str(s) => !s.is_empty(),
            }
        }

        pub fn as_num(&self) -> Result<f64, String> {
            match self {
                FVal::Num(n) => Ok(*n),
                FVal::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
                FVal::Str(s) => s
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| format!("«{s}» no es un número")),
                FVal::Null => Err("valor vacío donde se esperaba un número".into()),
            }
        }

        pub fn as_text(&self) -> String {
            match self {
                FVal::Null => String::new(),
                FVal::Num(n) => {
                    if n.fract() == 0.0 && n.abs() < 1e15 {
                        format!("{}", *n as i64)
                    } else {
                        format!("{n}")
                    }
                }
                FVal::Str(s) => s.clone(),
                FVal::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            }
        }
    }

    // ---------------- Lexer ----------------

    #[derive(Debug, Clone, PartialEq)]
    enum Tok {
        Num(f64),
        Str(String),
        Ident(String),
        Ref(String),
        Op(String),
        LParen,
        RParen,
        Comma,
    }

    fn lex(src: &str) -> Result<Vec<Tok>, String> {
        let chars: Vec<char> = src.chars().collect();
        let mut i = 0;
        let mut out = Vec::new();
        while i < chars.len() {
            let c = chars[i];
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            match c {
                '(' => {
                    out.push(Tok::LParen);
                    i += 1;
                }
                ')' => {
                    out.push(Tok::RParen);
                    i += 1;
                }
                ',' => {
                    out.push(Tok::Comma);
                    i += 1;
                }
                '"' | '\'' => {
                    let quote = c;
                    i += 1;
                    let mut s = String::new();
                    while i < chars.len() && chars[i] != quote {
                        s.push(chars[i]);
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err("cadena sin cerrar".into());
                    }
                    i += 1;
                    out.push(Tok::Str(s));
                }
                '{' => {
                    i += 1;
                    let mut s = String::new();
                    while i < chars.len() && chars[i] != '}' {
                        s.push(chars[i]);
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err("referencia {Campo} sin cerrar".into());
                    }
                    i += 1;
                    out.push(Tok::Ref(s.trim().to_string()));
                }
                '+' | '-' | '*' | '/' | '%' | '&' => {
                    out.push(Tok::Op(c.to_string()));
                    i += 1;
                }
                '=' => {
                    if i + 1 < chars.len() && chars[i + 1] == '=' {
                        i += 1;
                    }
                    out.push(Tok::Op("=".into()));
                    i += 1;
                }
                '!' => {
                    if i + 1 < chars.len() && chars[i + 1] == '=' {
                        out.push(Tok::Op("!=".into()));
                        i += 2;
                    } else {
                        return Err("«!» solo es válido en «!=»".into());
                    }
                }
                '<' | '>' => {
                    if i + 1 < chars.len() && chars[i + 1] == '=' {
                        out.push(Tok::Op(format!("{c}=")));
                        i += 2;
                    } else {
                        out.push(Tok::Op(c.to_string()));
                        i += 1;
                    }
                }
                _ if c.is_ascii_digit()
                    || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
                {
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        i += 1;
                    }
                    let s: String = chars[start..i].iter().collect();
                    let n = s
                        .parse::<f64>()
                        .map_err(|_| format!("número inválido: {s}"))?;
                    out.push(Tok::Num(n));
                }
                _ if c.is_alphabetic() || c == '_' => {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    out.push(Tok::Ident(chars[start..i].iter().collect()));
                }
                other => return Err(format!("carácter no válido en la fórmula: «{other}»")),
            }
        }
        Ok(out)
    }

    // ---------------- AST ----------------

    #[derive(Debug, Clone)]
    enum Expr {
        Lit(FVal),
        Ref(String),
        Unary(String, Box<Expr>),
        Binary(String, Box<Expr>, Box<Expr>),
        Call(String, Vec<Expr>),
    }

    struct Parser {
        toks: Vec<Tok>,
        pos: usize,
        depth: usize,
    }

    impl Parser {
        fn peek(&self) -> Option<&Tok> {
            self.toks.get(self.pos)
        }

        fn next(&mut self) -> Option<Tok> {
            let t = self.toks.get(self.pos).cloned();
            if t.is_some() {
                self.pos += 1;
            }
            t
        }

        fn enter(&mut self) -> Result<(), String> {
            self.depth += 1;
            if self.depth > 64 {
                return Err("fórmula demasiado compleja (anidamiento máximo 64)".into());
            }
            Ok(())
        }

        fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, String> {
            self.enter()?;
            let mut left = self.parse_unary()?;
            while let Some(Tok::Op(o)) = self.peek() {
                let prec = match o.as_str() {
                    "=" | "!=" => 1,
                    "<" | ">" | "<=" | ">=" => 2,
                    "&" => 3,
                    "+" | "-" => 4,
                    "*" | "/" | "%" => 5,
                    _ => break,
                };
                if prec < min_prec {
                    break;
                }
                let op = o.clone();
                self.next();
                let right = self.parse_expr(prec + 1)?;
                left = Expr::Binary(op, Box::new(left), Box::new(right));
            }
            self.depth -= 1;
            Ok(left)
        }

        fn parse_unary(&mut self) -> Result<Expr, String> {
            self.enter()?;
            let e = match self.peek() {
                Some(Tok::Op(o)) if o == "-" || o == "+" => {
                    let o = o.clone();
                    self.next();
                    Expr::Unary(o, Box::new(self.parse_unary()?))
                }
                _ => self.parse_primary()?,
            };
            self.depth -= 1;
            Ok(e)
        }

        fn parse_primary(&mut self) -> Result<Expr, String> {
            match self.next() {
                Some(Tok::Num(n)) => Ok(Expr::Lit(FVal::Num(n))),
                Some(Tok::Str(s)) => Ok(Expr::Lit(FVal::Str(s))),
                Some(Tok::Ref(name)) => Ok(Expr::Ref(name)),
                Some(Tok::Ident(id)) => {
                    let upper = id.to_uppercase();
                    match upper.as_str() {
                        "TRUE" | "VERDADERO" => Ok(Expr::Lit(FVal::Bool(true))),
                        "FALSE" | "FALSO" => Ok(Expr::Lit(FVal::Bool(false))),
                        _ => {
                            if self.peek() != Some(&Tok::LParen) {
                                return Err(format!(
                                    "identificador «{id}» desconocido; usa {{Nombre del campo}} para referencias"
                                ));
                            }
                            self.next();
                            let mut args = Vec::new();
                            if self.peek() != Some(&Tok::RParen) {
                                loop {
                                    args.push(self.parse_expr(1)?);
                                    match self.next() {
                                        Some(Tok::Comma) => continue,
                                        Some(Tok::RParen) => break,
                                        _ => {
                                            return Err(format!(
                                                "se esperaba ',' o ')' en {upper}(…)"
                                            ))
                                        }
                                    }
                                }
                            } else {
                                self.next();
                            }
                            Ok(Expr::Call(upper, args))
                        }
                    }
                }
                Some(Tok::LParen) => {
                    let e = self.parse_expr(1)?;
                    if self.next() != Some(Tok::RParen) {
                        return Err("paréntesis sin cerrar".into());
                    }
                    Ok(e)
                }
                other => Err(format!("expresión inesperada: {other:?}")),
            }
        }
    }

    fn parse(src: &str) -> Result<Expr, String> {
        if src.trim().is_empty() {
            return Err("la fórmula está vacía".into());
        }
        if src.chars().count() > 2000 {
            return Err("fórmula demasiado larga (máx. 2000 caracteres)".into());
        }
        let mut p = Parser {
            toks: lex(src)?,
            pos: 0,
            depth: 0,
        };
        let e = p.parse_expr(1)?;
        if p.pos != p.toks.len() {
            return Err("tokens sobrantes al final de la fórmula".into());
        }
        Ok(e)
    }

    // ---------------- Evaluación ----------------

    fn parse_date(s: &str) -> Option<NaiveDate> {
        let s = s.trim();
        if let Ok(d) = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d") {
            return Some(d);
        }
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.date_naive())
    }

    fn num_eq(a: &FVal, b: &FVal) -> bool {
        match (a, b) {
            (FVal::Null, FVal::Null) => true,
            (FVal::Str(x), FVal::Str(y)) => x == y,
            (FVal::Bool(x), FVal::Bool(y)) => x == y,
            _ => match (a.as_num(), b.as_num()) {
                (Ok(x), Ok(y)) => x == y,
                _ => a.as_text() == b.as_text(),
            },
        }
    }

    fn eval(e: &Expr, fields: &HashMap<String, FVal>) -> Result<FVal, String> {
        match e {
            Expr::Lit(v) => Ok(v.clone()),
            Expr::Ref(name) => fields
                .get(name)
                .cloned()
                .ok_or_else(|| format!("campo «{name}» no existe")),
            Expr::Unary(op, inner) => {
                let v = eval(inner, fields)?;
                if op == "-" {
                    Ok(FVal::Num(-v.as_num()?))
                } else {
                    Ok(FVal::Num(v.as_num()?))
                }
            }
            Expr::Binary(op, l, r) => {
                let a = eval(l, fields)?;
                let b = eval(r, fields)?;
                match op.as_str() {
                    "+" => Ok(FVal::Num(a.as_num()? + b.as_num()?)),
                    "-" => Ok(FVal::Num(a.as_num()? - b.as_num()?)),
                    "*" => Ok(FVal::Num(a.as_num()? * b.as_num()?)),
                    "/" => {
                        let d = b.as_num()?;
                        if d == 0.0 {
                            Err("división entre cero".into())
                        } else {
                            Ok(FVal::Num(a.as_num()? / d))
                        }
                    }
                    "%" => {
                        let d = b.as_num()?;
                        if d == 0.0 {
                            Err("módulo entre cero".into())
                        } else {
                            Ok(FVal::Num(a.as_num()? % d))
                        }
                    }
                    "&" => Ok(FVal::Str(format!("{}{}", a.as_text(), b.as_text()))),
                    "=" => Ok(FVal::Bool(num_eq(&a, &b))),
                    "!=" => Ok(FVal::Bool(!num_eq(&a, &b))),
                    "<" | ">" | "<=" | ">=" => {
                        let (x, y) = match (&a, &b) {
                            (FVal::Str(x), FVal::Str(y)) if x.parse::<f64>().is_err() => {
                                (FVal::Str(x.clone()), FVal::Str(y.clone()))
                            }
                            _ => (FVal::Num(a.as_num()?), FVal::Num(b.as_num()?)),
                        };
                        let ord = match (&x, &y) {
                            (FVal::Str(p), FVal::Str(q)) => p.cmp(q),
                            (FVal::Num(p), FVal::Num(q)) => {
                                p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal)
                            }
                            _ => std::cmp::Ordering::Equal,
                        };
                        use std::cmp::Ordering::*;
                        Ok(FVal::Bool(match op.as_str() {
                            "<" => ord == Less,
                            ">" => ord == Greater,
                            "<=" => ord != Greater,
                            _ => ord != Less,
                        }))
                    }
                    other => Err(format!("operador desconocido: {other}")),
                }
            }
            Expr::Call(name, args) => {
                let arity = |n: usize| -> Result<(), String> {
                    if args.len() != n {
                        Err(format!(
                            "{name} espera {n} argumento(s), recibió {}",
                            args.len()
                        ))
                    } else {
                        Ok(())
                    }
                };
                match name.as_str() {
                    "IF" => {
                        arity(3)?;
                        if eval(&args[0], fields)?.truthy() {
                            eval(&args[1], fields)
                        } else {
                            eval(&args[2], fields)
                        }
                    }
                    "CONCAT" => {
                        let mut s = String::new();
                        for a in args {
                            s.push_str(&eval(a, fields)?.as_text());
                        }
                        Ok(FVal::Str(s))
                    }
                    "ROUND" => {
                        if args.is_empty() || args.len() > 2 {
                            return Err("ROUND espera 1 o 2 argumentos".into());
                        }
                        let n = eval(&args[0], fields)?.as_num()?;
                        let decimals = if args.len() == 2 {
                            eval(&args[1], fields)?.as_num()? as i32
                        } else {
                            0
                        };
                        if !(-10..=10).contains(&decimals) {
                            return Err("ROUND: decimales fuera de rango (-10..10)".into());
                        }
                        let f = 10f64.powi(decimals);
                        Ok(FVal::Num((n * f).round() / f))
                    }
                    "ABS" => {
                        arity(1)?;
                        Ok(FVal::Num(eval(&args[0], fields)?.as_num()?.abs()))
                    }
                    "LEN" => {
                        arity(1)?;
                        Ok(FVal::Num(
                            eval(&args[0], fields)?.as_text().chars().count() as f64
                        ))
                    }
                    "UPPER" => {
                        arity(1)?;
                        Ok(FVal::Str(eval(&args[0], fields)?.as_text().to_uppercase()))
                    }
                    "LOWER" => {
                        arity(1)?;
                        Ok(FVal::Str(eval(&args[0], fields)?.as_text().to_lowercase()))
                    }
                    "TODAY" => {
                        arity(0)?;
                        Ok(FVal::Str(Utc::now().format("%Y-%m-%d").to_string()))
                    }
                    "DAYS" => {
                        arity(2)?;
                        let a = eval(&args[0], fields)?.as_text();
                        let b = eval(&args[1], fields)?.as_text();
                        let da =
                            parse_date(&a).ok_or_else(|| format!("DAYS: fecha inválida «{a}»"))?;
                        let db =
                            parse_date(&b).ok_or_else(|| format!("DAYS: fecha inválida «{b}»"))?;
                        Ok(FVal::Num((db - da).num_days() as f64))
                    }
                    other => Err(format!("función desconocida: {other}")),
                }
            }
        }
    }

    /// Evalúa una expresión con valores de campo por nombre.
    pub fn evaluate(
        expr: &str,
        fields: &HashMap<String, FVal>,
    ) -> Result<serde_json::Value, String> {
        let ast = parse(expr)?;
        Ok(eval(&ast, fields)?.to_json())
    }

    /// Valida la sintaxis sin evaluar (al crear/editar el campo).
    pub fn check_syntax(expr: &str) -> Result<(), String> {
        parse(expr).map(|_| ())
    }
}

// ===========================================================================
// Validación de valores por tipo
// ===========================================================================

fn email_valid(s: &str) -> bool {
    let re = regex::Regex::new(r"^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$").unwrap();
    re.is_match(s)
}

fn phone_valid(s: &str) -> bool {
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    (3..=15).contains(&digits)
        && s.chars()
            .all(|c| c.is_ascii_digit() || " +()-./".contains(c))
}

fn date_valid(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

fn datetime_valid(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").is_ok()
}

fn option_exists<'a>(config: &'a DbFieldConfig, id: &str) -> Option<&'a DbSelectOption> {
    config.options.iter().find(|o| o.id == id)
}

/// Normaliza y valida el valor de un campo. `Null` siempre se acepta (vacío).
fn validate_value(field: &DbField, value: &Value) -> Result<Value> {
    let err = |msg: String| NexusError::validation(format!("campo «{}»: {msg}", field.name));
    if value.is_null() {
        return Ok(Value::Null);
    }
    let as_str = || -> Result<String> {
        value
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| err("se esperaba un texto no vacío".into()))
    };
    match field.kind {
        DbFieldKind::Text => {
            let s = as_str()?;
            if s.chars().count() > 500 {
                return Err(err("texto demasiado largo (máx. 500)".into()));
            }
            Ok(json!(s))
        }
        DbFieldKind::LongText => {
            let s = as_str()?;
            validate::long_text(&s, "el texto", 50_000).map(|s| json!(s))
        }
        DbFieldKind::Integer => {
            let n = value
                .as_i64()
                .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
                .ok_or_else(|| err("se esperaba un número entero".into()))?;
            Ok(json!(n))
        }
        DbFieldKind::Decimal => {
            let n = value
                .as_f64()
                .or_else(|| {
                    value
                        .as_str()
                        .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok())
                })
                .filter(|n| n.is_finite())
                .ok_or_else(|| err("se esperaba un número decimal".into()))?;
            Ok(json!(n))
        }
        DbFieldKind::Date => {
            let s = as_str()?;
            if !date_valid(&s) {
                return Err(err(format!("fecha inválida «{s}» (usa YYYY-MM-DD)")));
            }
            Ok(json!(s))
        }
        DbFieldKind::Datetime => {
            let s = as_str()?;
            if !datetime_valid(&s) {
                return Err(err(format!(
                    "fecha y hora inválidas «{s}» (usa YYYY-MM-DDTHH:MM)"
                )));
            }
            Ok(json!(s))
        }
        DbFieldKind::Checkbox => {
            let b = match value {
                Value::Bool(b) => *b,
                Value::String(s) => match s.trim().to_lowercase().as_str() {
                    "true" | "1" | "sí" | "si" | "yes" => true,
                    "false" | "0" | "no" | "" => false,
                    other => return Err(err(format!("booleano inválido «{other}»"))),
                },
                Value::Number(n) => n.as_i64() == Some(1),
                _ => return Err(err("se esperaba true/false".into())),
            };
            Ok(json!(b))
        }
        DbFieldKind::Select => {
            let s = as_str()?;
            if option_exists(&field.config, &s).is_none() {
                return Err(err(format!("la opción «{s}» no existe en el campo")));
            }
            Ok(json!(s))
        }
        DbFieldKind::Multiselect => {
            let arr = value
                .as_array()
                .ok_or_else(|| err("se esperaba un array de opciones".into()))?;
            let mut out = Vec::new();
            for v in arr {
                let s = v
                    .as_str()
                    .ok_or_else(|| err("las opciones deben ser ids de texto".into()))?;
                if option_exists(&field.config, s).is_none() {
                    return Err(err(format!("la opción «{s}» no existe en el campo")));
                }
                if !out.contains(&json!(s)) {
                    out.push(json!(s));
                }
            }
            Ok(Value::Array(out))
        }
        DbFieldKind::Url => validate::url(&as_str()?).map(|s| json!(s)),
        DbFieldKind::Email => {
            let s = as_str()?;
            if !email_valid(&s) {
                return Err(err(format!("email inválido «{s}»")));
            }
            Ok(json!(s))
        }
        DbFieldKind::Phone => {
            let s = as_str()?;
            if !phone_valid(&s) {
                return Err(err(format!("teléfono inválido «{s}»")));
            }
            Ok(json!(s))
        }
        DbFieldKind::Image | DbFieldKind::File => {
            let s = as_str()?;
            if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(err("se esperaba el id de un archivo adjunto".into()));
            }
            Ok(json!(s))
        }
        DbFieldKind::Rating => {
            let max = field.config.max_rating.unwrap_or(5).clamp(1, 10);
            let n = value
                .as_i64()
                .or_else(|| value.as_f64().map(|f| f as i64))
                .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
                .ok_or_else(|| err("se esperaba una puntuación numérica".into()))?;
            if !(1..=max).contains(&n) {
                return Err(err(format!("la puntuación debe estar entre 1 y {max}")));
            }
            Ok(json!(n))
        }
        DbFieldKind::Relation => {
            let arr = value
                .as_array()
                .ok_or_else(|| err("se esperaba un array de ids de registro".into()))?;
            let mut out: Vec<Value> = Vec::new();
            for v in arr {
                let s = v
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| err("los ids de relación deben ser texto".into()))?;
                if !out.contains(&json!(s)) {
                    out.push(json!(s));
                }
            }
            let card = field.config.cardinality.unwrap_or_default();
            if matches!(card, DbCardinality::OneToOne | DbCardinality::OneToMany) && out.len() > 1 {
                return Err(err("esta relación admite como máximo un registro".into()));
            }
            Ok(Value::Array(out))
        }
        DbFieldKind::Formula => Err(err(
            "los campos fórmula son calculados; no se pueden escribir".into(),
        )),
    }
}

fn validate_field_config(db_field_kind: DbFieldKind, config: &DbFieldConfig) -> Result<()> {
    match db_field_kind {
        DbFieldKind::Select | DbFieldKind::Multiselect => {
            if config.options.len() > 100 {
                return Err(NexusError::validation("máximo 100 opciones por campo"));
            }
            let mut ids = HashSet::new();
            for o in &config.options {
                if o.id.trim().is_empty() || o.label.trim().is_empty() {
                    return Err(NexusError::validation(
                        "las opciones necesitan id y etiqueta",
                    ));
                }
                if o.label.chars().count() > 100 {
                    return Err(NexusError::validation("etiqueta de opción demasiado larga"));
                }
                if !ids.insert(&o.id) {
                    return Err(NexusError::validation(format!(
                        "opción duplicada: {}",
                        o.id
                    )));
                }
                if !OPTION_COLORS.contains(&o.color.as_str()) {
                    return Err(NexusError::validation(format!(
                        "color de opción inválido: {}",
                        o.color
                    )));
                }
            }
        }
        DbFieldKind::Relation => {
            if config
                .target_database_id
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(NexusError::validation(
                    "un campo relation necesita target_database_id",
                ));
            }
        }
        DbFieldKind::Formula => {
            let expr = config.expression.as_deref().unwrap_or("");
            formula::check_syntax(expr)
                .map_err(|e| NexusError::validation(format!("fórmula inválida: {e}")))?;
        }
        DbFieldKind::Rating => {
            if let Some(m) = config.max_rating {
                if !(1..=10).contains(&m) {
                    return Err(NexusError::validation("max_rating debe estar entre 1 y 10"));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ===========================================================================
// Servicio
// ===========================================================================

impl DatabaseService {
    // ------------------------------------------------------------------ bases

    pub async fn list(&self) -> Result<Vec<DatabaseSummary>> {
        repos::databases::list_databases(&self.db).await
    }

    pub async fn get(&self, id: &str) -> Result<DatabaseDetail> {
        let summaries = repos::databases::list_databases(&self.db).await?;
        let summary = summaries
            .into_iter()
            .find(|s| s.database.id == id)
            .ok_or_else(|| NexusError::not_found("base de datos", id))?;
        let fields = repos::databases::list_fields(&self.db, id).await?;
        let views = repos::databases::list_views(&self.db, id).await?;
        Ok(DatabaseDetail {
            summary,
            fields,
            views,
        })
    }

    fn validate_database_meta(
        name: &str,
        description: &str,
        icon: &str,
        color: &str,
    ) -> Result<()> {
        validate::title(name, "el nombre de la base")?;
        validate::long_text(description, "la descripción", 5_000)?;
        if !icon.is_empty()
            && (icon.len() > 50 || !icon.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        {
            return Err(NexusError::validation(
                "icono inválido (nombre de icono lucide)",
            ));
        }
        if !color.is_empty() && !DATABASE_COLORS.contains(&color) {
            return Err(NexusError::validation(format!("color inválido: {color}")));
        }
        Ok(())
    }

    pub async fn create(&self, data: CreateDatabase, actor: &str) -> Result<Database> {
        Self::validate_database_meta(&data.name, &data.description, &data.icon, &data.color)?;
        let mut data = data;
        data.name = data.name.trim().to_string();
        let database = repos::databases::create_database(&self.db, &new_id(), &data).await?;
        // Vista de tabla por defecto para que la base sea usable al instante.
        repos::databases::create_view(
            &self.db,
            &new_id(),
            &database.id,
            &CreateDbView {
                name: "Tabla".into(),
                kind: DbViewKind::Table,
                config: DbViewConfig::default(),
            },
        )
        .await?;
        repos::activity::record(
            &self.db,
            "created",
            "database",
            Some(&database.id),
            &format!("Base de datos creada: {}", database.name),
            actor,
            None,
        )
        .await?;
        Ok(database)
    }

    pub async fn update(&self, id: &str, patch: UpdateDatabase, actor: &str) -> Result<Database> {
        let before = repos::databases::get_database(&self.db, id).await?;
        let name = patch.name.clone().unwrap_or(before.name.clone());
        let desc = patch
            .description
            .clone()
            .unwrap_or(before.description.clone());
        let icon = patch.icon.clone().unwrap_or(before.icon.clone());
        let color = patch.color.clone().unwrap_or(before.color.clone());
        Self::validate_database_meta(&name, &desc, &icon, &color)?;
        let database = repos::databases::update_database(&self.db, id, &patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "database",
            Some(id),
            &format!("Base de datos actualizada: {}", database.name),
            actor,
            None,
        )
        .await?;
        Ok(database)
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let database = repos::databases::get_database(&self.db, id).await?;
        // Limpia las referencias entrantes desde otras bases antes de borrar.
        let record_ids = repos::databases::live_record_ids(&self.db, id).await?;
        let relation_fields = repos::databases::relation_fields_targeting(&self.db, id).await?;
        let mut tx = self.db.pool.begin().await.map_err(nexus_db::db_err)?;
        for field in relation_fields {
            if field.database_id == id {
                continue;
            }
            for rid in &record_ids {
                let refs = repos::databases::records_referencing_ex(&mut tx, &field, rid).await?;
                for (ref_id, mut values) in refs {
                    if let Some(Value::Array(arr)) = values.get_mut(&field.id) {
                        arr.retain(|v| v.as_str() != Some(rid.as_str()));
                    }
                    repos::databases::write_values(&mut tx, &ref_id, &values).await?;
                }
            }
        }
        repos::databases::soft_delete_database_tx(&mut tx, id).await?;
        tx.commit().await.map_err(nexus_db::db_err)?;
        repos::activity::record(
            &self.db,
            "deleted",
            "database",
            Some(id),
            &format!("Base de datos eliminada: {}", database.name),
            actor,
            None,
        )
        .await?;
        Ok(())
    }

    // ----------------------------------------------------------------- campos

    pub async fn create_field(
        &self,
        database_id: &str,
        mut data: CreateDbField,
        actor: &str,
    ) -> Result<DbField> {
        repos::databases::get_database(&self.db, database_id).await?;
        data.name = validate::title(&data.name, "el nombre del campo")?;
        let existing = repos::databases::list_fields(&self.db, database_id).await?;
        if existing.len() >= MAX_FIELDS {
            return Err(NexusError::LimitExceeded(format!(
                "máximo {MAX_FIELDS} campos por base"
            )));
        }
        if existing
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(&data.name))
        {
            return Err(NexusError::validation(format!(
                "ya existe un campo llamado «{}»",
                data.name
            )));
        }
        self.check_field_relations(&data.kind, &data.config, database_id)
            .await?;
        validate_field_config(data.kind, &data.config)?;
        let field = repos::databases::create_field(&self.db, &new_id(), database_id, &data).await?;
        repos::activity::record(
            &self.db,
            "created",
            "db_field",
            Some(&field.id),
            &format!("Campo creado: {} ({})", field.name, field.kind),
            actor,
            None,
        )
        .await?;
        Ok(field)
    }

    async fn check_field_relations(
        &self,
        kind: &DbFieldKind,
        config: &DbFieldConfig,
        database_id: &str,
    ) -> Result<()> {
        if *kind == DbFieldKind::Relation {
            if let Some(target) = &config.target_database_id {
                repos::databases::get_database(&self.db, target).await?;
                let _ = database_id;
            }
        }
        if *kind == DbFieldKind::Formula {
            if let Some(expr) = &config.expression {
                // Referencias a campos inexistentes se detectan al crear si la
                // base ya tiene campos; si no, se validan al evaluar.
                let fields = repos::databases::list_fields(&self.db, database_id).await?;
                if !fields.is_empty() {
                    let names: HashSet<String> = fields.iter().map(|f| f.name.clone()).collect();
                    for r in extract_refs(expr) {
                        if !names.contains(&r) {
                            return Err(NexusError::validation(format!(
                                "la fórmula referencia el campo «{r}», que no existe"
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn update_field(
        &self,
        id: &str,
        mut patch: UpdateDbField,
        actor: &str,
    ) -> Result<DbField> {
        let before = repos::databases::get_field(&self.db, id).await?;
        if let Some(n) = &patch.name {
            patch.name = Some(validate::title(n, "el nombre del campo")?);
            let siblings = repos::databases::list_fields(&self.db, &before.database_id).await?;
            if siblings.iter().any(|f| {
                f.id != id
                    && f.name
                        .eq_ignore_ascii_case(patch.name.as_deref().unwrap_or(""))
            }) {
                return Err(NexusError::validation(
                    "ya existe otro campo con ese nombre",
                ));
            }
        }
        let kind = patch.kind.unwrap_or(before.kind);
        let config = patch.config.clone().unwrap_or(before.config.clone());
        self.check_field_relations(&kind, &config, &before.database_id)
            .await?;
        validate_field_config(kind, &config)?;
        let field = repos::databases::update_field(&self.db, id, &patch).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "db_field",
            Some(id),
            &format!("Campo actualizado: {}", field.name),
            actor,
            None,
        )
        .await?;
        Ok(field)
    }

    /// Soft-delete: los valores se conservan en values_json (no se pierden
    /// datos); las vistas ignoran el campo borrado al renderizar.
    pub async fn delete_field(&self, id: &str, actor: &str) -> Result<()> {
        let field = repos::databases::get_field(&self.db, id).await?;
        repos::databases::soft_delete_field(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "db_field",
            Some(id),
            &format!("Campo eliminado: {} (valores conservados)", field.name),
            actor,
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn reorder_fields(
        &self,
        database_id: &str,
        ordered_ids: Vec<String>,
    ) -> Result<Vec<DbField>> {
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let current: HashSet<&str> = fields.iter().map(|f| f.id.as_str()).collect();
        let given: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
        if current != given {
            return Err(NexusError::validation(
                "field_ids debe contener exactamente los campos activos de la base",
            ));
        }
        repos::databases::reorder_fields(&self.db, database_id, &ordered_ids).await?;
        repos::databases::list_fields(&self.db, database_id).await
    }

    // -------------------------------------------------------------- registros

    /// Carga registros y enriquece: fórmulas calculadas + títulos de relación
    /// (ids huérfanos filtrados aquí, además de la limpieza al borrar).
    async fn enrich(&self, fields: &[DbField], records: Vec<DbRecord>) -> Result<Vec<DbRecord>> {
        let mut title_cache: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut out = Vec::with_capacity(records.len());
        for mut rec in records {
            for f in fields {
                match f.kind {
                    DbFieldKind::Formula => {
                        let mut env: HashMap<String, formula::FVal> = HashMap::new();
                        for other in fields {
                            if other.kind != DbFieldKind::Formula {
                                env.insert(
                                    other.name.clone(),
                                    formula::FVal::from_json(
                                        rec.values.get(&other.id).unwrap_or(&Value::Null),
                                    ),
                                );
                            }
                        }
                        let expr = f.config.expression.as_deref().unwrap_or("");
                        match formula::evaluate(expr, &env) {
                            Ok(v) => {
                                rec.computed.insert(f.id.clone(), v);
                            }
                            Err(e) => {
                                rec.computed.insert(f.id.clone(), Value::Null);
                                rec.formula_errors.insert(f.id.clone(), json!(e));
                            }
                        }
                    }
                    DbFieldKind::Relation => {
                        let Some(target) = &f.config.target_database_id else {
                            continue;
                        };
                        let ids: Vec<String> = rec
                            .values
                            .get(&f.id)
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if ids.is_empty() {
                            continue;
                        }
                        if !title_cache.contains_key(target) {
                            let hint = fields_title_hint(&self.db, target).await.ok().flatten();
                            let all = repos::databases::all_records(&self.db, target).await?;
                            let map: HashMap<String, String> = all
                                .into_iter()
                                .map(|r| (r.id.clone(), record_title(&r.values, hint.as_deref())))
                                .collect();
                            title_cache.insert(target.clone(), map);
                        }
                        let titles = &title_cache[target];
                        let chips: Vec<Value> = ids
                            .iter()
                            .filter_map(|id| titles.get(id).map(|t| json!({"id": id, "title": t})))
                            .collect();
                        rec.relation_titles
                            .insert(f.id.clone(), Value::Array(chips));
                    }
                    _ => {}
                }
            }
            out.push(rec);
        }
        Ok(out)
    }

    fn has_formula_sort(q: &RecordQuery, fields: &[DbField]) -> bool {
        q.sorts.iter().any(|s| {
            fields
                .iter()
                .any(|f| f.id == s.field_id && f.kind == DbFieldKind::Formula)
        })
    }

    pub async fn query_records(&self, database_id: &str, q: RecordQuery) -> Result<RecordPage> {
        repos::databases::get_database(&self.db, database_id).await?;
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let formula_sort = Self::has_formula_sort(&q, &fields);
        let queried =
            repos::databases::query_records(&self.db, database_id, &fields, &q, formula_sort)
                .await?;
        let mut records = self.enrich(&fields, queried.records).await?;
        let total = queried.total;
        if formula_sort {
            sort_in_memory(&mut records, &fields, &q.sorts);
            let offset = q.offset.unwrap_or(0).max(0) as usize;
            let limit = q.limit.unwrap_or(500).clamp(1, 5000) as usize;
            records = records.into_iter().skip(offset).take(limit).collect();
        }
        Ok(RecordPage { records, total })
    }

    pub async fn get_record(&self, database_id: &str, record_id: &str) -> Result<DbRecord> {
        let rec = repos::databases::get_record(&self.db, record_id).await?;
        if rec.database_id != database_id {
            return Err(NexusError::not_found("registro", record_id));
        }
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        Ok(self.enrich(&fields, vec![rec]).await?.remove(0))
    }

    /// Valida un mapa completo de valores (crear/import). Los campos fórmula
    /// se rechazan; los desconocidos también.
    async fn validate_values(
        &self,
        fields: &[DbField],
        values: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        let map: HashMap<&str, &DbField> = fields.iter().map(|f| (f.id.as_str(), f)).collect();
        let mut out = Map::new();
        for (fid, v) in values {
            let field = map
                .get(fid.as_str())
                .ok_or_else(|| NexusError::validation(format!("campo desconocido: {fid}")))?;
            let normalized = validate_value(field, v)?;
            if !normalized.is_null() {
                out.insert(fid.clone(), normalized);
            }
        }
        // Valida ids de relación contra registros vivos.
        for f in fields {
            if f.kind != DbFieldKind::Relation {
                continue;
            }
            let Some(Value::Array(arr)) = out.get(&f.id) else {
                continue;
            };
            let Some(target) = &f.config.target_database_id else {
                continue;
            };
            let live: HashSet<String> = repos::databases::live_record_ids(&self.db, target)
                .await?
                .into_iter()
                .collect();
            for v in arr {
                let id = v.as_str().unwrap_or_default();
                if !live.contains(id) {
                    return Err(NexusError::validation(format!(
                        "campo «{}»: el registro relacionado {id} no existe en la base destino",
                        f.name
                    )));
                }
            }
        }
        // Verifica que los adjuntos existan.
        for f in fields {
            if !matches!(f.kind, DbFieldKind::Image | DbFieldKind::File) {
                continue;
            }
            if let Some(Value::String(fid)) = out.get(&f.id) {
                repos::files::get(&self.db, fid).await.map_err(|_| {
                    NexusError::validation(format!(
                        "campo «{}»: el archivo {fid} no existe",
                        f.name
                    ))
                })?;
            }
        }
        Ok(out)
    }

    pub async fn create_record(
        &self,
        database_id: &str,
        data: CreateDbRecord,
        actor: &str,
    ) -> Result<DbRecord> {
        repos::databases::get_database(&self.db, database_id).await?;
        if repos::databases::count_records(&self.db, database_id).await? >= MAX_RECORDS {
            return Err(NexusError::LimitExceeded(format!(
                "máximo {MAX_RECORDS} registros por base"
            )));
        }
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let values = self.validate_values(&fields, &data.values).await?;
        let id = new_id();
        repos::databases::create_record(&self.db, &id, database_id, &values).await?;
        let rec = self.get_record(database_id, &id).await?;
        repos::activity::record(
            &self.db,
            "created",
            "db_record",
            Some(&id),
            "Registro creado en base de datos",
            actor,
            None,
        )
        .await?;
        Ok(rec)
    }

    pub async fn update_record(
        &self,
        database_id: &str,
        record_id: &str,
        patch: UpdateDbRecord,
        actor: &str,
    ) -> Result<DbRecord> {
        let row = repos::databases::get_record_row(&self.db, record_id).await?;
        if row.database_id != database_id {
            return Err(NexusError::not_found("registro", record_id));
        }
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let mut values = repos::databases::record_from_row(row)?.values;
        let patch_values = self.validate_values(&fields, &patch.values).await?;
        for (fid, v) in &patch.values {
            if v.is_null() {
                values.remove(fid);
            } else if let Some(nv) = patch_values.get(fid) {
                values.insert(fid.clone(), nv.clone());
            }
        }
        // Revalida relaciones completas tras el merge (cardinalidad).
        let map: HashMap<&str, &DbField> = fields.iter().map(|f| (f.id.as_str(), f)).collect();
        let revalidated: Vec<(String, Value)> = values
            .iter()
            .filter_map(|(fid, v)| {
                map.get(fid.as_str())
                    .map(|f| validate_value(f, v).map(|nv| (fid.clone(), nv)))
            })
            .collect::<Result<Vec<_>>>()?;
        for (fid, nv) in revalidated {
            values.insert(fid, nv);
        }
        let mut conn = self.db.pool.acquire().await.map_err(nexus_db::db_err)?;
        repos::databases::write_values(&mut conn, record_id, &values).await?;
        drop(conn);
        let rec = self.get_record(database_id, record_id).await?;
        repos::activity::record(
            &self.db,
            "updated",
            "db_record",
            Some(record_id),
            "Registro actualizado",
            actor,
            None,
        )
        .await?;
        Ok(rec)
    }

    /// Borrado (soft) + limpieza de referencias entrantes en otras bases.
    pub async fn delete_records(
        &self,
        database_id: &str,
        ids: Vec<String>,
        actor: &str,
    ) -> Result<i64> {
        repos::databases::get_database(&self.db, database_id).await?;
        if ids.is_empty() {
            return Ok(0);
        }
        if ids.len() > 1000 {
            return Err(NexusError::LimitExceeded(
                "máximo 1000 registros por borrado masivo".into(),
            ));
        }
        for id in &ids {
            let rec = repos::databases::get_record(&self.db, id).await?;
            if rec.database_id != database_id {
                return Err(NexusError::validation(format!(
                    "el registro {id} no pertenece a esta base"
                )));
            }
        }
        let relation_fields =
            repos::databases::relation_fields_targeting(&self.db, database_id).await?;
        let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let mut tx = self.db.pool.begin().await.map_err(nexus_db::db_err)?;
        for field in &relation_fields {
            for id in &ids {
                let refs = repos::databases::records_referencing_ex(&mut tx, field, id).await?;
                for (ref_id, mut values) in refs {
                    if field.database_id == database_id && id_set.contains(ref_id.as_str()) {
                        continue;
                    }
                    if let Some(Value::Array(arr)) = values.get_mut(&field.id) {
                        arr.retain(|v| !id_set.contains(v.as_str().unwrap_or_default()));
                    }
                    repos::databases::write_values(&mut tx, &ref_id, &values).await?;
                }
            }
        }
        tx.commit().await.map_err(nexus_db::db_err)?;
        repos::databases::soft_delete_records(&self.db, &ids).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "db_record",
            ids.first().map(String::as_str),
            &format!("{} registro(s) eliminado(s)", ids.len()),
            actor,
            None,
        )
        .await?;
        Ok(ids.len() as i64)
    }

    pub async fn record_preview(
        &self,
        database_id: &str,
        record_id: &str,
    ) -> Result<RecordPreview> {
        let database = repos::databases::get_database(&self.db, database_id).await?;
        let rec = repos::databases::get_record(&self.db, record_id).await?;
        if rec.database_id != database_id {
            return Err(NexusError::not_found("registro", record_id));
        }
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let title = record_title(&rec.values, first_text_field_id(&fields).as_deref());
        Ok(RecordPreview {
            record_id: rec.id,
            database_id: database.id,
            database_name: database.name,
            title,
            icon: database.icon,
            color: database.color,
        })
    }

    // ------------------------------------------------------------------ vistas

    pub async fn create_view(&self, database_id: &str, mut data: CreateDbView) -> Result<DbView> {
        repos::databases::get_database(&self.db, database_id).await?;
        data.name = validate::title(&data.name, "el nombre de la vista")?;
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        self.validate_view_config(data.kind, &data.config, &fields)?;
        repos::databases::create_view(&self.db, &new_id(), database_id, &data).await
    }

    fn validate_view_config(
        &self,
        kind: DbViewKind,
        config: &DbViewConfig,
        fields: &[DbField],
    ) -> Result<()> {
        let find = |id: &str| fields.iter().find(|f| f.id == id);
        if let Some(g) = &config.group_field_id {
            let f =
                find(g).ok_or_else(|| NexusError::validation("campo de agrupación inexistente"))?;
            if kind == DbViewKind::Kanban && f.kind != DbFieldKind::Select {
                return Err(NexusError::validation(
                    "el kanban agrupa por un campo select",
                ));
            }
        }
        if let Some(d) = &config.date_field_id {
            let f = find(d).ok_or_else(|| NexusError::validation("campo de fecha inexistente"))?;
            if !matches!(f.kind, DbFieldKind::Date | DbFieldKind::Datetime) {
                return Err(NexusError::validation(
                    "el calendario necesita un campo date o datetime",
                ));
            }
        }
        if let Some(i) = &config.image_field_id {
            let f = find(i).ok_or_else(|| NexusError::validation("campo de imagen inexistente"))?;
            if f.kind != DbFieldKind::Image {
                return Err(NexusError::validation("la galería necesita un campo image"));
            }
        }
        for id in config
            .visible_field_ids
            .iter()
            .chain(config.card_field_ids.iter())
        {
            if find(id).is_none() {
                return Err(NexusError::validation(format!(
                    "campo de vista inexistente: {id}"
                )));
            }
        }
        Ok(())
    }

    pub async fn update_view(&self, id: &str, patch: UpdateDbView) -> Result<DbView> {
        let before = repos::databases::get_view(&self.db, id).await?;
        let kind = patch.kind.unwrap_or(before.kind);
        let config = patch.config.clone().unwrap_or(before.config.clone());
        let fields = repos::databases::list_fields(&self.db, &before.database_id).await?;
        self.validate_view_config(kind, &config, &fields)?;
        let mut patch = patch;
        if let Some(n) = &patch.name {
            patch.name = Some(validate::title(n, "el nombre de la vista")?);
        }
        repos::databases::update_view(&self.db, id, &patch).await
    }

    pub async fn delete_view(&self, id: &str) -> Result<()> {
        repos::databases::delete_view(&self.db, id).await
    }

    // ------------------------------------------------------------ import/export

    pub fn parse_import(format: &str, content: &str) -> Result<ImportPreview> {
        if content.len() > MAX_IMPORT_BYTES {
            return Err(NexusError::LimitExceeded(
                "el archivo supera el máximo de 5 MB".into(),
            ));
        }
        let (columns, rows) = match format {
            "csv" => {
                let mut rdr = csv::ReaderBuilder::new()
                    .flexible(true)
                    .from_reader(content.as_bytes());
                let columns: Vec<String> = rdr
                    .headers()
                    .map_err(|e| NexusError::validation(format!("CSV inválido: {e}")))?
                    .iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                if columns.is_empty() || columns.iter().all(|c| c.is_empty()) {
                    return Err(NexusError::validation("el CSV no tiene cabeceras"));
                }
                let mut rows = Vec::new();
                for (i, r) in rdr.records().enumerate() {
                    if i >= MAX_IMPORT_ROWS {
                        return Err(NexusError::LimitExceeded(format!(
                            "máximo {MAX_IMPORT_ROWS} filas por importación"
                        )));
                    }
                    let r =
                        r.map_err(|e| NexusError::validation(format!("fila {}: {e}", i + 1)))?;
                    let mut row: Vec<String> = r.iter().map(|s| s.to_string()).collect();
                    row.resize(columns.len(), String::new());
                    row.truncate(columns.len());
                    rows.push(row);
                }
                (columns, rows)
            }
            "json" => {
                let v: Value = serde_json::from_str(content)
                    .map_err(|e| NexusError::validation(format!("JSON inválido: {e}")))?;
                let arr = v.as_array().ok_or_else(|| {
                    NexusError::validation("el JSON debe ser un array de objetos")
                })?;
                if arr.len() > MAX_IMPORT_ROWS {
                    return Err(NexusError::LimitExceeded(format!(
                        "máximo {MAX_IMPORT_ROWS} filas por importación"
                    )));
                }
                let mut columns: Vec<String> = Vec::new();
                // serde_json ordena las claves alfabéticamente; escaneamos el
                // texto para conservar el orden original de las columnas.
                for keys in json_object_key_order(content) {
                    for k in keys {
                        if !columns.contains(&k) {
                            columns.push(k);
                        }
                    }
                }
                for item in arr {
                    let obj = item.as_object().ok_or_else(|| {
                        NexusError::validation("cada elemento del JSON debe ser un objeto")
                    })?;
                    for k in obj.keys() {
                        if !columns.contains(k) {
                            columns.push(k.clone());
                        }
                    }
                }
                let rows = arr
                    .iter()
                    .map(|item| {
                        columns
                            .iter()
                            .map(|c| match item.get(c) {
                                None | Some(Value::Null) => String::new(),
                                Some(Value::String(s)) => s.clone(),
                                Some(other) => other.to_string(),
                            })
                            .collect()
                    })
                    .collect();
                (columns, rows)
            }
            other => {
                return Err(NexusError::validation(format!(
                    "formato no soportado: {other} (usa csv o json)"
                )))
            }
        };
        let total_rows = rows.len() as i64;
        Ok(ImportPreview {
            columns,
            rows: rows.into_iter().take(50).collect(),
            total_rows,
            format: format.to_string(),
        })
    }

    pub async fn import(&self, req: ImportRequest, actor: &str) -> Result<ImportResult> {
        repos::databases::get_database(&self.db, &req.database_id).await?;
        let preview = Self::parse_import(&req.format, &req.content)?;
        if req.mappings.is_empty() {
            return Err(NexusError::validation("mapea al menos una columna"));
        }
        let mut fields = repos::databases::list_fields(&self.db, &req.database_id).await?;
        let mut created_fields = Vec::new();

        // Crea los campos nuevos solicitados (dentro de la misma operación).
        let mut resolved: Vec<(usize, String)> = Vec::new(); // (índice columna, field_id)
        for m in &req.mappings {
            let col_idx = preview
                .columns
                .iter()
                .position(|c| c == &m.column)
                .ok_or_else(|| {
                    NexusError::validation(format!("columna desconocida: {}", m.column))
                })?;
            let field_id = match (&m.field_id, &m.new_field) {
                (Some(fid), _) => {
                    if !fields.iter().any(|f| &f.id == fid) {
                        return Err(NexusError::validation(format!(
                            "campo destino inexistente: {fid}"
                        )));
                    }
                    fid.clone()
                }
                (None, Some(nf)) => {
                    if let Some(existing) = fields
                        .iter()
                        .find(|f| f.name.eq_ignore_ascii_case(&nf.name))
                    {
                        existing.id.clone()
                    } else {
                        let f = self
                            .create_field(&req.database_id, nf.clone(), actor)
                            .await?;
                        created_fields.push(f.clone());
                        fields.push(f.clone());
                        f.id
                    }
                }
                (None, None) => {
                    return Err(NexusError::validation(format!(
                        "el mapeo de «{}» necesita field_id o new_field",
                        m.column
                    )))
                }
            };
            resolved.push((col_idx, field_id));
        }

        // Vuelve a parsear todas las filas (preview solo trae 50).
        let all_rows = Self::all_rows(&req.format, &req.content, &preview.columns)?;
        let count = repos::databases::count_records(&self.db, &req.database_id).await?;
        if count + all_rows.len() as i64 > MAX_RECORDS {
            return Err(NexusError::LimitExceeded(format!(
                "máximo {MAX_RECORDS} registros por base"
            )));
        }

        let mut errors: Vec<ImportRowError> = Vec::new();
        let mut valid: Vec<Map<String, Value>> = Vec::new();
        for (i, row) in all_rows.iter().enumerate() {
            let mut values = Map::new();
            let mut row_err: Option<String> = None;
            for (col_idx, fid) in &resolved {
                let raw = row.get(*col_idx).map(String::as_str).unwrap_or("").trim();
                if raw.is_empty() {
                    continue;
                }
                let Some(field) = fields.iter().find(|f| &f.id == fid) else {
                    continue;
                };
                match coerce_import_value(field, raw) {
                    Ok(v) => {
                        values.insert(fid.clone(), v);
                    }
                    Err(e) => {
                        row_err = Some(e.to_string());
                        break;
                    }
                }
            }
            if let Some(e) = row_err {
                errors.push(ImportRowError {
                    row: (i + 1) as i64,
                    message: e,
                });
                continue;
            }
            match self.validate_values(&fields, &values).await {
                Ok(v) => valid.push(v),
                Err(e) => errors.push(ImportRowError {
                    row: (i + 1) as i64,
                    message: e.to_string(),
                }),
            }
        }

        if !errors.is_empty() && !req.skip_invalid {
            return Err(NexusError::validation(format!(
                "{} fila(s) con errores; corrige el archivo o activa «omitir filas inválidas». Primer error (fila {}): {}",
                errors.len(),
                errors[0].row,
                errors[0].message
            )));
        }

        let mut tx = self.db.pool.begin().await.map_err(nexus_db::db_err)?;
        for values in &valid {
            repos::databases::insert_record(&mut tx, &new_id(), &req.database_id, values).await?;
        }
        tx.commit().await.map_err(nexus_db::db_err)?;

        repos::activity::record(
            &self.db,
            "created",
            "database",
            Some(&req.database_id),
            &format!(
                "Importados {} registro(s) ({})",
                valid.len(),
                req.format.to_uppercase()
            ),
            actor,
            None,
        )
        .await?;
        Ok(ImportResult {
            imported: valid.len() as i64,
            skipped: errors.len() as i64,
            errors,
            created_fields,
        })
    }

    fn all_rows(format: &str, content: &str, columns: &[String]) -> Result<Vec<Vec<String>>> {
        match format {
            "csv" => {
                let mut rdr = csv::ReaderBuilder::new()
                    .flexible(true)
                    .from_reader(content.as_bytes());
                let mut rows = Vec::new();
                for r in rdr.records() {
                    let r = r.map_err(|e| NexusError::validation(format!("CSV: {e}")))?;
                    let mut row: Vec<String> = r.iter().map(|s| s.to_string()).collect();
                    row.resize(columns.len(), String::new());
                    rows.push(row);
                }
                Ok(rows)
            }
            _ => {
                let v: Value = serde_json::from_str(content)
                    .map_err(|e| NexusError::validation(format!("JSON inválido: {e}")))?;
                Ok(v.as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .map(|item| {
                        columns
                            .iter()
                            .map(|c| match item.get(c) {
                                None | Some(Value::Null) => String::new(),
                                Some(Value::String(s)) => s.clone(),
                                Some(other) => other.to_string(),
                            })
                            .collect()
                    })
                    .collect())
            }
        }
    }

    pub async fn export(
        &self,
        database_id: &str,
        format: &str,
        view_id: Option<&str>,
    ) -> Result<ExportResult> {
        let database = repos::databases::get_database(&self.db, database_id).await?;
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let mut q = RecordQuery {
            limit: Some(5000),
            ..Default::default()
        };
        if let Some(vid) = view_id {
            let view = repos::databases::get_view(&self.db, vid).await?;
            if view.database_id != database_id {
                return Err(NexusError::validation("la vista no pertenece a esta base"));
            }
            q = repos::databases::view_query(&view.config);
        }
        let page = self.query_records(database_id, q).await?;
        let slug: String = database
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let slug = slug.trim_matches('-').to_string();
        match format {
            "csv" => {
                let mut wtr = csv::Writer::from_writer(Vec::new());
                let header: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                wtr.write_record(&header)
                    .map_err(|e| NexusError::Internal(e.to_string()))?;
                for rec in &page.records {
                    let row: Vec<String> = fields.iter().map(|f| export_cell(f, rec)).collect();
                    wtr.write_record(&row)
                        .map_err(|e| NexusError::Internal(e.to_string()))?;
                }
                let bytes = wtr
                    .into_inner()
                    .map_err(|e| NexusError::Internal(e.to_string()))?;
                Ok(ExportResult {
                    filename: format!("{slug}.csv"),
                    mime: "text/csv".into(),
                    content: String::from_utf8(bytes)
                        .map_err(|e| NexusError::Internal(e.to_string()))?,
                    row_count: page.records.len() as i64,
                })
            }
            "json" => {
                let mut arr = Vec::new();
                for rec in &page.records {
                    let mut obj = Map::new();
                    obj.insert("id".into(), json!(rec.id));
                    for f in &fields {
                        let v = if f.kind == DbFieldKind::Formula {
                            rec.computed.get(&f.id).cloned().unwrap_or(Value::Null)
                        } else {
                            rec.values.get(&f.id).cloned().unwrap_or(Value::Null)
                        };
                        obj.insert(f.name.clone(), v);
                    }
                    obj.insert("created_at".into(), json!(rec.created_at));
                    arr.push(Value::Object(obj));
                }
                Ok(ExportResult {
                    filename: format!("{slug}.json"),
                    mime: "application/json".into(),
                    content: serde_json::to_string_pretty(&arr)
                        .map_err(|e| NexusError::Internal(e.to_string()))?,
                    row_count: page.records.len() as i64,
                })
            }
            other => Err(NexusError::validation(format!(
                "formato no soportado: {other} (usa csv o json)"
            ))),
        }
    }

    // -------------------------------------------------------------- plantillas

    pub fn templates() -> Vec<DbTemplate> {
        templates::all()
    }

    pub async fn install_template(&self, template_id: &str, actor: &str) -> Result<DatabaseDetail> {
        let t = templates::all()
            .into_iter()
            .find(|t| t.id == template_id)
            .ok_or_else(|| NexusError::not_found("plantilla", template_id))?;
        let database = self
            .create(
                CreateDatabase {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    icon: t.icon.clone(),
                    color: t.color.clone(),
                },
                actor,
            )
            .await?;
        let mut by_name: HashMap<String, DbField> = HashMap::new();
        for tf in &t.fields {
            let f = self
                .create_field(
                    &database.id,
                    CreateDbField {
                        name: tf.name.clone(),
                        kind: tf.kind,
                        config: tf.config.clone(),
                        position: None,
                    },
                    actor,
                )
                .await?;
            by_name.insert(tf.name.clone(), f);
        }
        for sample in &t.sample_records {
            let mut values = Map::new();
            for (name, v) in sample {
                if let Some(f) = by_name.get(name) {
                    values.insert(f.id.clone(), v.clone());
                }
            }
            self.create_record(&database.id, CreateDbRecord { values }, actor)
                .await?;
        }
        // La vista de tabla por defecto ya existe: la primera plantilla-table
        // la actualiza en vez de duplicar; el resto se crean.
        let existing_views = repos::databases::list_views(&self.db, &database.id).await?;
        for (i, cv) in t.views.iter().enumerate() {
            let mut config = cv.config.clone();
            remap_view_config(&mut config, &by_name, &t.fields);
            if i == 0 && cv.kind == DbViewKind::Table {
                if let Some(v) = existing_views.first() {
                    self.update_view(
                        &v.id,
                        UpdateDbView {
                            name: Some(cv.name.clone()),
                            kind: Some(cv.kind),
                            config: Some(config),
                        },
                    )
                    .await?;
                    continue;
                }
            }
            self.create_view(
                &database.id,
                CreateDbView {
                    name: cv.name.clone(),
                    kind: cv.kind,
                    config,
                },
            )
            .await?;
        }
        self.get(&database.id).await
    }

    // ---------------------------------------------------------------- IA (solo propuestas)

    /// Convierte la respuesta JSON de la IA en propuestas tipadas y saneadas.
    /// Nunca persiste nada: el frontend revisa y confirma.
    pub fn parse_ai_structure(v: &Value) -> Result<AiStructureProposal> {
        let name = v
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| NexusError::Ai("la IA no propuso un nombre de base".into()))?;
        let fields = parse_ai_fields(v.get("fields").and_then(Value::as_array))?;
        let icon = v.get("icon").and_then(Value::as_str).unwrap_or("database");
        let icon =
            if icon.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && icon.len() <= 50 {
                icon.to_string()
            } else {
                "database".into()
            };
        let color = v.get("color").and_then(Value::as_str).unwrap_or("blue");
        let color = if DATABASE_COLORS.contains(&color) {
            color.to_string()
        } else {
            "blue".into()
        };
        Ok(AiStructureProposal {
            name: validate::title(name, "el nombre propuesto")
                .map_err(|e| NexusError::Ai(e.to_string()))?,
            description: v
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(500)
                .collect(),
            icon,
            color,
            fields,
        })
    }

    /// Detecta duplicados: heurística local (títulos iguales/similares) y,
    /// si la IA responde, sus pares adicionales. Devuelve solo sugerencias.
    pub async fn find_duplicates(
        &self,
        database_id: &str,
        ai_pairs: Option<Value>,
    ) -> Result<Vec<DuplicatePair>> {
        let fields = repos::databases::list_fields(&self.db, database_id).await?;
        let records = repos::databases::all_records(&self.db, database_id).await?;
        let title_field = first_text_field_id(&fields);
        let titles: Vec<(String, String)> = records
            .iter()
            .map(|r| {
                (
                    r.id.clone(),
                    record_title(&r.values, title_field.as_deref()),
                )
            })
            .collect();
        let mut pairs: Vec<DuplicatePair> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for i in 0..titles.len() {
            for j in (i + 1)..titles.len() {
                let a = titles[i].1.to_lowercase();
                let b = titles[j].1.to_lowercase();
                if a == "(sin título)" || b == "(sin título)" {
                    continue;
                }
                let score = if a == b {
                    1.0
                } else if a.contains(&b) || b.contains(&a) {
                    0.8
                } else {
                    let d = levenshtein(&a, &b);
                    let max_len = a.len().max(b.len()).max(1);
                    let s = 1.0 - (d as f64 / max_len as f64);
                    if s >= 0.85 {
                        s
                    } else {
                        0.0
                    }
                };
                if score >= 0.8 {
                    seen.insert((titles[i].0.clone(), titles[j].0.clone()));
                    pairs.push(DuplicatePair {
                        record_a: titles[i].0.clone(),
                        record_b: titles[j].0.clone(),
                        title_a: titles[i].1.clone(),
                        title_b: titles[j].1.clone(),
                        score,
                        reason: if score == 1.0 {
                            "Título idéntico".into()
                        } else {
                            "Título muy similar".into()
                        },
                    });
                }
            }
        }
        if let Some(Value::Array(arr)) = ai_pairs {
            let ids: HashSet<&str> = titles.iter().map(|(id, _)| id.as_str()).collect();
            let title_map: HashMap<&str, &str> = titles
                .iter()
                .map(|(id, t)| (id.as_str(), t.as_str()))
                .collect();
            for p in arr {
                let a = p.get("record_a").and_then(Value::as_str).unwrap_or("");
                let b = p.get("record_b").and_then(Value::as_str).unwrap_or("");
                if a == b || !ids.contains(a) || !ids.contains(b) {
                    continue;
                }
                let key = if a < b {
                    (a.to_string(), b.to_string())
                } else {
                    (b.to_string(), a.to_string())
                };
                if !seen.insert(key) {
                    continue;
                }
                pairs.push(DuplicatePair {
                    record_a: a.to_string(),
                    record_b: b.to_string(),
                    title_a: title_map[a].to_string(),
                    title_b: title_map[b].to_string(),
                    score: p
                        .get("score")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.7)
                        .clamp(0.0, 1.0),
                    reason: p
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("Sugerido por la IA")
                        .chars()
                        .take(200)
                        .collect(),
                });
            }
        }
        pairs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pairs.truncate(50);
        Ok(pairs)
    }
}

// ===========================================================================
// Helpers libres
// ===========================================================================

fn parse_ai_fields(arr: Option<&Vec<Value>>) -> Result<Vec<AiFieldProposal>> {
    let arr = arr.ok_or_else(|| NexusError::Ai("la IA no propuso campos".into()))?;
    let mut out = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    for f in arr.iter().take(40) {
        let Some(name) = f.get("name").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if name.is_empty() || name.chars().count() > 100 || !names.insert(name.to_lowercase()) {
            continue;
        }
        let Some(kind) = f
            .get("kind")
            .and_then(Value::as_str)
            .and_then(DbFieldKind::parse)
        else {
            continue;
        };
        // relation/formula de la IA se degradan a text: requieren configuración
        // explícita del usuario (destino/expresión) que la IA no debe inventar.
        let (kind, config) = match kind {
            DbFieldKind::Relation
            | DbFieldKind::Formula
            | DbFieldKind::Image
            | DbFieldKind::File => (DbFieldKind::Text, DbFieldConfig::default()),
            DbFieldKind::Select | DbFieldKind::Multiselect => {
                let options: Vec<DbSelectOption> = f
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|o| o.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .take(30)
                            .enumerate()
                            .map(|(i, label)| DbSelectOption {
                                id: format!("opt{}", i + 1),
                                label: label.trim().chars().take(100).collect(),
                                color: OPTION_COLORS[i % OPTION_COLORS.len()].to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if options.is_empty() {
                    (DbFieldKind::Text, DbFieldConfig::default())
                } else {
                    (
                        kind,
                        DbFieldConfig {
                            options,
                            ..Default::default()
                        },
                    )
                }
            }
            other => (other, DbFieldConfig::default()),
        };
        out.push(AiFieldProposal {
            name: name.to_string(),
            kind,
            config,
            reason: f
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(200)
                .collect(),
        });
    }
    if out.is_empty() {
        return Err(NexusError::Ai(
            "la IA no propuso ningún campo válido".into(),
        ));
    }
    Ok(out)
}

fn extract_refs(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c2 in chars.by_ref() {
                if c2 == '}' {
                    break;
                }
                name.push(c2);
            }
            let name = name.trim().to_string();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// Orden de aparición de las claves de los objetos de nivel superior de un
/// array JSON (best-effort; el contenido ya fue validado con serde_json).
fn json_object_key_order(content: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut depth = 0i64;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    let mut string_is_key = false;
    let mut keys: Vec<String> = Vec::new();
    let mut expect_key = false;
    for c in content.chars() {
        if in_string {
            if escaped {
                escaped = false;
                current.push(c);
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                if string_is_key {
                    keys.push(current.clone());
                }
                current.clear();
            } else {
                current.push(c);
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                string_is_key = depth == 2 && expect_key;
                current.clear();
            }
            '{' => {
                depth += 1;
                if depth == 2 {
                    keys = Vec::new();
                    expect_key = true;
                }
            }
            '}' => {
                if depth == 2 {
                    out.push(std::mem::take(&mut keys));
                }
                depth -= 1;
            }
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 2 => expect_key = true,
            ':' if depth == 2 => expect_key = false,
            _ => {}
        }
    }
    out
}

async fn fields_title_hint(db: &Db, database_id: &str) -> Result<Option<String>> {
    let fields = repos::databases::list_fields(db, database_id).await?;
    Ok(first_text_field_id(&fields))
}

fn first_text_field_id(fields: &[DbField]) -> Option<String> {
    fields
        .iter()
        .find(|f| f.kind == DbFieldKind::Text)
        .or_else(|| fields.iter().find(|f| f.kind == DbFieldKind::LongText))
        .map(|f| f.id.clone())
}

fn record_title(values: &Map<String, Value>, title_field: Option<&str>) -> String {
    if let Some(fid) = title_field {
        if let Some(s) = values
            .get(fid)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return s.to_string();
        }
    }
    values
        .values()
        .find_map(|v| v.as_str().filter(|s| !s.trim().is_empty()))
        .unwrap_or("(sin título)")
        .to_string()
}

fn export_cell(field: &DbField, rec: &DbRecord) -> String {
    let v = if field.kind == DbFieldKind::Formula {
        rec.computed.get(&field.id).unwrap_or(&Value::Null)
    } else {
        rec.values.get(&field.id).unwrap_or(&Value::Null)
    };
    match field.kind {
        DbFieldKind::Select => v
            .as_str()
            .and_then(|id| option_exists(&field.config, id))
            .map(|o| o.label.clone())
            .unwrap_or_default(),
        DbFieldKind::Multiselect => v
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(|id| {
                        option_exists(&field.config, id)
                            .map(|o| o.label.clone())
                            .unwrap_or_else(|| id.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default(),
        DbFieldKind::Relation => rec
            .relation_titles
            .get(&field.id)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("title").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default(),
        _ => match v {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        },
    }
}

/// Convierte una celda de texto de import al valor JSON del tipo destino.
fn coerce_import_value(field: &DbField, raw: &str) -> Result<Value> {
    let err = |msg: String| NexusError::validation(format!("campo «{}»: {msg}", field.name));
    match field.kind {
        DbFieldKind::Select | DbFieldKind::Multiselect => {
            let parts: Vec<&str> = if field.kind == DbFieldKind::Multiselect {
                raw.split([';', '|'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                vec![raw]
            };
            let mut ids = Vec::new();
            for p in parts {
                let opt = field
                    .config
                    .options
                    .iter()
                    .find(|o| o.label.eq_ignore_ascii_case(p) || o.id == p)
                    .ok_or_else(|| err(format!("la opción «{p}» no existe")))?;
                ids.push(json!(opt.id));
            }
            if field.kind == DbFieldKind::Select {
                Ok(ids.into_iter().next().unwrap_or(Value::Null))
            } else {
                Ok(Value::Array(ids))
            }
        }
        DbFieldKind::Checkbox => Ok(json!(matches!(
            raw.to_lowercase().as_str(),
            "true" | "1" | "sí" | "si" | "yes" | "x"
        ))),
        DbFieldKind::Integer | DbFieldKind::Decimal | DbFieldKind::Rating => {
            validate_value(field, &json!(raw))
        }
        DbFieldKind::Relation => Err(err(
            "las relaciones no se importan desde texto (necesitan ids de registro)".into(),
        )),
        DbFieldKind::Formula => Err(err("los campos fórmula no se importan".into())),
        DbFieldKind::Image | DbFieldKind::File => Err(err(
            "los adjuntos no se importan desde texto (sube el archivo primero)".into(),
        )),
        _ => validate_value(field, &json!(raw)),
    }
}

fn sort_in_memory(records: &mut [DbRecord], fields: &[DbField], sorts: &[nexus_core::DbSort]) {
    let map: HashMap<&str, &DbField> = fields.iter().map(|f| (f.id.as_str(), f)).collect();
    records.sort_by(|a, b| {
        for s in sorts {
            let Some(f) = map.get(s.field_id.as_str()) else {
                continue;
            };
            let va = if f.kind == DbFieldKind::Formula {
                a.computed.get(&f.id)
            } else {
                a.values.get(&f.id)
            };
            let vb = if f.kind == DbFieldKind::Formula {
                b.computed.get(&f.id)
            } else {
                b.values.get(&f.id)
            };
            let ord = compare_values(va, vb);
            let ord = if s.dir == DbSortDir::Desc {
                ord.reverse()
            } else {
                ord
            };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        a.position.cmp(&b.position)
    });
}

fn compare_values(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let empty = |v: Option<&Value>| match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        _ => false,
    };
    match (empty(a), empty(b)) {
        (true, true) => return Equal,
        (true, false) => return Greater, // vacíos al final
        (false, true) => return Less,
        _ => {}
    }
    let a = a.unwrap();
    let b = b.unwrap();
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        return x.partial_cmp(&y).unwrap_or(Equal);
    }
    if let (Some(x), Some(y)) = (a.as_bool(), b.as_bool()) {
        return x.cmp(&y);
    }
    let text = |v: &Value| match v {
        Value::String(s) => s.to_lowercase(),
        other => other.to_string().to_lowercase(),
    };
    text(a).cmp(&text(b))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Reescribe una config de vista de plantilla usando nombres simbólicos
/// ("$Nombre del campo") a ids reales.
fn remap_view_config(
    config: &mut DbViewConfig,
    by_name: &HashMap<String, DbField>,
    fields: &[DbTemplateField],
) {
    let resolve = |sym: &str| -> Option<String> {
        let name = sym.strip_prefix('$')?;
        by_name.get(name).map(|f| f.id.clone())
    };
    config.visible_field_ids = config
        .visible_field_ids
        .iter()
        .filter_map(|s| resolve(s))
        .collect();
    config.card_field_ids = config
        .card_field_ids
        .iter()
        .filter_map(|s| resolve(s))
        .collect();
    config.group_field_id = config.group_field_id.as_deref().and_then(resolve);
    config.date_field_id = config.date_field_id.as_deref().and_then(resolve);
    config.image_field_id = config.image_field_id.as_deref().and_then(resolve);
    config.filters.filters.retain_mut(|f| {
        match resolve(&f.field_id) {
            Some(id) => {
                f.field_id = id;
                // Valores de select simbólicos ("$campo:etiqueta") → id de opción.
                if let Some(s) = f.value.as_str() {
                    if let Some((fname, label)) =
                        s.strip_prefix('$').and_then(|s| s.split_once(':'))
                    {
                        if let Some(tf) = fields.iter().find(|t| t.name == fname) {
                            if let Some(opt) = tf.config.options.iter().find(|o| o.label == label) {
                                f.value = json!(opt.id);
                            }
                        }
                    }
                }
                true
            }
            None => false,
        }
    });
    config.sorts.retain_mut(|s| match resolve(&s.field_id) {
        Some(id) => {
            s.field_id = id;
            true
        }
        None => false,
    });
}

// ===========================================================================
// Plantillas reales
// ===========================================================================

mod templates {
    use super::*;

    fn opt(id: &str, label: &str, color: &str) -> DbSelectOption {
        DbSelectOption {
            id: id.into(),
            label: label.into(),
            color: color.into(),
        }
    }

    fn text(name: &str) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: DbFieldKind::Text,
            config: DbFieldConfig::default(),
        }
    }

    fn long(name: &str) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: DbFieldKind::LongText,
            config: DbFieldConfig::default(),
        }
    }

    fn sel(name: &str, options: Vec<DbSelectOption>) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: DbFieldKind::Select,
            config: DbFieldConfig {
                options,
                ..Default::default()
            },
        }
    }

    fn multi(name: &str, options: Vec<DbSelectOption>) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: DbFieldKind::Multiselect,
            config: DbFieldConfig {
                options,
                ..Default::default()
            },
        }
    }

    fn kind(name: &str, k: DbFieldKind) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: k,
            config: DbFieldConfig::default(),
        }
    }

    fn formula(name: &str, expr: &str) -> DbTemplateField {
        DbTemplateField {
            name: name.into(),
            kind: DbFieldKind::Formula,
            config: DbFieldConfig {
                expression: Some(expr.into()),
                ..Default::default()
            },
        }
    }

    fn rec(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn table_view(name: &str) -> CreateDbView {
        CreateDbView {
            name: name.into(),
            kind: DbViewKind::Table,
            config: DbViewConfig::default(),
        }
    }

    fn kanban_view(name: &str, group: &str) -> CreateDbView {
        CreateDbView {
            name: name.into(),
            kind: DbViewKind::Kanban,
            config: DbViewConfig {
                group_field_id: Some(format!("${group}")),
                ..Default::default()
            },
        }
    }

    fn calendar_view(name: &str, date: &str) -> CreateDbView {
        CreateDbView {
            name: name.into(),
            kind: DbViewKind::Calendar,
            config: DbViewConfig {
                date_field_id: Some(format!("${date}")),
                ..Default::default()
            },
        }
    }

    fn list_view(name: &str) -> CreateDbView {
        CreateDbView {
            name: name.into(),
            kind: DbViewKind::List,
            config: DbViewConfig::default(),
        }
    }

    pub fn all() -> Vec<DbTemplate> {
        vec![
            DbTemplate {
                id: "videojuegos".into(),
                name: "Videojuegos".into(),
                description: "Tu biblioteca de videojuegos: plataforma, estado y puntuación.".into(),
                icon: "gamepad-2".into(),
                color: "purple".into(),
                fields: vec![
                    text("Título"),
                    sel("Plataforma", vec![
                        opt("pc", "PC", "blue"), opt("ps5", "PlayStation 5", "purple"),
                        opt("switch", "Nintendo Switch", "red"), opt("xbox", "Xbox", "green"),
                    ]),
                    sel("Estado", vec![
                        opt("pendiente", "Pendiente", "gray"), opt("jugando", "Jugando", "blue"),
                        opt("completado", "Completado", "green"), opt("abandonado", "Abandonado", "orange"),
                    ]),
                    kind("Nota", DbFieldKind::Rating),
                    multi("Género", vec![
                        opt("rpg", "RPG", "purple"), opt("accion", "Acción", "red"),
                        opt("aventura", "Aventura", "blue"), opt("estrategia", "Estrategia", "green"),
                    ]),
                    kind("Horas jugadas", DbFieldKind::Integer),
                    kind("Fecha de compra", DbFieldKind::Date),
                    long("Comentario"),
                ],
                sample_records: vec![
                    rec(&[
                        ("Título", json!("The Legend of Zelda: Tears of the Kingdom")),
                        ("Plataforma", json!("switch")), ("Estado", json!("completado")),
                        ("Nota", json!(5)), ("Horas jugadas", json!(185)),
                        ("Género", json!(["aventura", "accion"])),
                        ("Fecha de compra", json!("2023-05-12")),
                        ("Comentario", json!("Física y construcción brillante; el subsuelo podría dar más de sí.")),
                    ]),
                    rec(&[
                        ("Título", json!("Baldur's Gate 3")),
                        ("Plataforma", json!("pc")), ("Estado", json!("jugando")),
                        ("Nota", json!(5)), ("Horas jugadas", json!(92)),
                        ("Fecha de compra", json!("2023-08-03")),
                    ]),
                    rec(&[
                        ("Título", json!("Elden Ring")),
                        ("Plataforma", json!("ps5")), ("Estado", json!("pendiente")),
                        ("Horas jugadas", json!(0)),
                    ]),
                ],
                views: vec![table_view("Biblioteca"), kanban_view("Por estado", "Estado")],
            },
            DbTemplate {
                id: "peliculas_series".into(),
                name: "Películas y series".into(),
                description: "Watchlist con tipo, estado, plataforma y valoración.".into(),
                icon: "clapperboard".into(),
                color: "red".into(),
                fields: vec![
                    text("Título"),
                    sel("Tipo", vec![opt("pelicula", "Película", "blue"), opt("serie", "Serie", "purple")]),
                    sel("Estado", vec![
                        opt("quiero_ver", "Quiero ver", "gray"), opt("viendo", "Viendo", "blue"),
                        opt("vista", "Vista", "green"),
                    ]),
                    kind("Año", DbFieldKind::Integer),
                    kind("Valoración", DbFieldKind::Rating),
                    sel("Plataforma", vec![
                        opt("netflix", "Netflix", "red"), opt("prime", "Prime Video", "blue"),
                        opt("max", "Max", "purple"), opt("cine", "Cine", "yellow"),
                    ]),
                    long("Notas"),
                ],
                sample_records: vec![
                    rec(&[
                        ("Título", json!("Dune: Parte Dos")), ("Tipo", json!("pelicula")),
                        ("Estado", json!("vista")), ("Año", json!(2024)),
                        ("Valoración", json!(5)), ("Plataforma", json!("cine")),
                    ]),
                    rec(&[
                        ("Título", json!("Severance")), ("Tipo", json!("serie")),
                        ("Estado", json!("viendo")), ("Año", json!(2022)), ("Valoración", json!(5)),
                    ]),
                    rec(&[
                        ("Título", json!("Perfect Days")), ("Tipo", json!("pelicula")),
                        ("Estado", json!("quiero_ver")), ("Año", json!(2023)),
                    ]),
                ],
                views: vec![table_view("Todo"), kanban_view("Watchlist", "Estado")],
            },
            DbTemplate {
                id: "libros".into(),
                name: "Libros".into(),
                description: "Biblioteca personal: autor, estado de lectura, páginas y progreso.".into(),
                icon: "book-open".into(),
                color: "yellow".into(),
                fields: vec![
                    text("Título"),
                    text("Autor"),
                    sel("Estado", vec![
                        opt("pendiente", "Pendiente", "gray"), opt("leyendo", "Leyendo", "blue"),
                        opt("leido", "Leído", "green"),
                    ]),
                    kind("Páginas", DbFieldKind::Integer),
                    kind("Páginas leídas", DbFieldKind::Integer),
                    formula("Progreso %", "IF({Páginas} > 0, ROUND({Páginas leídas} / {Páginas} * 100, 0), 0)"),
                    kind("Valoración", DbFieldKind::Rating),
                    kind("Fecha de inicio", DbFieldKind::Date),
                ],
                sample_records: vec![
                    rec(&[
                        ("Título", json!("El nombre del viento")), ("Autor", json!("Patrick Rothfuss")),
                        ("Estado", json!("leido")), ("Páginas", json!(872)), ("Páginas leídas", json!(872)),
                        ("Valoración", json!(5)), ("Fecha de inicio", json!("2024-01-10")),
                    ]),
                    rec(&[
                        ("Título", json!("Piranesi")), ("Autor", json!("Susanna Clarke")),
                        ("Estado", json!("leyendo")), ("Páginas", json!(272)), ("Páginas leídas", json!(120)),
                        ("Fecha de inicio", json!("2025-03-02")),
                    ]),
                    rec(&[
                        ("Título", json!("Proyecto Hail Mary")), ("Autor", json!("Andy Weir")),
                        ("Estado", json!("pendiente")), ("Páginas", json!(496)), ("Páginas leídas", json!(0)),
                    ]),
                ],
                views: vec![table_view("Biblioteca"), kanban_view("Por estado", "Estado"), calendar_view("Inicios de lectura", "Fecha de inicio")],
            },
            DbTemplate {
                id: "inventario_hardware".into(),
                name: "Inventario hardware".into(),
                description: "Equipos y componentes: categoría, ubicación, garantía y valor.".into(),
                icon: "cpu".into(),
                color: "cyan".into(),
                fields: vec![
                    text("Equipo"),
                    sel("Categoría", vec![
                        opt("portatil", "Portátil", "blue"), opt("sobremesa", "Sobremesa", "purple"),
                        opt("periferico", "Periférico", "green"), opt("componente", "Componente", "orange"),
                        opt("red", "Red", "gray"),
                    ]),
                    text("Marca y modelo"),
                    text("Número de serie"),
                    kind("Cantidad", DbFieldKind::Integer),
                    kind("Precio unitario", DbFieldKind::Decimal),
                    formula("Valor total", "ROUND({Cantidad} * {Precio unitario}, 2)"),
                    kind("Fecha de compra", DbFieldKind::Date),
                    kind("Fin de garantía", DbFieldKind::Date),
                    sel("Estado", vec![
                        opt("en_uso", "En uso", "green"), opt("almacenado", "Almacenado", "gray"),
                        opt("averiado", "Averiado", "red"), opt("vendido", "Vendido", "yellow"),
                    ]),
                    text("Ubicación"),
                ],
                sample_records: vec![
                    rec(&[
                        ("Equipo", json!("Portátil de trabajo")), ("Categoría", json!("portatil")),
                        ("Marca y modelo", json!("Framework Laptop 13")), ("Cantidad", json!(1)),
                        ("Precio unitario", json!(1249.0)), ("Fecha de compra", json!("2024-02-15")),
                        ("Fin de garantía", json!("2026-02-15")), ("Estado", json!("en_uso")),
                        ("Ubicación", json!("Escritorio")),
                    ]),
                    rec(&[
                        ("Equipo", json!("Monitor 27\" 4K")), ("Categoría", json!("periferico")),
                        ("Marca y modelo", json!("Dell U2723QE")), ("Cantidad", json!(2)),
                        ("Precio unitario", json!(489.99)), ("Estado", json!("en_uso")),
                        ("Ubicación", json!("Escritorio")),
                    ]),
                    rec(&[
                        ("Equipo", json!("SSD NVMe 2TB")), ("Categoría", json!("componente")),
                        ("Marca y modelo", json!("Samsung 990 Pro")), ("Cantidad", json!(1)),
                        ("Precio unitario", json!(159.9)), ("Estado", json!("almacenado")),
                        ("Ubicación", json!("Cajón componentes")),
                    ]),
                ],
                views: vec![table_view("Inventario"), kanban_view("Por estado", "Estado")],
            },
            DbTemplate {
                id: "cursos".into(),
                name: "Cursos".into(),
                description: "Formación en curso: plataforma, progreso y fechas.".into(),
                icon: "graduation-cap".into(),
                color: "green".into(),
                fields: vec![
                    text("Curso"),
                    text("Plataforma"),
                    kind("Enlace", DbFieldKind::Url),
                    sel("Estado", vec![
                        opt("pendiente", "Pendiente", "gray"), opt("en_curso", "En curso", "blue"),
                        opt("terminado", "Terminado", "green"),
                    ]),
                    kind("Horas totales", DbFieldKind::Integer),
                    kind("Horas completadas", DbFieldKind::Integer),
                    formula("Progreso %", "IF({Horas totales} > 0, ROUND({Horas completadas} / {Horas totales} * 100, 0), 0)"),
                    kind("Fecha de inicio", DbFieldKind::Date),
                    kind("Valoración", DbFieldKind::Rating),
                ],
                sample_records: vec![
                    rec(&[
                        ("Curso", json!("Rust for Beginners")), ("Plataforma", json!("Microsoft Learn")),
                        ("Enlace", json!("https://learn.microsoft.com/training/paths/rust-first-steps/")),
                        ("Estado", json!("en_curso")), ("Horas totales", json!(6)), ("Horas completadas", json!(3)),
                        ("Fecha de inicio", json!("2025-04-01")),
                    ]),
                    rec(&[
                        ("Curso", json!("SQL avanzado")), ("Plataforma", json!("Coursera")),
                        ("Estado", json!("terminado")), ("Horas totales", json!(20)), ("Horas completadas", json!(20)),
                        ("Valoración", json!(4)),
                    ]),
                ],
                views: vec![table_view("Cursos"), kanban_view("Por estado", "Estado"), calendar_view("Inicios", "Fecha de inicio")],
            },
            DbTemplate {
                id: "coleccion_personal".into(),
                name: "Colección personal".into(),
                description: "Coleccionismo: categoría, adquisición, valor estimado y fotos.".into(),
                icon: "gem".into(),
                color: "pink".into(),
                fields: vec![
                    text("Objeto"),
                    sel("Categoría", vec![
                        opt("figuras", "Figuras", "purple"), opt("monedas", "Monedas", "yellow"),
                        opt("vinilos", "Vinilos", "blue"), opt("cromos", "Cromos", "green"),
                        opt("otros", "Otros", "gray"),
                    ]),
                    long("Descripción"),
                    kind("Foto", DbFieldKind::Image),
                    kind("Fecha de adquisición", DbFieldKind::Date),
                    kind("Precio pagado", DbFieldKind::Decimal),
                    kind("Valor estimado", DbFieldKind::Decimal),
                    formula("Revalorización", "ROUND({Valor estimado} - {Precio pagado}, 2)"),
                    kind("Estado de conservación", DbFieldKind::Rating),
                ],
                sample_records: vec![
                    rec(&[
                        ("Objeto", json!("Vinilo Kind of Blue (ed. 2010)")), ("Categoría", json!("vinilos")),
                        ("Descripción", json!("Miles Davis. Reedición 180 g, funda en buen estado.")),
                        ("Fecha de adquisición", json!("2022-11-20")), ("Precio pagado", json!(24.99)),
                        ("Valor estimado", json!(35.0)), ("Estado de conservación", json!(4)),
                    ]),
                    rec(&[
                        ("Objeto", json!("2 € conmemorativos Grecia 2004")), ("Categoría", json!("monedas")),
                        ("Fecha de adquisición", json!("2019-06-01")), ("Precio pagado", json!(8.0)),
                        ("Valor estimado", json!(12.0)), ("Estado de conservación", json!(5)),
                    ]),
                ],
                views: vec![table_view("Colección"), CreateDbView {
                    name: "Galería".into(), kind: DbViewKind::Gallery,
                    config: DbViewConfig { image_field_id: Some("$Foto".into()), ..Default::default() },
                }],
            },
            DbTemplate {
                id: "aplicaciones".into(),
                name: "Aplicaciones".into(),
                description: "Software que usas: plataforma, licencia, coste y renovaciones.".into(),
                icon: "app-window".into(),
                color: "blue".into(),
                fields: vec![
                    text("Aplicación"),
                    sel("Plataforma", vec![
                        opt("escritorio", "Escritorio", "blue"), opt("web", "Web", "purple"),
                        opt("movil", "Móvil", "green"),
                    ]),
                    sel("Licencia", vec![
                        opt("gratis", "Gratuita", "green"), opt("pago_unico", "Pago único", "blue"),
                        opt("suscripcion", "Suscripción", "orange"), opt("open_source", "Open source", "gray"),
                    ]),
                    kind("Coste anual", DbFieldKind::Decimal),
                    kind("Renovación", DbFieldKind::Date),
                    kind("Enlace", DbFieldKind::Url),
                    kind("Imprescindible", DbFieldKind::Checkbox),
                    long("Notas"),
                ],
                sample_records: vec![
                    rec(&[
                        ("Aplicación", json!("Obsidian")), ("Plataforma", json!("escritorio")),
                        ("Licencia", json!("gratis")), ("Coste anual", json!(0)),
                        ("Enlace", json!("https://obsidian.md")), ("Imprescindible", json!(true)),
                    ]),
                    rec(&[
                        ("Aplicación", json!("JetBrains All Products")), ("Plataforma", json!("escritorio")),
                        ("Licencia", json!("suscripcion")), ("Coste anual", json!(289.0)),
                        ("Renovación", json!("2026-01-15")), ("Imprescindible", json!(true)),
                    ]),
                    rec(&[
                        ("Aplicación", json!("Bitwarden")), ("Plataforma", json!("web")),
                        ("Licencia", json!("open_source")), ("Coste anual", json!(10.0)),
                        ("Imprescindible", json!(true)),
                    ]),
                ],
                views: vec![table_view("Aplicaciones"), calendar_view("Renovaciones", "Renovación"), list_view("Lista compacta")],
            },
            DbTemplate {
                id: "seguimiento_proyectos".into(),
                name: "Seguimiento de proyectos".into(),
                description: "Proyectos personales con estado, fechas, presupuesto y desviación.".into(),
                icon: "kanban".into(),
                color: "orange".into(),
                fields: vec![
                    text("Proyecto"),
                    long("Descripción"),
                    sel("Estado", vec![
                        opt("idea", "Idea", "gray"), opt("planificado", "Planificado", "purple"),
                        opt("en_curso", "En curso", "blue"), opt("bloqueado", "Bloqueado", "red"),
                        opt("terminado", "Terminado", "green"),
                    ]),
                    kind("Prioridad", DbFieldKind::Rating),
                    kind("Fecha de inicio", DbFieldKind::Date),
                    kind("Fecha objetivo", DbFieldKind::Date),
                    kind("Presupuesto", DbFieldKind::Decimal),
                    kind("Gastado", DbFieldKind::Decimal),
                    formula("Desviación", "ROUND({Gastado} - {Presupuesto}, 2)"),
                    formula("Días restantes", "IF({Fecha objetivo} != \"\", DAYS(TODAY(), {Fecha objetivo}), 0)"),
                ],
                sample_records: vec![
                    rec(&[
                        ("Proyecto", json!("Rediseño del portfolio")),
                        ("Descripción", json!("Nueva web personal con casos de estudio.")),
                        ("Estado", json!("en_curso")), ("Prioridad", json!(4)),
                        ("Fecha de inicio", json!("2025-05-01")), ("Fecha objetivo", json!("2026-03-01")),
                        ("Presupuesto", json!(200.0)), ("Gastado", json!(59.0)),
                    ]),
                    rec(&[
                        ("Proyecto", json!("Domótica del salón")),
                        ("Estado", json!("planificado")), ("Prioridad", json!(2)),
                        ("Presupuesto", json!(350.0)), ("Gastado", json!(0.0)),
                    ]),
                    rec(&[
                        ("Proyecto", json!("Curso de fotografía")),
                        ("Estado", json!("terminado")), ("Prioridad", json!(3)),
                        ("Presupuesto", json!(120.0)), ("Gastado", json!(120.0)),
                    ]),
                ],
                views: vec![
                    table_view("Proyectos"), kanban_view("Tablero", "Estado"),
                    calendar_view("Fechas objetivo", "Fecha objetivo"),
                ],
            },
        ]
    }
}

// ===========================================================================
// Tests del parser de fórmulas (unidad)
// ===========================================================================

#[cfg(test)]
mod formula_tests {
    use super::formula::{evaluate, FVal};
    use std::collections::HashMap;

    fn env(pairs: &[(&str, FVal)]) -> HashMap<String, FVal> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn arithmetic_and_precedence() {
        let e = HashMap::new();
        assert_eq!(evaluate("2 + 3 * 4", &e).unwrap(), serde_json::json!(14.0));
        assert_eq!(
            evaluate("(2 + 3) * 4", &e).unwrap(),
            serde_json::json!(20.0)
        );
        assert_eq!(evaluate("10 % 3", &e).unwrap(), serde_json::json!(1.0));
        assert_eq!(evaluate("-5 + 2", &e).unwrap(), serde_json::json!(-3.0));
        assert!(evaluate("1 / 0", &e).is_err());
    }

    #[test]
    fn if_and_comparisons() {
        let e = env(&[("Nota", FVal::Num(7.0))]);
        assert_eq!(
            evaluate("IF({Nota} >= 5, \"Aprobado\", \"Suspenso\")", &e).unwrap(),
            serde_json::json!("Aprobado")
        );
        assert_eq!(
            evaluate("IF(1 != 2, 10, 20)", &HashMap::new()).unwrap(),
            serde_json::json!(10.0)
        );
    }

    #[test]
    fn concat_and_functions() {
        let e = env(&[("Nombre", FVal::Str("Ada".into()))]);
        assert_eq!(
            evaluate("UPPER({Nombre}) & \" \" & CONCAT(\"L\", \"ovelace\")", &e).unwrap(),
            serde_json::json!("ADA Lovelace")
        );
        assert_eq!(
            evaluate("ROUND(3.146, 2)", &HashMap::new()).unwrap(),
            serde_json::json!(3.15)
        );
        assert_eq!(
            evaluate("ABS(-4)", &HashMap::new()).unwrap(),
            serde_json::json!(4.0)
        );
        assert_eq!(
            evaluate("LEN(\"hola\")", &HashMap::new()).unwrap(),
            serde_json::json!(4.0)
        );
        assert_eq!(
            evaluate("LOWER(\"HOLA\")", &HashMap::new()).unwrap(),
            serde_json::json!("hola")
        );
    }

    #[test]
    fn dates_today_and_days() {
        let e = HashMap::new();
        assert_eq!(
            evaluate("DAYS(\"2025-01-01\", \"2025-01-31\")", &e).unwrap(),
            serde_json::json!(30.0)
        );
        let today = evaluate("TODAY()", &e).unwrap();
        assert!(today.as_str().unwrap().len() == 10);
        assert!(evaluate("DAYS(\"no-fecha\", TODAY())", &e).is_err());
    }

    #[test]
    fn field_refs_and_errors() {
        let e = env(&[("Precio", FVal::Num(100.0))]);
        assert_eq!(
            evaluate("ROUND({Precio} * 1.21, 2)", &e).unwrap(),
            serde_json::json!(121.0)
        );
        let err = evaluate("{Inexistente} + 1", &e).unwrap_err();
        assert!(err.contains("Inexistente"));
        assert!(evaluate("2 +", &e).is_err());
        assert!(evaluate("HACK(1)", &e).is_err());
        assert!(evaluate("{SinCerrar + 1", &e).is_err());
        assert!(evaluate("1; DROP TABLE databases", &e).is_err());
    }
}
