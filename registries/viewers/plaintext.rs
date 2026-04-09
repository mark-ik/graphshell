use crate::registries::atomic::viewer::{EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput};

pub(crate) struct PlaintextEmbeddedViewer;

impl EmbeddedViewer for PlaintextEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:plaintext"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        ui.label(ctx.node_url);
        ui.separator();

        let mut intents = Vec::new();

        match load_plaintext_content(ctx.node_url, ctx.file_access_policy) {
            Ok(PlaintextContent::Text(content)) => {
                let markdown_mode = is_markdown(ctx);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if markdown_mode {
                        render_markdown(ui, &content, ctx.node_key, &mut intents);
                    } else {
                        let mut read_only = content;
                        ui.add(
                            egui::TextEdit::multiline(&mut read_only)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    }
                });
            }
            Ok(PlaintextContent::HexPreview(hex)) => {
                ui.small("Binary content detected; showing hex preview.");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut read_only = hex;
                    ui.add(
                        egui::TextEdit::multiline(&mut read_only)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            }
            Err(error) => {
                ui.small(error);
            }
        }

        EmbeddedViewerOutput {
            intents,
            app_commands: Vec::new(),
        }
    }
}

fn is_markdown(ctx: &EmbeddedViewerContext<'_>) -> bool {
    if ctx.mime_hint == Some("text/markdown") || ctx.mime_hint == Some("text/x-markdown") {
        return true;
    }
    ctx.node_url
        .rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md"))
}

// --- Markdown rendering via pulldown-cmark ---

fn render_markdown(
    ui: &mut egui::Ui,
    markdown: &str,
    node_key: crate::graph::NodeKey,
    intents: &mut Vec<crate::app::GraphIntent>,
) {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);

    let mut text_buf = String::new();
    let mut heading_level: Option<HeadingLevel> = None;
    let mut emphasis = false;
    let mut strong = false;
    let mut code_block = false;
    let mut link_url: Option<String> = None;
    let mut in_list_item = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                heading_level = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                heading_level = None;
            }
            Event::Start(Tag::Emphasis) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                emphasis = true;
            }
            Event::End(TagEnd::Emphasis) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                emphasis = false;
            }
            Event::Start(Tag::Strong) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                strong = true;
            }
            Event::End(TagEnd::Strong) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                strong = false;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                link_url = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                link_url = None;
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                if !text_buf.is_empty() {
                    let code = std::mem::take(&mut text_buf);
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().extreme_bg_color)
                        .show(ui, |ui| {
                            let mut display = code;
                            ui.add(
                                egui::TextEdit::multiline(&mut display)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                }
                code_block = false;
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                in_list_item = true;
            }
            Event::End(TagEnd::Item) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                in_list_item = false;
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                ui.add_space(4.0);
            }
            Event::Text(text) => {
                if code_block {
                    text_buf.push_str(&text);
                } else if in_list_item && text_buf.is_empty() {
                    text_buf.push_str("• ");
                    text_buf.push_str(&text);
                } else {
                    text_buf.push_str(&text);
                }
            }
            Event::Code(code) => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                ui.label(egui::RichText::new(code.as_ref()).monospace());
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
            }
            Event::Rule => {
                flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
                ui.separator();
            }
            _ => {}
        }
    }
    flush_text(ui, &mut text_buf, heading_level, emphasis, strong, &link_url, node_key, intents);
}

fn flush_text(
    ui: &mut egui::Ui,
    buf: &mut String,
    heading: Option<pulldown_cmark::HeadingLevel>,
    emphasis: bool,
    strong: bool,
    link_url: &Option<String>,
    node_key: crate::graph::NodeKey,
    intents: &mut Vec<crate::app::GraphIntent>,
) {
    if buf.is_empty() {
        return;
    }
    let text = std::mem::take(buf);

    let mut rich = egui::RichText::new(&text);
    if let Some(level) = heading {
        rich = rich.strong();
        rich = match level {
            pulldown_cmark::HeadingLevel::H1 => rich.size(24.0),
            pulldown_cmark::HeadingLevel::H2 => rich.size(20.0),
            pulldown_cmark::HeadingLevel::H3 => rich.size(17.0),
            _ => rich.size(15.0),
        };
    }
    if strong {
        rich = rich.strong();
    }
    if emphasis {
        rich = rich.italics();
    }

    if let Some(url) = link_url {
        let response = ui.add(egui::Label::new(
            rich.color(egui::Color32::from_rgb(100, 149, 237)),
        ).sense(egui::Sense::click()));
        if response.clicked() {
            intents.push(crate::app::GraphIntent::SetNodeUrl {
                key: node_key,
                new_url: url.clone(),
            });
        }
        response.on_hover_text(url);
    } else {
        ui.label(rich);
    }
}

// --- Plaintext content loading ---

const PLAINTEXT_HEX_PREVIEW_BYTES: usize = 4096;

enum PlaintextContent {
    Text(String),
    HexPreview(String),
}

fn load_plaintext_content(url: &str, policy: &crate::prefs::FileAccessPolicy) -> Result<PlaintextContent, String> {
    let path = crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(url, policy)?;
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("Failed to read '{}': {err}", path.display()))?;
    Ok(decode_plaintext_content(&bytes))
}

fn decode_plaintext_content(bytes: &[u8]) -> PlaintextContent {
    match std::str::from_utf8(bytes) {
        Ok(text) => PlaintextContent::Text(text.to_string()),
        Err(_) => {
            let preview_len = bytes.len().min(PLAINTEXT_HEX_PREVIEW_BYTES);
            let mut hex = String::new();
            for (row, chunk) in bytes[..preview_len].chunks(16).enumerate() {
                let offset = row * 16;
                hex.push_str(&format!("{offset:08x}: "));
                for byte in chunk {
                    hex.push_str(&format!("{byte:02x} "));
                }
                hex.push('\n');
            }
            if bytes.len() > preview_len {
                hex.push_str("\n... truncated binary preview ...\n");
            }
            PlaintextContent::HexPreview(hex)
        }
    }
}
