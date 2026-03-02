# Unified Source Directory Mapping Plan

**Date**: 2026-03-02  
**Status**: Planned (feature-gated)  
**Priority**: Post-filesystem-ingest expansion (not a pre-renderer/WGPU blocker)

**Related**:
- `2026-03-02_filesystem_ingest_graph_mapping_plan.md`
- `../2026-03-01_complete_feature_inventory.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../../TERMINOLOGY.md`

---

## Unified Mapping Plan

### Goal

Define a canonical feature where Graphshell can automatically map source hierarchies into graph/workbench structure across:

- local filesystems,
- network file roots/shares,
- website domain/path structures.

The mapping model keeps one consistent semantic contract:

- content leaves map to graph nodes,
- directory/domain scopes map to frame contexts,
- scope membership maps to deterministic scope tags.

### Dependency Gate (Hard Block)

This feature is blocked until **filesystem ingest** reaches ready state per `2026-03-02_filesystem_ingest_graph_mapping_plan.md` acceptance criteria.

Minimum prerequisite closure:

1. Filesystem ingest action gating + diagnostics are active.
2. Files -> nodes mapping is idempotent via canonical address normalization.
3. Folders -> frames mapping is stable under re-import.
4. Folder-tag attachment is deterministic.

If any prerequisite is not met, unified source mapping remains unavailable and reports blocked-state diagnostics.

### Canonical Source Model

#### Source roots

A source root is one of:

- Local root (`file://` path root),
- Network root (share or mounted network path normalized to canonical URI),
- Web root (`scheme://host[:port]` domain root).

#### Scope units

- Local/network directories are treated as directory scopes.
- Web paths are treated as domain-directory scopes (`/a/b` under host root).
- Scope nodes are not introduced in MVP; scope is represented by frame context + tags.

#### Leaf units

- Local/network files are leaf content units.
- Web pages/documents are leaf content units.
- Each leaf maps to one graph node keyed by canonical address.

### Mapping Semantics

#### Node mapping

- Each discovered leaf maps to one node with canonical address + detected `AddressKind`.
- Re-map is idempotent by canonical address key.
- Title defaults to basename/title segment; existing node naming overrides remain authoritative.

#### Frame mapping

- Each first-class scope (directory or domain subtree root) maps to one Frame in the target Workbench.
- Frame naming:
  - local/network: relative path from selected root,
  - web: `<host>` or `<host>/<path-prefix>`.
- Frame creation and routing always flow through workbench authority.

#### Scope-tag semantics

- Imported leaves receive deterministic scope tags:
  - local/network: `scope:dir:<normalized-relative-path>`
  - web: `scope:web:<host>/<normalized-path-prefix>`
- This phase does not require a new `EdgeKind`; metadata/tag semantics remain primary.

### Discovery / Crawl Rules

#### Local + network

- Depth limits are mandatory.
- Include/exclude filters are mandatory.
- Symlink or link-jump behavior is explicit and conservative by default.
- Permission denials are non-fatal and diagnostic-visible.

#### Web domain mapping

- Domain mapping starts from an explicit seed URL or host root.
- Crawl policy is constrained by:
  - same-origin default,
  - depth/page-count limits,
  - content-type allowlist,
  - robots/meta policy posture (implementation-defined policy doc required before enablement).
- Off-domain links are recorded as references but not expanded in default mode.

### UX Surface

Planned entry points:

1. Command Palette actions:
   - `import.filesystem` (prerequisite path),
   - `import.source_map` (unified local/network/web map).
2. Import settings surface for crawl depth, filters, and caps.

Required UX behavior:

- preflight summary (estimated scope and active limits),
- progress counters by source type,
- cancelability without graph corruption,
- completion summary (mapped, skipped, blocked, capped).

### Diagnostics and Safety

Required diagnostic classes:

- unified mapping started/completed/failed,
- blocked-by-prerequisite-gate,
- skipped-by-policy/filter/permission,
- traversal cap reached,
- canonicalization conflict resolved.

Safety rules:

- Read-only mapping for local/network sources.
- URL/path normalization before node/frame operations.
- Explicit trust boundary handling between local/network/web source kinds.

### Phase Breakdown

#### Phase 1 — Prerequisite closure

- Complete filesystem-ingest plan acceptance criteria.
- Introduce prerequisite readiness probe consumed by `import.source_map` availability.

#### Phase 2 — Shared mapping core

- Introduce source-agnostic traversal/mapping pipeline with per-source adapters.
- Keep canonical keying + idempotency rules shared.

#### Phase 3 — Local/network adapter

- Extend filesystem ingest walker to mounted/network roots using same safety/limit controls.
- Emit unified summaries and diagnostics.

#### Phase 4 — Web domain adapter

- Add constrained domain/path traversal under explicit crawl policy.
- Map host/path scopes into frames/tags using same core semantics.

#### Phase 5 — UX and policy hardening

- Finalize preflight UI, cancellation UX, and policy diagnostics.
- Add acceptance coverage for mixed-source mapping sessions.

### Acceptance Criteria

1. Unified source mapping action is unavailable with explicit blocked reason until filesystem-ingest prerequisites are met.
2. Local/network/web leaves map to nodes idempotently by canonical address.
3. Directory/domain scopes map to stable frame contexts with deterministic labels.
4. Scope tags are applied deterministically and remain stable across re-map runs.
5. Source-specific limits/filters are enforced and reflected in diagnostics summaries.
6. Mixed-source sessions preserve graph/workbench consistency on cancel/failure.
7. Default web mapping behavior remains same-origin and bounded by policy caps.

---

## Findings

- Graphshell already has the semantic building blocks (`AddressKind`, node identity, frame semantics) needed for a unified mapping model.
- The filesystem-ingest plan is the correct prerequisite because it validates idempotent local traversal, frame routing, and ingest diagnostics.
- A shared mapping core with source adapters minimizes drift between local/network/web behavior.

---

## Progress

- 2026-03-02: Created canonical expansion plan for unified local/network/web directory-domain mapping, explicitly gated behind filesystem-ingest readiness.
