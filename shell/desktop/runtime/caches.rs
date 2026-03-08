/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Stage 6 data-plane caches.
//!
//! These caches are explicitly non-authoritative runtime accelerators.
//! They must not mutate lifecycle state or emit reducer intents.

use std::sync::Arc;
use std::time::Duration;

use moka::notification::RemovalCause;
use moka::sync::Cache;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::graph::NodeKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CacheKey {
    Thumbnail(NodeKey),
    ParsedMetadata(String),
    Suggestion(String),
    SnapshotArtifact(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheKind {
    Thumbnail,
    ParsedMetadata,
    Suggestion,
    SnapshotArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewarmHint {
    pub(crate) kind: CacheKind,
    pub(crate) key: CacheKey,
    pub(crate) cause: RemovalCause,
}

#[derive(Debug, Clone)]
pub(crate) struct CachePolicy {
    pub(crate) ttl: Duration,
    pub(crate) thumbnail_capacity: u64,
    pub(crate) metadata_capacity: u64,
    pub(crate) suggestion_capacity: u64,
    pub(crate) snapshot_capacity: u64,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(300),
            thumbnail_capacity: 512,
            metadata_capacity: 512,
            suggestion_capacity: 2048,
            snapshot_capacity: 256,
        }
    }
}

pub(crate) struct RuntimeCaches {
    thumbnail_cache: Cache<NodeKey, Arc<Vec<u8>>>,
    metadata_cache: Cache<String, Arc<Value>>,
    suggestion_cache: Cache<String, Arc<Vec<String>>>,
    snapshot_cache: Cache<String, Arc<Vec<u8>>>,
}

impl RuntimeCaches {
    pub(crate) fn new(
        policy: CachePolicy,
        rewarm_tx: Option<mpsc::UnboundedSender<RewarmHint>>,
    ) -> Self {
        let ttl = policy.ttl;

        let thumbnail_cache = Cache::builder()
            .max_capacity(policy.thumbnail_capacity)
            .time_to_live(ttl)
            .eviction_listener(build_listener(
                CacheKind::Thumbnail,
                rewarm_tx.clone(),
                |key| CacheKey::Thumbnail(*key),
            ))
            .build();

        let metadata_cache = Cache::builder()
            .max_capacity(policy.metadata_capacity)
            .time_to_live(ttl)
            .eviction_listener(build_listener(
                CacheKind::ParsedMetadata,
                rewarm_tx.clone(),
                |key: &String| CacheKey::ParsedMetadata(key.clone()),
            ))
            .build();

        let suggestion_cache = Cache::builder()
            .max_capacity(policy.suggestion_capacity)
            .time_to_live(ttl)
            .eviction_listener(build_listener(
                CacheKind::Suggestion,
                rewarm_tx.clone(),
                |key: &String| CacheKey::Suggestion(key.clone()),
            ))
            .build();

        let snapshot_cache = Cache::builder()
            .max_capacity(policy.snapshot_capacity)
            .time_to_live(ttl)
            .eviction_listener(build_listener(
                CacheKind::SnapshotArtifact,
                rewarm_tx,
                |key: &String| CacheKey::SnapshotArtifact(key.clone()),
            ))
            .build();

        Self {
            thumbnail_cache,
            metadata_cache,
            suggestion_cache,
            snapshot_cache,
        }
    }

    pub(crate) fn insert_thumbnail(&self, key: NodeKey, bytes: Vec<u8>) {
        self.thumbnail_cache.insert(key, Arc::new(bytes));
    }

    pub(crate) fn get_thumbnail(&self, key: NodeKey) -> Option<Arc<Vec<u8>>> {
        self.thumbnail_cache.get(&key)
    }

    pub(crate) fn insert_parsed_metadata(&self, key: String, value: Value) {
        self.metadata_cache.insert(key, Arc::new(value));
    }

    pub(crate) fn get_parsed_metadata(&self, key: &str) -> Option<Arc<Value>> {
        self.metadata_cache.get(key)
    }

    pub(crate) fn insert_suggestions(&self, key: String, suggestions: Vec<String>) {
        self.suggestion_cache.insert(key, Arc::new(suggestions));
    }

    pub(crate) fn get_suggestions(&self, key: &str) -> Option<Arc<Vec<String>>> {
        self.suggestion_cache.get(key)
    }

    pub(crate) fn insert_snapshot_artifact(&self, key: String, bytes: Vec<u8>) {
        self.snapshot_cache.insert(key, Arc::new(bytes));
    }

    pub(crate) fn get_snapshot_artifact(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        self.snapshot_cache.get(key)
    }

    #[cfg(test)]
    fn run_pending_tasks_for_tests(&self) {
        self.thumbnail_cache.run_pending_tasks();
        self.metadata_cache.run_pending_tasks();
        self.suggestion_cache.run_pending_tasks();
        self.snapshot_cache.run_pending_tasks();
    }
}

fn build_listener<K, V, F>(
    kind: CacheKind,
    rewarm_tx: Option<mpsc::UnboundedSender<RewarmHint>>,
    to_key: F,
) -> impl Fn(Arc<K>, Arc<V>, RemovalCause) + Send + Sync + 'static
where
    K: Send + Sync + 'static,
    V: Send + Sync + 'static,
    F: Fn(&K) -> CacheKey + Send + Sync + Clone + 'static,
{
    move |key, _value, cause| {
        if let Some(tx) = &rewarm_tx {
            let _ = tx.send(RewarmHint {
                kind,
                key: to_key(&key),
                cause,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_caches_roundtrip_values() {
        let caches = RuntimeCaches::new(CachePolicy::default(), None);

        let node = NodeKey::new(42);
        caches.insert_thumbnail(node, vec![1, 2, 3]);
        assert_eq!(
            caches.get_thumbnail(node).as_ref().map(|v| v.as_slice()),
            Some(&[1, 2, 3][..])
        );

        caches.insert_parsed_metadata("meta:1".to_string(), serde_json::json!({"k":"v"}));
        assert_eq!(
            caches
                .get_parsed_metadata("meta:1")
                .as_deref()
                .and_then(|v| v.get("k"))
                .and_then(Value::as_str),
            Some("v")
        );

        caches.insert_suggestions("q:rust".to_string(), vec!["rust book".to_string()]);
        assert_eq!(
            caches
                .get_suggestions("q:rust")
                .as_deref()
                .and_then(|s| s.first())
                .map(String::as_str),
            Some("rust book")
        );

        caches.insert_snapshot_artifact("snap:1".to_string(), vec![9, 8, 7]);
        assert_eq!(
            caches
                .get_snapshot_artifact("snap:1")
                .as_ref()
                .map(|v| v.as_slice()),
            Some(&[9, 8, 7][..])
        );
    }

    #[tokio::test]
    async fn eviction_listener_emits_rewarm_hints() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let policy = CachePolicy {
            ttl: Duration::from_secs(300),
            thumbnail_capacity: 1,
            metadata_capacity: 1,
            suggestion_capacity: 1,
            snapshot_capacity: 1,
        };
        let caches = RuntimeCaches::new(policy, Some(tx));

        caches.insert_thumbnail(NodeKey::new(1), vec![1]);
        caches.insert_thumbnail(NodeKey::new(2), vec![2]);
        caches.run_pending_tasks_for_tests();

        // Capacity=1 guarantees an eviction on second insert.
        let hint = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("expected a rewarm hint")
            .expect("channel should remain open");

        assert_eq!(hint.kind, CacheKind::Thumbnail);
        assert!(matches!(hint.key, CacheKey::Thumbnail(_)));
        assert!(matches!(hint.cause, RemovalCause::Size));
    }
}
