/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub(crate) struct ClientStorageRuntimeSummary {
    pub registered_endpoints: usize,
    pub local_shelves: usize,
    pub session_sheds: usize,
    pub private_scopes: usize,
    pub pending_deletions: usize,
}

pub(crate) trait ClientStorageManager: Send + Sync {
    fn runtime_summary(&self) -> ClientStorageRuntimeSummary;
}

pub(crate) type ClientStorageManagerHandle = Arc<dyn ClientStorageManager>;

#[derive(Debug, Default)]
pub(crate) struct NoopClientStorageManager;

impl ClientStorageManager for NoopClientStorageManager {
    fn runtime_summary(&self) -> ClientStorageRuntimeSummary {
        ClientStorageRuntimeSummary::default()
    }
}

