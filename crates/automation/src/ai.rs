//! The AI assistant's tools (AJS fork, by autolisp.vn). They describe and build requests; they
//! never send one: the batch path opens no socket (`cargo xtask netcheck`). The desktop app sends
//! them with the user's own key.

use serde_json::{Value, json};

use super::Automation;
use super::error::{Result, ToolError};
use super::json::Args;

impl Automation {
    pub(crate) fn ai_providers(&self) -> Value {
        json!({
            "providers": pdfcraft_ai::PROVIDERS.iter().map(|p| json!({
                "id": p.id(),
                "label": p.label(),
                "default_base_url": p.default_base_url(),
                "suggested_models": p.suggested_models(),
                "key_env": p.key_env(),
                "needs_key": p.needs_key(),
                "key_page": p.key_page(),
            })).collect::<Vec<_>>(),
            "agents": pdfcraft_ai::AGENTS.iter().map(|a| json!({
                "id": a.id, "label": a.label, "description": a.description, "default_prompt": a.default_prompt,
            })).collect::<Vec<_>>(),
            "credit": pdfcraft_ai::CREDIT,
        })
    }

    pub(crate) fn ai_context(&mut self, a: &Args) -> Result<Value> {
        let agent_id = a.opt_str("agent")?.unwrap_or("ask");
        let Some(agent) = pdfcraft_ai::AGENTS.iter().find(|x| x.id == agent_id) else {
            let ids: Vec<&str> = pdfcraft_ai::AGENTS.iter().map(|x| x.id).collect();
            return Err(ToolError::InvalidArgs(format!("agent must be one of {}, not {agent_id:?}", ids.join(", "))));
        };
        let max_chars = match a.opt_int("max_chars")? {
            None => pdfcraft_ai::DEFAULT_CONTEXT_CHARS,
            Some(n) if (1..=2_000_000).contains(&n) => usize::try_from(n).unwrap_or(pdfcraft_ai::DEFAULT_CONTEXT_CHARS),
            Some(n) => return Err(ToolError::InvalidArgs(format!("max_chars must be 1 to 2000000, not {n}"))),
        };
        // Sending a document's text to a model takes it out, as copying does.
        let doc = self.doc_to_copy(a)?;
        let (name, source) = (doc.name.clone(), doc.export_source());
        let pages = match a.opt_ints("pages")? {
            Some(_) => self.pages(a, "pages")?,
            None => (0..source.pages).collect(),
        };
        let texts = pdfcraft_ai::extract_page_texts(source.bytes, source.config, &pages);
        let context = pdfcraft_ai::document_context(&texts, max_chars);
        let system = pdfcraft_ai::system_prompt(agent, &name, &context, a.opt_str("instructions")?.unwrap_or_default());
        let question = a.opt_str("question")?.map(str::trim).filter(|q| !q.is_empty()).unwrap_or(agent.default_prompt);
        Ok(json!({
            "agent": agent.id,
            "system": system,
            "messages": [{ "role": "user", "content": question }],
            "pages_sent": context.pages_sent,
            "pages_without_text": context.pages_without_text,
            "truncated": context.truncated,
            "chars": context.chars,
        }))
    }
}
