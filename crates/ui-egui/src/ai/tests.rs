use std::sync::{Arc, Mutex};

use egui_kittest::{Harness, kittest::Queryable};
use pdfcraft_ai::{AiError, HttpRequest, HttpResponse, ProviderKind, Role, Transport};
use serde_json::json;

use super::{Profile, error_text};
use crate::PdfCraftApp;

/// Answers like the provider would, and keeps what was sent.
struct Fake {
    status: u16,
    body: String,
    sent: Mutex<Vec<HttpRequest>>,
}

impl Transport for Fake {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, AiError> {
        self.sent.lock().unwrap().push(request.clone());
        Ok(HttpResponse { status: self.status, body: self.body.clone() })
    }
}

fn fake(status: u16, body: serde_json::Value) -> Arc<Fake> {
    Arc::new(Fake { status, body: body.to_string(), sent: Mutex::new(Vec::new()) })
}

const ANSWER: &str = "**Hợp đồng số 12** (trang 1).\n- Thanh toán ở Điều 5 (trang 2).";

/// An app with a two-page document, an OpenAI connection whose key was typed this session, and
/// `transport` in place of the network.
fn app(transport: Arc<Fake>) -> PdfCraftApp {
    let bytes = pdfcraft_engine::Session::new().create_from_text("Hop dong", "Hop dong so 12\u{c}Dieu 5 thanh toan").unwrap();
    let mut app = PdfCraftApp::new();
    app.run_inline = true;
    app.set_option("language", "en").unwrap();
    app.open_bytes("hop-dong.pdf", None, bytes.as_ref().clone()).unwrap();
    app.ai.profiles.push(Profile {
        uid: 1,
        name: "Work".into(),
        kind: ProviderKind::OpenAi,
        base_url: String::new(),
        model: "gpt-5-mini".into(),
        key_blob: String::new(),
    });
    app.ai.active = Some(1);
    app.ai.remember_key(1, "sk-test-typed".into());
    app.ai.transport = Some(transport);
    app
}

#[test]
fn asking_sends_the_document_and_shows_the_answer() {
    let t = fake(200, json!({ "model": "gpt-5-mini", "choices": [{ "message": { "content": ANSWER }, "finish_reason": "stop" }] }));
    let mut app = app(t.clone());
    app.run_command("ai.summary");
    assert!(app.ai.open);
    assert_eq!(app.ai.agent, "summary");
    assert_eq!(app.ai.input, "Summarize this document.", "the agent's usual request, not yet sent");
    assert!(t.sent.lock().unwrap().is_empty(), "nothing is sent before the user says so");
    app.ai_send();
    app.poll_ai();
    let sent = t.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    let r = &sent[0];
    assert_eq!(r.url, "https://api.openai.com/v1/chat/completions");
    assert!(r.headers.contains(&("Authorization".into(), "Bearer sk-test-typed".into())));
    let body: serde_json::Value = serde_json::from_str(r.body.as_deref().unwrap()).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with(pdfcraft_ai::agent("summary").instructions));
    assert!(system.contains("<document name=\"hop-dong.pdf\">"), "{system}");
    assert!(system.contains("[Page 1]\nHop dong so 12") && system.contains("[Page 2]\nDieu 5 thanh toan"), "{system}");
    assert_eq!(body["messages"][1]["content"], "Summarize this document.");
    assert_eq!(app.ai.messages.len(), 2);
    assert_eq!(app.ai.messages[1].role, Role::Assistant);
    assert_eq!(app.ai.messages[1].text, ANSWER);
    assert!(matches!(&app.ai.status, Some(Ok(note)) if note.contains("2 page(s)")), "{:?}", app.ai.status);
    assert!(app.ai.input.is_empty());
}

#[test]
fn the_current_page_alone_can_be_sent_and_the_conversation_continues() {
    let t = fake(200, json!({ "choices": [{ "message": { "content": "ok" } }] }));
    let mut app = app(t.clone());
    app.ai.whole_document = false;
    app.ai.input = "Điều 5 nói gì?".into();
    app.ai_send();
    app.poll_ai();
    app.ai.input = "Và trang 1?".into();
    app.ai_send();
    app.poll_ai();
    let sent = t.sent.lock().unwrap().clone();
    let body: serde_json::Value = serde_json::from_str(sent[1].body.as_deref().unwrap()).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("[Page 1]") && !system.contains("[Page 2]"), "only the page in view: {system}");
    // System, then the first question and answer, then the follow-up.
    assert_eq!(body["messages"].as_array().unwrap().len(), 4);
    assert_eq!(body["messages"][3]["content"], "Và trang 1?");
}

#[test]
fn a_refused_key_says_what_to_do_and_keeps_the_question() {
    let t = fake(401, json!({ "error": { "message": "Incorrect API key provided" } }));
    let mut app = app(t);
    app.ai.input = "Tóm tắt".into();
    app.ai_send();
    app.poll_ai();
    assert!(app.ai.messages.is_empty(), "the failed question isn't part of the conversation");
    assert_eq!(app.ai.input, "Tóm tắt", "it is back in the box to send again");
    let Some(Err(e)) = &app.ai.status else { panic!("{:?}", app.ai.status) };
    assert!(e.contains("refused the API key") && e.contains("Incorrect API key provided"), "{e}");
}

#[test]
fn nothing_is_sent_without_a_connection_or_a_document() {
    let t = fake(200, json!({}));
    let mut app = PdfCraftApp::new();
    app.run_inline = true;
    app.set_option("language", "en").unwrap();
    app.ai.transport = Some(t.clone());
    app.run_command("ai.assistant");
    assert!(app.ai.open && app.ai.settings_open, "with no connection the settings open too");
    app.ai_send();
    assert!(matches!(&app.ai.status, Some(Err(e)) if e.contains("Add an AI connection")));
    let mut app = self::app(t.clone());
    app.run_command("file.close_all");
    app.ai_send();
    assert!(matches!(&app.ai.status, Some(Err(e)) if e.contains("Open a document")), "{:?}", app.ai.status);
    assert!(t.sent.lock().unwrap().is_empty());
}

#[test]
fn settings_keep_keys_only_encrypted() {
    let mut app = app(fake(200, json!({})));
    app.ai.profiles[0].key_blob = "dpapi:0011aabb".into();
    app.ai.agent = "translate";
    app.ai.context_chars = 50_000;
    let saved = app.ai.to_json();
    let text = saved.to_string();
    assert!(!text.contains("sk-test-typed"), "a key typed this session is never saved in clear: {text}");
    assert!(text.contains("dpapi:0011aabb"));
    let mut restored = super::AiState::default();
    restored.apply_json(&saved);
    assert_eq!(restored.profiles, app.ai.profiles);
    assert_eq!((restored.active, restored.agent, restored.context_chars), (Some(1), "translate", 50_000));
    // A hand-edited file with a key in clear text: the key is ignored, the connection kept.
    let mut edited = saved.clone();
    edited["profiles"][0]["key"] = json!("sk-live-plain");
    edited["context_chars"] = json!(1);
    edited["agent"] = json!("nonsense");
    edited["profiles"][0]["provider"] = json!("gemini");
    let mut restored = super::AiState::default();
    restored.apply_json(&edited);
    assert_eq!(restored.profiles[0].key_blob, "");
    assert_eq!(restored.profiles[0].kind, ProviderKind::Gemini);
    assert_eq!(restored.context_chars, super::MIN_CONTEXT_CHARS);
    assert_eq!(restored.agent, "ask");
    // The whole preferences round trip carries it.
    let prefs = app.persist();
    let mut again = PdfCraftApp::new();
    again.restore(&prefs);
    assert_eq!(again.ai.profiles, app.ai.profiles);
}

#[test]
fn every_error_reads_in_the_interface_language() {
    for e in [
        AiError::NotConfigured("x".into()),
        AiError::Unauthorized("x".into()),
        AiError::RateLimited("x".into()),
        AiError::ModelNotFound("x".into()),
        AiError::Rejected("x".into()),
        AiError::Server(500, "x".into()),
        AiError::Network("x".into()),
        AiError::BadReply("x".into()),
        AiError::Refused("x".into()),
    ] {
        assert!(error_text(&e).ends_with("(x)"), "{e:?}");
    }
}

#[test]
fn the_windows_show_the_credit_and_answer_through_the_interface() {
    let t = fake(200, json!({ "choices": [{ "message": { "content": ANSWER } }] }));
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).build_eframe(|_| {
        let mut app = app(t.clone());
        app.run_command("ai.assistant");
        app.run_command("ai.settings");
        app
    });
    h.run_steps(3);
    h.get_all_by_label("AI integration by autolisp.vn").next().unwrap();
    h.get_by_label("Work");
    h.state_mut().ai.input = "Điều 5 nói gì?".into();
    h.get_by_label("Send").click();
    h.run_steps(3);
    assert_eq!(h.state().ai.messages.len(), 2);
    // Shown without its Markdown, the list item with a bullet the interface font draws.
    h.get_by_label(&super::window::readable(ANSWER));
    // Adding a connection shows the form; Cancel leaves the connections as they were.
    h.get_by_label("Add connection…").click();
    h.run_steps(2);
    assert!(h.state().ai.draft.is_some());
    h.get_by_label("Cancel").click_accesskit();
    h.run_steps(2);
    assert!(h.state().ai.draft.is_none());
    assert_eq!(h.state().ai.profiles.len(), 1);
}

#[test]
fn answers_are_shown_without_markdown_markers() {
    let md = "## **Tóm tắt**\n\nTài liệu là **hợp đồng** (trang 1).\n- Giá trị: 1.250.000.000 đồng\n  * Đợt 1: 30%\nsnake_case_name stays";
    assert_eq!(
        super::window::readable(md),
        "Tóm tắt\n\nTài liệu là hợp đồng (trang 1).\n• Giá trị: 1.250.000.000 đồng\n  • Đợt 1: 30%\nsnake_case_name stays"
    );
}
