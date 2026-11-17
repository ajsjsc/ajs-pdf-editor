//! The AI assistant (AJS fork, written by autolisp.vn): the user connects their own provider
//! account — OpenAI, Claude, Gemini or an OpenAI-compatible server — with their own API key, picks
//! an agent and a model, and asks about the open document.
//!
//! Off until the user adds a connection: nothing is sent anywhere before that, and every request
//! is one the user starts. Keys are saved encrypted for the Windows account
//! (`pdfcraft_platform::secret`) and live in memory only otherwise; they are never logged or put
//! in a request's address. Requests run on a worker thread, so the window stays responsive; the
//! provider protocol is `pdfcraft_ai`'s, the HTTPS transport `ai/http.rs`.

use std::sync::{Arc, mpsc};

use pdfcraft_ai::{AiError, ChatMessage, ChatReply, ChatRequest, Connection, ProviderKind, Role, Transport};
use pdfcraft_engine::DocId;
use serde_json::{Value, json};

use crate::PdfCraftApp;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod http;
mod window;

#[cfg(test)]
mod tests;

pub(crate) use window::show;

/// The guides on the AJS Wiki, opened from the settings.
pub(crate) const GUIDE_KEYS: &str = "https://autolisp.vn/w/ape-lay-api-key";
pub(crate) const GUIDE_SETUP: &str = "https://autolisp.vn/w/ape-tich-hop-ai";
/// The longest answer asked for, in tokens: long enough for a translated page range or a full
/// summary, short enough to stay within a request's time.
const MAX_ANSWER_TOKENS: u32 = 8192;
/// Messages kept in one conversation; older ones are dropped from what is sent.
const MAX_TURNS: usize = 40;
/// The context budget can be set from this to [`MAX_CONTEXT_CHARS`] characters.
pub(crate) const MIN_CONTEXT_CHARS: usize = 2_000;
pub(crate) const MAX_CONTEXT_CHARS: usize = 2_000_000;

/// One saved connection: a provider account the user added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Profile {
    /// Stable within the settings; never reused while the profile exists.
    pub uid: u64,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// The key as `pdfcraft_platform::secret::protect` wrote it; empty when none is saved.
    pub key_blob: String,
}

impl Profile {
    pub(crate) fn label(&self) -> String {
        if self.name.trim().is_empty() { self.kind.label().to_owned() } else { self.name.clone() }
    }
}

/// What a worker sends back.
pub(crate) enum Outcome {
    /// The answer to the conversation's last message.
    Answer { doc: Option<DocId>, result: Result<ChatReply, AiError>, pages_sent: usize, truncated: bool },
    /// A connection's models.
    Models { uid: u64, result: Result<Vec<String>, AiError> },
}

/// A request on its way.
pub(crate) struct Job {
    receiver: mpsc::Receiver<(Outcome, Option<(u64, String)>)>,
    /// What the window says while it waits.
    pub what: String,
}

/// Everything the assistant keeps: settings (saved with the preferences) and the conversation.
pub(crate) struct AiState {
    pub open: bool,
    pub settings_open: bool,
    pub profiles: Vec<Profile>,
    /// The uid of the connection in use.
    pub active: Option<u64>,
    pub agent: &'static str,
    /// Send the whole document (`true`) or the current page only.
    pub whole_document: bool,
    pub context_chars: usize,
    /// The user's own standing instructions, added to every agent's.
    pub instructions: String,
    /// The conversation, about [`Self::conversation_doc`].
    pub messages: Vec<ChatMessage>,
    pub conversation_doc: Option<DocId>,
    pub input: String,
    pub job: Option<Job>,
    /// The last error or note, shown above the input.
    pub status: Option<Result<String, String>>,
    /// Model lists fetched from the providers, by connection uid.
    pub models: std::collections::HashMap<u64, Vec<String>>,
    /// Keys in clear text, by connection uid: read from the saved form once, or typed this session.
    keys: std::collections::HashMap<u64, String>,
    /// The connection being added or changed in the settings.
    pub draft: Option<window::Draft>,
    /// A text field of the assistant has the keyboard: the pages leave the keys alone.
    pub typing: bool,
    /// Sends the requests; tests replace it.
    pub transport: Option<Arc<dyn Transport + Send + Sync>>,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            open: false,
            settings_open: false,
            profiles: Vec::new(),
            active: None,
            agent: pdfcraft_ai::AGENTS[0].id,
            whole_document: true,
            context_chars: pdfcraft_ai::DEFAULT_CONTEXT_CHARS,
            instructions: String::new(),
            messages: Vec::new(),
            conversation_doc: None,
            input: String::new(),
            job: None,
            status: None,
            models: Default::default(),
            keys: Default::default(),
            draft: None,
            typing: false,
            transport: default_transport(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn default_transport() -> Option<Arc<dyn Transport + Send + Sync>> {
    Some(Arc::new(http::HttpTransport))
}

/// The web build sends nothing: a browser page can't call these APIs with the user's key safely.
#[cfg(target_arch = "wasm32")]
fn default_transport() -> Option<Arc<dyn Transport + Send + Sync>> {
    None
}

impl AiState {
    pub(crate) fn active_profile(&self) -> Option<&Profile> {
        self.active.and_then(|uid| self.profiles.iter().find(|p| p.uid == uid)).or_else(|| self.profiles.first())
    }

    pub(crate) fn has_key(&self, p: &Profile) -> bool {
        !p.key_blob.is_empty() || self.keys.get(&p.uid).is_some_and(|k| !k.is_empty()) || env_key(p.kind).is_some()
    }

    /// Keep `key` for `uid` this session (it was just typed or read back).
    pub(crate) fn remember_key(&mut self, uid: u64, key: String) {
        self.keys.insert(uid, key);
    }

    pub(crate) fn forget(&mut self, uid: u64) {
        self.profiles.retain(|p| p.uid != uid);
        self.keys.remove(&uid);
        self.models.remove(&uid);
        if self.active == Some(uid) {
            self.active = self.profiles.first().map(|p| p.uid);
        }
    }

    pub(crate) fn next_uid(&self) -> u64 {
        self.profiles.iter().map(|p| p.uid).max().unwrap_or(0).saturating_add(1)
    }

    /// The settings as saved with the preferences: keys only in their encrypted form.
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "profiles": self.profiles.iter().map(|p| json!({
                "id": p.uid, "name": p.name, "provider": p.kind.id(), "base_url": p.base_url, "model": p.model, "key": p.key_blob,
            })).collect::<Vec<_>>(),
            "active": self.active,
            "agent": self.agent,
            "whole_document": self.whole_document,
            "context_chars": self.context_chars,
            "instructions": self.instructions,
        })
    }

    /// Restore [`Self::to_json`]; anything malformed keeps its default.
    pub(crate) fn apply_json(&mut self, v: &Value) {
        if let Some(list) = v["profiles"].as_array() {
            self.profiles = list
                .iter()
                .take(64)
                .filter_map(|p| {
                    Some(Profile {
                        uid: p["id"].as_u64()?,
                        name: text(&p["name"], 200),
                        kind: ProviderKind::from_id(p["provider"].as_str()?)?,
                        base_url: text(&p["base_url"], 500),
                        model: text(&p["model"], 200),
                        // Only an encrypted value is taken; a key in clear text is never read from settings.
                        key_blob: p["key"].as_str().filter(|k| k.starts_with("dpapi:") && k.len() < 65_536).unwrap_or_default().to_owned(),
                    })
                })
                .collect();
            let mut seen = std::collections::HashSet::new();
            self.profiles.retain(|p| seen.insert(p.uid));
        }
        self.active = v["active"].as_u64().filter(|uid| self.profiles.iter().any(|p| p.uid == *uid)).or_else(|| self.profiles.first().map(|p| p.uid));
        if let Some(agent) = v["agent"].as_str() {
            self.agent = pdfcraft_ai::agent(agent).id;
        }
        if let Some(whole) = v["whole_document"].as_bool() {
            self.whole_document = whole;
        }
        if let Some(n) = v["context_chars"].as_u64().and_then(|n| usize::try_from(n).ok()) {
            self.context_chars = n.clamp(MIN_CONTEXT_CHARS, MAX_CONTEXT_CHARS);
        }
        self.instructions = text(&v["instructions"], 4000);
    }
}

/// A saved string, cut to `max` characters.
fn text(v: &Value, max: usize) -> String {
    v.as_str().unwrap_or_default().chars().take(max).collect()
}

/// The provider's key from its environment variable, if the user set one.
fn env_key(kind: ProviderKind) -> Option<String> {
    kind.key_env().iter().filter_map(|name| std::env::var(name).ok()).map(|v| v.trim().to_owned()).find(|v| !v.is_empty())
}

/// The key of `p` in clear text: typed this session, else read back from its saved form (slow:
/// call off the UI thread), else the environment's. The second value is what to remember.
fn key_for(p: &Profile, cached: Option<String>) -> Result<(String, Option<(u64, String)>), AiError> {
    if let Some(k) = cached.filter(|k| !k.is_empty()) {
        return Ok((k, None));
    }
    if !p.key_blob.is_empty() {
        // Elsewhere than Windows this is an error that says so (pdfcraft_platform::secret).
        return match pdfcraft_platform::secret::unprotect(&p.key_blob) {
            Ok(k) => Ok((k.clone(), Some((p.uid, k)))),
            Err(e) => Err(AiError::NotConfigured(format!("the saved key can't be read on this account or computer ({e}); enter it again"))),
        };
    }
    Ok((env_key(p.kind).unwrap_or_default(), None))
}

impl PdfCraftApp {
    /// Open the assistant with `agent` chosen (`None` keeps the current one), ready to send its
    /// usual request. Nothing is sent until the user says so.
    pub(crate) fn open_ai(&mut self, agent: Option<&str>) {
        self.ai.open = true;
        if let Some(id) = agent {
            let agent = pdfcraft_ai::agent(id);
            self.ai.agent = agent.id;
            if self.ai.input.trim().is_empty() || pdfcraft_ai::AGENTS.iter().any(|a| tl!(a.default_prompt) == self.ai.input) {
                self.ai.input = tl!(agent.default_prompt).to_owned();
            }
        }
        if self.ai.profiles.is_empty() {
            self.ai.settings_open = true;
        }
    }

    /// Send the input (or the agent's usual request) with the active document as context.
    pub(crate) fn ai_send(&mut self) {
        if self.ai.job.is_some() {
            return;
        }
        let Some(profile) = self.ai.active_profile().cloned() else {
            self.ai.status = Some(Err(tl!("Add an AI connection with your API key first (AI settings).").into()));
            self.ai.settings_open = true;
            return;
        };
        let Some((view, id)) = self.active_ids() else {
            self.ai.status = Some(Err(tl!("Open a document to ask about.").into()));
            return;
        };
        let Some(doc) = self.session.get(id) else { return };
        // Sending the text to a provider takes it out of the document, as copying does.
        if let Err(e) = doc.check_copying() {
            self.ai.status = Some(Err(e.to_string()));
            return;
        }
        let Some(transport) = self.ai.transport.clone() else {
            self.ai.status = Some(Err(tl!("The AI assistant needs the desktop app.").into()));
            return;
        };
        let agent = pdfcraft_ai::agent(self.ai.agent);
        let typed = self.ai.input.trim().to_owned();
        let question = if typed.is_empty() { tl!(agent.default_prompt).to_owned() } else { typed };
        if self.ai.conversation_doc != Some(id) {
            self.ai.messages.clear();
            self.ai.conversation_doc = Some(id);
        }
        self.ai.messages.push(ChatMessage { role: Role::User, text: question });
        self.ai.input.clear();
        self.ai.status = None;

        let source = doc.export_source();
        let pages: Vec<usize> = if self.ai.whole_document { (0..source.pages).collect() } else { vec![self.views[view].current] };
        let name = doc.name.clone();
        let skip = self.ai.messages.len().saturating_sub(MAX_TURNS);
        let messages: Vec<ChatMessage> = self.ai.messages.iter().skip(skip).cloned().collect();
        let (budget, instructions) = (self.ai.context_chars, self.ai.instructions.clone());
        let cached = self.ai.keys.get(&profile.uid).cloned();
        let connection = Connection { kind: profile.kind, base_url: profile.base_url.clone(), api_key: String::new(), model: profile.model.clone() };
        let what = crate::i18n::fmt(tl!("Asking {model}…"), &[("model", connection.model())]);
        let doc_id = Some(id);
        self.ai_spawn(what, move || {
            let (key, remember) = match key_for(&profile, cached) {
                Ok(k) => k,
                Err(e) => return (Outcome::Answer { doc: doc_id, result: Err(e), pages_sent: 0, truncated: false }, None),
            };
            let connection = Connection { api_key: key, ..connection };
            let texts = pdfcraft_ai::extract_page_texts(source.bytes, source.config, &pages);
            let context = pdfcraft_ai::document_context(&texts, budget);
            let request =
                ChatRequest { system: pdfcraft_ai::system_prompt(agent, &name, &context, &instructions), messages, max_tokens: MAX_ANSWER_TOKENS };
            let result = pdfcraft_ai::chat(&connection, &request, transport.as_ref());
            (Outcome::Answer { doc: doc_id, result, pages_sent: context.pages_sent.len(), truncated: context.truncated }, remember)
        });
    }

    /// Fetch the models connection `uid` can use (from the settings).
    pub(crate) fn ai_fetch_models(&mut self, profile: Profile, key: Option<String>) {
        if self.ai.job.is_some() {
            return;
        }
        let Some(transport) = self.ai.transport.clone() else { return };
        let cached = key.or_else(|| self.ai.keys.get(&profile.uid).cloned());
        let what = crate::i18n::fmt(tl!("Asking {provider} for its models…"), &[("provider", profile.kind.label())]);
        self.ai_spawn(what, move || {
            let uid = profile.uid;
            let (key, remember) = match key_for(&profile, cached) {
                Ok(k) => k,
                Err(e) => return (Outcome::Models { uid, result: Err(e) }, None),
            };
            let connection = Connection { kind: profile.kind, base_url: profile.base_url.clone(), api_key: key, model: profile.model.clone() };
            (Outcome::Models { uid, result: pdfcraft_ai::list_models(&connection, transport.as_ref()) }, remember)
        });
    }

    /// Run `work` on a worker thread (inline in tests that ask for it).
    fn ai_spawn(&mut self, what: String, work: impl FnOnce() -> (Outcome, Option<(u64, String)>) + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        self.ai.job = Some(Job { receiver, what });
        let ctx = self.ctx.clone();
        let run = move || {
            let _ = sender.send(work());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        if !self.run_inline {
            if let Err(e) = std::thread::Builder::new().name("ai-request".into()).spawn(run) {
                self.ai.job = None;
                self.ai.status = Some(Err(e.to_string()));
            }
            return;
        }
        run();
    }

    /// Take a worker's answer, if one came (each frame).
    pub(crate) fn poll_ai(&mut self) {
        let Some(job) = &self.ai.job else { return };
        let (outcome, remember) = match job.receiver.try_recv() {
            Ok(got) => got,
            Err(mpsc::TryRecvError::Empty) => return,
            // A panic in the worker ends its thread without an answer; the app carries on.
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ai.job = None;
                self.ai.status = Some(Err(tl!("The AI request stopped unexpectedly.").into()));
                return;
            }
        };
        self.ai.job = None;
        if let Some((uid, key)) = remember {
            self.ai.keys.insert(uid, key);
        }
        match outcome {
            Outcome::Answer { doc, result, pages_sent, truncated } => {
                // The answer belongs to the conversation that asked; one started since gets none.
                if doc != self.ai.conversation_doc {
                    return;
                }
                match result {
                    Ok(reply) => {
                        let mut note = crate::i18n::fmt(
                            tl!("{model} answered, using the text of {n} page(s)."),
                            &[("model", &reply.model), ("n", &pages_sent.to_string())],
                        );
                        if truncated {
                            note.push(' ');
                            note.push_str(tl!("Only part of the document was sent; raise the limit in AI settings to send more."));
                        }
                        if reply.truncated {
                            note.push(' ');
                            note.push_str(tl!("The answer was cut at its length limit."));
                        }
                        self.ai.messages.push(ChatMessage { role: Role::Assistant, text: reply.text });
                        self.ai.status = Some(Ok(note));
                    }
                    Err(e) => {
                        // The question stays in the input to send again; it isn't part of the conversation.
                        if let Some(last) = self.ai.messages.pop_if(|m| m.role == Role::User) {
                            self.ai.input = last.text;
                        }
                        self.ai.status = Some(Err(error_text(&e)));
                    }
                }
            }
            Outcome::Models { uid, result } => match result {
                Ok(models) => {
                    let n = models.len();
                    self.ai.models.insert(uid, models);
                    self.ai.status = Some(Ok(crate::i18n::fmt(tl!("Connected: {n} models available."), &[("n", &n.to_string())])));
                }
                Err(e) => self.ai.status = Some(Err(error_text(&e))),
            },
        }
    }
}

/// `e` in the interface language, with the provider's own message after it.
pub(crate) fn error_text(e: &AiError) -> String {
    let (what, detail) = match e {
        AiError::NotConfigured(m) => (tl!("The AI connection isn't set up."), m.as_str()),
        AiError::Unauthorized(m) => (tl!("The provider refused the API key. Check it in AI settings, or create a new key."), m.as_str()),
        AiError::RateLimited(m) => (tl!("Too many requests, or no credit left on the provider account. Wait a moment or check billing."), m.as_str()),
        AiError::ModelNotFound(m) => (tl!("This model isn't available to your account. Choose another model."), m.as_str()),
        AiError::Rejected(m) => {
            (tl!("The provider refused the request. The document may be too long for this model: lower the limit in AI settings."), m.as_str())
        }
        AiError::Server(_, m) => (tl!("The provider had an error. Try again later."), m.as_str()),
        AiError::Network(m) => (tl!("Couldn't reach the provider. Check the network, proxy or API address."), m.as_str()),
        AiError::BadReply(m) => (tl!("The provider's reply couldn't be read."), m.as_str()),
        AiError::Refused(m) => (tl!("The model declined to answer."), m.as_str()),
    };
    if detail.is_empty() { what.to_owned() } else { format!("{what} ({detail})") }
}
