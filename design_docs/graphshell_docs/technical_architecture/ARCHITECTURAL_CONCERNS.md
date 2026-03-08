# Architectural Concerns and Contradictions

**Last Updated**: 2026-03-08
**Purpose**: This document summarizes key architectural contradictions, gaps, and unresolved questions. Items marked **✅ Resolved** are kept for historical context only; they are not active concerns. See `PLANNING_REGISTER.md` for current open work.

> **Note**: All items below were identified as of 2026-02-17. Resolution status was updated 2026-03-08 based on the current doc/code state.

---

## 1. ✅ Resolved — Contradiction in the "Source of Truth"

There is a foundational ambiguity regarding the primary source of application state.

- **Conflict**: [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md) and [GRAPHSHELL_AS_BROWSER.md](GRAPHSHELL_AS_BROWSER.md) previously diverged on source-of-truth language.
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

- **Over-Engineering**: [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md) notes that the custom, multi-threaded physics worker is "unnecessary for browsing-scale graphs."
- **Known Bugs**: The same document identifies a bug in the force calculation ("doubling effective attraction").
- **Resolution**: Migration to `egui_graphs` force-directed layout is the adopted path and active runtime direction.
- **Status (Feb 17)**: Treat this as historical context plus tuning follow-up, not as a core architectural contradiction.

---

## 4. ✅ Resolved — Intent Boundary Completeness

The desired architecture for managing state is not yet fully implemented.

- **The Ideal**: [GRAPHSHELL_AS_BROWSER.md](GRAPHSHELL_AS_BROWSER.md) describes a clean "intent-based" model where all state mutations are funneled through a single, predictable processing point.
- **The Historical Gap**: Earlier implementations mixed polling/direct wiring with partial intent flow, increasing fragility.
- **Status (Feb 17)**: Significantly addressed. Lifecycle helper-local apply paths were removed, legacy lifecycle path deleted, and frame boundary comments/tests updated. Residual direct runtime APIs are in effect/reconciliation layers by design.

---

## 5. ⚠️ Partially Resolved — Duplicated State Management

The documentation explicitly identifies areas of duplicated state.

- **Selection State**: [implementation_strategy/2026-02-14_selection_semantics_plan.md](../../archive_docs/checkpoint_2026-02-19/2026-02-14_selection_semantics_plan.md) was created to address the problem of duplicated selection state between different components. This is a known weakness in the current component wiring that can lead to UI inconsistencies and bugs.
- **Status (Feb 17)**: Keep as active concern only to the extent unresolved items remain in the selection semantics plan.

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
- **Status (Feb 17)**: Implemented for desktop graphshell paths (semantic event -> reducer demote/unmap -> tile crash banner/actions), as reflected in [implementation_strategy/2026-02-21_lifecycle_intent_model.md](../implementation_strategy/2026-02-21_lifecycle_intent_model.md) and [implementation_strategy/2026-02-20_embedder_decomposition_plan.md](../implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md). Remaining limitations are upstream API surface concerns (for example web-content accessibility), not missing crash policy wiring in graphshell desktop.

---

## 8. ⚠️ In Progress — Monolithic UI Component

The primary UI components have been reduced through recent decomposition but some remain large.

- **Original Concern**: `desktop/gui.rs` was ~1,741 lines (as of Feb 21), well above the ~800-1000 guideline.
- **Decomposition completed (as of 2026-03-08)**: `shell/desktop/ui/gui.rs` is now **681 lines**. `toolbar_ui.rs` is **408 lines** (coordinator) with 8 focused submodules under `shell/desktop/ui/toolbar/`. `gui_frame.rs` is **426 lines** with 9 submodules under `shell/desktop/ui/gui_frame/`. `gui_orchestration.rs` is **1,834 lines** (still large; primary orchestration façade).
- **Remaining — significant (as of 2026-03-08)**:
  - `graph_app.rs`: **14,714 lines** — the dominant hotspot; substantially larger than the ~6k noted previously and still growing. No dedicated decomposition plan exists.
  - `render/mod.rs`: **5,617 lines** — also larger than the ~3.4k noted previously. No dedicated decomposition plan exists.
  - `runtime/diagnostics.rs`: **3,466 lines** — large but diagnostics-focused; lower priority.
  - `runtime/registries/mod.rs`: **2,514 lines** — registry hub; manageable but worth watching.
- **Impact**: `gui`/`toolbar`/`gui_frame` decomposition is complete and successful. `graph_app.rs` and `render/mod.rs` are now the dominant hotspots and the primary candidates for the next decomposition pass. Neither is covered by an active plan.

---

## 9. External Pattern Note: Freenet Contract/Delegate Separation

- **Observation**: Freenet's explicit split between shared/public execution and private/identity execution reinforces Graphshell's need for hard authority boundaries between graph/lifecycle runtime state and identity/secret handling.
- **Graphshell Relevance**: Keep reducer/reconciliation authority boundaries central; avoid new direct hint shortcuts that bypass adapters in compositor/render flows.
- **Process Discipline**: Tie normative architecture claims to executable proofs (tests/harness/snapshots) to avoid spec/implementation drift during rapid migration.
- **Reference**: [../research/2026-02-27_freenet_takeaways_for_graphshell.md](../research/2026-02-27_freenet_takeaways_for_graphshell.md)

---

## Open Concerns — Identified 2026-03-08

The following concerns were identified during a cross-doc audit and are not yet resolved. They are tracked here until a plan or spec closes them.

### O1. 🔴 `AddressKind` migration to typed `Address` enum not yet scheduled

The `graphshell-core` extraction plan introduces a typed `Address` enum (`Http(Url)`, `File(PathBuf)`, `Onion`, `Ipfs(Cid)`, `Gemini`, `Custom`) as the long-term cross-platform address type. The current `AddressKind` six-variant hint enum is the runtime stopgap. No plan currently schedules the migration. This needs a plan when IPFS/Gemini/Tor resolvers become implementation priorities.

### O2. 🔴 `GraphshellClip` address family has no plan

`AddressKind::GraphshellClip` and `ClipViewer` are declared in the UCM spec as always-on, but there is no dated implementation plan covering the clip-address canonicalization (`graphshell://clip/<uuid>` namespace is described as "pending resolution" in the spec). The clipping plan (`2026-02-11_clipping_dom_extraction_plan.md`) covers DOM extraction; the address family resolution is not yet tracked.

### O3. 🟡 `RendererKind` hint enum (core extraction plan §2.2) not reconciled with `viewer_override`

The core extraction plan introduces `RendererKind` as a core-layer hint for which renderer a node prefers. The UCM spec uses `viewer_override: Option<ViewerId>`. These may be the same concept at different layers, or `RendererKind` may be a new distinct field. The relationship is unresolved.

### O4. ✅ Resolved — `SimpleDocument` / `EngineTarget` spec written

The adaptation pipeline types (`SimpleDocument`, `EngineTarget`, `RenderPolicy`) now have a dedicated canonical spec:
`../implementation_strategy/viewer/2026-03-08_simple_document_engine_target_spec.md`

This spec covers: block type definitions, producer mapping (Gemini, Reader Mode, Markdown), target selection policy, `RenderPolicy` defaults, CSP generation, `NativeReader` rendering, pipeline structure, and downstream feature dependency table. The text-editor short-circuit exemption is documented.

### O5. ✅ Resolved — `FilePermissionGuard` behavior fully specified

`universal_content_model_spec.md §8.1` now contains the complete specification: home-directory definition across platforms (Linux/macOS `HOME`, Windows `USERPROFILE`/`FOLDERID_Profile`), `FileAccessPolicy` struct in `AppPreferences`, prompt UX (modal, per-directory, `RequestFilePermission` semantic event), denial propagation (`FallbackViewer` + `viewer.permission.denied` diagnostic channel), and the hard prerequisite relationship to filesystem ingest.

### O6. ✅ Resolved — fjall storage rationale documented

`SUBSYSTEM_STORAGE.md §4.2` now contains a "Why fjall for the WAL" rationale covering: append-only log semantics, crash-safe LSM failure guarantees, pure Rust (no C FFI), keyspace versioning for schema migration, and the WASM-clean boundary (fjall stays host-side, WAL entry types are WASM-clean structs).
