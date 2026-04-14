/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable MiddleNet DOM representation.
//!
//! Uses html5ever for parsing into a generic Node tree.

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{expanded_name, local_name, namespace_url, ns, QualName, Attribute, parse_document};
use std::borrow::Cow;

/// A lightweight, arena-allocated HTML DOM node.
#[derive(Debug, Clone)]
pub struct DomNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub data: NodeData,
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
    },
    Text(String),
}

/// The MiddleNet portable document structure.
#[derive(Debug, Clone)]
pub struct Document {
    pub nodes: Vec<DomNode>,
    pub root: usize,
    pub title: Option<String>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            nodes: vec![DomNode {
                id: 0,
                parent: None,
                children: vec![],
                data: NodeData::Document,
            }],
            root: 0,
            title: None,
        }
    }

    /// Parse HTML string into our arena-backed DOM tree.
    pub fn parse(html: &str) -> Self {
        let mut sink = Self::new();
        let mut parser = parse_document(sink, Default::default());
        parser.process(StrTendril::from_slice(html));
        let mut doc = parser.finish();
        doc.extract_title();
        doc
    }

    pub fn to_gemini(&self) -> String {
        let mut out = String::new();
        self.to_gemini_node(self.root, &mut out);
        out
    }

    fn to_gemini_node(&self, id: usize, out: &mut String) {
        match &self.nodes[id].data {
            NodeData::Document => {
                for &child in &self.nodes[id].children {
                    self.to_gemini_node(child, out);
                }
            }
            NodeData::Element { name, attrs } => {
                let tag = name.local.as_ref();
                match tag {
                    "h1" | "h2" | "h3" => {
                        let level = match tag {
                            "h1" => "# ",
                            "h2" => "## ",
                            _ => "### ",
                        };
                        out.push_str(level);
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, out);
                        }
                        out.push('\n');
                    }
                    "a" => {
                        let href = attrs.iter().find(|a| a.name.local.as_ref() == "href").map(|a| a.value.as_ref()).unwrap_or("");
                        let mut text = String::new();
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, &mut text);
                        }
                        out.push_str(&format!("=> {} {}", href, text.trim()));
                        out.push('\n');
                    }
                    "p" | "li" => {
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, out);
                        }
                        out.push('\n');
                    }
                    "blockquote" => {
                        out.push_str("> ");
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, out);
                        }
                        out.push('\n');
                    }
                    "pre" => {
                        out.push_str("```\n");
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, out);
                        }
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                        out.push_str("```\n");
                    }
                    _ => {
                        for &child in &self.nodes[id].children {
                            self.to_gemini_node(child, out);
                        }
                    }
                }
            }
            NodeData::Text(t) => {
                out.push_str(t);
            }
        }
    }

    fn extract_title(&mut self) {
        let title_node = self.find_node_by_name(self.root, &local_name!("title"));
        if let Some(id) = title_node {
            let mut title_text = String::new();
            for &child_id in &self.nodes[id].children.clone() {
                if let NodeData::Text(ref text) = self.nodes[child_id].data {
                    title_text.push_str(text);
                }
            }
            if !title_text.trim().is_empty() {
                self.title = Some(title_text.trim().to_string());
            }
        }
    }

    fn find_node_by_name(&self, current: usize, target: &html5ever::LocalName) -> Option<usize> {
        if let NodeData::Element { ref name, .. } = self.nodes[current].data {
            if &name.local == target {
                return Some(current);
            }
        }
        for &child in &self.nodes[current].children {
            if let Some(found) = self.find_node_by_name(child, target) {
                return Some(found);
            }
        }
        None
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

// Ensure our document arena acts as an html5ever TreeSink.
impl TreeSink for Document {
    type Handle = usize;
    type Output = Document;

    fn finish(self) -> Self::Output {
        self
    }

    fn parse_error(&mut self, _msg: Cow<'static, str>) {}

    fn get_document(&mut self) -> Self::Handle {
        self.root
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> html5ever::ExpandedName<'a> {
        let node = &self.nodes[*target];
        match &node.data {
            NodeData::Element { name, .. } => name.expanded(),
            _ => unreachable!(),
        }
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>, _flags: ElementFlags) -> Self::Handle {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            id,
            parent: None,
            children: vec![],
            data: NodeData::Element { name, attrs },
        });
        id
    }

    fn create_comment(&mut self, _text: StrTendril) -> Self::Handle {
        // Skip comments
        let id = self.nodes.len();
        self.nodes.push(DomNode { id, parent: None, children: vec![], data: NodeData::Text(String::new()) });
        id
    }

    fn create_pi(&mut self, _target: StrTendril, _data: StrTendril) -> Self::Handle {
        // Skip processing instructions
        let id = self.nodes.len();
        self.nodes.push(DomNode { id, parent: None, children: vec![], data: NodeData::Text(String::new()) });
        id
    }

    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        match child {
            NodeOrText::AppendNode(node_id) => {
                self.nodes[node_id].parent = Some(*parent);
                self.nodes[*parent].children.push(node_id);
            }
            NodeOrText::AppendText(text) => {
                let text_str = text.to_string();
                
                // Try appending to the last child if it's already a text node
                if let Some(&last_id) = self.nodes[*parent].children.last() {
                    let last_node = &mut self.nodes[last_id];
                    if let NodeData::Text(ref mut t) = last_node.data {
                        t.push_str(&text_str);
                        return;
                    }
                }
                
                let id = self.nodes.len();
                self.nodes.push(DomNode {
                    id,
                    parent: Some(*parent),
                    children: vec![],
                    data: NodeData::Text(text_str),
                });
                self.nodes[*parent].children.push(id);
            }
        }
    }

    fn append_based_on_parent_node(&mut self, element: &Self::Handle, prev_element: &Self::Handle, child: NodeOrText<Self::Handle>) {
        if let Some(parent) = self.nodes[*element].parent {
            self.append(&parent, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&mut self, _name: StrTendril, _public_id: StrTendril, _system_id: StrTendril) {}

    fn get_template_contents(&mut self, target: &Self::Handle) -> Self::Handle {
        *target
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn set_quirks_mode(&mut self, _mode: QuirksMode) {}

    fn append_before_sibling(&mut self, sibling: &Self::Handle, child: NodeOrText<Self::Handle>) {
        if let Some(parent) = self.nodes[*sibling].parent {
            match child {
                NodeOrText::AppendNode(node_id) => {
                    self.nodes[node_id].parent = Some(parent);
                    let siblings = &mut self.nodes[parent].children;
                    if let Some(pos) = siblings.iter().position(|x| x == sibling) {
                        siblings.insert(pos, node_id);
                    }
                }
                NodeOrText::AppendText(text) => {
                    let id = self.nodes.len();
                    self.nodes.push(DomNode {
                        id,
                        parent: Some(parent),
                        children: vec![],
                        data: NodeData::Text(text.to_string()),
                    });
                    let siblings = &mut self.nodes[parent].children;
                    if let Some(pos) = siblings.iter().position(|x| x == sibling) {
                        siblings.insert(pos, id);
                    }
                }
            }
        }
    }

    fn add_attrs_if_missing(&mut self, target: &Self::Handle, new_attrs: Vec<Attribute>) {
        if let NodeData::Element { ref mut attrs, .. } = self.nodes[*target].data {
            for new_attr in new_attrs {
                if !attrs.iter().any(|a| a.name == new_attr.name) {
                    attrs.push(new_attr);
                }
            }
        }
    }

    fn remove_from_parent(&mut self, target: &Self::Handle) {
        if let Some(parent) = self.nodes[*target].parent {
            self.nodes[parent].children.retain(|x| x != target);
            self.nodes[*target].parent = None;
        }
    }

    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        let children = std::mem::take(&mut self.nodes[*node].children);
        for child in &children {
            self.nodes[*child].parent = Some(*new_parent);
        }
        self.nodes[*new_parent].children.extend(children);
    }
}






