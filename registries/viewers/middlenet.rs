/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::middlenet::document::{SimpleBlock, SimpleDocument};
use crate::middlenet::engine::{MiddleNetEngine, MiddleNetLoadResult};
use crate::middlenet::misfin::{
    self, MisfinAddress, MisfinIdentitySpec,
};
use crate::middlenet::source::{MiddleNetContent, MiddleNetContentKind, MiddleNetSource};
use crate::middlenet::transport::titan_upload;
use crate::registries::atomic::viewer::{
    EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput,
};
use crate::services::persistence::types::NodeAuditEventKind;

pub(crate) struct MiddleNetEmbeddedViewer;

#[derive(Debug, Clone, Default)]
struct MiddleNetViewerState {
    titan_seed_signature: String,
    titan_draft: String,
    titan_mime: String,
    titan_token: String,
    titan_status: Option<String>,
    misfin_recipient_seed: String,
    misfin_from_mailbox: String,
    misfin_from_host: String,
    misfin_blurb: String,
    misfin_subject: String,
    misfin_body: String,
    misfin_wire_message: String,
    misfin_status: Option<String>,
}

impl EmbeddedViewer for MiddleNetEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:middlenet"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        let mut intents = Vec::new();
        let mut app_commands = Vec::new();
        let mut state = load_viewer_state(ctx.node_key);
        let parsed_url = url::Url::parse(ctx.node_url).ok();

        if let Some(url) = parsed_url.as_ref()
            && url.scheme() == "misfin"
        {
            render_misfin_view(ui, ctx, url, &mut state, &mut intents, &mut app_commands);
            store_viewer_state(ctx.node_key, state);
            return EmbeddedViewerOutput {
                intents,
                app_commands,
            };
        }

        let Some(source) = MiddleNetSource::detect(ctx.node_url, ctx.mime_hint) else {
            ui.small("MiddleNet viewer could not classify this content yet.");
            store_viewer_state(ctx.node_key, state);
            return EmbeddedViewerOutput {
                intents,
                app_commands,
            };
        };

        ui.heading("MiddleNet");
        ui.small(format!("Lane: {}", source.content_kind.label()));
        ui.small(ctx.node_url);
        ui.separator();

        match load_for_viewer(ctx, source, parsed_url.as_ref()) {
            MiddleNetLoadResult::Parsed(content) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    render_document(ui, ctx.node_key, &content, &mut intents);
                    if let Some(url) = parsed_url.as_ref()
                        && matches!(url.scheme(), "gemini" | "titan")
                        && content.source.content_kind == MiddleNetContentKind::GeminiText
                    {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        render_titan_editor(
                            ui,
                            ctx.node_key,
                            url,
                            &content,
                            &mut state,
                            &mut intents,
                            &mut app_commands,
                        );
                    }
                });
            }
            MiddleNetLoadResult::TransportPending { note, .. } => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 180, 60),
                    "Transport not wired yet",
                );
                ui.small(note);
            }
            MiddleNetLoadResult::TransportError { error, .. } => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 120),
                    "MiddleNet transport error",
                );
                ui.small(error);
            }
            MiddleNetLoadResult::Unsupported { note, .. } => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 180, 60),
                    "Adapter not wired yet",
                );
                ui.small(note);
            }
            MiddleNetLoadResult::ParseError { error, .. } => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 120),
                    "MiddleNet parse error",
                );
                ui.small(error);
            }
        }

        store_viewer_state(ctx.node_key, state);
        EmbeddedViewerOutput {
            intents,
            app_commands,
        }
    }
}

fn viewer_state_store() -> &'static Mutex<HashMap<crate::graph::NodeKey, MiddleNetViewerState>> {
    static STORE: OnceLock<Mutex<HashMap<crate::graph::NodeKey, MiddleNetViewerState>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_viewer_state(node_key: crate::graph::NodeKey) -> MiddleNetViewerState {
    viewer_state_store()
        .lock()
        .expect("middlenet viewer state lock poisoned")
        .get(&node_key)
        .cloned()
        .unwrap_or_default()
}

fn store_viewer_state(node_key: crate::graph::NodeKey, state: MiddleNetViewerState) {
    viewer_state_store()
        .lock()
        .expect("middlenet viewer state lock poisoned")
        .insert(node_key, state);
}

fn load_for_viewer(
    ctx: &EmbeddedViewerContext<'_>,
    mut source: MiddleNetSource,
    parsed_url: Option<&url::Url>,
) -> MiddleNetLoadResult {
    if let Some(url) = parsed_url
        && url.scheme() == "titan"
    {
        let mut gemini_url = url.clone();
        let _ = gemini_url.set_scheme("gemini");
        source.canonical_uri = Some(gemini_url.to_string());
    }

    if !ctx.node_url.starts_with("file://") {
        return MiddleNetEngine::load_remote(source);
    }

    let path = match crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(
        ctx.node_url,
        ctx.file_access_policy,
    ) {
        Ok(path) => path,
        Err(error) => {
            return MiddleNetLoadResult::ParseError {
                source,
                error,
            };
        }
    };

    let body = match std::fs::read_to_string(&path) {
        Ok(body) => body,
        Err(error) => {
            return MiddleNetLoadResult::ParseError {
                source,
                error: format!("Failed to read '{}': {error}", path.display()),
            };
        }
    };

    MiddleNetEngine::parse_text(source, &body)
}

fn render_misfin_view(
    ui: &mut egui::Ui,
    ctx: &EmbeddedViewerContext<'_>,
    url: &url::Url,
    state: &mut MiddleNetViewerState,
    intents: &mut Vec<crate::app::GraphIntent>,
    app_commands: &mut Vec<crate::app::AppCommand>,
) {
    ui.heading("MiddleNet");
    ui.small("Lane: Misfin");
    ui.small(ctx.node_url);
    ui.separator();

    let recipient = match MisfinAddress::from_url(url) {
        Ok(recipient) => recipient,
        Err(error) => {
            ui.colored_label(egui::Color32::from_rgb(220, 120, 120), "Misfin address error");
            ui.small(error);
            return;
        }
    };

    let recipient_seed = recipient.as_addr_spec();
    if state.misfin_recipient_seed != recipient_seed {
        state.misfin_recipient_seed = recipient_seed.clone();
        if state.misfin_from_host.is_empty() {
            state.misfin_from_host = recipient.host.clone();
        }
        if state.misfin_wire_message.is_empty() {
            state.misfin_wire_message = recipient_seed.clone();
        }
        state.misfin_status = None;
    }

    ui.strong(format!("To: {recipient_seed}"));
    ui.small("Misfin transactions stay single-line on the wire, but this draft surface now keeps a richer gemmail preview alongside an explicit distilled wire line.");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("From mailbox");
        ui.add(egui::TextEdit::singleline(&mut state.misfin_from_mailbox).desired_width(180.0));
        ui.label("Host");
        ui.add(egui::TextEdit::singleline(&mut state.misfin_from_host).desired_width(220.0));
    });
    ui.horizontal(|ui| {
        ui.label("Blurb");
        ui.add(egui::TextEdit::singleline(&mut state.misfin_blurb).desired_width(f32::INFINITY));
    });
    ui.horizontal(|ui| {
        ui.label("Subject");
        ui.add(egui::TextEdit::singleline(&mut state.misfin_subject).desired_width(f32::INFINITY));
    });
    ui.label("Body preview");
    ui.add(
        egui::TextEdit::multiline(&mut state.misfin_body)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .desired_rows(6),
    );

    if state.misfin_wire_message.trim().is_empty() {
        state.misfin_wire_message = derive_misfin_wire_message(&state.misfin_subject, &state.misfin_body);
    }

    ui.horizontal(|ui| {
        ui.label("Wire line");
        ui.add(
            egui::TextEdit::singleline(&mut state.misfin_wire_message).desired_width(f32::INFINITY),
        );
    });
    ui.small("The preview below can stay multiline. Send uses only the distilled wire line above.");

    let preview = compose_misfin_gemmail_preview(&recipient, state);
    let preview_document = misfin::parse_gemmail(&preview).body_document();
    egui::CollapsingHeader::new("Gemmail preview")
        .default_open(true)
        .show(ui, |ui| {
            render_document(
                ui,
                ctx.node_key,
                &MiddleNetContent {
                    source: MiddleNetSource {
                        canonical_uri: Some(ctx.node_url.to_string()),
                        title_hint: state.misfin_subject.is_empty().then_some(None).unwrap_or_else(|| Some(state.misfin_subject.clone())),
                        content_kind: MiddleNetContentKind::GeminiText,
                    },
                    document: preview_document,
                },
                intents,
            );
            ui.add_space(6.0);
            let mut preview_text = preview;
            ui.add(
                egui::TextEdit::multiline(&mut preview_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(6)
                    .interactive(false),
            );
        });

    if ui.button("Send Misfin message").clicked() {
        let sender_address = match MisfinAddress::parse(&format!(
            "{}@{}",
            state.misfin_from_mailbox.trim(),
            state.misfin_from_host.trim()
        )) {
            Ok(address) => address,
            Err(error) => {
                state.misfin_status = Some(error);
                render_status(ui, state.misfin_status.as_deref(), true);
                return;
            }
        };

        let sender = MisfinIdentitySpec {
            address: sender_address,
            blurb: trim_optional(&state.misfin_blurb),
        };

        let wire_message = if state.misfin_wire_message.trim().is_empty() {
            derive_misfin_wire_message(&state.misfin_subject, &state.misfin_body)
        } else {
            state.misfin_wire_message.trim().to_string()
        };

        if wire_message.is_empty() {
            let message = "Misfin send needs a one-line wire message.".to_string();
            state.misfin_status = Some(message.clone());
            queue_node_notice(
                app_commands,
                ctx.node_key,
                crate::app::UiNotificationLevel::Error,
                message.clone(),
                "Misfin draft blocked".to_string(),
                message,
            );
            render_status(ui, state.misfin_status.as_deref(), true);
            return;
        }

        match misfin::send_message(url, &sender, &wire_message) {
            Ok(outcome) => {
                if let Some(permanent_redirect) = outcome.permanent_redirect.as_ref() {
                    intents.push(crate::app::GraphIntent::SetNodeUrl {
                        key: ctx.node_key,
                        new_url: misfin::url_string_for_address(permanent_redirect, url.port()),
                    });
                }

                let status_text = format!(
                    "Misfin status {} for {}{}{}",
                    outcome.status,
                    outcome.final_recipient.as_addr_spec(),
                    if outcome.meta.is_empty() { "" } else { ": " },
                    outcome.meta,
                );
                let level = notice_level_for_status(outcome.status, outcome.permanent_redirect.is_some());
                state.misfin_status = Some(status_text.clone());
                queue_node_notice(
                    app_commands,
                    ctx.node_key,
                    level,
                    status_text.clone(),
                    "Misfin send".to_string(),
                    if let Some(permanent_redirect) = outcome.permanent_redirect.as_ref() {
                        format!("{} -> {}", outcome.final_recipient.as_addr_spec(), permanent_redirect.as_addr_spec())
                    } else {
                        status_text
                    },
                );
            }
            Err(error) => {
                state.misfin_status = Some(error.clone());
                queue_node_notice(
                    app_commands,
                    ctx.node_key,
                    crate::app::UiNotificationLevel::Error,
                    error.clone(),
                    "Misfin send".to_string(),
                    error,
                );
            }
        }
    }

    render_status(ui, state.misfin_status.as_deref(), state.misfin_status.is_some());
}

fn render_titan_editor(
    ui: &mut egui::Ui,
    node_key: crate::graph::NodeKey,
    url: &url::Url,
    content: &MiddleNetContent,
    state: &mut MiddleNetViewerState,
    intents: &mut Vec<crate::app::GraphIntent>,
    app_commands: &mut Vec<crate::app::AppCommand>,
) {
    let seed_text = content.document.to_gemini();
    let seed_signature = format!("{}\n{}", url, seed_text);
    if state.titan_seed_signature != seed_signature {
        state.titan_seed_signature = seed_signature;
        state.titan_draft = seed_text;
        if state.titan_mime.is_empty() {
            state.titan_mime = "text/gemini".to_string();
        }
        state.titan_status = None;
    }

    ui.heading("Titan");
    ui.small("Edit the current Gemini document and push it back through Titan.");
    ui.horizontal(|ui| {
        ui.label("MIME");
        ui.add(egui::TextEdit::singleline(&mut state.titan_mime).desired_width(160.0));
        ui.label("Token");
        ui.add(egui::TextEdit::singleline(&mut state.titan_token).desired_width(180.0));
    });
    ui.add(
        egui::TextEdit::multiline(&mut state.titan_draft)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .desired_rows(12),
    );

    if ui.button("Upload via Titan").clicked() {
        let mime = trim_optional(&state.titan_mime);
        let token = trim_optional(&state.titan_token);
        match titan_upload(url, state.titan_draft.as_bytes(), mime.as_deref(), token.as_deref()) {
            Ok(outcome) => {
                let status = outcome.status;
                let status_text = format!("Titan status {status}: {}", outcome.meta);
                state.titan_status = Some(status_text.clone());

                if (20..=29).contains(&status) {
                    intents.push(crate::app::GraphIntent::SetNodeUrl {
                        key: node_key,
                        new_url: url.to_string(),
                    });
                    queue_node_notice(
                        app_commands,
                        node_key,
                        crate::app::UiNotificationLevel::Success,
                        status_text.clone(),
                        "Titan upload".to_string(),
                        status_text,
                    );
                } else if (30..=39).contains(&status)
                    && let Some(redirect_url) = resolve_redirect_url(url, &outcome.meta)
                {
                    intents.push(crate::app::GraphIntent::SetNodeUrl {
                        key: node_key,
                        new_url: redirect_url,
                    });
                    queue_node_notice(
                        app_commands,
                        node_key,
                        crate::app::UiNotificationLevel::Warning,
                        status_text.clone(),
                        "Titan upload".to_string(),
                        format!("redirected: {}", outcome.meta),
                    );
                } else {
                    queue_node_notice(
                        app_commands,
                        node_key,
                        notice_level_for_status(status, false),
                        status_text.clone(),
                        "Titan upload".to_string(),
                        status_text,
                    );
                }
            }
            Err(error) => {
                state.titan_status = Some(error.clone());
                queue_node_notice(
                    app_commands,
                    node_key,
                    crate::app::UiNotificationLevel::Error,
                    error.clone(),
                    "Titan upload".to_string(),
                    error,
                );
            }
        }
    }

    render_status(ui, state.titan_status.as_deref(), state.titan_status.is_some());
}

fn render_status(ui: &mut egui::Ui, status: Option<&str>, active: bool) {
    let Some(status) = status else {
        return;
    };
    let color = if active && status.to_ascii_lowercase().contains("status 2") {
        egui::Color32::from_rgb(120, 190, 120)
    } else {
        egui::Color32::from_rgb(220, 180, 60)
    };
    ui.add_space(6.0);
    ui.colored_label(color, status);
}

fn resolve_redirect_url(base: &url::Url, meta: &str) -> Option<String> {
    base.join(meta)
        .or_else(|_| url::Url::parse(meta))
        .ok()
        .map(|url| url.to_string())
}

fn trim_optional(input: &str) -> Option<String> {
    let trimmed = input.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn derive_misfin_wire_message(subject: &str, body: &str) -> String {
    let subject = subject.trim();
    let body_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();

    match (subject.is_empty(), body_line.is_empty()) {
        (false, false) => format!("{subject}: {body_line}"),
        (false, true) => subject.to_string(),
        (true, false) => body_line.to_string(),
        (true, true) => String::new(),
    }
}

fn compose_misfin_gemmail_preview(recipient: &MisfinAddress, state: &MiddleNetViewerState) -> String {
    let mut lines = Vec::new();

    let from_mailbox = state.misfin_from_mailbox.trim();
    let from_host = state.misfin_from_host.trim();
    if !from_mailbox.is_empty() && !from_host.is_empty() {
        let mut sender_line = format!("< {}@{}", from_mailbox, from_host);
        if let Some(blurb) = trim_optional(&state.misfin_blurb) {
            sender_line.push(' ');
            sender_line.push_str(&blurb);
        }
        lines.push(sender_line);
    }

    lines.push(format!(": {}", recipient.as_addr_spec()));

    if let Some(subject) = trim_optional(&state.misfin_subject) {
        lines.push(format!("# {subject}"));
    }

    let body = state.misfin_body.trim();
    if !body.is_empty() {
        lines.push(body.to_string());
    }

    lines.join("\n")
}

fn notice_level_for_status(
    status: u16,
    redirected: bool,
) -> crate::app::UiNotificationLevel {
    if redirected || (30..=39).contains(&status) {
        crate::app::UiNotificationLevel::Warning
    } else if (20..=29).contains(&status) {
        crate::app::UiNotificationLevel::Success
    } else {
        crate::app::UiNotificationLevel::Error
    }
}

fn queue_node_notice(
    app_commands: &mut Vec<crate::app::AppCommand>,
    key: crate::graph::NodeKey,
    level: crate::app::UiNotificationLevel,
    message: String,
    action: String,
    detail: String,
) {
    app_commands.push(crate::app::AppCommand::NodeStatusNotice {
        request: crate::app::NodeStatusNoticeRequest {
            key,
            level,
            message,
            audit_event: Some(NodeAuditEventKind::ActionRecorded { action, detail }),
        },
    });
}

fn render_document(
    ui: &mut egui::Ui,
    node_key: crate::graph::NodeKey,
    content: &MiddleNetContent,
    intents: &mut Vec<crate::app::GraphIntent>,
) {
    if let Some(title) = content.source.title_hint.as_deref() {
        ui.strong(title);
        ui.add_space(6.0);
    }

    let SimpleDocument::Blocks(blocks) = &content.document;
    for block in blocks {
        match block {
            SimpleBlock::Heading { level, text } => {
                let size = match level {
                    1 => 24.0,
                    2 => 20.0,
                    _ => 17.0,
                };
                ui.label(egui::RichText::new(text).strong().size(size));
            }
            SimpleBlock::Paragraph(text) => {
                ui.label(text);
            }
            SimpleBlock::Link { text, href } => {
                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(text).color(egui::Color32::from_rgb(100, 149, 237)),
                    )
                    .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    intents.push(crate::app::GraphIntent::SetNodeUrl {
                        key: node_key,
                        new_url: href.clone(),
                    });
                }
                response.on_hover_text(href);
            }
            SimpleBlock::Quote(text) => {
                ui.colored_label(egui::Color32::from_gray(170), format!("> {text}"));
            }
            SimpleBlock::CodeFence { lang, text } => {
                if let Some(lang) = lang {
                    ui.small(format!("Code: {lang}"));
                }
                let mut display = text.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut display)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            }
            SimpleBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        ui.label(format!("{}. {item}", index + 1));
                    } else {
                        ui.label(format!("* {item}"));
                    }
                }
            }
            SimpleBlock::Rule => {
                ui.separator();
            }
        }
        ui.add_space(4.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misfin_wire_message_prefers_subject_and_first_body_line() {
        assert_eq!(
            derive_misfin_wire_message("Lane update", "first line\nsecond line"),
            "Lane update: first line"
        );
        assert_eq!(derive_misfin_wire_message("", "first line\nsecond line"), "first line");
    }

    #[test]
    fn misfin_preview_includes_sender_recipient_and_subject() {
        let state = MiddleNetViewerState {
            misfin_from_mailbox: "mark".to_string(),
            misfin_from_host: "example.com".to_string(),
            misfin_blurb: "Lanepost".to_string(),
            misfin_subject: "Hello".to_string(),
            misfin_body: "Body line".to_string(),
            ..MiddleNetViewerState::default()
        };
        let recipient = MisfinAddress::parse("friend@example.net").expect("recipient should parse");
        let preview = compose_misfin_gemmail_preview(&recipient, &state);

        assert!(preview.contains("< mark@example.com Lanepost"));
        assert!(preview.contains(": friend@example.net"));
        assert!(preview.contains("# Hello"));
        assert!(preview.contains("Body line"));
    }
}