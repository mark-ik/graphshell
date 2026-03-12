# ClientStorageManager Implementation Plan

**Date**: 2026-03-11  
**Status**: Active phased plan  
**Purpose**: Convert the `GraphStore` vs `ClientStorageManager` architecture note into an execution plan for a future Servo-compatible browser-origin storage authority.

**Related**:
- `2026-03-11_graphstore_vs_client_storage_manager_note.md`
- `SUBSYSTEM_STORAGE.md`
- `2026-03-08_unified_storage_architecture_plan.md`
- `../../technical_architecture/VERSO_AS_PEER.md`
- `../../research/2026-03-04_standards_alignment_report.md`

---

## ClientStorageManager Plan

### Goal

Introduce a future `ClientStorageManager` as the runtime authority for
browser-origin site data, distinct from `GraphStore`, with:

- in-memory authoritative shed/shelf/bucket/bottle metadata
- pluggable physical storage backends
- endpoint registration for web-origin storage clients
- async deletion semantics compatible with storage proxy maps
- explicit placement in `AppServices` / Verso runtime, not in `GraphWorkspace`

### Scope Boundary

This plan does **not** replace `GraphStore` and does **not** redefine Graphshell
app durability as WHATWG storage hierarchy. `GraphStore` remains the landed
authority for Graphshell WAL/snapshot/archive/layout persistence.

This plan also assumes a **Servo-compatible-first** posture: Graphshell should
not build a rival browser-storage model, but may build host/runtime orchestration
above Servo's storage authority where backend interoperability requires it.

### Phase P0 — Runtime Seam and Type Floor

**Objective**: establish the runtime seam and compile-time vocabulary before any
backend or endpoint work.

Work:

1. Create the planned module namespace under `mods/native/verso/client_storage/`.
2. Define the type floor: `StorageKey`, `StorageScope`, `StorageShed`,
   `StorageShelf`, `StorageBucket`, `StorageBottle`, `EndpointDescriptor`,
   `PendingBucketDeletion`.
3. Define the three core traits:
   `ClientStorageManager`, `ClientStorageBackend`, `StorageEndpointClient`.
4. Add a runtime slot in `AppServices` for the future manager handle.
5. Add a thin `bridge.rs` seam showing that Servo/Verso requests storage through
   the manager instead of through direct backend access.
6. Add a placeholder `StorageInteropCoordinator` seam for Graphshell-level
  backend orchestration without making it a third storage authority.

Done gate:

- `ClientStorageManager` is placed on the Verso/AppServices runtime side.
- No graph/reducer-owned module becomes the owner of shelf/bucket metadata.
- Public type naming uses `storage key`, not `origin`, for the authority model.
- Interop orchestration is separated from storage truth ownership.

### Phase P1 — Metadata Loading and Persistence

**Objective**: make metadata authoritative in memory while preserving durable
reconstruction through a backend snapshot.

Work:

1. Define `ClientStorageSnapshot` as the persisted metadata envelope.
2. Implement `ClientStorageBackend::load_metadata()` and
   `persist_metadata()` in a reference backend.
3. Implement `ClientStorageManagerImpl::from_snapshot(...)`.
4. Add startup wiring so the manager is reconstructed before endpoint use.
5. Add diagnostics for metadata load success/failure and persist failure.

Done gate:

- Startup reconstructs shelves/buckets/bottles from persisted metadata.
- Runtime bottle resolution does not require synchronous database reads on the
  hot path.
- Metadata failure is surfaced distinctly from endpoint payload failure.

### Phase P2 — Bucket Lifecycle and Root Allocation

**Objective**: make bucket creation and durable root allocation explicit.

Work:

1. Implement `create_bucket(...)` with support for multiple buckets per storage
   key, not just an implicit `default` hard-code.
2. Implement backend `ensure_bucket_root(...)` for physical root allocation.
3. Introduce `BucketGeneration` so logical bucket names can survive async
   deletion/recreation.
4. Implement `set_bucket_persistence(...)` for `best-effort` vs `persistent`
   mode on local buckets.
5. Persist all bucket metadata updates immediately after authority changes.

Done gate:

- Multiple buckets are represented in metadata even if only `default` is wired
  to real endpoints initially.
- Bucket persistence mode is manager-owned metadata, not an endpoint-local flag.
- Physical bucket roots are allocated through the backend but mapped by the
  manager.

### Phase P3 — Endpoint Registration and Bottle Access

**Objective**: make storage clients consume a shared hierarchy instead of
inventing parallel registries.

Work:

1. Implement endpoint registration for a declared set of storage identifiers.
2. Implement `obtain_bottle(...)` and endpoint-facing bottle handles.
3. Add `StorageTypeSet` support for local vs session-capable endpoints.
4. Add a first endpoint integration path that proves bottle acquisition works
   through the manager.
5. Ensure endpoint clients receive stable logical handles, not mutable access to
   the manager's internal maps.

Done gate:

- At least one endpoint client resolves bottle access exclusively through the
  manager.
- Endpoint clients do not own bucket creation, quota policy, or site clearing.
- Session/local type constraints are enforced at the manager boundary.

### Phase P4 — Async Deletion and Site Data Clearing

**Objective**: make deletion semantics compatible with the Storage Standard's
proxy-map model.

Work:

1. Implement `delete_bucket_async(...)` and `clear_storage_key(...)`.
2. Add a deletion queue that stores `PendingBucketDeletion` entries.
3. Implement backend `schedule_bucket_root_deletion(...)`.
4. Ensure deletion marks metadata first, persists the logical state, then
   schedules physical deletion.
5. Allow a new bucket generation with the same logical name to be created while
   an older generation is still pending deletion.

Done gate:

- Logical deletion does not require waiting for all physical cleanup to finish.
- A new generation of the same bucket can be created safely after deletion is
  scheduled.
- Site-data clearing works at the manager authority level, not via ad hoc
  endpoint cleanup choreography.

### Phase P4A — Graphshell Reference Policy and Compound Actions

**Objective**: define how graph/node lifecycle interacts with site-data actions
without conflating the two ownership models.

Work:

1. Define `reference truth` vs `storage truth` policy explicitly.
2. Add explicit compound actions: delete node only, clear site data only,
  delete node and clear site data.
3. Ensure no default node-deletion path implicitly purges site data.
4. Add storage-context association metadata sufficient for UI prompts and
  explicit clear operations.

Done gate:

- Node deletion and site-data deletion are distinct by default.
- Any destructive cascade to site data is explicit and reviewable.
- UI and runtime can identify which storage context a node/backend binding is
  associated with.

### Phase P4B — Servo/Wry Interop Closure

**Objective**: make backend switching honest and policy-driven.

Work:

1. Implement `StorageInteropCoordinator` transition classes:
  shared logical context, cloned compatibility context, isolated fallback
  context.
2. Define which data classes are treated as shareable, clonable, or isolated by
  default.
3. Add user-facing commands for reload/switch/forget profile/clear current
  storage context.
4. Emit interop diagnostics when exact continuity cannot be preserved.

Done gate:

- Servo/Wry backend switching uses explicit transition policy.
- The runtime does not silently assume physical storage compatibility between
  Servo and Wry.
- Interop limitations are diagnosable and user-visible when relevant.

### Phase P5 — Session, Private Scope, and Health Closure

**Objective**: close the lifecycle semantics that differ from durable local
storage.

Work:

1. Implement traversable-owned session sheds.
2. Implement private-scope local sheds and wholesale teardown.
3. Add usage/quota estimation hooks and diagnostics.
4. Add integrity/health reporting for metadata load, bucket operations, session
   teardown, and private-scope destruction.
5. Document which failure classes belong to manager metadata vs endpoint-local
   payloads.

Done gate:

- Session storage lifecycle is separated from durable local storage.
- Private scope data is isolated from durable local sheds and torn down as a
  unit.
- Diagnostic coverage exists for the manager's own lifecycle and failure model.

---

## Findings

- `ClientStorageManager` belongs in `AppServices` / Verso runtime, alongside
  other browser-runtime services, not in `GraphWorkspace` and not in
  `GraphStore`.
- The runtime seam should mirror the existing Graphshell authority model:
  runtime-owned service, reducer-independent, with a thin integration bridge to
  Servo-facing code.
- The in-memory hierarchy must remain authoritative. The physical backend is a
  persistence/allocation service, not the conceptual owner of shelves/buckets.
- `BucketGeneration` is the simplest clean carrier for async deletion and bucket
  recreation without race-prone path reuse.
- The manager's metadata integrity and an endpoint's payload integrity are two
  distinct failure classes and must remain diagnosable as such.
- Graphshell still needs a host-side orchestration role for backend
  interoperability, especially Servo/Wry switching, but that role must remain a
  policy layer rather than a third storage authority.
- Node deletion is not a safe proxy for storage ownership; any site-data purge
  cascade must be an explicit compound action or reference-policy decision.

---

## Progress

- 2026-03-11: Added the architecture note separating `GraphStore` from future
  `ClientStorageManager` and sketched traits/types/runtime placement.
- 2026-03-11: Updated `SUBSYSTEM_STORAGE.md` to adopt the WHATWG Storage
  Standard for future browser-origin storage coordination only.
- 2026-03-11: Added a `Verso as Storage Runtime Host` section to the Verso
  architecture doc so runtime placement is visible in the browser-capability
  docs.
- 2026-03-11: This plan created to sequence the work into P0–P5 slices with
  explicit done gates.