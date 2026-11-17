//! The agents the assistant offers: each one is a role with its own instructions. The user picks
//! one, as they pick a model, before asking.

/// One role of the assistant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Agent {
    /// The stable name saved in settings and taken by tools.
    pub id: &'static str,
    /// The name shown (English; the app translates it).
    pub label: &'static str,
    /// What it does, for the agent list (English; translated by the app).
    pub description: &'static str,
    /// Lucide icon name.
    pub icon: &'static str,
    /// What the model is told before the document and the question.
    pub instructions: &'static str,
    /// What is asked when the user sends nothing of their own (a one-click action).
    pub default_prompt: &'static str,
}

/// Every agent, the default first. An array, so `AGENTS[0]` is checked when it compiles.
pub static AGENTS: [Agent; 5] = [
    Agent {
        id: "ask",
        label: "Ask about this document",
        description: "Answers questions using the document, citing pages",
        icon: "message-circle-reply",
        instructions: "You are a careful assistant that answers questions about a PDF document. Use only the document text \
            provided, and say so when the answer is not in it. After each fact, cite the page it comes from as (page N). \
            Answer in the language of the question.",
        default_prompt: "What is this document about? List its main points with their pages.",
    },
    Agent {
        id: "summary",
        label: "Summarize",
        description: "Short summary with the key points and their pages",
        icon: "sparkles",
        instructions: "You summarize PDF documents. Write a short overview (2-4 sentences), then the key points as a bulleted \
            list, each with its page as (page N), then any dates, amounts, deadlines or obligations. Do not add facts that \
            are not in the document. Write in Vietnamese unless the user asks for another language.",
        default_prompt: "Summarize this document.",
    },
    Agent {
        id: "translate",
        label: "Translate",
        description: "Translates the text, keeping headings and lists",
        icon: "languages",
        instructions: "You translate documents. Translate the document text into Vietnamese unless the user names another \
            language. Keep headings, numbering, lists and tables as plain text in the same order, mark each page as \
            '--- Page N ---', and do not summarize or leave anything out.",
        default_prompt: "Translate this document into Vietnamese.",
    },
    Agent {
        id: "proofread",
        label: "Proofread",
        description: "Finds spelling, grammar and consistency errors",
        icon: "list-checks",
        instructions: "You proofread documents. List spelling, grammar, punctuation and consistency errors (names, numbers, \
            units, dates) as: page, the wrong text, the correction, and a short reason. Do not rewrite text that is \
            correct. Answer in the language of the document.",
        default_prompt: "Proofread this document.",
    },
    Agent {
        id: "extract",
        label: "Extract data",
        description: "Pulls out tables, quantities, dates and names",
        icon: "file-spreadsheet",
        instructions: "You extract structured data from documents. Return the requested items (by default: names, \
            organizations, dates, amounts, quantities and their units, codes and reference numbers) as a Markdown table \
            with a Page column. Copy values exactly as written; do not compute or guess missing values.",
        default_prompt: "Extract the important data from this document as a table.",
    },
];

/// The agent named `id`, or the default one.
pub fn agent(id: &str) -> &'static Agent {
    AGENTS.iter().find(|a| a.id == id).unwrap_or(&AGENTS[0])
}
