/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Embedder runtime state: owned WebViews and repaint/update/close flags.

use std::cell::{Cell, RefCell};

use servo::{WebView, WebViewId};

use crate::shell::desktop::host::running_app_state::WebViewCollection;

pub(super) struct WindowRuntimeState {
    pub(super) webviews: RefCell<WebViewCollection>,
    pub(super) close_scheduled: Cell<bool>,
    pub(super) needs_update: Cell<bool>,
    pub(super) needs_repaint: Cell<bool>,
}

impl Default for WindowRuntimeState {
    fn default() -> Self {
        Self {
            webviews: Default::default(),
            close_scheduled: Default::default(),
            needs_update: Default::default(),
            needs_repaint: Default::default(),
        }
    }
}

impl WindowRuntimeState {
    pub(super) fn should_close(&self) -> bool {
        self.close_scheduled.get()
    }

    pub(super) fn schedule_close(&self) {
        self.close_scheduled.set(true);
    }

    pub(super) fn contains_webview(&self, id: WebViewId) -> bool {
        self.webviews.borrow().contains(id)
    }

    pub(super) fn webview_by_id(&self, id: WebViewId) -> Option<WebView> {
        self.webviews.borrow().get(id).cloned()
    }

    pub(super) fn add_webview(&self, webview: WebView) {
        self.webviews.borrow_mut().add(webview);
    }

    pub(super) fn remove_webview(&self, webview_id: WebViewId) -> bool {
        self.webviews.borrow_mut().remove(webview_id).is_some()
    }

    pub(super) fn webview_ids(&self) -> Vec<WebViewId> {
        self.webviews.borrow().creation_order.clone()
    }

    pub(super) fn webviews(&self) -> Vec<(WebViewId, WebView)> {
        self.webviews
            .borrow()
            .all_in_creation_order()
            .map(|(id, webview)| (id, webview.clone()))
            .collect()
    }

    pub(super) fn newest_webview_id(&self) -> Option<WebViewId> {
        self.webviews.borrow().newest().map(|webview| webview.id())
    }

    pub(super) fn for_each_webview(&self, mut f: impl FnMut(&WebView)) {
        for webview in self.webviews.borrow().values() {
            f(webview);
        }
    }

    pub(super) fn set_needs_update(&self) {
        self.needs_update.set(true);
    }

    pub(super) fn take_needs_update(&self) -> bool {
        self.needs_update.take()
    }

    pub(super) fn set_needs_repaint(&self) {
        self.needs_repaint.set(true);
    }

    pub(super) fn take_needs_repaint(&self) -> bool {
        self.needs_repaint.take()
    }
}

