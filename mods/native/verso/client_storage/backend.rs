/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use super::types::ClientStorageSnapshot;

pub(crate) trait ClientStorageBackend: Send + Sync {
    fn load_metadata(&self) -> Result<ClientStorageSnapshot, String>;
    fn persist_metadata(&self, snapshot: &ClientStorageSnapshot) -> Result<(), String>;
}

pub(crate) type ClientStorageBackendHandle = Arc<dyn ClientStorageBackend>;

