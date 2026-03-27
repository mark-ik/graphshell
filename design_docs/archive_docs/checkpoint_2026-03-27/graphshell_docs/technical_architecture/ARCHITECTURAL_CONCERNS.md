# Architectural Concerns and Contradictions

**Last Updated**: 2026-03-08
**Purpose**: This document summarizes key architectural contradictions, gaps, and unresolved questions. Items marked **✅ Resolved** are kept for historical context only; they are not active concerns. See `PLANNING_REGISTER.md` for current open work.

> **Note**: All items below were identified as of 2026-02-17. Resolution status was updated 2026-03-08 based on the current doc/code state.

---

## 1. ✅ Resolved — Contradiction in the "Source of Truth"

There is a foundational ambiguity regarding the primary source of application state.

- **Conflict**: [ARCHITECTURAL_OVERVIEW.md](../../../../graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) and [GRAPHSHELL_AS_BROWSER.md](../../../../graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) previously diverged on source-of-truth language.
- **Impact**: This conflict was a source of synchronization bugs between graph representation and webview/tile runtime state. The current strategy resolves this via explicit authority domains.
- **Status (Feb 17)**: Largely reduced in runtime paths. Current architecture and implementation strategy converge on graph/intents as the control-plane source of truth, with webviews treated as effectful runtime state reconciled from graph/lifecycle intents.

---

## 2. ✅ Resolved — Gaps in the Delegate-Driven Navigation Plan

The plan to move to a delegate-driven model is a critical improvement, but reveals further challenges.

- **Identity Migration**: Implemented (UUID identity + URL multi-map model). The former URL-identity concern is no longer a blocker.
- **Bidirectional Flow**: Implemented for current scope. User actions in Graphshell UI route through intent/reconciliation paths and direct per-webview targeting where appropriate (legacy command variants removed except `ReloadAll`).
- **Status (Feb 17)**: Remaining delegate concern is empirical callback ordering nuances under navigation patterns; this is now traced and documented in the architecture plan.

---

## 3. ✅ Resolved — Physics Complexity (Historical Concern)

The former custom physics engine was identified as a source of issues.

- **Over-Engineering**: [ARCHITECTURAL_OVERVIEW.md](../../../../graphshell_docs/technical_architecture/ARCHITECTURAL_OVERVIEW.md) notes that the custom, multi-threaded physics worker is "unnecessary for browsing-scale graphs."
- **Known Bugs**: The same document identifies a bug in the force calculation ("doubling effective attraction").
- **Resolution**: Migration to `egui_graphs` force-directed layout is the adopted path and active runtime direction.
- **Status (Feb 17)**: Treat this as historical context plus tuning follow-up, not as a core architectural contradiction.

---

## 4. ✅ Resolved — Intent Boundary Completeness

The desired architecture for managing state is not yet fully implemented.

- **The Ideal**: [GRAPHSHELL_AS_BROWSER.md](../../../../graphshell_docs/technical_architecture/GRAPHSHELL_AS_BROWSER.md) describes a clean "intent-based" model where all state mutations are funneled through a single, predictable processing point.
- **The Historical Gap**: Earlier implementations mixed polling/direct wiring with partial intent flow, increasing fragility.
- **Status (Feb 17)**: Significantly addressed. Lifecycle helper-local apply paths were removed, legacy lifecycle path deleted, and frame boundary comments/tests updated. Residual direct runtime APIs are in effect/reconciliation layers by design.

---

## 5. ✅ Resolved — Duplicated State Management

The documentation explicitly identifies areas of duplicated state.

- **Selection State**: The old global-selection concern is fully addressed. Canonical selection ownership is now the active `GraphViewId` within the active Frame, as documented in [focus_and_region_navigation_spec.md](../../../../graphshell_docs/implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md) §4.7, and the W2 per-view migration is recorded as landed in [2026-03-03_pre_wgpu_plot.md](../../../../graphshell_docs/implementation_strategy/2026-03-03_pre_wgpu_plot.md).
- **Residual Concern (closed 2026-03-27)**: The `workspace.selected_nodes` compatibility mirror and `workspace.selected_nodes_by_view` canonical owner described in the 2026-03-08 note no longer exist in the codebase. Neither field is present in any `.rs` file. Selection state is owned by `GraphViewState` and accessed through the view query layer. No cleanup action remains.

---

## 6. ✅ Resolved — Lacking Unit Test Coverage for Critical Components

The architecture of the UI and webview integration components makes them difficult to test in isolation, which is a potential quality risk.

- **Gap**: The `codebase_guide.md` notes that `desktop/gui.rs` and `desktop/webview_controller.rs` have no dedicated unit tests and are only covered by integration tests.
- **Impact**: These modules contain the most complex and critical logic for integrating with Servo. A lack of unit tests makes refactoring risky and can lead to regressions. An architecture that is difficult to unit-test often indicates tight coupling between components.
- **Status (Feb 17)**: Stale as written. Both modules now have focused unit coverage (intent conversion/order tests, lifecycle reconciliation/backpressure classifier tests, controller reconciliation tests). Remaining risk is complexity/coverage breadth, not complete absence of unit tests.

---

## 7. ✅ Resolved — Underspecified Crash Handling Strategy

The architectural documents do not specify how the application should behave when a sandboxed web content process crashes.

- **Gap**: A robust browser architecture must be resilient to crashes in content processes. It is unclear from the documents whether such a crash would be gracefully handled (e.g., by displaying a "crashed tab" message) or if it would risk taking down the entire Graphshell application.
- **Impact**: Without a clear strategy, the application's stability is at risk from misbehaving web content.
- **Status (Feb 17)**: Implemented for desktop graphshell paths (semantic event -> reducer demote/unmap -> tile crash banner/actions), as reflected in [implementation_strategy/2026-02-21_lifecycle_intent_model.md](../../../../graphshell_docs/implementation_strategy/2026-02-21_lifecycle_intent_model.md) and the [archived embedder decomposition plan](../../../checkpoint_2026-03-22/graphshell_docs/implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md). Remaining limitations are upstream API surface concerns (for example web-content accessibility), not missing crash policy wiring in graphshell desktop.

---

## 8. ✅ Resolved — Monolithic UI Component

The original concern (monolithic `desktop/gui.rs`, monolithic `render/mod.rs`, monolithic `graph_app.rs`) has been fully addressed through successive decomposition passes.

- **`gui.rs`** → **681 lines**. `toolbar_ui.rs` (408 lines, 8 submodules), `gui_frame.rs` (426 lines, 9 submodules), `gui_orchestration.rs` (1,834 lines, primary orchestration façade — acceptable as a coordinator).
- **`graph_app.rs`** → **2,108 lines** (down from 11,269). No longer a hotspot.
- **`render/mod.rs`** → **1,342 lines of functional code + 1,824 lines of tests = 3,166 total**. Functional portion is well within guideline; the plan's exit target of "under ~4k lines" is met. Coherent extractions (`render/panels.rs`, `render/command_palette.rs`, `render/canvas_overlays.rs`, `render/graph_info.rs`, `render/semantic_tags.rs`, `render/canvas_input.rs`) now own their domains; `render/mod.rs` is the graph canvas orchestration owner it was intended to be.

**Remaining large files that are not concerns** (coherent, single-domain, not mixed-responsibility):

- `render/panels.rs`: 3,524 lines — owns all panel/tool-pane UI by design.
- `render/command_palette.rs`: 2,025 lines — owns command palette assembly by design.
- `runtime/diagnostics.rs`: 3,466 lines — diagnostics-focused; lower priority.
- `runtime/registries/mod.rs`: 2,514 lines — registry hub; manageable.

---

## 9. External Pattern Note: Freenet Contract/Delegate Separation

- **Observation**: Freenet's explicit split between shared/public execution and private/identity execution reinforces Graphshell's need for hard authority boundaries between graph/lifecycle runtime state and identity/secret handling.
- **Graphshell Relevance**: Keep reducer/reconciliation authority boundaries central; avoid new direct hint shortcuts that bypass adapters in compositor/render flows.
- **Process Discipline**: Tie normative architecture claims to executable proofs (tests/harness/snapshots) to avoid spec/implementation drift during rapid migration.
- **Reference**: [../../../../graphshell_docs/research/2026-02-27_freenet_takeaways_for_graphshell.md](../../../../graphshell_docs/research/2026-02-27_freenet_takeaways_for_graphshell.md)

---

## Open Concerns — Identified 2026-03-08

The following concerns were identified during a cross-doc audit and are not yet resolved. They are tracked here until a plan or spec closes them.

### O1. ✅ Resolved 2026-03-27 — `Address` typed enum fully migrated; `url: String` field retired from `Node`; `PersistedAddress` introduced

The typed `Address` enum (`Http(String)`, `File(String)`, `Data(String)`, `Clip(String)`, `Directory(String)`, `Custom(String)`) is now the sole source of address state on `Node`. All migration stages are complete:

- **Stages A–C** (2026-03-26): `Address` introduced, `address_kind: AddressKind` field removed from `Node`, `UpdateNodeAddressKind` WAL entry and `GraphIntent::UpdateNodeAddressKind` retired.
- **Stage D** (2026-03-27): All ~35 external call sites migrated from `.url` field access to `.url()` method.
- **Stage E** (2026-03-27): `pub url: String` removed from `Node`; `Node::url()` now delegates to `self.address.as_url_str()`.
- **Stage C.2** (2026-03-27): `PersistedAddress` enum added to `services/persistence/types.rs`; `PersistedNode` now carries `address: PersistedAddress` as the canonical field (with legacy `url: String` written alongside for backward compat). Old snapshots lacking the `address` field deserialize via `#[serde(default)]` fallback to the `url` field.

`Address::Clip` stores the full URL (`verso://clip/<id>`) rather than just the id, so `as_url_str()` is a uniform round-trip identity across all variants. Variants use `String` payloads (not `url::Url`/`PathBuf`) for rkyv/WASM compatibility.

### O2. ✅ Resolved 2026-03-27 — Clip address convention established; `verso://clip/<id>` is canonical

The concern as written was about missing canonicalization and no dated plan. Both are now moot:

- **Canonical scheme**: `verso://clip/<clip_id>` — emitted by `VersoAddress::Clip` via `Display` (`util.rs`).
- **Legacy alias**: `graphshell://clip/<clip_id>` — accepted by both `VersoAddress::parse` and `address_from_url` for backward compat; neither is written in new code.
- **Parsed form**: `VersoAddress::Clip(String)` (`util.rs`) at the shell routing layer; `Address::Clip(String)` (`model/graph/mod.rs`) at the graph model layer — both store the full URL.
- **Routing**: `WorkbenchIntent::OpenClipUrl`, `GraphBrowserApp::resolve_clip_route`, and `AddressKind::GraphshellClip` viewer-registry routing are all wired.
- **Capture**: `app/clip_capture.rs` creates clip nodes with `verso://clip/<id>` URLs.

The open sub-item from the UCM spec (`ClipViewer` rendering pipeline) is a viewer implementation concern, not an address-family canonicalization concern. Track it under the viewer/renderer roadmap if needed.

### O3. ✅ Resolved 2026-03-27 — `RendererKind` retired; `viewer_override: Option<ViewerId>` is canonical

`RendererKind` was a spec-only concept in the core extraction plan that was never implemented in code. The UCM spec (`universal_content_model_spec.md §4.1`) already defined the same concept more precisely as `viewer_override: Option<ViewerId>` — a user-set node field that forces a specific viewer, stored in graph data, taking precedence over address/MIME-based selection.

Resolution: the core extraction plan ([2026-03-08_graphshell_core_extraction_plan.md](../../../../graphshell_docs/technical_architecture/2026-03-08_graphshell_core_extraction_plan.md)) was updated to replace all three `RendererKind` references with `viewer_override: Option<ViewerId>` and a pointer to UCM §4.1. No code changes required — neither field had landed in any `.rs` file. When viewer selection is implemented, follow the UCM spec.

### O4. ✅ Resolved — `SimpleDocument` / `EngineTarget` spec written

The adaptation pipeline types (`SimpleDocument`, `EngineTarget`, `RenderPolicy`) now have a dedicated canonical spec:
[2026-03-08_simple_document_engine_target_spec.md](../../../../graphshell_docs/implementation_strategy/viewer/2026-03-08_simple_document_engine_target_spec.md)

This spec covers: block type definitions, producer mapping (Gemini, Reader Mode, Markdown), target selection policy, `RenderPolicy` defaults, CSP generation, `NativeReader` rendering, pipeline structure, and downstream feature dependency table. The text-editor short-circuit exemption is documented.

### O5. ✅ Resolved — `FilePermissionGuard` behavior fully specified

`universal_content_model_spec.md §8.1` now contains the complete specification: home-directory definition across platforms (Linux/macOS `HOME`, Windows `USERPROFILE`/`FOLDERID_Profile`), `FileAccessPolicy` struct in `AppPreferences`, prompt UX (modal, per-directory, `RequestFilePermission` semantic event), denial propagation (`FallbackViewer` + `viewer.permission.denied` diagnostic channel), and the hard prerequisite relationship to filesystem ingest.

### O6. ✅ Resolved — fjall storage rationale documented

`SUBSYSTEM_STORAGE.md §4.2` now contains a "Why fjall for the WAL" rationale covering: append-only log semantics, crash-safe LSM failure guarantees, pure Rust (no C FFI), keyspace versioning for schema migration, and the WASM-clean boundary (fjall stays host-side, WAL entry types are WASM-clean structs).