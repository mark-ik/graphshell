/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Layout phase with Taffy + Parley.

use taffy::{prelude::*, Taffy};
use parley::LayoutContext;

/// Manages layout and geometry generation using Taffy (Flexbox/CSS Grid style).
pub struct MiddleNetLayout {
    tree: Taffy,
    text: LayoutContext<()>,
}

impl Default for MiddleNetLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddleNetLayout {
    pub fn new() -> Self {
        Self {
            tree: Taffy::new(),
            text: LayoutContext::new(),
        }
    }

    /// Creates a generic block container node in the layout tree.
    pub fn create_block(&mut self, children: &[Node]) -> Node {
        let style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size: Size {
                width: auto(),
                height: auto(),
            },
            ..Default::default()
        };
        self.tree.new_with_children(style, children).unwrap()
    }

    /// Creates a leaf node representing measured text.
    pub fn create_text_leaf(&mut self, _text_content: &str) -> Node {
        // Here we would normally use Parley to measure the text and create a leaf node
        // with the specific measured dimensions. For now, we stub it with a fixed size
        // or a simple Taffy leaf.
        let style = Style {
            size: Size { width: Dimension::Points(120.0), height: Dimension::Points(24.0) },
            ..Default::default()
        };
        self.tree.new_leaf(style).unwrap()
    }

    /// Computes the final layout tree given an available screen size.
    pub fn compute(&mut self, root: Node, screen_width: f32, screen_height: f32) -> Result<(), taffy::error::TaffyError> {
        let available_space = Size {
            width: AvailableSpace::Definite(screen_width),
            height: AvailableSpace::Definite(screen_height),
        };
        self.tree.compute_layout(root, available_space)
    }

    /// Retrieves the computed geometry layout of a node.
    pub fn get_layout(&self, node: Node) -> Result<&Layout, taffy::error::TaffyError> {
        self.tree.layout(node)
    }

}

