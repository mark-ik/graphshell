/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod context_map;
pub(crate) mod coordinator;
pub(crate) mod transition_policy;

#[allow(unused_imports)]
pub(crate) use context_map::{BrowserStorageBackend, StorageContextBinding, StorageContextId};
#[allow(unused_imports)]
pub(crate) use coordinator::{
    NoopStorageInteropCoordinator, StorageInteropCoordinator, StorageInteropCoordinatorHandle,
};
#[allow(unused_imports)]
pub(crate) use transition_policy::{BackendTransitionPlan, StorageTransitionClass};
