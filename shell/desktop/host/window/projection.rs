/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphshell projection state: pane focus, input retargeting, chrome projection,
//! dialog ownership, and visible pane snapshots.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use servo::WebViewId;

use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::workbench::pane_model::PaneId;

use super::{ChromeProjectionSource, DialogOwner, InputTarget};

pub(super) struct WindowProjectionState {
    pub(super) focused_pane: Cell<Option<PaneId>>,
    pub(super) input_target: Cell<Option<InputTarget>>,
    pub(super) chrome_projection_source: Cell<Option<ChromeProjectionSource>>,
    pub(super) dialog_owner: Cell<Option<DialogOwner>>,
    pub(super) visible_node_panes: RefCell<Vec<PaneId>>,
}

impl Default for WindowProjectionState {
    fn default() -> Self {
        Self {
            focused_pane: Cell::new(None),
            input_target: Cell::new(None),
            chrome_projection_source: Cell::new(None),
            dialog_owner: Cell::new(None),
            visible_node_panes: RefCell::new(Vec::new()),
        }
    }
}

impl WindowProjectionState {
    pub(super) fn focused_pane(&self) -> Option<PaneId> {
        self.focused_pane.get()
    }

    pub(super) fn set_focused_pane(&self, pane_id: Option<PaneId>) {
        self.focused_pane.set(pane_id);
    }

    pub(super) fn input_target(&self) -> Option<InputTarget> {
        self.input_target.get()
    }

    pub(super) fn set_input_target(&self, target: Option<InputTarget>) {
        self.input_target.set(target);
    }

    pub(super) fn chrome_projection_source(&self) -> Option<ChromeProjectionSource> {
        self.chrome_projection_source.get()
    }

    pub(super) fn set_chrome_projection_source(&self, source: Option<ChromeProjectionSource>) {
        self.chrome_projection_source.set(source);
    }

    pub(super) fn dialog_owner(&self) -> Option<DialogOwner> {
        self.dialog_owner.get()
    }

    pub(super) fn set_dialog_owner(&self, owner: Option<DialogOwner>) {
        self.dialog_owner.set(owner);
    }

    pub(super) fn set_visible_node_panes(&self, pane_ids: Vec<PaneId>) {
        *self.visible_node_panes.borrow_mut() = pane_ids;
    }

    pub(super) fn visible_renderer_ids(&self) -> Vec<WebViewId> {
        let mut seen = HashSet::new();
        self.visible_node_panes
            .borrow()
            .iter()
            .filter_map(|pane_id| registries::phase1_renderer_attachment_for_pane(*pane_id))
            .filter_map(|attachment| {
                seen.insert(attachment.renderer_id)
                    .then_some(attachment.renderer_id)
            })
            .collect()
    }

    pub(super) fn explicit_input_webview_id(&self) -> Option<WebViewId> {
        match self.input_target() {
            Some(InputTarget::Host) => None,
            Some(InputTarget::Renderer(renderer_id)) => Some(renderer_id),
            Some(InputTarget::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => self.focused_pane().and_then(|pane_id| {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }),
        }
    }

    pub(super) fn targeted_input_webview_id(&self) -> Option<WebViewId> {
        match self.input_target() {
            Some(InputTarget::Host) => None,
            Some(InputTarget::Renderer(renderer_id)) => Some(renderer_id),
            Some(InputTarget::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    pub(super) fn explicit_dialog_webview_id(&self) -> Option<WebViewId> {
        match self.dialog_owner() {
            Some(DialogOwner::Renderer(renderer_id)) => Some(renderer_id),
            Some(DialogOwner::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    pub(super) fn explicit_chrome_webview_id(&self) -> Option<WebViewId> {
        match self.chrome_projection_source() {
            Some(ChromeProjectionSource::Renderer(renderer_id)) => Some(renderer_id),
            Some(ChromeProjectionSource::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    pub(super) fn dialog_owner_for_webview(&self, webview_id: WebViewId) -> DialogOwner {
        registries::phase1_pane_for_renderer(webview_id)
            .map(DialogOwner::Pane)
            .unwrap_or(DialogOwner::Renderer(webview_id))
    }

    pub(super) fn sync_explicit_targets_for_webview(&self, webview_id: WebViewId) {
        let pane_id = registries::phase1_pane_for_renderer(webview_id);
        self.set_focused_pane(pane_id);
        self.set_input_target(Some(InputTarget::Renderer(webview_id)));
        self.set_chrome_projection_source(Some(ChromeProjectionSource::Renderer(webview_id)));
        self.set_dialog_owner(Some(self.dialog_owner_for_webview(webview_id)));
    }

    pub(super) fn clear_explicit_targets_for_closed_webview(
        &self,
        webview_id: WebViewId,
        detached_pane_id: Option<PaneId>,
    ) {
        if self.focused_pane() == detached_pane_id {
            self.set_focused_pane(None);
        }

        if matches!(
            self.input_target(),
            Some(InputTarget::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.input_target(), detached_pane_id),
            (Some(InputTarget::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_input_target(None);
        }

        if matches!(
            self.chrome_projection_source(),
            Some(ChromeProjectionSource::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.chrome_projection_source(), detached_pane_id),
            (Some(ChromeProjectionSource::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_chrome_projection_source(None);
        }

        if matches!(
            self.dialog_owner(),
            Some(DialogOwner::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.dialog_owner(), detached_pane_id),
            (Some(DialogOwner::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_dialog_owner(None);
        }
    }
}

