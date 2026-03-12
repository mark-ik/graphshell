/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::manager::ClientStorageManagerHandle;

#[derive(Clone, Default)]
pub(crate) struct ClientStorageBridge {
    manager: Option<ClientStorageManagerHandle>,
}

impl ClientStorageBridge {
    pub(crate) fn new(manager: Option<ClientStorageManagerHandle>) -> Self {
        Self { manager }
    }

    pub(crate) fn manager(&self) -> Option<ClientStorageManagerHandle> {
        self.manager.clone()
    }
}

pub(crate) type NoopClientStorageBridge = ClientStorageBridge;
