//! What goes over the wire to each provider and what comes back.
//!
//! Every function here is pure except [`chat`] and [`list_models`], which hand the request to the
//! caller's [`Transport`]. The key travels only in a header, never in the address, so it can't end
//! up in a proxy log or a browser history.

use serde_json::{Value, json};

use crate::provider::{Connection, ProviderKind, check_base_url};

/// The Anthropic API version sent with every Claude request.
const ANTHROPIC_VERSION: &str = "2026-10-21";
/// The most reply bytes read. Replies are text; this is far beyond any answer and stops a
/// misbehaving server from filling memory.
pub const MAX_REPLY_BYTES: u64 = 8 * 1024 * 1024;
/// Longest provider error message passed on to the user.
const MAX_ERROR_CHARS: usize = 400;

/// Who said a message of the conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
}

/// One question to the model: the instructions (with the document) and the conversation so far,
/// ending with the user's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<ChatMessage>,
    /// The longest answer allowed, in tokens.
    pub max_tokens: u32,
}

/// The model's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatReply {
    pub text: String,
    /// The model that answered, as the provider names it.
    pub model: String,
    /// The answer stopped at `max_tokens` and is incomplete.
    pub truncated: bool,
}

/// An HTTP request for the caller's [`Transport`] to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    /// `GET` or `POST`.
    pub method: &'static str,
    pub url: String,
    /// Includes the key. Never log them.
    pub headers: Vec<(String, String)>,
    /// JSON, for `POST`.
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Sends requests. The desktop app's sends them over HTTPS; tests answer them in memory.
pub trait Transport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, AiError>;
}

/// Why a request didn't give an answer. Each says what to do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiError {
    /// No usable connection: no key, or an address that isn't allowed.
    NotConfigured(String),
    /// The key was refused (401/403).
    Unauthorized(String),
    /// Too many requests or no credit left (429).
    RateLimited(String),
    /// The model doesn't exist or the account can't use it (404).
    ModelNotFound(String),
    /// The provider refused the request as made (400/413/422), with its reason.
    Rejected(String),
    /// The provider failed (5xx).
    Server(u16, String),
    /// The provider couldn't be reached: no network, DNS, TLS, proxy, timeout.
    Network(String),
    /// The reply wasn't the provider's format.
    BadReply(String),
    /// The model declined to answer.
    Refused(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(m) => write!(f, "the AI connection isn't set up: {m}"),
            Self::Unauthorized(m) => write!(f, "the provider refused the API key (check it, or create a new one): {m}"),
            Self::RateLimited(m) => write!(f, "too many requests or no credit left on the account; wait or check billing: {m}"),
            Self::ModelNotFound(m) => write!(f, "the model isn't available to this account; choose another: {m}"),
            Self::Rejected(m) => write!(f, "the provider refused the request: {m}"),
            Self::Server(status, m) => write!(f, "the provider had an error (HTTP {status}); try again later: {m}"),
            Self::Network(m) => write!(f, "couldn't reach the provider (check the network, proxy or address): {m}"),
            Self::BadReply(m) => write!(f, "the provider's reply couldn't be read: {m}"),
            Self::Refused(m) => write!(f, "the model declined to answer: {m}"),
        }
    }
}

impl std::error::Error for AiError {}

/// Send `request` over `transport` and read the answer.
pub fn chat(connection: &Connection, request: &ChatRequest, transport: &dyn Transport) -> Result<ChatReply, AiError> {
    let http = build_chat(connection, request)?;
    let response = transport.send(&http)?;
    parse_chat(connection.kind, &response)
}

/// The models the account can use, as the provider lists them (sorted, chat models only where the
/// provider says which).
pub fn list_models(connection: &Connection, transport: &dyn Transport) -> Result<Vec<String>, AiError> {
    let http = build_models(connection)?;
    let response = transport.send(&http)?;
    parse_models(connection.kind, &response)
}

/// The base URL, checked, and the headers that carry the key.
fn endpoint(connection: &Connection) -> Result<(String, Vec<(String, String)>), AiError> {
    let base = connection.base_url().to_owned();
    check_base_url(&base).map_err(AiError::NotConfigured)?;
    let key = connection.api_key.trim();
    if key.is_empty() && connection.kind.needs_key() {
        return Err(AiError::NotConfigured("no API key; add one in AI settings".into()));
    }
    if key.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AiError::NotConfigured("the API key contains spaces or line breaks; paste it again".into()));
    }
    let mut headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
    match connection.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            if !key.is_empty() {
                headers.push(("Authorization".into(), format!("Bearer {key}")));
            }
        }
        ProviderKind::Anthropic => {
            headers.push(("x-api-key".into(), key.to_owned()));
            headers.push(("anthropic-version".into(), ANTHROPIC_VERSION.into()));
        }
        ProviderKind::Gemini => headers.push(("x-goog-api-key".into(), key.to_owned())),
    }
    Ok((base, headers))
}

/// The request that asks `connection`'s model `request`.
pub fn build_chat(connection: &Connection, request: &ChatRequest) -> Result<HttpRequest, AiError> {
    let (base, headers) = endpoint(connection)?;
    let model = connection.model();
    if model.is_empty() || model.chars().any(char::is_control) {
        return Err(AiError::NotConfigured("choose a model".into()));
    }
    let max_tokens = request.max_tokens.max(1);
    let role = |r: Role, model_word: &'static str| match r {
        Role::User => "user",
        Role::Assistant => model_word,
    };
    let (url, body) = match connection.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            let mut messages = vec![json!({ "role": "system", "content": request.system })];
            messages.extend(request.messages.iter().map(|m| json!({ "role": role(m.role, "assistant"), "content": m.text })));
            // OpenAI's newer models take only max_completion_tokens; local servers know max_tokens.
            let limit = if connection.kind == ProviderKind::OpenAi { "max_completion_tokens" } else { "max_tokens" };
            let mut body = json!({ "model": model, "messages": messages });
            body[limit] = json!(max_tokens);
            (format!("{base}/chat/completions"), body)
        }
        ProviderKind::Anthropic => {
            let messages: Vec<Value> = request.messages.iter().map(|m| json!({ "role": role(m.role, "assistant"), "content": m.text })).collect();
            (format!("{base}/messages"), json!({ "model": model, "max_tokens": max_tokens, "system": request.system, "messages": messages }))
        }
        ProviderKind::Gemini => {
            let contents: Vec<Value> =
                request.messages.iter().map(|m| json!({ "role": role(m.role, "model"), "parts": [{ "text": m.text }] })).collect();
            let name = model.strip_prefix("models/").unwrap_or(model);
            (
                format!("{base}/models/{}:generateContent", encode_path(name)),
                json!({
                    "systemInstruction": { "parts": [{ "text": request.system }] },
                    "contents": contents,
                    "generationConfig": { "maxOutputTokens": max_tokens },
                }),
            )
        }
    };
    Ok(HttpRequest { method: "POST", url, headers, body: Some(body.to_string()) })
}

/// The request that lists `connection`'s models.
pub fn build_models(connection: &Connection) -> Result<HttpRequest, AiError> {
    let (base, mut headers) = endpoint(connection)?;
    headers.retain(|(name, _)| name != "Content-Type");
    let url = match connection.kind {
        // 1000 per page: the whole list in one request.
        ProviderKind::Anthropic => format!("{base}/models?limit=1000"),
        ProviderKind::Gemini => format!("{base}/models?pageSize=1000"),
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => format!("{base}/models"),
    };
    Ok(HttpRequest { method: "GET", url, headers, body: None })
}

/// The answer in `response`, or why there is none.
pub fn parse_chat(kind: ProviderKind, response: &HttpResponse) -> Result<ChatReply, AiError> {
    let v = json_of(response)?;
    let text_of = |parts: Option<&Vec<Value>>, key: &str| -> String {
        parts
            .into_iter()
            .flatten()
            .filter(|p| key != "text" || p["type"].as_str().is_none_or(|t| t == "text"))
            .filter_map(|p| p[key].as_str())
            .collect::<Vec<_>>()
            .join("")
    };
    let (text, model, stop) = match kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            let choice = &v["choices"][0];
            let message = &choice["message"];
            if let Some(refusal) = message["refusal"].as_str().filter(|r| !r.is_empty()) {
                return Err(AiError::Refused(short(refusal)));
            }
            let text = match &message["content"] {
                Value::String(s) => s.clone(),
                Value::Array(parts) => text_of(Some(parts), "text"),
                _ => String::new(),
            };
            (text, v["model"].as_str(), choice["finish_reason"].as_str().map(|s| s == "length"))
        }
        ProviderKind::Anthropic => {
            if v["stop_reason"].as_str() == Some("refusal") {
                let why = v["stop_details"]["explanation"].as_str().or(v["stop_details"]["category"].as_str()).unwrap_or("no reason given");
                return Err(AiError::Refused(short(why)));
            }
            (text_of(v["content"].as_array(), "text"), v["model"].as_str(), v["stop_reason"].as_str().map(|s| s == "max_tokens"))
        }
        ProviderKind::Gemini => {
            if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
                return Err(AiError::Refused(short(reason)));
            }
            let candidate = &v["candidates"][0];
            let finish = candidate["finishReason"].as_str();
            if matches!(finish, Some("SAFETY" | "RECITATION" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII")) {
                return Err(AiError::Refused(short(finish.unwrap_or_default())));
            }
            let parts = candidate["content"]["parts"].as_array();
            // Gemini marks its reasoning parts `thought: true`; only the answer is shown.
            let answer: Vec<Value> = parts.into_iter().flatten().filter(|p| p["thought"].as_bool() != Some(true)).cloned().collect();
            (text_of(Some(&answer), "text"), v["modelVersion"].as_str(), finish.map(|s| s == "MAX_TOKENS"))
        }
    };
    if text.trim().is_empty() {
        return Err(AiError::BadReply("the reply has no text".into()));
    }
    Ok(ChatReply { text, model: model.unwrap_or_default().to_owned(), truncated: stop.unwrap_or(false) })
}

/// The model names in `response`.
pub fn parse_models(kind: ProviderKind, response: &HttpResponse) -> Result<Vec<String>, AiError> {
    let v = json_of(response)?;
    let mut names: Vec<String> = match kind {
        ProviderKind::Gemini => v["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["supportedGenerationMethods"].as_array().is_none_or(|ms| ms.iter().any(|x| x.as_str() == Some("generateContent"))))
            .filter_map(|m| m["name"].as_str())
            .map(|n| n.strip_prefix("models/").unwrap_or(n).to_owned())
            .collect(),
        _ => v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).map(str::to_owned).collect(),
    };
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(AiError::BadReply("the provider listed no models".into()));
    }
    Ok(names)
}

/// The JSON body of a successful response, or the error the status and the provider's message say.
fn json_of(response: &HttpResponse) -> Result<Value, AiError> {
    let parsed: Option<Value> = serde_json::from_str(&response.body).ok();
    if !(200..300).contains(&response.status) {
        // OpenAI, Anthropic and Gemini all put the reason at error.message.
        let message = parsed
            .as_ref()
            .and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).or(v["message"].as_str()))
            .map(short)
            .unwrap_or_else(|| short(response.body.trim()));
        return Err(match response.status {
            401 | 403 => AiError::Unauthorized(message),
            404 => AiError::ModelNotFound(message),
            429 => AiError::RateLimited(message),
            400 | 409 | 413 | 422 => AiError::Rejected(message),
            status => AiError::Server(status, message),
        });
    }
    parsed.ok_or_else(|| AiError::BadReply(format!("not JSON: {}", short(response.body.trim()))))
}

/// `s` cut to [`MAX_ERROR_CHARS`] characters.
fn short(s: &str) -> String {
    match s.char_indices().nth(MAX_ERROR_CHARS) {
        Some((at, _)) => format!("{}…", s.get(..at).unwrap_or(s)),
        None => s.to_owned(),
    }
}

/// `name` safe as one path segment: letters, digits and `-._` stay, everything else is %-encoded.
fn encode_path(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
