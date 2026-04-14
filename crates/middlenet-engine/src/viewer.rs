use crate::dom::{Document, NodeData};

#[derive(Debug, Clone)]
pub enum DisplayAction {
    Text(String),
    Heading { level: u8, text: String },
    Paragraph(String),
    ListItem(String),
    Link { text: String, href: String },
    Quote(String),
    CodeFence { lang: Option<String>, text: String },
    Separator,
    Spacer(f32),
}

pub struct RenderResult {
    pub display_list: Vec<DisplayAction>,
    pub ready_to_paint: bool,
}

pub trait HostViewerAdapter {
    fn render_document(&mut self, document: &Document) -> RenderResult;
}

pub fn generate_display_list(document: &Document) -> RenderResult {
    let mut display_list = Vec::new();
    fn visit(doc: &Document, id: usize, list: &mut Vec<DisplayAction>) {
        let node = &doc.nodes[id];
        match &node.data {
            NodeData::Document => {
                for &child in &node.children {
                    visit(doc, child, list);
                }
            }
            NodeData::Element { name, attrs } => {
                let tag = name.local.as_ref();
                let text = get_text_content(doc, id);
                
                match tag {
                    "h1" | "h2" | "h3" => {
                        let level = match tag { "h1" => 1, "h2" => 2, _ => 3 };
                        if !text.is_empty() {
                            list.push(DisplayAction::Heading { level, text });
                        }
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "p" => {
                        if !text.is_empty() {
                            list.push(DisplayAction::Paragraph(text));
                        }
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "li" => {
                        list.push(DisplayAction::ListItem(text));
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "a" => {
                        let href = attrs.iter().find(|a| a.name.local.as_ref() == "href").map(|a| a.value.as_ref().to_string()).unwrap_or_default();
                        list.push(DisplayAction::Link { text, href });
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "blockquote" => {
                        list.push(DisplayAction::Quote(text));
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "pre" => {
                        list.push(DisplayAction::CodeFence { lang: None, text });
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    "hr" => {
                        list.push(DisplayAction::Separator);
                        list.push(DisplayAction::Spacer(4.0));
                    }
                    _ => {
                        for &child in &node.children {
                            visit(doc, child, list);
                        }
                    }
                }
            }
            NodeData::Text(t) => {
                if !t.trim().is_empty() {
                    list.push(DisplayAction::Text(t.to_string()));
                    list.push(DisplayAction::Spacer(4.0));
                }
            }
        }
    }
    visit(document, document.root, &mut display_list);
    RenderResult {
        display_list,
        ready_to_paint: true,
    }
}

fn get_text_content(document: &Document, id: usize) -> String {
    let mut text = String::new();
    collect_text(document, id, &mut text);
    text
}

fn collect_text(document: &Document, id: usize, text: &mut String) {
    let node = &document.nodes[id];
    if let NodeData::Text(t) = &node.data {
        text.push_str(t);
    }
    for &child in &node.children {
        collect_text(document, child, text);
    }
}

