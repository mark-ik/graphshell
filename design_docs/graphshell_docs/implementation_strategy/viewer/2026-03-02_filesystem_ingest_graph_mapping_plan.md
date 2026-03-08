# Filesystem Ingest Graph Mapping Plan

**Date**: 2026-03-02  
**Status**: Planned (feature-gated)  
**Priority**: Pre-networking feature expansion (not a pre-renderer/WGPU blocker)

**Related**:

- `../2026-03-01_complete_feature_inventory.md`
- `../../technical_architecture/GRAPHSHELL_AS_BROWSER.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../../research/2026-02-27_viewer_state_matrix.md`
- `../subsystem_history/2026-02-11_bookmarks_history_import_plan.md`
- `../../TERMINOLOGY.md`

---

## Filesystem Ingest Plan

### Goal

Define a canonical feature that ingests a local filesystem selection into Graphshell where:

- files are represented as graph nodes,
- folders are represented as frame contexts,
- folder membership is represented as folder-tag links,
- ingest is blocked behind baseline document-viewer readiness.

### Feature Gate (Hard Block)

Filesystem ingest is blocked until common file/document viewer coverage is operational in active runtime paths.

Required viewer readiness baseline:

1. `viewer:plaintext` and `viewer:markdown` are fully active.
2. `viewer:pdf` and `viewer:csv` are active in node-pane paths (not selection-only placeholders).
3. A baseline non-web binary fallback path exists (`viewer:metadata` or equivalent explicit fallback behavior).
4. File access safety contract is enforced (`FilePermissionGuard` and address-kind mapping remain
   authoritative). `FilePermissionGuard` is defined in UCM Step 9
   (`2026-02-24_universal_content_model_plan.md`); that step must reach its done gate before
   filesystem ingest Phase 1 can close.

**Browse-in-place vs ingest**: The `DirectoryViewer` (UCM Step 6) provides in-tile local
navigation (`GraphIntent::NavigateNode`) — this is browse-in-place, not ingest. Dragging a file
from `DirectoryViewer` to the graph canvas creates a new node (`GraphIntent::CreateNode`). Bulk
directory import is this plan's responsibility, not the `DirectoryViewer`'s.

If these gates are not met, ingest actions must remain unavailable and explicitly report blocked-state diagnostics.

### Canonical Mapping Model

#### Node mapping

- Each imported file path maps to one node (`address_kind = File`, canonical `file://` URL).
- Node title defaults to basename; MIME hint is inferred by current detection policy.
- Re-import is idempotent on canonicalized file URL.

#### Frame mapping

- Each imported folder maps to one Frame context in the target Workbench.
- The frame label defaults to relative folder path from ingest root.
- Frame creation must route through workbench authority (no direct tile-tree mutation outside existing routing contracts).

#### Folder-tag link semantics

- Folder membership is represented by a folder-tag link contract:
  - node tags include normalized folder path tags (example: `folder:docs/specs`),
  - optional `UserGrouped` edge links may be created between sibling nodes for local navigation adjacency.
- This plan does not introduce a new `EdgeKind`; folder semantics are metadata/tag-first in this phase.

### Import Scope Rules

- Ingest root is user-selected directory.
- Depth-limited recursion is required (default depth gate to avoid graph explosion).
- Ignore/include filters are required (extensions, hidden files, max file count).
- Symlink handling must be explicit (skip by default for safety in MVP).

### UX Surface

- Entry points (planned):
  1. Command Palette action: `import.filesystem`.
  2. Settings route under persistence/import surfaces.
- Progress feedback: explicit counts (folders scanned, files imported, skipped items).
- Cancelability: ingest must be cancelable without partial corruption.

### Diagnostics and Safety

Required diagnostics channels (exact IDs can be finalized in implementation slice):

- ingest started/completed/failed,
- blocked-by-viewer-gate,
- skipped-by-filter,
- skipped-by-permission,
- limit-reached truncation.

Safety rules:

- Read-only ingest (no filesystem mutation).
- Normalize and validate paths before node creation.
- Enforce file permission boundaries through existing guard contracts.

### Phase Breakdown

#### Phase 1 — Viewer readiness closure

- Close active-path readiness for common document viewers.
- Add explicit readiness probe used by ingest action availability.

#### Phase 2 — Ingest core (files -> nodes)

- Directory walker + filters + idempotent node creation.
- File URL normalization + MIME hint pass-through.

#### Phase 3 — Folder topology (folders -> frames)

- Frame creation/routing for folder contexts.
- Deterministic frame naming and dedup behavior.

#### Phase 4 — Folder-tag links + UX + diagnostics

- Folder tag generation and attachment.
- Optional sibling adjacency edges under user-configurable mode.
- Progress, cancel, summary, and diagnostics wiring.

### Acceptance Criteria

1. Ingest action is unavailable with explicit blocked reason when viewer readiness gate is not met.
2. Importing a directory creates file nodes with `AddressKind::File` and canonicalized `file://` URLs.
3. Importing nested folders creates corresponding frame contexts with stable labels.
4. Folder tags are attached deterministically to imported nodes.
5. Re-importing same directory is idempotent (no duplicate nodes for same canonical file URL).
6. Depth/limit/filter controls are applied and reflected in diagnostics summaries.
7. Canceling ingest preserves graph consistency and leaves no partial invalid state.

---

## Findings

- Current runtime supports file-address node representation and MIME inference, but not bulk directory-to-graph ingest.
- Viewer registry and state-matrix docs indicate declared viewer targets that are not all active in node-pane embed paths.
- Workbench frame semantics are mature enough to host folder-to-frame mapping without introducing new structural terms.
- Existing bookmarks/history import planning provides a compatible pattern (action-driven import + intent-based mutation).

---

## Progress

- 2026-03-02: Created canonical feature plan with hard viewer-readiness gate and explicit files->nodes / folders->frames mapping.
- 2026-03-02: Defined folder-tag link approach as metadata/tag-first for MVP (no new edge kind in this slice).
- 2026-03-02: Added phased rollout and acceptance criteria for issue seeding.
