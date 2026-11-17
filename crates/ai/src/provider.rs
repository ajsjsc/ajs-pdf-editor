//! The providers the assistant can use, and one configured connection to one of them.

/// Which API a connection speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// OpenAI's Chat Completions API.
    OpenAi,
    /// Anthropic's Messages API (Claude).
    Anthropic,
    /// Google's Gemini API (Google AI Studio keys).
    Gemini,
    /// Any server that speaks OpenAI's Chat Completions API: Ollama, LM Studio, OpenRouter,
    /// Groq, DeepSeek, Mistral and others. The base URL says which.
    OpenAiCompatible,
}

/// Every provider, in the order the settings list them.
pub const PROVIDERS: &[ProviderKind] = &[ProviderKind::OpenAi, ProviderKind::Anthropic, ProviderKind::Gemini, ProviderKind::OpenAiCompatible];

impl ProviderKind {
    /// The stable name saved in settings and taken by tools.
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::OpenAiCompatible => "openai_compatible",
        }
    }

    /// The provider named `id` ([`Self::id`]).
    pub fn from_id(id: &str) -> Option<Self> {
        PROVIDERS.iter().copied().find(|p| p.id() == id)
    }

    /// The name shown to people (a product name, not translated).
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI (ChatGPT)",
            Self::Anthropic => "Anthropic (Claude)",
            Self::Gemini => "Google Gemini (AI Studio)",
            Self::OpenAiCompatible => "OpenAI-compatible (Ollama, LM Studio, OpenRouter…)",
        }
    }

    /// The API's base URL when the connection names none.
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            // Ollama's address on the same computer; LM Studio uses http://localhost:1234/v1.
            Self::OpenAiCompatible => "http://localhost:11434/v1",
        }
    }

    /// Models offered before the provider's own list has been fetched. The first is the default.
    /// The list from the provider ([`crate::list_models`]) is what counts; these only save typing.
    pub fn suggested_models(self) -> &'static [&'static str] {
        match self {
            Self::OpenAi => &["gpt-6.1-sol", "gpt-6-luna", "gpt-6.1-terra", "gpt-6-astra"],
            Self::Anthropic => &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-2"],
            Self::Gemini => &["gemini-4-argon", "gemini-4-hydrogen", "gemini-4-carbon"],
            Self::OpenAiCompatible => &["llama3.2", "qwen2.5", "gemma3"],
        }
    }

    /// Environment variables a key may come from, in order, when none is saved.
    pub fn key_env(self) -> &'static [&'static str] {
        match self {
            Self::OpenAi => &["OPENAI_API_KEY"],
            Self::Anthropic => &["ANTHROPIC_API_KEY"],
            Self::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
            Self::OpenAiCompatible => &[],
        }
    }

    /// Whether requests need a key. A local OpenAI-compatible server usually doesn't.
    pub fn needs_key(self) -> bool {
        !matches!(self, Self::OpenAiCompatible)
    }

    /// Where the user creates a key (opened from the settings; never fetched by the app).
    pub fn key_page(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("https://platform.openai.com/api-keys"),
            Self::Anthropic => Some("https://platform.claude.com/settings/keys"),
            Self::Gemini => Some("https://aistudio.google.com/apikey"),
            Self::OpenAiCompatible => None,
        }
    }
}

/// One provider account the user connected: which API, where, with which key and model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    pub kind: ProviderKind,
    /// The API's base URL; empty means [`ProviderKind::default_base_url`].
    pub base_url: String,
    /// The secret key, in memory only. Saving it is the caller's job (encrypted).
    pub api_key: String,
    /// The model to ask; empty means the first of [`ProviderKind::suggested_models`].
    pub model: String,
}

impl Connection {
    pub fn new(kind: ProviderKind) -> Self {
        Self { kind, base_url: String::new(), api_key: String::new(), model: String::new() }
    }

    /// The base URL requests go to, without a trailing `/`.
    pub fn base_url(&self) -> &str {
        let url = self.base_url.trim();
        if url.is_empty() { self.kind.default_base_url() } else { url.trim_end_matches('/') }
    }

    /// The model requests name.
    pub fn model(&self) -> &str {
        let model = self.model.trim();
        if model.is_empty() { self.kind.suggested_models().first().copied().unwrap_or_default() } else { model }
    }
}

/// `Ok` when `url` may carry a key and a document: HTTPS anywhere, or plain HTTP to this
/// computer only (a local server such as Ollama). Anything else would send them in clear text.
pub fn check_base_url(url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("https://") {
        return if host_of(rest).is_empty() { Err(format!("{url} names no host")) } else { Ok(()) };
    }
    if let Some(rest) = lower.strip_prefix("http://") {
        let host = host_of(rest);
        let loopback = host == "localhost" || host == "[::1]" || host.starts_with("127.") || host.ends_with(".localhost");
        return if loopback { Ok(()) } else { Err(format!("{url}: plain http is allowed only for this computer (localhost); use https")) };
    }
    Err(format!("{url}: the address must start with https:// (or http://localhost for a local server)"))
}

/// The host of the part of a URL after its scheme, without a port.
fn host_of(rest: &str) -> &str {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // No user info: a key never belongs in the address.
    let authority = authority.rsplit('@').next().unwrap_or_default();
    if authority.starts_with('[') {
        return authority.split_inclusive(']').next().unwrap_or_default();
    }
    authority.split(':').next().unwrap_or_default()
}
