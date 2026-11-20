//! Google login (OAuth loopback flow) and the few Gmail REST calls we use.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::LazyLock;
use std::time::Duration;

use base64::Engine;
use base64::engine::{GeneralPurpose, general_purpose};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::config::{DATA_DIR, credentials_path, token_path};

pub type Result<T> = std::result::Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

// ---------- login ----------

/// Full Gmail access: the only scope that allows permanent delete (batchDelete).
const SCOPE: &str = "https://mail.google.com/";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Serialize, Deserialize, Clone)]
struct Token {
    access_token: String,
    refresh_token: String,
    scope: String,
    /// unix seconds
    expires_at: i64,
}

/// Held while refreshing or logging in, so parallel calls don't open several logins.
static TOKEN: Mutex<Option<Token>> = Mutex::const_new(None);

#[derive(Deserialize, Clone)]
struct OAuthClient {
    client_id: String,
    client_secret: String,
}

pub fn has_credentials() -> bool {
    credentials_path().exists()
}

fn oauth_client() -> Result<OAuthClient> {
    #[derive(Deserialize)]
    struct File {
        installed: Option<OAuthClient>,
        web: Option<OAuthClient>,
    }
    let text = std::fs::read_to_string(credentials_path())
        .map_err(|_| format!("credentials.json missing in {}", DATA_DIR.display()))?;
    let f: File = serde_json::from_str(&text).map_err(err)?;
    f.installed.or(f.web).ok_or_else(|| "credentials.json has no OAuth client".into())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
    expires_in: i64,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

async fn token_request(params: &[(&str, &str)]) -> Result<TokenResponse> {
    let c = oauth_client()?;
    let mut form = vec![("client_id", c.client_id.as_str()), ("client_secret", c.client_secret.as_str())];
    form.extend_from_slice(params);
    let r = HTTP.post("https://oauth2.googleapis.com/token").form(&form).send().await.map_err(err)?;
    if !r.status().is_success() {
        return Err(format!("Google token error: {}", r.text().await.unwrap_or_default()));
    }
    r.json().await.map_err(err)
}

/// Opens Google login in the browser and waits for it to redirect back to a local port.
async fn login() -> Result<Token> {
    let client = oauth_client()?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(err)?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr().map_err(err)?.port());
    let state = format!("{:x}{:x}", RandomState::new().hash_one(1), RandomState::new().hash_one(2));
    let url = Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", client.client_id.as_str()),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", &state),
        ],
    )
    .map_err(err)?;
    let _ = open::that(url.as_str());

    let code = tokio::time::timeout(LOGIN_TIMEOUT, async {
        loop {
            let (mut sock, _) = listener.accept().await.map_err(err)?;
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let query: HashMap<String, String> = Url::parse(&format!("http://localhost{path}"))
                .map(|u| u.query_pairs().into_owned().collect())
                .unwrap_or_default();
            if query.get("state") != Some(&state) {
                continue; // favicon or a stray request
            }
            let body = if query.contains_key("code") { "Logged in to mailsort. You can close this tab." } else { "Login failed." };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(response.as_bytes()).await;
            return query
                .get("code")
                .cloned()
                .ok_or_else(|| format!("Google login failed: {}", query.get("error").cloned().unwrap_or_default()));
        }
    })
    .await
    .map_err(|_| "Google login timed out".to_string())??;

    let t = token_request(&[("code", &code), ("redirect_uri", &redirect), ("grant_type", "authorization_code")]).await?;
    Ok(Token {
        access_token: t.access_token,
        refresh_token: t.refresh_token.unwrap_or_default(),
        scope: t.scope.unwrap_or_default(),
        expires_at: now() + t.expires_in - 60,
    })
}

async fn access_token() -> Result<String> {
    let mut slot = TOKEN.lock().await;
    if slot.is_none() {
        *slot = std::fs::read_to_string(token_path()).ok().and_then(|t| serde_json::from_str(&t).ok());
    }
    if slot.as_ref().is_some_and(|t| !t.scope.split(' ').any(|s| s == SCOPE)) {
        *slot = None; // saved login lacks the scope we need
    }
    if let Some(t) = slot.as_ref().filter(|t| t.expires_at > now()) {
        return Ok(t.access_token.clone());
    }
    // Refresh fails once Google expires it (7 days for test-mode apps): log in again.
    let refreshed = match slot.as_ref().filter(|t| !t.refresh_token.is_empty()) {
        Some(old) => token_request(&[("refresh_token", &old.refresh_token), ("grant_type", "refresh_token")])
            .await
            .ok()
            .map(|r| Token {
                access_token: r.access_token,
                refresh_token: old.refresh_token.clone(),
                scope: r.scope.unwrap_or_else(|| old.scope.clone()),
                expires_at: now() + r.expires_in - 60,
            }),
        None => None,
    };
    let token = match refreshed {
        Some(t) => t,
        None => login().await?,
    };
    std::fs::write(token_path(), serde_json::to_string(&token).map_err(err)?).map_err(err)?;
    let access = token.access_token.clone();
    *slot = Some(token);
    Ok(access)
}

// ---------- Gmail API ----------

/// Subset of Gmail's Message resource that we use.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MessagePart {
    pub mime_type: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub body: Option<PartBody>,
    pub parts: Option<Vec<MessagePart>>,
}

#[derive(Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Deserialize, Default)]
pub struct PartBody {
    pub data: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    snippet: Option<String>,
    internal_date: Option<String>,
    payload: MessagePart,
}

/// Gmail allows ~15,000 quota units per user per minute; bursts get 429 or 403 "rate/quota".
fn is_rate_limit(status: u16, body: &str) -> bool {
    status == 429 || (status == 403 && (body.contains("ateLimitExceeded") || body.contains("Quota exceeded")))
}

/// Google's error JSON turned into a sentence a person can act on.
pub fn friendly_error(status: u16, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_else(|| body.chars().take(160).collect());
    match status {
        _ if is_rate_limit(status, body) => "Gmail is limiting how fast mailsort can make requests. Wait a minute, then press Sync.".into(),
        401 => "Your Google login expired. Press Sync to log in again.".into(),
        403 => format!("Gmail refused the request: {message}"),
        404 => "That email no longer exists in Gmail.".into(),
        500..=599 => "Gmail is having trouble right now. Try again in a moment.".into(),
        _ => format!("Gmail error {status}: {message}"),
    }
}

async fn gmail<T: DeserializeOwned>(method: Method, path: &str, query: &[(&str, &str)], body: Option<Value>) -> Result<T> {
    const RETRIES: u32 = 5;
    for attempt in 0..=RETRIES {
        let token = access_token().await?;
        let mut req = HTTP
            .request(method.clone(), format!("https://gmail.googleapis.com/gmail/v1/users/me/{path}"))
            .bearer_auth(token)
            .query(query);
        req = match &body {
            Some(b) => req.json(b),
            None => req.body(Vec::new()), // POSTs without a body still need Content-Length: 0
        };
        let r = req.send().await.map_err(|_| "Can't reach Gmail. Check your internet connection.".to_string())?;
        let status = r.status().as_u16();
        let text = r.text().await.map_err(err)?;
        if (200..300).contains(&status) {
            // batchDelete answers 204 with no body
            return serde_json::from_str(if text.is_empty() { "null" } else { &text }).map_err(err);
        }
        if is_rate_limit(status, &text) && attempt < RETRIES {
            // Back off 2, 4, 8, 16, 32 s, staggered so parallel requests don't retry in lockstep.
            let jitter = RandomState::new().hash_one(attempt) % 1000;
            tokio::time::sleep(Duration::from_millis(2000 * 2u64.pow(attempt) + jitter)).await;
            continue;
        }
        return Err(friendly_error(status, &text));
    }
    unreachable!("the last attempt always returns")
}

/// Message ids with a label (INBOX, TRASH), newest first. `complete` is false if `limit` cut it short.
pub async fn list_messages(label: &str, limit: usize) -> Result<(Vec<String>, bool)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct List {
        messages: Option<Vec<Id>>,
        next_page_token: Option<String>,
    }
    #[derive(Deserialize)]
    struct Id {
        id: String,
    }

    let mut ids = Vec::new();
    let mut page: Option<String> = None;
    loop {
        let max = (limit - ids.len()).min(500).to_string();
        let mut q = vec![("labelIds", label), ("maxResults", max.as_str()), ("includeSpamTrash", "true")];
        if let Some(p) = &page {
            q.push(("pageToken", p));
        }
        let r: List = gmail(Method::GET, "messages", &q, None).await?;
        ids.extend(r.messages.unwrap_or_default().into_iter().map(|m| m.id));
        page = r.next_page_token;
        if page.is_none() || ids.len() >= limit {
            break;
        }
    }
    Ok((ids, page.is_none()))
}

pub struct Headers {
    pub sender: String,
    pub subject: String,
    pub snippet: String,
    /// "2026-11-18 09:05", local time, sortable
    pub date: String,
}

pub async fn get_headers(id: &str) -> Result<Headers> {
    let m: Message = gmail(
        Method::GET,
        &format!("messages/{id}"),
        &[("format", "metadata"), ("metadataHeaders", "From"), ("metadataHeaders", "Subject")],
        None,
    )
    .await?;
    let header = |name: &str| {
        m.payload.headers.iter().flatten().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.clone()).unwrap_or_default()
    };
    let millis = m.internal_date.as_deref().and_then(|d| d.parse().ok()).unwrap_or(0);
    let date = chrono::DateTime::from_timestamp_millis(millis)
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
    Ok(Headers { sender: header("From"), subject: header("Subject"), snippet: unescape(m.snippet.as_deref().unwrap_or("")), date })
}

/// (mime, text) of the message, preferring HTML over plain text.
pub async fn get_body(id: &str) -> Result<(String, String)> {
    let m: Message = gmail(Method::GET, &format!("messages/{id}"), &[("format", "full")], None).await?;
    Ok(extract_body(&m.payload))
}

/// Moves to Gmail Trash, recoverable for 30 days.
pub async fn trash(id: &str) -> Result<()> {
    gmail::<Value>(Method::POST, &format!("messages/{id}/trash"), &[], None).await.map(drop)
}

pub async fn untrash(id: &str) -> Result<()> {
    gmail::<Value>(Method::POST, &format!("messages/{id}/untrash"), &[], None).await.map(drop)
}

/// Permanently deletes, skipping Trash. Cannot be undone. Gmail takes up to 1000 ids per call.
pub async fn delete_forever(ids: &[String]) -> Result<()> {
    for chunk in ids.chunks(1000) {
        gmail::<Value>(Method::POST, "messages/batchDelete", &[], Some(serde_json::json!({ "ids": chunk }))).await?;
    }
    Ok(())
}

/// Gmail encodes bodies as base64url, with or without padding.
const BASE64URL: GeneralPurpose = GeneralPurpose::new(&base64::alphabet::URL_SAFE, general_purpose::NO_PAD_INDIFFERENT);

pub fn extract_body(payload: &MessagePart) -> (String, String) {
    let mut found: HashMap<&str, String> = HashMap::new();
    let mut parts = vec![payload];
    while let Some(p) = parts.pop() {
        parts.extend(p.parts.iter().flatten());
        let mime = p.mime_type.as_deref().unwrap_or("");
        if let (Some(data), "text/html" | "text/plain") = (p.body.as_ref().and_then(|b| b.data.as_ref()), mime) {
            if let Ok(bytes) = BASE64URL.decode(data) {
                found.entry(if mime == "text/html" { "text/html" } else { "text/plain" })
                    .or_insert_with(|| String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    for mime in ["text/html", "text/plain"] {
        if let Some(text) = found.remove(mime) {
            return (mime.into(), text);
        }
    }
    ("text/plain".into(), String::new())
}

/// Gmail snippets come HTML-escaped.
fn unescape(s: &str) -> String {
    let numeric = regex::Regex::new(r"&#(\d+);").unwrap();
    numeric
        .replace_all(s, |c: &regex::Captures| c[1].parse().ok().and_then(char::from_u32).map(String::from).unwrap_or_default())
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(mime: &str, text: &str) -> MessagePart {
        let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text);
        MessagePart { mime_type: Some(mime.into()), body: Some(PartBody { data: Some(data) }), ..Default::default() }
    }

    #[test]
    fn extract_body_prefers_html_then_plain() {
        let msg = MessagePart {
            mime_type: Some("multipart/alternative".into()),
            parts: Some(vec![part("text/plain", "hi"), part("text/html", "<b>hi</b>")]),
            ..Default::default()
        };
        assert_eq!(extract_body(&msg), ("text/html".into(), "<b>hi</b>".into()));
        assert_eq!(extract_body(&part("text/plain", "hi")), ("text/plain".into(), "hi".into()));
        assert_eq!(extract_body(&MessagePart::default()), ("text/plain".into(), String::new()));
    }

    #[test]
    fn errors_read_like_sentences() {
        let quota = r#"{"error":{"code":403,"message":"Quota exceeded for quota metric 'Total Query Cost'"}}"#;
        assert!(is_rate_limit(403, quota));
        assert!(friendly_error(403, quota).starts_with("Gmail is limiting"));
        assert!(!is_rate_limit(403, r#"{"error":{"message":"Insufficient Permission"}}"#));
        assert_eq!(friendly_error(403, r#"{"error":{"message":"Insufficient Permission"}}"#), "Gmail refused the request: Insufficient Permission");
        assert!(friendly_error(401, "").contains("login expired"));
    }

    #[test]
    fn unescape_snippets() {
        assert_eq!(unescape("it&#39;s &quot;ok&quot; &amp; &lt;b&gt;"), "it's \"ok\" & <b>");
    }
}
