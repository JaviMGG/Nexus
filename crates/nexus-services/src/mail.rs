use std::collections::HashMap;

use base64::Engine;
use chrono::DateTime;
use imap::{ClientBuilder, Connection, ConnectionMode, TlsKind};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use nexus_core::{new_id, NexusError, Result};
use nexus_db::mail as mail_repo;
use nexus_db::{repos, Db};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::settings::SettingsService;

const ACCOUNTS_KEY: &str = "mail";
const SNIPPET_LEN: usize = 200;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MailAccount {
    pub id: String,
    pub name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub use_tls: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MailMessage {
    pub message_id: String,
    pub account_id: String,
    pub folder: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MailTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Clone)]
pub struct MailService {
    pub db: Db,
    pub settings: SettingsService,
}

impl MailService {
    pub fn new(db: Db) -> Self {
        Self {
            settings: SettingsService { db: db.clone() },
            db,
        }
    }

    pub async fn list_accounts(&self) -> Result<Vec<MailAccount>> {
        Ok(
            repos::settings::get_json::<Vec<MailAccount>>(&self.db, ACCOUNTS_KEY)
                .await?
                .unwrap_or_default(),
        )
    }

    pub async fn save_account(
        &self,
        mut account: MailAccount,
        password: Option<String>,
    ) -> Result<MailAccount> {
        if account.id.trim().is_empty() {
            account.id = new_id();
        }
        account.name = account.name.trim().to_string();
        account.imap_host = account.imap_host.trim().to_string();
        account.smtp_host = account.smtp_host.trim().to_string();
        account.username = account.username.trim().to_string();
        if account.name.is_empty() {
            return Err(NexusError::validation(
                "la cuenta de correo necesita un nombre",
            ));
        }
        if account.imap_host.is_empty() || account.smtp_host.is_empty() {
            return Err(NexusError::validation(
                "los servidores IMAP y SMTP son obligatorios",
            ));
        }
        if account.imap_port == 0 || account.smtp_port == 0 {
            return Err(NexusError::validation(
                "los puertos deben ser mayores que 0",
            ));
        }
        if account.username.is_empty() {
            return Err(NexusError::validation("falta el usuario de correo"));
        }

        if let Some(password) = password.filter(|p| !p.trim().is_empty()) {
            self.settings
                .save_secret(&format!("mail:{}", account.id), password.trim())
                .await?;
        }

        let mut accounts = self.list_accounts().await?;
        match accounts.iter_mut().find(|a| a.id == account.id) {
            Some(existing) => *existing = account.clone(),
            None => accounts.push(account.clone()),
        }
        repos::settings::set_json(&self.db, ACCOUNTS_KEY, &accounts).await?;
        Ok(account)
    }

    pub async fn delete_account(&self, id: &str) -> Result<()> {
        let mut accounts = self.list_accounts().await?;
        let before = accounts.len();
        accounts.retain(|a| a.id != id);
        if accounts.len() == before {
            return Err(NexusError::not_found("cuenta de correo", id));
        }
        repos::settings::set_json(&self.db, ACCOUNTS_KEY, &accounts).await?;
        self.settings.delete_secret(&format!("mail:{id}")).await?;
        mail_repo::delete(&self.db, id).await?;
        Ok(())
    }

    pub async fn test_account(&self, id: &str) -> Result<MailTestResult> {
        let account = self.find_account(id).await?;
        let password = self.password_for(id).await?.ok_or_else(|| {
            NexusError::validation(format!("la cuenta de correo '{id}' no tiene contraseña"))
        })?;
        let imap_result = tokio::task::spawn_blocking({
            let account = account.clone();
            let password = password.clone();
            move || test_imap(&account, &password)
        })
        .await;
        let message = match imap_result {
            Ok(Ok(message)) => message,
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(NexusError::Internal(format!("worker IMAP: {e}"))),
        };
        let transport = smtp_transport(&account, &password)?;
        let smtp_ok = transport
            .test_connection()
            .await
            .map_err(|e| infra_error("conectar por SMTP", e))?;
        if !smtp_ok {
            return Err(NexusError::Internal(
                "SMTP no aceptó la conexión".to_string(),
            ));
        }
        Ok(MailTestResult {
            ok: true,
            message: format!("{message}; SMTP correcto"),
        })
    }

    pub async fn fetch_new(&self) -> Result<Vec<MailMessage>> {
        let accounts = self.list_accounts().await?;
        let mut all = Vec::new();
        for account in accounts {
            let id = account.id.clone();
            let last_uid = match mail_repo::get_last_uid(&self.db, &id).await {
                Ok(uid) => uid,
                Err(e) => {
                    warn!("no se pudo leer el estado de correo de '{id}': {e}");
                    continue;
                }
            };
            let password = match self.password_for(&id).await {
                Ok(Some(password)) => password,
                Ok(None) => {
                    warn!("la cuenta de correo '{id}' no tiene contraseña; se omite");
                    continue;
                }
                Err(e) => {
                    warn!("no se pudo leer la contraseña de '{id}': {e}");
                    continue;
                }
            };
            let result =
                tokio::task::spawn_blocking(move || fetch_account(&account, &password, last_uid))
                    .await;
            match result {
                Ok(Ok((messages, new_uid))) => {
                    if new_uid > last_uid {
                        if let Err(e) = mail_repo::set_last_uid(&self.db, &id, new_uid).await {
                            warn!("no se pudo actualizar el último UID de '{id}': {e}");
                        }
                    }
                    all.extend(messages);
                }
                Ok(Err(e)) => warn!("error al leer correo de '{id}': {e}"),
                Err(e) => warn!("worker IMAP de '{id}': {e}"),
            }
        }
        Ok(all)
    }

    pub async fn send(
        &self,
        account_id: &str,
        to: &str,
        subject: &str,
        body: &str,
        in_reply_to: Option<&str>,
    ) -> Result<()> {
        let account = self.find_account(account_id).await?;
        let password = self.password_for(account_id).await?.ok_or_else(|| {
            NexusError::validation(format!(
                "la cuenta de correo '{account_id}' no tiene contraseña"
            ))
        })?;
        let from = parse_mailbox(&account.username, &account.name)?;
        let to: Mailbox = to
            .trim()
            .parse()
            .map_err(|_| NexusError::validation(format!("destinatario inválido: {to}")))?;
        let mut builder = Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN);
        if let Some(id) = in_reply_to.map(str::trim).filter(|id| !id.is_empty()) {
            let id = if id.starts_with('<') && id.ends_with('>') {
                id.to_string()
            } else {
                format!("<{id}>")
            };
            builder = builder.in_reply_to(id);
        }
        let email = builder
            .body(body.to_string())
            .map_err(|e| infra_error("construir mensaje", e))?;

        let transport = smtp_transport(&account, &password)?;
        transport
            .send(email)
            .await
            .map_err(|e| infra_error("enviar correo", e))?;
        Ok(())
    }

    async fn find_account(&self, id: &str) -> Result<MailAccount> {
        let needle = id.trim();
        self.list_accounts()
            .await?
            .into_iter()
            .find(|a| a.id == needle || a.name.eq_ignore_ascii_case(needle))
            .ok_or_else(|| NexusError::not_found("cuenta de correo", id))
    }

    async fn password_for(&self, id: &str) -> Result<Option<String>> {
        self.settings.get_named_secret(&format!("mail:{id}")).await
    }
}

fn connect(account: &MailAccount) -> Result<imap::Client<Connection>> {
    let mode = if account.use_tls {
        ConnectionMode::AutoTls
    } else {
        ConnectionMode::Auto
    };
    ClientBuilder::new(account.imap_host.as_str(), account.imap_port)
        .tls_kind(TlsKind::Rust)
        .mode(mode)
        .connect()
        .map_err(|e| imap_error("conectar", e))
}

fn smtp_transport(
    account: &MailAccount,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let tls = if account.use_tls {
        let params = TlsParameters::new(account.smtp_host.clone())
            .map_err(|e| infra_error("configurar TLS SMTP", e))?;
        if account.smtp_port == 465 {
            Tls::Wrapper(params)
        } else {
            Tls::Required(params)
        }
    } else {
        Tls::None
    };
    Ok(
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(account.smtp_host.as_str())
            .port(account.smtp_port)
            .tls(tls)
            .credentials(Credentials::new(
                account.username.clone(),
                password.to_string(),
            ))
            .build(),
    )
}

fn test_imap(account: &MailAccount, password: &str) -> Result<String> {
    let client = connect(account)?;
    let mut session = client
        .login(&account.username, password)
        .map_err(|e| imap_error("autenticar", e.0))?;
    let mailbox = session
        .select("INBOX")
        .map_err(|e| imap_error("seleccionar INBOX", e))?;
    let _ = session.logout();
    Ok(format!(
        "Conexión correcta con {} (INBOX: {} mensajes)",
        account.username, mailbox.exists
    ))
}

fn fetch_account(
    account: &MailAccount,
    password: &str,
    last_uid: i64,
) -> Result<(Vec<MailMessage>, i64)> {
    let client = connect(account)?;
    let mut session = client
        .login(&account.username, password)
        .map_err(|e| imap_error("autenticar", e.0))?;
    let result = (|| -> Result<(Vec<MailMessage>, i64)> {
        session
            .select("INBOX")
            .map_err(|e| imap_error("seleccionar INBOX", e))?;
        let min_uid = (last_uid as u64).saturating_add(1);
        if min_uid > u32::MAX as u64 {
            return Ok((Vec::new(), last_uid));
        }
        let query = format!("UID {min_uid}:*");
        let mut uids: Vec<u32> = session
            .uid_search(query)
            .map_err(|e| imap_error("buscar UIDs", e))?
            .into_iter()
            .collect();
        uids.sort_unstable();
        let mut messages = Vec::new();
        let mut max_uid = last_uid;
        for chunk in uids.chunks(100) {
            let set = chunk
                .iter()
                .map(|uid| uid.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let fetches = session
                .uid_fetch(&set, "(UID BODY.PEEK[HEADER] BODY.PEEK[TEXT])")
                .map_err(|e| imap_error("descargar mensajes", e))?;
            for fetch in fetches.iter() {
                let Some(uid) = fetch.uid else {
                    continue;
                };
                if (uid as i64) > max_uid {
                    max_uid = uid as i64;
                }
                if let Some(message) = parse_message(account, uid, fetch) {
                    messages.push(message);
                }
            }
        }
        Ok((messages, max_uid))
    })();
    let _ = session.logout();
    result
}

fn parse_message(
    account: &MailAccount,
    uid: u32,
    fetch: &imap::types::Fetch<'_>,
) -> Option<MailMessage> {
    let headers = parse_headers(fetch.header().unwrap_or(&[]));
    let body = fetch.text().or_else(|| fetch.body()).unwrap_or(&[]);
    let message_id = headers
        .get("message-id")
        .map(|s| s.trim().trim_matches(['<', '>']).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", account.id, uid));
    let from = headers
        .get("from")
        .map(|s| decode_header_value(s))
        .unwrap_or_default();
    let subject = headers
        .get("subject")
        .map(|s| decode_header_value(s))
        .unwrap_or_default();
    let date = parse_date(headers.get("date").map(String::as_str).unwrap_or(""));
    Some(MailMessage {
        message_id,
        account_id: account.id.clone(),
        folder: "INBOX".to_string(),
        from,
        subject,
        date,
        snippet: snippet_from(&headers, body),
    })
}

fn parse_headers(raw: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(raw);
    let mut headers = HashMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, value)) = current.as_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((key, value)) = current.take() {
            headers.insert(key, value);
        }
        if let Some((key, value)) = line.split_once(':') {
            current = Some((key.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    if let Some((key, value)) = current {
        headers.insert(key, value);
    }
    headers
}

fn parse_date(raw: &str) -> String {
    let value = raw.trim().to_string();
    DateTime::parse_from_rfc2822(&value)
        .map(|d| d.to_rfc3339())
        .unwrap_or(value)
}

fn snippet_from(headers: &HashMap<String, String>, raw: &[u8]) -> String {
    let raw_text = String::from_utf8_lossy(raw).into_owned();
    let content_type = headers.get("content-type").cloned().unwrap_or_default();
    let type_lower = content_type.to_ascii_lowercase();
    let mut transfer = headers
        .get("content-transfer-encoding")
        .cloned()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut body_text = raw_text.clone();
    let mut is_html = type_lower.contains("html");

    if type_lower.contains("multipart/") {
        if let Some(boundary) = boundary_param(&content_type) {
            let marker = format!("--{boundary}");
            let mut chosen: Option<(HashMap<String, String>, &str)> = None;
            for part in raw_text.split(&marker).skip(1) {
                let part = part.trim_start_matches(['\r', '\n']);
                if part.starts_with("--") {
                    break;
                }
                let (part_headers, part_body) = split_str(part);
                let part_type = part_headers
                    .get("content-type")
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                if part_type.contains("text/plain") {
                    chosen = Some((part_headers, part_body));
                    break;
                }
                if chosen.is_none() {
                    chosen = Some((part_headers, part_body));
                }
            }
            if let Some((part_headers, part_body)) = chosen {
                transfer = part_headers
                    .get("content-transfer-encoding")
                    .cloned()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                is_html = part_headers
                    .get("content-type")
                    .map(|s| s.to_ascii_lowercase().contains("html"))
                    .unwrap_or(false);
                body_text = part_body.to_string();
            }
        }
    }

    if transfer.contains("base64") {
        if let Some(decoded) = decode_base64(&body_text) {
            body_text = decoded;
        }
    } else if transfer.contains("quoted-printable") {
        body_text = decode_quoted_printable(&body_text);
    }
    if is_html {
        body_text = strip_html(&body_text);
    }
    let normalized = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let short: String = chars.by_ref().take(SNIPPET_LEN).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

fn split_str(raw: &str) -> (HashMap<String, String>, &str) {
    let separator = raw
        .find("\r\n\r\n")
        .map(|p| p + 4)
        .or_else(|| raw.find("\n\n").map(|p| p + 2));
    match separator {
        Some(p) => (parse_headers(&raw.as_bytes()[..p]), &raw[p..]),
        None => (parse_headers(raw.as_bytes()), ""),
    }
}

fn boundary_param(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key.eq_ignore_ascii_case("boundary") {
            Some(value.trim().trim_matches('"').to_string())
        } else {
            None
        }
    })
}

fn decode_header_value(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input.trim();
    while let Some(start) = rest.find("=?") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("?=") else {
            out.push_str(&rest[start..]);
            return out.trim().to_string();
        };
        let encoded = &after[..end];
        let parts: Vec<&str> = encoded.splitn(4, '?').collect();
        if parts.len() >= 4 {
            out.push_str(&decode_word(parts[0], parts[1], parts[2]));
        } else {
            out.push_str("=?");
            out.push_str(encoded);
            out.push_str("?=");
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn decode_word(charset: &str, encoding: &str, text: &str) -> String {
    let bytes = match encoding.to_ascii_lowercase().as_str() {
        "b" => decode_base64_bytes(text).unwrap_or_else(|| text.as_bytes().to_vec()),
        "q" => decode_q_bytes(text, true),
        _ => text.as_bytes().to_vec(),
    };
    if charset.eq_ignore_ascii_case("utf-8") || charset.eq_ignore_ascii_case("us-ascii") {
        String::from_utf8(bytes)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    } else {
        bytes.into_iter().map(char::from).collect()
    }
}

fn decode_base64(input: &str) -> Option<String> {
    let bytes = decode_base64_bytes(input)?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn decode_base64_bytes(input: &str) -> Option<Vec<u8>> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()
}

fn decode_quoted_printable(input: &str) -> String {
    let cleaned = input.replace("=\r\n", "").replace("=\n", "");
    let bytes = decode_q_bytes(&cleaned, false);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn decode_q_bytes(input: &str, underscore_as_space: bool) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'=' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(high), Some(low)) => {
                    out.push(high * 16 + low);
                    i += 3;
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b'_' if underscore_as_space => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn strip_html(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn parse_mailbox(value: &str, display: &str) -> Result<Mailbox> {
    let direct: std::result::Result<Mailbox, _> = value.trim().parse();
    direct
        .or_else(|_| format!("{} <{}>", display.trim(), value.trim()).parse())
        .map_err(|_| NexusError::validation(format!("dirección de correo inválida: {value}")))
}

fn imap_error(operation: &str, error: imap::Error) -> NexusError {
    infra_error(format!("IMAP {operation}"), error)
}

fn infra_error(operation: impl std::fmt::Display, error: impl std::fmt::Display) -> NexusError {
    NexusError::Internal(format!("{operation}: {error}"))
}
