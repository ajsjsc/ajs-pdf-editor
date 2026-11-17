//! The assistant's two windows: the conversation, and the settings where connections are added.
//! Both are movable windows over the pages, so the document stays in view while it is discussed.

use pdfcraft_ai::{AGENTS, PROVIDERS, ProviderKind, Role};

use super::{GUIDE_KEYS, GUIDE_SETUP, MAX_CONTEXT_CHARS, MIN_CONTEXT_CHARS, Profile};
use crate::{PdfCraftApp, theme, widgets};

/// A connection being added or changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Draft {
    /// The connection changed, or `None` for a new one.
    pub uid: Option<u64>,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// A key typed now; empty keeps the saved one.
    pub key: String,
    pub show_key: bool,
}

impl Draft {
    fn new(kind: ProviderKind) -> Self {
        Self { uid: None, name: String::new(), kind, base_url: String::new(), model: String::new(), key: String::new(), show_key: false }
    }

    fn of(p: &Profile) -> Self {
        Self {
            uid: Some(p.uid),
            name: p.name.clone(),
            kind: p.kind,
            base_url: p.base_url.clone(),
            model: p.model.clone(),
            key: String::new(),
            show_key: false,
        }
    }
}

/// Draw the assistant's windows that are open (each frame).
pub(crate) fn show(app: &mut PdfCraftApp, ctx: &egui::Context) {
    app.poll_ai();
    let mut typing = false;
    if app.ai.open {
        typing |= chat_window(app, ctx);
    }
    if app.ai.settings_open {
        typing |= settings_window(app, ctx);
    }
    app.ai.typing = typing;
}

/// The credit line: the AJS logo and who wrote the integration.
fn credit(ui: &mut egui::Ui, app: &mut PdfCraftApp) {
    ui.horizontal(|ui| {
        widgets::ajs_mark(ui, 22.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if ui.link(tl!(pdfcraft_ai::CREDIT)).on_hover_text(pdfcraft_ai::CREDIT_URL).clicked() {
                app.open_url(pdfcraft_ai::CREDIT_URL);
            }
            ui.small(tl!("Uses your own AI account and API key."));
        });
    });
}

/// The conversation window. `true` while one of its text fields has the keyboard.
fn chat_window(app: &mut PdfCraftApp, ctx: &egui::Context) -> bool {
    let t = theme::Tokens::get(ctx);
    let mut open = true;
    let mut typing = false;
    egui::Window::new(tl!("AI assistant"))
        .id(egui::Id::unique("ajs_ai_assistant"))
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .order(egui::Order::Foreground)
        .default_size(egui::vec2(480.0, 600.0))
        .min_width(380.0)
        .default_pos(egui::pos2(360.0, 90.0))
        .show(ctx, |ui| {
            widgets::begin_dialog(ui);
            credit(ui, app);
            ui.separator();
            if app.ai.profiles.is_empty() {
                ui.label(tl!(
                    "No AI connection yet. Add your own API key from OpenAI, Anthropic (Claude), Google Gemini, or a local server to start."
                ));
                ui.horizontal(|ui| {
                    if ui.button(tl!("AI settings…")).clicked() {
                        app.ai.settings_open = true;
                    }
                    if ui.link(tl!("How to get an API key")).clicked() {
                        app.open_url(GUIDE_KEYS);
                    }
                });
                return;
            }
            choosers(ui, app);
            ui.separator();
            conversation(ui, app, &t);
            typing = input(ui, app, &t);
        });
    if !open {
        app.ai.open = false;
    }
    typing
}

/// Connection, model, agent and scope.
fn choosers(ui: &mut egui::Ui, app: &mut PdfCraftApp) {
    egui::Grid::new("ai_choosers").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(tl!("Connection"));
        ui.horizontal(|ui| {
            let current = app.ai.active_profile().map(Profile::label).unwrap_or_default();
            craft_ui::ComboBox::from_id_salt("ai_connection").selected_text(current).width(220.0).show_ui(ui, |ui| {
                for p in &app.ai.profiles {
                    let label = format!("{} — {}", p.label(), p.kind.label());
                    if ui.selectable_label(app.ai.active == Some(p.uid), label).clicked() {
                        app.ai.active = Some(p.uid);
                    }
                }
            });
            if widgets::icons::button(ui, "settings", 22.0, false, tl!("AI settings…")).clicked() {
                app.ai.settings_open = true;
            }
        });
        ui.end_row();

        ui.label(tl!("Model"));
        ui.horizontal(|ui| {
            let Some(p) = app.ai.active_profile().cloned() else { return };
            let model =
                if p.model.trim().is_empty() { p.kind.suggested_models().first().copied().unwrap_or_default().to_owned() } else { p.model.clone() };
            let mut choices: Vec<String> =
                app.ai.models.get(&p.uid).cloned().unwrap_or_else(|| p.kind.suggested_models().iter().map(|s| (*s).to_owned()).collect());
            if !choices.contains(&model) {
                choices.insert(0, model.clone());
            }
            let mut chosen = None;
            craft_ui::ComboBox::from_id_salt("ai_model").selected_text(&model).width(220.0).height(320.0).show_ui(ui, |ui| {
                for m in &choices {
                    if ui.selectable_label(*m == model, m).clicked() {
                        chosen = Some(m.clone());
                    }
                }
            });
            if let Some(m) = chosen
                && let Some(profile) = app.ai.profiles.iter_mut().find(|q| q.uid == p.uid)
            {
                profile.model = m;
            }
            let busy = app.ai.job.is_some();
            if !busy && widgets::icons::button(ui, "rotate-cw", 22.0, false, tl!("Load the models of this account")).clicked() {
                app.ai_fetch_models(p, None);
            }
        });
        ui.end_row();

        ui.label(tl!("Agent"));
        let agent = pdfcraft_ai::agent(app.ai.agent);
        let mut picked = None;
        craft_ui::ComboBox::from_id_salt("ai_agent").selected_text(tl!(agent.label)).width(220.0).show_ui(ui, |ui| {
            for a in &AGENTS {
                if ui.selectable_label(a.id == agent.id, tl!(a.label)).on_hover_text(tl!(a.description)).clicked() {
                    picked = Some(a.id);
                }
            }
        });
        if let Some(id) = picked {
            app.open_ai(Some(id));
        }
        ui.end_row();

        ui.label(tl!("Document text"));
        ui.horizontal(|ui| {
            widgets::radio_value(ui, &mut app.ai.whole_document, true, tl!("Whole document"));
            widgets::radio_value(ui, &mut app.ai.whole_document, false, tl!("Current page"));
        });
        ui.end_row();
    });
}

/// The messages so far.
fn conversation(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &theme::Tokens) {
    let height = (ui.available_height() - 150.0).max(120.0);
    let mut copy = None;
    egui::ScrollArea::vertical().id_salt("ai_messages").max_height(height).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        if app.ai.messages.is_empty() {
            ui.colored_label(t.text_muted, tl!("Ask a question about the open document, or choose an agent and press Send."));
        }
        for (i, m) in app.ai.messages.iter().enumerate() {
            let who = if m.role == Role::User { tl!("You") } else { tl!("AI") };
            ui.horizontal(|ui| {
                ui.strong(who);
                if m.role == Role::Assistant && ui.small_button(tl!("Copy")).clicked() {
                    copy = Some(i);
                }
            });
            let shown = if m.role == Role::Assistant { readable(&m.text) } else { m.text.clone() };
            ui.add(egui::Label::new(shown).selectable(true).wrap());
            ui.add_space(6.0);
        }
    });
    if let Some(text) = copy.and_then(|i| app.ai.messages.get(i)).map(|m| m.text.clone()) {
        ui.ctx().copy_text(text);
        app.notify_tr("Answer copied.");
    }
}

/// The question box and its buttons. `true` while the box has the keyboard.
fn input(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &theme::Tokens) -> bool {
    if let Some(job) = &app.ai.job {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(&job.what);
        });
    }
    match &app.ai.status {
        Some(Ok(note)) => {
            ui.colored_label(t.text_muted, note);
        }
        Some(Err(error)) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None => {}
    }
    let edit = ui.add(
        egui::TextEdit::multiline(&mut app.ai.input)
            .id(egui::Id::unique("ai_input"))
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .hint_text(tl!("Type your question… (Ctrl+Enter sends)")),
    );
    let send_key = edit.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
    let busy = app.ai.job.is_some();
    let mut send = false;
    ui.horizontal(|ui| {
        send = ui.add_enabled(!busy, egui::Button::new(tl!("Send"))).clicked();
        if ui.add_enabled(!app.ai.messages.is_empty() && !busy, egui::Button::new(tl!("New conversation"))).clicked() {
            app.ai.messages.clear();
            app.ai.status = None;
        }
    });
    if let Some(p) = app.ai.active_profile() {
        let note = crate::i18n::fmt(tl!("The document text is sent to {provider} when you press Send."), &[("provider", p.kind.label())]);
        ui.small(note);
    }
    if (send || send_key) && !busy {
        app.ai_send();
    }
    edit.has_focus()
}

/// The settings window. `true` while one of its text fields has the keyboard.
fn settings_window(app: &mut PdfCraftApp, ctx: &egui::Context) -> bool {
    let t = theme::Tokens::get(ctx);
    let mut open = true;
    let mut typing = false;
    egui::Window::new(tl!("AI settings"))
        .id(egui::Id::unique("ajs_ai_settings"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .order(egui::Order::Foreground)
        .default_size(egui::vec2(540.0, 700.0))
        .default_pos(egui::pos2(880.0, 90.0))
        .show(ctx, |ui| {
            widgets::begin_dialog(ui);
            credit(ui, app);
            ui.separator();
            egui::ScrollArea::vertical().id_salt("ai_settings_scroll").max_height(f32::INFINITY).show(ui, |ui| {
                connections(ui, app, &t);
                ui.separator();
                if app.ai.draft.is_some() {
                    typing |= draft_form(ui, app, &t);
                    ui.separator();
                }
                ui.strong(tl!("Every request"));
                ui.horizontal(|ui| {
                    ui.label(tl!("Most document characters sent"));
                    ui.add(egui::DragValue::new(&mut app.ai.context_chars).range(MIN_CONTEXT_CHARS..=MAX_CONTEXT_CHARS).speed(1000.0));
                });
                ui.small(tl!("About 4 characters make a token; providers charge per token. Longer documents are cut and the assistant says so."));
                ui.label(tl!("Your instructions for every answer (optional)"));
                let r = ui.add(egui::TextEdit::multiline(&mut app.ai.instructions).desired_rows(2).desired_width(f32::INFINITY).hint_text(tl!("For example: answer briefly, use Vietnamese engineering terms")));
                typing |= r.has_focus();
                ui.separator();
                ui.small(tl!(
                    "Keys are saved encrypted for your Windows account and are never shown, logged or sent anywhere but to their provider. The document's text goes to the provider only when you press Send."
                ));
                ui.horizontal(|ui| {
                    if ui.link(tl!("How to get an API key")).clicked() {
                        app.open_url(GUIDE_KEYS);
                    }
                    ui.label("·");
                    if ui.link(tl!("How to set up the AI assistant")).clicked() {
                        app.open_url(GUIDE_SETUP);
                    }
                });
            });
            if let Some(status) = &app.ai.status
                && app.ai.draft.is_some()
            {
                match status {
                    Ok(note) => ui.colored_label(t.text_muted, note),
                    Err(error) => ui.colored_label(ui.visuals().error_fg_color, error),
                };
            }
        });
    if !open {
        app.ai.settings_open = false;
        app.ai.draft = None;
    }
    typing
}

/// The saved connections.
fn connections(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &theme::Tokens) {
    ui.strong(tl!("Connections"));
    if app.ai.profiles.is_empty() {
        ui.colored_label(t.text_muted, tl!("None yet. Add one with your provider's API key."));
    }
    let mut edit = None;
    let mut remove = None;
    let mut pick = None;
    for p in &app.ai.profiles {
        // The name and the buttons on one row, the details on their own below, so a long
        // provider name never runs under the buttons.
        ui.horizontal(|ui| {
            if ui.add(widgets::radio(app.ai.active == Some(p.uid), p.label())).clicked() {
                pick = Some(p.uid);
            }
            if ui.small_button(tl!("Change…")).clicked() {
                edit = Some(Draft::of(p));
            }
            if ui.small_button(tl!("Remove")).clicked() {
                remove = Some(p.uid);
            }
        });
        let key = if app.ai.has_key(p) { tl!("key saved") } else { tl!("no key") };
        let model = if p.model.is_empty() { p.kind.suggested_models().first().copied().unwrap_or_default() } else { &p.model };
        ui.indent(("ai_connection_details", p.uid), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(format!("{} · {model} · {key}", p.kind.label())).small().color(t.text_muted)).wrap());
        });
    }
    if let Some(uid) = pick {
        app.ai.active = Some(uid);
    }
    if let Some(uid) = remove {
        app.ai.forget(uid);
        if app.ai.draft.as_ref().is_some_and(|d| d.uid == Some(uid)) {
            app.ai.draft = None;
        }
    }
    if let Some(d) = edit {
        app.ai.draft = Some(d);
        app.ai.status = None;
    }
    if app.ai.draft.is_none() && ui.button(tl!("Add connection…")).clicked() {
        app.ai.draft = Some(Draft::new(ProviderKind::OpenAi));
        app.ai.status = None;
    }
}

/// The form for the connection being added or changed. `true` while a field has the keyboard.
fn draft_form(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &theme::Tokens) -> bool {
    let Some(mut d) = app.ai.draft.take() else { return false };
    let saved = d.uid.and_then(|uid| app.ai.profiles.iter().find(|p| p.uid == uid)).cloned();
    let mut typing = false;
    let mut action = None;
    ui.strong(if d.uid.is_some() { tl!("Change connection") } else { tl!("New connection") });
    let kind_before = d.kind;
    egui::Grid::new("ai_draft").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(tl!("Provider"));
        craft_ui::ComboBox::from_id_salt("ai_provider").selected_text(d.kind.label()).width(300.0).show_ui(ui, |ui| {
            for k in PROVIDERS {
                if ui.selectable_label(d.kind == *k, k.label()).clicked() && d.kind != *k {
                    d.kind = *k;
                    d.model.clear();
                    d.base_url.clear();
                }
            }
        });
        ui.end_row();

        ui.label(tl!("Name"));
        typing |= ui.add(egui::TextEdit::singleline(&mut d.name).hint_text(d.kind.label()).desired_width(300.0)).has_focus();
        ui.end_row();

        ui.label(tl!("API key"));
        ui.horizontal(|ui| {
            let hint = match &saved {
                Some(p) if !p.key_blob.is_empty() => tl!("saved — type a new key to replace it"),
                _ if !d.kind.needs_key() => tl!("usually not needed for a local server"),
                _ => tl!("paste your key here"),
            };
            typing |= ui.add(egui::TextEdit::singleline(&mut d.key).password(!d.show_key).hint_text(hint).desired_width(240.0)).has_focus();
            ui.add(widgets::checkbox(&mut d.show_key, tl!("Show")));
        });
        ui.end_row();

        if let Some(page) = d.kind.key_page() {
            ui.label("");
            if ui.link(tl!("Get a key from the provider")).on_hover_text(page).clicked() {
                app.open_url(page);
            }
            ui.end_row();
        }

        ui.label(tl!("Model"));
        ui.horizontal(|ui| {
            let suggested = d.kind.suggested_models().first().copied().unwrap_or_default();
            typing |= ui.add(egui::TextEdit::singleline(&mut d.model).hint_text(suggested).desired_width(200.0)).has_focus();
            // A new connection's test stores its models under the uid it will be saved with.
            let uid = d.uid.unwrap_or_else(|| app.ai.next_uid());
            let list: Vec<String> =
                app.ai.models.get(&uid).cloned().unwrap_or_else(|| d.kind.suggested_models().iter().map(|s| (*s).to_owned()).collect());
            craft_ui::ComboBox::from_id_salt("ai_draft_models").selected_text(tl!("Choose")).width(80.0).height(320.0).show_ui(ui, |ui| {
                for m in &list {
                    if ui.selectable_label(d.model == *m, m).clicked() {
                        d.model = m.clone();
                    }
                }
            });
        });
        ui.end_row();

        ui.label(tl!("API address"));
        typing |= ui.add(egui::TextEdit::singleline(&mut d.base_url).hint_text(d.kind.default_base_url()).desired_width(300.0)).has_focus();
        ui.end_row();
    });
    // Another provider: the models listed for the old one no longer apply.
    if d.kind != kind_before {
        let uid = d.uid.unwrap_or_else(|| app.ai.next_uid());
        app.ai.models.remove(&uid);
    }
    if d.kind == ProviderKind::OpenAiCompatible {
        ui.small(tl!("Ollama: http://localhost:11434/v1 · LM Studio: http://localhost:1234/v1 · OpenRouter: https://openrouter.ai/api/v1"));
    }
    if !d.base_url.trim().is_empty()
        && let Err(e) = pdfcraft_ai::check_base_url(&d.base_url)
    {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    let busy = app.ai.job.is_some();
    ui.horizontal(|ui| {
        if ui.button(tl!("Save")).clicked() {
            action = Some("save");
        }
        if ui.add_enabled(!busy, egui::Button::new(tl!("Test connection"))).on_hover_text(tl!("Lists the account's models; costs nothing")).clicked()
        {
            action = Some("test");
        }
        if ui.button(tl!("Cancel")).clicked() {
            action = Some("cancel");
        }
    });
    let _ = t;
    match action {
        Some("save") => {
            save_draft(app, d);
            return typing;
        }
        Some("test") => {
            let uid = d.uid.unwrap_or_else(|| app.ai.next_uid());
            let profile = Profile {
                uid,
                name: d.name.clone(),
                kind: d.kind,
                base_url: d.base_url.trim().to_owned(),
                model: d.model.trim().to_owned(),
                key_blob: saved.as_ref().map(|p| p.key_blob.clone()).unwrap_or_default(),
            };
            let key = Some(d.key.trim().to_owned()).filter(|k| !k.is_empty());
            app.ai_fetch_models(profile, key);
        }
        Some(_) => {
            app.ai.status = None;
            return typing;
        }
        None => {}
    }
    app.ai.draft = Some(d);
    typing
}

/// Save the draft as a connection, its key encrypted.
fn save_draft(app: &mut PdfCraftApp, d: Draft) {
    if !d.base_url.trim().is_empty()
        && let Err(e) = pdfcraft_ai::check_base_url(&d.base_url)
    {
        app.ai.status = Some(Err(e));
        app.ai.draft = Some(d);
        return;
    }
    let uid = d.uid.unwrap_or_else(|| app.ai.next_uid());
    let old = app.ai.profiles.iter().find(|p| p.uid == uid).cloned();
    let mut key_blob = old.as_ref().map(|p| p.key_blob.clone()).unwrap_or_default();
    let key = d.key.trim().to_owned();
    if !key.is_empty() {
        match pdfcraft_platform::secret::protect(&key) {
            Ok(blob) => key_blob = blob,
            Err(e) => {
                // The key still works until the app closes; the user is told it wasn't saved.
                key_blob.clear();
                app.ai.status = Some(Err(crate::i18n::fmt(
                    tl!("The key couldn't be saved encrypted ({e}); it is kept until you close the app."),
                    &[("e", &e.to_string())],
                )));
            }
        }
        app.ai.remember_key(uid, key);
    }
    let profile = Profile {
        uid,
        name: d.name.trim().to_owned(),
        kind: d.kind,
        base_url: d.base_url.trim().to_owned(),
        model: d.model.trim().to_owned(),
        key_blob,
    };
    match app.ai.profiles.iter_mut().find(|p| p.uid == uid) {
        Some(p) => *p = profile,
        None => app.ai.profiles.push(profile),
    }
    app.ai.active = Some(uid);
    app.ai.draft = None;
    if !matches!(app.ai.status, Some(Err(_))) {
        app.ai.status = Some(Ok(tl!("Connection saved.").into()));
    }
}

/// An answer as it reads: models write Markdown, which the window shows as plain text, so its
/// markers come off (`**bold**`, `# heading`, `* item`). Copy keeps the answer as it came.
pub(crate) fn readable(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    for (i, line) in markdown.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let indent = line.get(..line.len() - trimmed.len()).unwrap_or_default();
        let body = match trimmed.strip_prefix('#') {
            Some(_) => trimmed.trim_start_matches('#').trim_start(),
            None => trimmed,
        };
        let body = match body.strip_prefix("* ") {
            Some(rest) => format!("• {rest}"),
            None => match body.strip_prefix("- ") {
                Some(rest) => format!("• {rest}"),
                None => body.to_owned(),
            },
        };
        out.push_str(indent);
        out.push_str(&body.replace("**", "").replace("__", ""));
    }
    out
}
