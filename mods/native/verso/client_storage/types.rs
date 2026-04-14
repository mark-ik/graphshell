/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct StorageIdentifier(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct BucketName(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct BucketGeneration(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct TraversableStorageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct PrivateStorageScopeId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct StoragePartitionKey {
    pub partition_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct StorageKey {
    pub origin: String,
    pub partition: Option<StoragePartitionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BucketMode {
    #[default]
    BestEffort,
    Persistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StorageScope {
    Local,
    Session(TraversableStorageId),
    Private(PrivateStorageScopeId),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StorageShed {
    pub shelves: HashMap<StorageKey, StorageShelf>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StorageShelf {
    pub key: StorageKey,
    pub bucket_map: HashMap<BucketName, StorageBucket>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StorageBucket {
    pub name: BucketName,
    pub mode: BucketMode,
    pub generation: BucketGeneration,
    pub bottle_map: HashMap<StorageIdentifier, StorageBottle>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StorageBottle {
    pub endpoint: StorageIdentifier,
    pub quota_hint: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EndpointDescriptor {
    pub identifier: StorageIdentifier,
    pub supports_local: bool,
    pub supports_session: bool,
    pub quota_hint: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BucketLocator {
    pub scope: StorageScope,
    pub key: StorageKey,
    pub bucket: BucketName,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PendingBucketDeletion {
    pub locator: BucketLocator,
    pub generation: BucketGeneration,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClientStorageSnapshot {
    pub local_shed: StorageShed,
    pub session_sheds: HashMap<TraversableStorageId, StorageShed>,
    pub private_local_sheds: HashMap<PrivateStorageScopeId, StorageShed>,
    pub registered_endpoints: HashMap<StorageIdentifier, EndpointDescriptor>,
    pub pending_deletions: Vec<PendingBucketDeletion>,
}

impl Default for StorageScope {
    fn default() -> Self {
        Self::Local
    }
}

