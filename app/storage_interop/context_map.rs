/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct StorageContextId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum BrowserStorageBackend {
    #[default]
    Servo,
    Wry,
    Other(String),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StorageContextBinding {
    pub context_id: StorageContextId,
    pub backend: BrowserStorageBackend,
    pub node_debug_id: Option<u64>,
    pub pane_debug_id: Option<String>,
}
