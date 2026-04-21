/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod backend;
pub(crate) mod bridge;
pub(crate) mod endpoint;
pub(crate) mod manager;
pub(crate) mod types;

#[allow(unused_imports)]
pub(crate) use backend::{ClientStorageBackend, ClientStorageBackendHandle};
#[allow(unused_imports)]
pub(crate) use bridge::{ClientStorageBridge, NoopClientStorageBridge};
#[allow(unused_imports)]
pub(crate) use endpoint::{StorageEndpointClient, StorageEndpointClientHandle};
#[allow(unused_imports)]
pub(crate) use manager::{
    ClientStorageManager, ClientStorageManagerHandle, ClientStorageRuntimeSummary,
    NoopClientStorageManager,
};
#[allow(unused_imports)]
pub(crate) use types::{
    BucketGeneration, BucketLocator, BucketMode, BucketName, ClientStorageSnapshot,
    EndpointDescriptor, PendingBucketDeletion, PrivateStorageScopeId, StorageBottle, StorageBucket,
    StorageIdentifier, StorageKey, StoragePartitionKey, StorageScope, StorageShed, StorageShelf,
    TraversableStorageId,
};
