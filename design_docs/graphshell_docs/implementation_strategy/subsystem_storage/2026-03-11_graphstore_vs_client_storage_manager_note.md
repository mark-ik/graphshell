# GraphStore vs ClientStorageManager

**Date**: 2026-03-11  
**Status**: Active architecture note  
**Purpose**: Define the clean architectural seam between Graphshell app-state durability (`GraphStore`) and a future Servo-compatible web-origin storage authority (`ClientStorageManager`).

**Related**:
- `SUBSYSTEM_STORAGE.md`
- `2026-03-08_unified_storage_architecture_plan.md`
- `2026-03-11_client_storage_manager_implementation_plan.md`
- `storage_and_persistence_integrity_spec.md`
- `../../research/2026-03-04_standards_alignment_report.md`

---

## 1. Problem Statement

Graphshell already has a substantial persistence subsystem centered on `GraphStore`.
That subsystem is responsible for Graphshell's own durable app state:

- graph WAL and snapshots
- workspace layout persistence
- archive persistence
- recovery, integrity, and encryption health

That is not the same problem as browser client storage for web-origin APIs such
as IndexedDB, localStorage, Cache API, OPFS, or future bucket-aware storage
clients.

The WHATWG Storage Standard applies to the latter problem. Its canonical model
is storage-key-scoped and hierarchical: storage shed -> storage shelf -> storage
bucket -> storage bottle. A future Servo-facing `ClientStorageManager` should
implement that model. `GraphStore` should not be forced into it.

---

## 2. Boundary

| Concern | `GraphStore` | Future `ClientStorageManager` |
| --- | --- | --- |
| Primary scope | Graphshell app durability | Web-origin client storage |
| Authority key | Graphshell graph/workspace identity | WHATWG storage key |
| Main data classes | Graph, layouts, archives, settings payloads | Buckets, bottles, endpoint storage roots, quota/persistence metadata |
| Canonical model | WAL + snapshots + archives | Shed + shelf + bucket + bottle |
| Durability contract | App recovery and integrity | Site data lifecycle and storage API coordination |
| Clearing semantics | Clear graph / layouts / archives | Clear site data / delete bucket / async purge |
| Session model | App/workbench session persistence | Local vs session storage split |
| Standard target | Internal Graphshell contracts | WHATWG Storage Standard |

Key rule:

`GraphStore` is the persistence authority for Graphshell-owned application
state. `ClientStorageManager` would be the storage authority for browser-origin
storage clients. They may share lower-level crypto/path/diagnostic utilities,
but they are not the same subsystem surface and should not share a conceptual
model.

Compatibility rule:

Graphshell should treat Servo's storage-spec work as the canonical target model
for browser-origin storage. Any Graphshell-layer client-storage code must be
Servo-compatible first and should avoid inventing a rival hierarchy or metadata
model that would later need to be translated back into Servo terms.

---

## 3. Future ClientStorageManager Role

`ClientStorageManager` is the in-memory authority for origin-scoped storage
metadata and policy. It should own:

- storage-key to shelf resolution
- bucket metadata and bucket mode (`best-effort` / `persistent`)
- bottle metadata per registered storage endpoint
- session-vs-local storage separation
- private browsing isolation
- usage/quota accounting hooks
- site-data clearing and asynchronous deletion scheduling
- endpoint-neutral lookup of physical storage roots

It should not directly embed every storage client's data model. The bottle's map
is a high-level abstraction over a given endpoint's stored data; endpoint
implementations remain responsible for their own internal representation.

Graphshell should prefer one of two implementation postures:

1. a thin host-facing adapter over Servo's eventual storage authority, or
2. a staging layer that uses Servo-compatible concepts, naming, and ownership so
    it can collapse into Servo's implementation later without semantic churn.

It should not pursue a third, independent browser-storage architecture.

---

## 4. Recommended Runtime Shape

The manager should load all authoritative metadata into memory during startup,
then persist metadata changes through a pluggable backend. This keeps spec-level
operations synchronous or cheaply asynchronous at the runtime boundary, while
still allowing different physical storage implementations.

Suggested internal state:

- `local_shed: StorageShed`
- `session_sheds: HashMap<TraversableId, StorageShed>`
- `private_local_sheds: HashMap<PrivateScopeId, StorageShed>`
- `registered_endpoints: HashMap<StorageIdentifier, EndpointDescriptor>`
- `pending_deletions: Vec<PendingBucketDeletion>`

The backend should persist metadata and allocate physical bucket roots, but it
should not be the authority for the live hierarchy. The manager is the authority;
the backend is a persistence and storage-allocation service.

Above that, Graphshell itself still has a legitimate host/runtime role for
backend orchestration, especially where Servo and Wry need interoperability
policy rather than shared physical storage formats.

---

## 4AA. Graphshell Storage Interop Layer

Graphshell should define a small host-side orchestration layer above browser
storage authority. A useful name is **StorageInteropCoordinator**.

This layer is not a third storage authority. It does not own storage keys,
shelves, buckets, bottles, quota, or bucket lifecycle semantics. Instead it
owns backend orchestration and compatibility policy for browser runtimes.

Recommended responsibilities:

- map nodes/panes/views to browser storage contexts or profile identities
- decide whether a backend switch is shared-context, cloned-context, or
    isolated-fallback
- route explicit user commands such as "clear site data for current node" or
    "reload in Wry" to the correct backend authority
- mediate Wry profile/session handling so it stays conceptually compatible with
    Servo, even if the physical data formats differ
- expose diagnostics about backend-specific storage continuity limits

Recommended non-responsibilities:

- owning the Storage Standard hierarchy
- redefining storage-key semantics
- persisting canonical bucket metadata separately from Servo-compatible storage
- silently merging Servo and Wry physical storage stores

---

## 4A. Concrete Servo-Facing Module Plan

Within Graphshell's current architecture, `ClientStorageManager` should live on
the Verso / browser-runtime side of the system, not in `GraphStore`, and not in
the reducer-owned graph domain.

Recommended module layout:

```text
mods/native/verso/client_storage/
        mod.rs                   // public facade + runtime wiring entrypoints
        manager.rs               // ClientStorageManagerImpl; in-memory authority
        types.rs                 // storage-key, shed/shelf/bucket/bottle types
        backend.rs               // ClientStorageBackend trait + persistence contracts
        endpoint.rs              // StorageEndpointClient trait + endpoint descriptors
        quota.rs                 // usage/quota estimation and policy hooks
        deletion.rs              // async bucket/site-data deletion queue
        private_scope.rs         // private browsing scope isolation helpers
        session.rs               // traversable/session shed ownership helpers
        diagnostics.rs           // storage-manager diagnostic channel emission
        bridge.rs                // thin Servo/Verso integration boundary

    app/storage_interop/
        mod.rs                   // host-facing orchestration facade
        coordinator.rs           // StorageInteropCoordinator
        context_map.rs           // node/pane/backend -> storage context mapping
        transition_policy.rs     // Servo <-> Wry transition decisions
        wry_profile.rs           // Wry-specific profile/session helpers
        commands.rs              // explicit user-facing clear/reload/forget actions
```

If Graphshell later completes the planned core/host split, only the pure value
types from `types.rs` are candidates for extraction into a host-shared crate.
The manager, backend, deletion queue, and Servo bridge remain host/runtime code.
    The `storage_interop` layer is also host/runtime code.

---

## 4B. Placement in Existing Authority Domains

`ClientStorageManager` should follow the same authority separation Graphshell
already uses elsewhere:

- **GraphWorkspace / reducer domain**: does not own browser site-data metadata
    and does not mutate storage shelves/buckets/bottles.
- **AppServices / runtime instances**: owns the live `ClientStorageManager`
    handle, just as it owns runtime services such as embedder state.
- **Verso / EmbedderCore bridge**: obtains bottle handles or storage lookups
    from the manager, but does not become the authority for shelf/bucket metadata.
- **StorageInteropCoordinator**: runtime-owned Graphshell policy layer that can
    coordinate Servo/Wry backend transitions and user-facing compound actions,
    but does not become the owner of storage truth.

This keeps site-data policy where it belongs: runtime-owned, not graph-owned.

---

## 4C. Core Runtime Types

Suggested concrete type inventory:

```rust
pub struct StorageKey {
        pub origin: ImmutableOrigin,
        pub partition: Option<StoragePartitionKey>,
}

pub enum StorageScope {
        Local,
        Session(TraversableId),
        Private(PrivateScopeId),
}

pub struct StorageShed {
        pub shelves: HashMap<StorageKey, StorageShelf>,
}

pub struct StorageShelf {
        pub key: StorageKey,
        pub bucket_map: HashMap<BucketName, StorageBucket>,
}

pub struct StorageBucket {
        pub name: BucketName,
        pub mode: BucketMode,
        pub generation: BucketGeneration,
        pub bottle_map: HashMap<StorageIdentifier, StorageBottle>,
        pub root: PhysicalBucketRoot,
}

pub struct StorageBottle {
        pub endpoint: StorageIdentifier,
        pub quota_hint: Option<u64>,
        pub proxy_generation: BucketGeneration,
}

pub struct EndpointDescriptor {
        pub identifier: StorageIdentifier,
        pub storage_types: StorageTypeSet,
        pub quota_hint: Option<u64>,
}

pub struct PendingBucketDeletion {
        pub locator: BucketLocator,
        pub generation: BucketGeneration,
        pub ticket: DeletionTicket,
}
```

Notes:

- `StorageKey` must be future-proofed for partitioning; do not hard-code
    origin-only semantics into naming.
- `BucketGeneration` is the simplest way to make async deletion safe: a new
    bucket with the same logical name can exist while an older generation is still
    being deleted.
- `PhysicalBucketRoot` is implementation-defined and backend-owned, but the
    manager keeps the authoritative mapping.

---

## 4D. Ownership Rules

1. `GraphStore` owns Graphshell app durability only.
2. `ClientStorageManagerImpl` owns all in-memory shelf/bucket/bottle metadata.
3. `ClientStorageBackend` owns durable metadata persistence and physical bucket
     root allocation/deletion only.
4. `StorageEndpointClient` implementations own endpoint-specific bottle usage,
     but not bucket creation, persistence mode, quota policy, or site clearing.
5. Session sheds are owned by the manager and keyed by `TraversableId`; endpoint
     clients must not retain authority after the traversable is closed.
6. Private scope sheds are isolated from durable local sheds and must be torn
     down wholesale when the private scope ends.
7. Servo callbacks and embedder code may request storage handles, but must not
     mutate the hierarchy except through manager APIs.
8. The deletion worker may delete physical roots asynchronously, but must not
     change logical metadata except through manager-owned state transitions.
9. `StorageInteropCoordinator` may map nodes/panes/backends to storage contexts
   and invoke explicit clear/reload actions, but it must not become the source
   of truth for storage-key, bucket, or quota metadata.

---

## 4E. Startup and Runtime Flow

Recommended boot sequence:

1. App startup constructs `AppServices`.
2. Verso runtime initializes `EmbedderCore`.
3. `ClientStorageBackend::load_metadata()` loads persisted storage metadata.
4. `ClientStorageManagerImpl::from_snapshot(...)` reconstructs in-memory sheds,
     shelves, buckets, and endpoint descriptors.
5. Registered storage endpoints are attached.
6. Deletion queue resumes any orphaned pending deletions.
7. Runtime stores the manager handle in `AppServices`.

Request path:

1. Servo/Verso asks for a bottle using `(scope, storage key, bucket, endpoint)`.
2. Manager resolves or creates the shelf and bucket in memory.
3. Manager asks backend to ensure the bucket root exists if needed.
4. Manager returns a bottle handle to the endpoint client.
5. Endpoint client performs its own API-specific storage operations under that
     bottle/root.

Deletion path:

1. Manager marks the bucket generation as deleted in memory.
2. Manager persists updated metadata.
3. Backend schedules physical deletion for the old generation.
4. New bucket generation may be created immediately with the same logical name.

Graphshell interop path:

1. A node/pane is associated with a backend runtime and a storage context id.
2. If the user requests reload in another backend, `StorageInteropCoordinator`
    resolves the transition policy.
3. The coordinator chooses one of: shared logical context, cloned compatibility
    context, or isolated fallback context.
4. The target backend is started with the selected context binding and any
    explicit continuity warnings/diagnostics.

---

## 4F. Threading and Synchronization Posture

The manager should be optimized for cheap reads from in-memory metadata.

Recommended posture:

- `ClientStorageManagerImpl` behind `Arc<RwLock<...>>` or an equivalent runtime
    ownership model.
- Read-side resolution of shelves/buckets/bottles should avoid synchronous disk
    lookups.
- Metadata persistence and physical directory deletion happen off the hot path.
- Endpoint clients should receive stable handles containing logical identity and
    physical root information, not mutable references into the manager's internal
    maps.

This matches the Zulip design discussion: spec-facing lookups should resolve
against already-loaded in-memory hierarchy, not by blocking on a database for
every bottle request.

---

## 4G. Diagnostics and Failure Boundaries

Recommended diagnostic families for this future module:

- `client_storage.metadata.loaded`
- `client_storage.metadata.persist_failed`
- `client_storage.bucket.created`
- `client_storage.bucket.deletion_scheduled`
- `client_storage.bucket.deletion_failed`
- `client_storage.quota.estimate_failed`
- `client_storage.scope.private_destroyed`
- `client_storage.session_shed.cleared`

Important failure rule:

metadata failure and payload failure must be distinguished. A broken IndexedDB
database file is not the same category of fault as a corrupted bucket registry.
The manager owns registry integrity; endpoint clients own endpoint-local data
integrity.

For Graphshell interop, add a third category:

- **interop continuity failure** — the browser backend switch could not preserve
    storage continuity exactly, so Graphshell had to fall back to a cloned or
    isolated context. This is not a registry corruption and not an endpoint
    payload corruption; it is a host-level compatibility limitation.

---

## 4H. Reference Truth vs Storage Truth

Graphshell nodes are references to content and browsing contexts. They are not,
by default, the owners of the underlying browser site data.

Rules:

1. Deleting a node does **not** automatically delete site data.
2. Site data is owned by the browser storage authority (`ClientStorageManager`
     for Servo-backed contexts; backend-specific profile manager for Wry-backed
     contexts).
3. Graphshell may expose explicit compound actions such as:
     - delete node only
     - clear site data only
     - delete node and clear site data
4. If Graphshell wants automatic cleanup heuristics, they must operate on
     storage-key or context reference policy, not on the assumption that one node
     owns one site's data.

This is a policy hierarchy, not an ownership hierarchy:

- browser storage authority owns storage truth
- Graphshell owns reference truth
- cross-effects happen only through explicit policy

---

## 4I. Servo <-> Wry Compatibility Rules

Graphshell should assume that Servo and Wry will often have different physical
storage implementations and on-disk formats.

Therefore compatibility should be defined at the policy level, not by assuming
binary-compatible stores.

Recommended transition classes:

| Transition class | Meaning | When to use |
| --- | --- | --- |
| Shared logical context | Same logical storage context id is preserved across backends | Only when semantics and implementation support it safely |
| Cloned compatibility context | Relevant state is copied or approximated into a backend-specific context | When continuity is desired but exact store sharing is unsafe |
| Isolated fallback context | New backend starts with an isolated context/profile | When compatibility is uncertain or unsupported |

Recommended default posture:

- Servo is the canonical target for browser-origin storage semantics.
- Wry fallback should be treated as a peer runtime with its own profile/store.
- Graphshell should not promise exact physical storage sharing between Servo and
    Wry unless proven safe for a given data class.

Likely continuity classes by default:

- cookies / permissions: maybe clonable, backend-dependent
- localStorage / sessionStorage: maybe clonable, not assumed shareable
- IndexedDB / Cache API / OPFS / service workers: not assumed shareable;
    default to isolated or explicitly migrated contexts

User-facing commands should make the policy explicit:

- reload in Servo
- reload in Wry
- reload in isolated fallback context
- clear site data for current storage context
- forget Wry fallback profile

---

## 5. Trait Sketch

```rust
pub trait ClientStorageManager {
    fn obtain_local_shelf(
        &self,
        key: &StorageKey,
        scope: StorageScope,
    ) -> Result<StorageShelfHandle, StorageError>;

    fn obtain_session_shelf(
        &self,
        traversable: TraversableId,
        key: &StorageKey,
    ) -> Result<StorageShelfHandle, StorageError>;

    fn obtain_bottle(
        &self,
        scope: StorageScope,
        key: &StorageKey,
        bucket: BucketName,
        endpoint: StorageIdentifier,
    ) -> Result<StorageBottleHandle, StorageError>;

    fn create_bucket(
        &mut self,
        scope: StorageScope,
        key: &StorageKey,
        name: BucketName,
    ) -> Result<BucketDescriptor, StorageError>;

    fn delete_bucket_async(
        &mut self,
        scope: StorageScope,
        key: &StorageKey,
        name: BucketName,
    ) -> Result<DeletionTicket, StorageError>;

    fn set_bucket_persistence(
        &mut self,
        scope: StorageScope,
        key: &StorageKey,
        name: BucketName,
        mode: BucketMode,
    ) -> Result<(), StorageError>;

    fn estimate_usage(
        &self,
        scope: StorageScope,
        key: &StorageKey,
    ) -> Result<StorageEstimate, StorageError>;

    fn clear_storage_key(
        &mut self,
        scope: StorageScope,
        key: &StorageKey,
    ) -> Result<DeletionTicket, StorageError>;
}

pub trait ClientStorageBackend {
    fn load_metadata(&self) -> Result<ClientStorageSnapshot, StorageError>;

    fn persist_metadata(
        &self,
        snapshot: &ClientStorageSnapshot,
    ) -> Result<(), StorageError>;

    fn ensure_bucket_root(
        &self,
        locator: &BucketLocator,
    ) -> Result<PhysicalBucketRoot, StorageError>;

    fn schedule_bucket_root_deletion(
        &self,
        root: &PhysicalBucketRoot,
        generation: BucketGeneration,
    ) -> Result<DeletionTicket, StorageError>;

    fn estimate_bucket_usage(
        &self,
        locator: &BucketLocator,
    ) -> Result<u64, StorageError>;
}

pub trait StorageEndpointClient {
    fn identifier(&self) -> StorageIdentifier;
    fn storage_types(&self) -> StorageTypeSet;

    fn open_bottle(
        &self,
        bottle: &StorageBottleHandle,
    ) -> Result<Box<dyn EndpointStorageHandle>, StorageError>;
}
```

Interface notes:

- `ClientStorageManager` owns the spec hierarchy and policy.
- `ClientStorageBackend` is pluggable and implementation-defined.
- `StorageEndpointClient` lets IndexedDB, localStorage, Cache API, OPFS, and
  future clients consume the same hierarchy without each inventing separate
  registry logic.

Concrete assignment:

- `manager.rs` owns `ClientStorageManagerImpl`.
- `backend.rs` owns the trait plus the default backend adapter.
- `endpoint.rs` owns endpoint registration and endpoint-facing handles.
- `bridge.rs` owns the only Servo/Verso-facing integration seam.

---

## 6. Design Rules

1. Use `storage key`, not `origin`, in public API and metadata naming.
2. Persist bucket/shelf/bottle metadata; named buckets must survive restart.
3. Load metadata into memory during initialization.
4. Support multiple buckets from the start, even if only `default` is fully
   wired initially.
5. Treat async deletion as first-class; deletion should not require all existing
   endpoint handles to close first.
6. Keep session storage separate from local durable storage.
7. Do not fold Graphshell app durability into the Storage Standard hierarchy.
8. Keep Graphshell's browser-storage orchestration Servo-compatible first; avoid
    creating a rival browser-storage model.
9. Treat node deletion and site-data deletion as distinct operations unless an
    explicit compound policy says otherwise.
10. Treat Servo/Wry continuity as a host-level compatibility policy problem,
     not as proof of shared physical storage.

---

## 7. Near-Term Guidance

For Graphshell documentation and future Servo-facing work, use this split:

- `GraphStore` remains the owner of Graphshell app durability.
- `ClientStorageManager` is the reserved name for future WHATWG-style browser
  client storage coordination.
- Shared helpers are acceptable below that seam, but not a merged conceptual
  model.
