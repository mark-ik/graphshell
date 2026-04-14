/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! WebRender / wgpu backend integration
//!
//! Submits display lists to WebRender wgpu backend.

use webrender::Transaction;
use webrender::api::{
    DisplayListBuilder, RenderNotifier, PipelineId, DocumentId, IdNamespace, SpaceAndClipInfo, ColorF, CommonItemProperties, Epoch,
};
use webrender::api::units::{LayoutSize, LayoutRect};

/// The main compositor interface
pub struct MiddleNetCompositor {
    pipeline_id: PipelineId,
    document_id: DocumentId,
}

impl Default for MiddleNetCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddleNetCompositor {
    pub fn new() -> Self {
        Self {
            pipeline_id: PipelineId(0, 0),
            // Normally provided by the renderer instance when the window creates it.
            // Keeping stub IDs for now.
            document_id: DocumentId::new(IdNamespace(0), 0), 
        }
    }

    /// Prepares a WebRender transaction to submit drawn content for layout nodes.
    pub fn build_transaction(&self, width: f32, height: f32) -> Transaction {
        let size = LayoutSize::new(width, height);
        let mut builder = DisplayListBuilder::new(self.pipeline_id);
        
        let space_and_clip = SpaceAndClipInfo::root_scroll(self.pipeline_id);

        builder.begin();

        // Stub: push a placeholder background clearing rectangle representing the body
        let rect = LayoutRect::from_size(size);
        let common = CommonItemProperties::new(rect, space_and_clip);
        builder.push_rect(
            &common,
            rect,
            ColorF::new(0.0, 0.0, 0.0, 0.0), // transparent fallback
        );

        // Later: Traverse the Taffy layout tree using `layout.rs` results 
        // to push text/rects/images onto this builder.

        let mut txn = Transaction::new();
        txn.set_display_list(
            Epoch(0),
            builder.end(),
        );

        txn
    }
}

