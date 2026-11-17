//! pdfcraft-ai — L4 "bring your own model" support for the AI assistant (AJS fork, written by
//! autolisp.vn).
//!
//! The user connects their own provider account (OpenAI, Anthropic Claude, Google Gemini, or any
//! OpenAI-compatible server such as Ollama or LM Studio) with their own API key. This crate knows
//! how to talk to each provider — the request it sends and the reply it reads — which agents
//! (roles) the assistant offers, and how a document's text becomes the context of a question.
//!
//! It opens no socket: [`chat`] and [`list_models`] hand an [`HttpRequest`] to a [`Transport`]
//! the caller supplies. The desktop app supplies one over HTTPS; the batch path
//! (`pdfcraft-cli`, `pdfcraft-automation`) never does, so `cargo xtask netcheck` still holds.
//! Nothing here is on by default: without a configured connection nothing is sent anywhere.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod agents;
pub mod context;
pub mod provider;
pub mod wire;

#[cfg(test)]
mod tests;

pub use agents::{AGENTS, Agent, agent};
pub use context::{DEFAULT_CONTEXT_CHARS, DocContext, document_context, extract_page_texts, system_prompt};
pub use provider::{Connection, PROVIDERS, ProviderKind, check_base_url};
pub use wire::{
    AiError, ChatMessage, ChatReply, ChatRequest, HttpRequest, HttpResponse, MAX_REPLY_BYTES, Role, Transport, build_chat, build_models, chat,
    list_models, parse_chat, parse_models,
};

/// Shown with the assistant: who wrote the AI integration of this fork.
pub const CREDIT: &str = "AI integration by autolisp.vn";
/// Where the credit links to.
pub const CREDIT_URL: &str = "https://autolisp.vn";
