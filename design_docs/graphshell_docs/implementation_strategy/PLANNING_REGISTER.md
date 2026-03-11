# Planning Register

**Status**: Active / Canonical (revised 2026-02-26)
**Purpose**: Single source for execution priorities, issue-ready backlog stubs, and implementation guidance.

## Canonical Companion Docs

- [subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md](subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md) - Consolidated UX execution control-plane (baseline done-gate + milestone checklist + issue-domain mapping).
- [2026-02-28_ux_contract_register.md](2026-02-28_ux_contract_register.md) - Cross-spec UX ownership map and contract register.
- [2026-03-03_pre_wgpu_feature_validation_gate_checklist.md](2026-03-03_pre_wgpu_feature_validation_gate_checklist.md) - Feature/validation-gate-only closure checklist for pre-wgpu readiness.
- [2026-03-03_spec_conflict_resolution_register.md](2026-03-03_spec_conflict_resolution_register.md) - Priority-ordered spec conflict and terminology resolution register for pre-wgpu closure.

## Contents

0. Surface Composition Architecture (2026-02-26 Gap Analysis & Remediation)
1. Immediate Priorities Register (10/10/10)
2. Latest Checkpoint Delta (Code + Doc Audit)
3. Merge-Safe Lane Execution Reference (Canonical)
4. Register Size Guardrails + Archive Receipts
5. Top 10 Active Execution Lanes (Strategic / Completion-Oriented)
6. Prospective Lane Catalog (Comprehensive)
7. Forgotten Concepts for Adoption (Vision / Research)
8. Quickest Improvements (Low-Effort / High-Leverage)
9. Historical Tail (Archived)
10. Suggested Tracker Labels
11. Import Notes

### Contents Notes

- `§0` is the 2026-02-26 compositor/render-pipeline gap analysis, architectural extension, and issue plan. It defines the **Surface Composition Contract** — a first-class render-pipeline architecture that replaces servoshell-inherited compositor assumptions.
- `§1A` is the canonical sequencing control-plane section.
- `§1C` is the current prioritized lane board.
- `§1D` is the comprehensive lane catalog (including prospective and incubation lanes).
- Historical tail payload has been archived to dated receipts under `design_docs/archive_docs/checkpoint_2026-02-27/` to keep this file focused on active execution control-plane content.

### Cross-Plan Dependency Guardrail (2026-03-10)

When a lane step depends on another subsystem's carrier model (for example,
an action payload contract depending on history/persistence edge carriers), the
dependent step must include all of the following before it can be marked done:

1. Explicit link to the prerequisite strategy/spec doc in the step text.
2. A blocking note stating completion is gated by the prerequisite carrier path.
3. A done-gate item that names the full end-to-end carrier path (intent,
mutation, delta, and persistence where applicable).

If any of these are missing, the dependent step is treated as partial and must
not be marked complete in lane status updates.

### Structural Groundwork Guardrail (2026-03-10)

When a lane claims a generic action or authority boundary over an entity type
(`PaneId`, `RendererId`, etc.), the underlying structure must exist on every
entity variant the action is supposed to target before the step can be marked
done.

Examples:

1. `workbench:split_*` / `workbench:close_pane` are not honestly unblocked
   until graph, node, and tool panes all carry stable `PaneId`.
2. A registry table must not invent a conceptual surface (`CommandPalette` as a
   tool pane, for example) if the implemented authority model uses a different
   carrier (`WorkbenchIntent::OpenCommandPalette`).

If implementation reveals that the planned abstraction is missing prerequisite
structure, the plan must be updated immediately with that prerequisite instead
of leaving the lane to "complete" against a patchwork model.

### Workflow Activation Realism Guardrail (2026-03-10)

When a workflow/session-mode plan claims atomic multi-profile activation, the
plan must distinguish between:

1. Runtime-owned active authorities that can actually be switched transactionally.
2. Persisted-default carriers that only emulate activation by writing future
   defaults/settings.

If some workflow constituents still use persisted defaults instead of stateful
runtime authorities, the lane may still proceed, but the plan must:

1. Name the activation as adapter-based or partial rather than fully transactional.
2. Link the missing runtime authority lane explicitly (for example, Sector D for
   canvas/physics active-profile ownership).
3. Avoid claiming rollback/WAL semantics that the current carrier model cannot enforce.

### Post-Completion Stabilization Policy (2026-03-10)

Lane ordering is now completion-first, then stabilization:

1. Complete core implementation lanes to a coherent milestone body (target: near
   `v0.0.2` system completeness).
2. Run an explicit inter-plan audit checkpoint (coverage, contract parity,
   blocker drift, and tracker/doc sync).
3. Execute stabilization as a bounded hardening pass over the now-complete body,
   instead of as a continuous first lane.

Exception rule:
- Critical break/failure regressions that block normal use can still preempt for
  a short hotfix slice, but the default execution posture is completion-first.

---

## 0. Surface Composition Architecture (2026-02-26 Gap Analysis & Remediation)

### 0.1 Problem Statement

Graphshell's tile compositor (`shell/desktop/workbench/tile_compositor.rs`) inherits a servoshell-era pattern that treats web content composition as "another egui layer" rather than a first-class render pipeline with explicit pass ownership and backend contracts. This produces a class of bugs where:

- Focus/hover/selection affordances paint at the wrong z-order relative to composited web content (the "focus ring hidden under document view" symptom in the Stabilization Bug Register).
- GL state leaks across compositor callback boundaries when `render_to_parent` callbacks execute without save/restore contracts.
- egui `Order::Middle` layer IDs for both webview content and focus rings are the mechanism for z-order control, but egui's layer ordering is intended for UI widget stacking, not for managing a heterogeneous render pipeline mixing GL-composited textures, OS-native overlays, and egui-native draw calls.
- The Wry integration strategy (`2026-02-23_wry_integration_strategy.md`) documents the texture-vs-overlay distinction but does not define a canonical overlay affordance policy that survives when both backends coexist at runtime.

This is not a bug in any single file — it is an **architectural gap** between the servoshell-inherited single-backend assumption and Graphshell's multi-backend, multi-surface reality.

### 0.2 Architectural Diagnosis (Mapped to Canonical Terms)

The gap analysis maps to four missing architectural contracts, expressed in canonical Graphshell terminology:

| Missing Contract | Architectural Location | Canonical Owner | Current State |
| --- | --- | --- | --- |
| **Surface Composition Contract** (node viewer pane render pipeline) | `tile_compositor.rs`, `tile_render_pass.rs` | `WorkbenchSurfaceRegistry` + `ViewerSurfaceRegistry` | Implicit; egui layer ordering is the only mechanism |
| **Compositor Adapter** (GL callback isolation wrapper) | `tile_compositor::composite_active_node_pane_webviews()` | Verso mod / `EmbedderCore` boundary | Raw `PaintCallback` + `render_to_parent` with no explicit GL-state contract |
| **Render Mode Enum** (backend-authoritative tile rendering classification) | `TileKind::Node(NodePaneState)` | `ViewerRegistry` + tile runtime | Inferred from side effects; `render_path_hint` diagnostics exist but are not authoritative |
| **Overlay Affordance Policy** (focus/hover/selection ring rendering rules per backend mode) | `tile_compositor.rs` (focus/hover rings), `tile_render_pass.rs` (diagnostics overlay) | `PresentationDomainRegistry` + per-backend `Viewer` trait | Single hard-coded egui `LayerId` path regardless of backend |

### 0.3 Architectural Solution: Surface Composition Contract

**Core principle**: Stop treating web content composition as "just another egui layer." Make it a first-class render pipeline with explicit pass ownership and backend contracts.

The solution introduces four architectural components, each rooted in existing Graphshell registry/domain/surface architecture:

#### 0.3.1 Surface Composition Pass Model

Each node viewer pane's per-frame render is decomposed into three **composition passes**, executed in strict order:

| Pass | Name | Responsibility | Owner |
| --- | --- | --- | --- |
| **Pass 1** | UI Chrome Pass | Tab chrome, pane borders, workbench tile structure (`egui_tiles` layout) | `WorkbenchSurfaceRegistry` |
| **Pass 2** | Content Pass | Web content rendering (Servo composited texture callback **or** Wry overlay sync **or** egui-native embedded viewer) | `ViewerSurfaceRegistry` via `Viewer` trait |
| **Pass 3** | Overlay Affordance Pass | Focus ring, hover ring, selection indicator, diagnostics overlays, tile rearrange affordances | `PresentationDomainRegistry` (affordance policy) + per-`TileRenderMode` implementation |

**Key invariant**: Pass 3 always renders *after* Pass 2 within the same frame for the same tile. For composited backends (Servo texture mode), this means the overlay affordance draws over the composited content in the same GL pipeline. For overlay backends (Wry), Pass 3 renders in the tile chrome/gutter region because the OS-native window cannot be occluded by egui draw calls — the affordance policy adapts to the render mode.

This removes ambiguity from `egui::Order::*` assumptions. The pass model is not an egui concept; it is a Graphshell-owned sequencing contract that uses egui primitives internally but does not depend on egui layer ordering for correctness.

#### 0.3.2 Compositor Adapter (GL State Isolation)

The current path in `tile_compositor.rs` directly invokes the Servo `render_to_parent` callback inside a `PaintCallback` / `CallbackFn`. This callback borrows the GL context and has no contract about:

- GL state save/restore (or state scrub) around the callback
- Clipping/viewport setup and teardown
- Error handling for failed `make_current` calls
- Post-content overlay rendering sequencing

**Solution**: Introduce a `CompositorAdapter` wrapper that owns:

1. **Callback invocation ordering** — ensures content pass completes before overlay pass begins
2. **GL state save/restore** — saves relevant GL state before `render_to_parent`, restores after (or scrubs to known-good defaults)
3. **Clipping/viewport contract** — ensures the `rect_in_parent` calculation and viewport setup are correct and documented
4. **Post-content overlay rendering hook** — the adapter exposes a slot where Pass 3 overlay affordances can be injected after content rendering

The `CompositorAdapter` lives at the Verso mod / `EmbedderCore` boundary — it wraps the Servo-specific `render_to_parent` callback while remaining generic enough that other texture-mode viewers (`viewer:image` rendering to a texture, future GPU-accelerated renderers) can use the same pass contract.

**Implementation target**: `shell/desktop/workbench/compositor_adapter.rs` (new module under the workbench subtree).

#### 0.3.3 Tile Render Mode Enum (Runtime-Authoritative)

Each node viewer pane carries an explicit render mode, resolved at viewer attachment time from the `ViewerRegistry`:

```rust
/// The render pipeline mode for a node viewer pane tile.
/// Authoritative at runtime; drives compositor pass selection and overlay affordance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileRenderMode {
    /// Viewer renders to a GPU texture owned by Graphshell (Servo, GPU-accelerated native viewers).
    /// Overlay affordances draw in the same compositor pipeline after content.
    CompositedTexture,

    /// Viewer creates an OS-native overlay window (Wry).
    /// Overlay affordances draw in tile chrome/gutter/frame, not over content.
    NativeOverlay,

    /// Viewer renders directly into egui UI region (plaintext, metadata, embedded viewers).
    /// Overlay affordances draw via standard egui layering (no special compositor path).
    EmbeddedEgui,

    /// No viewer attached or viewer failed to initialize.
    /// Renders placeholder/fallback content; overlay affordances draw on placeholder.
    Placeholder,
}
```

**Where it lives**: On `NodePaneState` (the existing pane-state struct inside `TileKind::Node`). Resolved when a viewer is attached to a tile (or when the viewer changes) via `ViewerRegistry` query. Diagnostics and runtime UI read this field directly — no inference from side effects.

**Alignment with existing work**:
- The `render_path_hint` diagnostics field (already present) becomes a projection of `TileRenderMode` rather than an independent diagnostic string.
- The `Viewer` trait's `is_overlay_mode()` method (from the Wry integration strategy) maps directly: `is_overlay_mode() == true` → `NativeOverlay`, `render_embedded() returns true` → `EmbeddedEgui` or `CompositedTexture` depending on texture involvement.
- The `overlay_tiles: HashSet<TileId>` tracking proposed in the Wry strategy is replaced by the more general `TileRenderMode` on each pane state.
- `lane:viewer-platform` (`#92`) and `lane:spec-code-parity` (`#99`) track the broader viewer selection/capability work that this enum complements.

#### 0.3.4 Overlay Affordance Policy (Per-Render-Mode)

The overlay affordance policy defines how focus rings, hover indicators, selection outlines, tile-rearrange grips, and diagnostics overlays render for each `TileRenderMode`:

| `TileRenderMode` | Focus Ring | Hover Ring | Rearrange Grip | Diagnostics Overlay |
| --- | --- | --- | --- | --- |
| `CompositedTexture` | Draw in compositor pipeline after content (Pass 3) — **over** content | Same (Pass 3) | Pass 3 | Pass 3 |
| `NativeOverlay` | Draw in tile chrome border/gutter — **around** content, not over it | Gutter highlight or tab-strip indicator | Chrome-region only; OS overlay cannot be occluded | Chrome-region or separate egui window |
| `EmbeddedEgui` | Standard egui stroke on pane rect (widget-level, after viewer `render_embedded`) | Standard egui hover response | Standard egui drag affordance | Standard egui overlay layer |
| `Placeholder` | Standard egui stroke on placeholder rect | Standard egui hover | Standard egui | Standard egui |

This policy is defined in the `PresentationDomainRegistry` as an affordance resolution rule. The Tile Render Pass (`tile_render_pass.rs`) and Tile Compositor (`tile_compositor.rs`) consult the policy (or encode it directly in the pass dispatch) rather than hard-coding a single `LayerId` path.

**Key behavioral fix**: The current "focus ring hidden under document view" bug occurs because the focus ring and web content both paint at `egui::Order::Middle` without guaranteed ordering. Under the Surface Composition Contract, the compositor adapter ensures focus rings (Pass 3) always execute after content (Pass 2) for `CompositedTexture` tiles. For `NativeOverlay` tiles, the policy explicitly shifts affordances to chrome regions, accepting the z-order constraint rather than fighting it.

### 0.4 Servoshell Technical Debt Retirement (Compositor-Specific)

This section maps the servoshell-inherited code that the Surface Composition Contract replaces or wraps, distinguishing what is reusable from what must be retired:

| Servoshell Inheritance | Current Location | Disposition | Notes |
| --- | --- | --- | --- |
| Raw `PaintCallback` + `render_to_parent` GL composition | `tile_compositor.rs` lines 154–175 | **Wrap** in `CompositorAdapter` | The `render_to_parent` callback itself is valid Servo API; only the uncontracted invocation path changes |
| `egui::Order::Middle` layer IDs for both content and rings | `tile_compositor.rs` lines 99–100, 173 | **Replace** with pass-model dispatch | Content layer ID remains; ring layer IDs move to Pass 3 with render-mode-aware z-order |
| `make_current` / `prepare_for_rendering` / `paint` / `present` sequence | `tile_compositor.rs` lines 142–161 | **Reuse** inside `CompositorAdapter` | The sequence is correct Servo API usage; adapter wraps it with state isolation |
| Focus ring as hard-coded fade-in stroke at `Order::Middle` | `tile_compositor.rs` lines 185–196 | **Replace** with overlay affordance policy | Ring rendering becomes render-mode-aware |
| Monolithic `composite_active_node_pane_webviews` function | `tile_compositor.rs` | **Decompose** into pass-model dispatch | Function remains as top-level orchestrator but delegates to per-mode composition paths |
| Fork-shaped host/UI/frame orchestration | `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `shell/desktop/host/window.rs`, `shell/desktop/host/running_app_state.rs` | **Continue `lane:embedder-debt` decomposition** | Embedder decomposition plan (Stage 4) already targets this; Surface Composition Contract is complementary, not overlapping |
| Servoshell-era naming/comments in composition paths | `tile_compositor.rs` comments referencing servoshell patterns | **Retire** | Replace with Graphshell-canonical terminology as composition paths are touched |

### 0.5 Relationship to Existing Plans and Lanes

| Existing Lane/Plan | Relationship to Surface Composition Contract | Action |
| --- | --- | --- |
| `lane:stabilization` (`#88`) | The "tile focus ring hidden under document view" bug is the **primary symptom** this architecture resolves. | Add compositor pass contract child issue under `#88` |
| `lane:embedder-debt` (`#90`) | Embedder decomposition (Stage 4: GUI decomposition) is complementary; the Surface Composition Contract targets the render pipeline specifically, while embedder debt targets host/UI boundary coupling. | Add GL state isolation wrapper child issue under `#90` |
| `lane:viewer-platform` (`#92`) | The `TileRenderMode` enum is a viewer-platform concern; it makes the texture-vs-overlay-vs-embedded distinction runtime-authoritative. | Add render mode enum child issue under `#92` |
| `lane:spec-code-parity` (`#99`) | The overlay affordance policy is a spec/code parity concern — the Wry strategy doc describes z-order constraints but no formal policy exists. | Add affordance policy doc slice under `#99` |
| `2026-02-23_wry_integration_strategy.md` | The `overlay_tiles: HashSet<TileId>` tracking proposed there is superseded by `TileRenderMode` on `NodePaneState`. The `Viewer` trait contract (`render_embedded` / `sync_overlay` / `is_overlay_mode`) is fully compatible; `TileRenderMode` is the runtime-resolved outcome of those trait queries. | Update Wry strategy to reference `TileRenderMode` when compositor adapter lands |
| `aspect_render/aspect_render/2026-02-20_embedder_decomposition_plan.md` | Stage 3 (`EmbedderCore` isolation) is complete. The `CompositorAdapter` wraps the rendering paths that `EmbedderCore` exposes; it does not re-entangle embedder coupling. | No conflict; `CompositorAdapter` is a clean consumer of `EmbedderCore` APIs |
| `SYSTEM_REGISTER.md` (routing rules) | The Surface Composition Contract does not introduce new routing mechanisms. Pass ordering is a render-pipeline concern (direct call within the same module/frame), not a Signal or Intent. | No Signal/Intent routing changes needed |
| `SUBSYSTEM_DIAGNOSTICS.md` | The compositor adapter should emit diagnostics events for GL state save/restore, callback duration, and pass ordering. The existing `tile_compositor.paint` channel is reusable. | Extend diagnostics channel coverage when adapter lands |

### 0.6 Issue Plan (New Issues for Hub/Lane Assignment)

These issues resolve the architectural gap identified in §0.2. Each is a discrete, mergeable slice scoped to avoid cross-lane hotspot conflicts.

#### Issue: Compositor Pass Contract + CompositorAdapter (child of `lane:stabilization` / `#88`)

**Title**: Introduce Surface Composition Pass Model and CompositorAdapter for GL state isolation

**Scope**:
1. Create `shell/desktop/workbench/compositor_adapter.rs` module
2. Implement `CompositorAdapter` struct wrapping `render_to_parent` callback with GL state save/restore
3. Decompose `composite_active_node_pane_webviews` into pass-model dispatch (Content Pass → Overlay Affordance Pass)
4. Move focus ring / hover ring rendering from current `Order::Middle` co-layer to post-content Pass 3
5. Add diagnostics spans for GL state save/restore and pass sequencing
6. Add invariant test: overlay affordance callback executes after content callback for `CompositedTexture` tiles

**Done gate**:
- Focus ring renders over composited web content during tile rearrange (the primary symptom is resolved)
- `CompositorAdapter` GL state save/restore wrapper is tested with a mock callback
- Diagnostics prove pass order (content before overlay) in compositor frame samples
- `cargo check` and `cargo test` pass; no regressions in tile composition

**Hotspots**: `tile_compositor.rs`, new `compositor_adapter.rs`
**Lane**: `lane:stabilization` (`#88`)
**Labels**: `architecture`, `priority/top10`, `lane:stabilization`

#### Issue: TileRenderMode Enum + Runtime Surfacing (child of `lane:viewer-platform` / `#92`)

**Title**: Add `TileRenderMode` to `NodePaneState` with ViewerRegistry-driven resolution

**Scope**:
1. Define `TileRenderMode` enum (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`)
2. Add `render_mode: TileRenderMode` field to `NodePaneState`
3. Resolve `TileRenderMode` from `ViewerRegistry` at viewer attachment time (when viewer is assigned to a tile)
4. Replace `overlay_tiles: HashSet<TileId>` (Wry plan) with `TileRenderMode` query on pane state
5. Project `render_path_hint` diagnostics field from `TileRenderMode` (single source of truth)
6. Wire compositor pass dispatch to branch on `TileRenderMode` (content pass selection)

**Done gate**:
- `TileRenderMode` is set on every `NodePaneState` at viewer attachment time
- Diagnostics inspector shows render mode per tile (from `TileRenderMode`, not inference)
- Compositor branches on render mode for content pass (even if only `CompositedTexture` is implemented initially)
- `cargo check` and `cargo test` pass

**Hotspots**: `tile_kind.rs` (pane state), `tile_runtime.rs` (viewer attachment), `tile_compositor.rs` (dispatch)
**Lane**: `lane:viewer-platform` (`#92`)
**Labels**: `architecture`, `viewer`, `lane:viewer-platform`

#### Issue: Overlay Affordance Policy by Render Mode (child of `lane:spec-code-parity` / `#99`)

**Title**: Define and implement overlay affordance policy per `TileRenderMode`

**Scope**:
1. Document canonical affordance policy table (§0.3.4) in a design doc or as inline architecture comments
2. Implement per-render-mode affordance dispatch in tile compositor / tile render pass
3. For `CompositedTexture`: focus/hover rings draw in Pass 3 (after content)
4. For `NativeOverlay`: focus/hover rings draw in tile chrome border region (documented limitation)
5. For `EmbeddedEgui` and `Placeholder`: standard egui widget-level strokes (current behavior, validated)
6. Add regression test: focus ring visibility covers at least `CompositedTexture` and `Placeholder` modes

**Done gate**:
- Affordance rendering is render-mode-aware (not one hard-coded path)
- `NativeOverlay` path has explicit documented limitation (chrome-region only, cannot occlude OS overlay)
- Spec/code parity claim for overlay z-order behavior is accurate
- `cargo check` and `cargo test` pass

**Hotspots**: `tile_compositor.rs`, `tile_render_pass.rs`
**Lane**: `lane:spec-code-parity` (`#99`)
**Labels**: `architecture`, `ui`, `lane:spec-code-parity`

#### Issue: GL State Invariant Testing for Compositor Callbacks (child of `lane:embedder-debt` / `#90`)

**Title**: Add GL state isolation invariants and tests for Servo compositor callback paths

**Scope**:
1. Define GL state invariant contract: callback must not leak GL state (viewport, scissor, blend mode, active texture unit, bound framebuffer) across composition passes
2. Implement save/restore or scrub-to-defaults in `CompositorAdapter` (if not already fully covered by the primary issue)
3. Add focused test validating GL state before/after a mock `render_to_parent` callback
4. Add diagnostics channel `compositor.gl_state_violation` for runtime detection of state leaks (severity: Warn)
5. Document GL state contract in `compositor_adapter.rs` module-level doc comment

**Done gate**:
- GL state invariant test exists and passes
- `compositor.gl_state_violation` diagnostics channel is registered and emits on detected leaks
- No GL state leak regressions from compositor callback paths in headed smoke tests
- `cargo check` and `cargo test` pass

**Hotspots**: new `compositor_adapter.rs`, `tile_compositor.rs`
**Lane**: `lane:embedder-debt` (`#90`)
**Labels**: `architecture`, `lane:embedder-debt`, `diag`

### 0.7 Terminology Extensions

The following terms are proposed additions to `TERMINOLOGY.md` for the architecture described in this section. They are consistent with existing canonical terminology and fill gaps exposed by the compositor analysis.

| Term | Definition | Category |
| --- | --- | --- |
| **Surface Composition Contract** | The formal specification of how a node viewer pane tile's render frame is decomposed into ordered composition passes (UI Chrome, Content, Overlay Affordance), with backend-specific adaptations per `TileRenderMode`. Defined in the Planning Register §0.3 and implemented through the compositor adapter and pass-model dispatch. | Tile Tree Architecture |
| **Composition Pass** | One of three ordered rendering phases within a single node viewer pane tile's frame: (1) UI Chrome Pass, (2) Content Pass, (3) Overlay Affordance Pass. Pass ordering is a Graphshell-owned sequencing contract that uses egui primitives internally but does not depend on egui layer ordering for correctness. | Tile Tree Architecture |
| **CompositorAdapter** | A wrapper around backend-specific content rendering callbacks (e.g., Servo `render_to_parent`) that owns callback invocation ordering, GL state save/restore, clipping/viewport contracts, and the post-content overlay rendering hook. Lives at the `EmbedderCore` / workbench boundary. | Tile Tree Architecture |
| **TileRenderMode** | The runtime-authoritative render pipeline classification for a node viewer pane tile: `CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, or `Placeholder`. Resolved from `ViewerRegistry` at viewer attachment time. Drives compositor pass selection and overlay affordance policy. Supersedes inference from side effects and the Wry strategy's proposed `overlay_tiles: HashSet<TileId>`. | Tile Tree Architecture |
| **Overlay Affordance Policy** | The per-`TileRenderMode` rules governing how focus rings, hover indicators, selection outlines, and diagnostics overlays are rendered relative to content. For `CompositedTexture` tiles, affordances draw over content in the compositor pipeline; for `NativeOverlay` tiles, affordances draw in chrome/gutter regions. Owned by `PresentationDomainRegistry`. | Visual System / Presentation Domain |

### 0.8 Sequencing and Priority

The four issues in §0.6 should execute in this dependency order:

1. **TileRenderMode Enum** — foundational; all other slices depend on render mode being queryable on pane state. Low risk (additive data model change + viewer attachment wiring). **Lane**: `lane:viewer-platform` (`#92`).
2. **Compositor Pass Contract + CompositorAdapter** — the primary fix for the z-order symptom. Depends on `TileRenderMode` to branch on render mode. Medium risk (GL state manipulation, render sequencing). **Lane**: `lane:stabilization` (`#88`).
3. **Overlay Affordance Policy** — the policy layer that makes Pass 3 render-mode-aware. Depends on both `TileRenderMode` and the compositor adapter. Low risk (rendering logic, no GL state). **Lane**: `lane:spec-code-parity` (`#99`).
4. **GL State Invariant Testing** — hardening/testing layer. Depends on the compositor adapter existing. Low risk (test-only + diagnostics). **Lane**: `lane:embedder-debt` (`#90`).

**Merge-safe assessment**: Issues 1 and 4 touch different hotspots; issues 2 and 3 share `tile_compositor.rs` and should be serialized (2 before 3). Issue 1 can be landed independently before 2/3/4. The four-issue sequence fits inside one merge window if serialized, or two merge windows if split (1 alone, then 2→3→4).

### 0.9 Stabilization Bug Register Update

The following entry in the Stabilization Bug Register (§1A) should be updated to reflect this architecture:

**Bug**: "Tile rearrange focus indicator hidden under document view"

**Updated architectural context**: The symptom is the primary motivator for the Surface Composition Contract (§0.3). The root cause is that focus ring and web content both paint at `egui::Order::Middle` without guaranteed ordering, and the `render_to_parent` GL callback has no post-content overlay hook. The fix class is the compositor pass model (Content Pass → Overlay Affordance Pass) with `CompositorAdapter` GL state isolation. For composited tiles, Pass 3 draws the focus ring after web content in the same GL pipeline. For Wry overlay tiles, the policy shifts affordances to tile chrome (documented limitation). The bug is not addressable by "more egui layers" — it requires an explicit compositor pass model with backend-aware overlay policy and callback state isolation.

**Updated done gate**: Servo focus affordance visible during tile rearrange (Pass 3 over Pass 2 for `CompositedTexture` mode); Wry path has explicit chrome-region affordance and documented limitation; `CompositorAdapter` GL state isolation test passes; diagnostics prove pass ordering in compositor frame samples.

### 0.10 Foundation-First Activation (Appendix A Operationalization)

Project phase statement (2026-02-26): **fix the foundation to enable aspirational capabilities**.

The historical Appendix A opportunity inventory from the now-archived `viewer/2026-02-26_composited_viewer_pass_contract.md` is retained here so follow-on compositor work stays in active planning rather than a stale implementation note. To avoid speculative drift, execution is constrained to a foundation-first sequence where architecture slices are landed before capability expansion.

#### 0.10.1 Foundation slice order (must-run)

1. **Pass-order + render-mode correctness (A.0/A.3 baseline)**
  - Land `TileRenderMode` runtime authority + compositor pass ordering proofs.
  - Blockers cleared: hidden focus ring symptom, inferred render-path ambiguity.
2. **Compositor invariants and forensic tooling (A.1 + A.3)**
  - Extend adapter diagnostics from "detect leak" to "replay + chaos-verify" mode.
  - Blockers cleared: low reproducibility of GL regressions.
3. **Performance and resource safety rails (A.8 + A.7)**
  - Differential composition before GPU budget/degradation.
  - Blockers cleared: unnecessary full-frame recomposition and opaque GPU pressure failure.
4. **Backend control-plane maturity (A.2 + A.9 groundwork)**
  - Hot-swap intent scaffolding + telemetry schema (local-only first).
  - Blockers cleared: backend choice is static and anecdotal.

#### 0.10.2 Foundation now vs later (scope discipline)

**Now (architecture-enabling):**
- A.1 Replay capture scaffolding (diagnostics-backed)
- A.3 Chaos mode harness (feature-gated)
- A.8 Differential composition hooks
- A.7 GPU budget accounting + explicit degradation diagnostics
- A.2 Hot-swap intent/model contract (without full state parity guarantees)

**Later (capability expansion after foundation):**
- A.6 cross-tile cinematic transitions
- A.5 content-aware adaptive overlay styling
- A.10 mod-hosted compositor extension passes
- A.9 Verse-published telemetry races (keep local telemetry first)
- A.4 upstream/shared protocol packaging to Verso once Graphshell contract stabilizes

Archive note (2026-03-10):
- The original Appendix A source note has been archived under `design_docs/archive_docs/checkpoint_2026-03-10/graphshell_docs/implementation_strategy/viewer/2026-02-26_composited_viewer_pass_contract.md`.
- This section is now the active home for any retained deferred compositor capability ideas from that archive.

#### 0.10.3 Issue seeding from foundation slices

Foundation child issues opened (2026-02-26):

1. `#166` — `Compositor replay traces for callback-state forensics` (`lane:stabilization` / parent `#88`)
2. `#171` — `Compositor chaos mode for GL isolation invariants` (`lane:embedder-debt` / parent `#90`)
3. `#167` — `Differential composition for unchanged composited tiles` (`lane:stabilization` / parent `#88`)
4. `#168` — `Per-tile GPU budget and degradation diagnostics` (`lane:viewer-platform` / parent `#92`)
5. `#169` — `Viewer backend hot-swap intent and state contract` (`lane:viewer-platform` / parent `#92`)
6. `#170` — `Backend telemetry schema (local-first, Verse-ready)` (`lane:runtime-followon` / parent `#91`)

Duplicate cleanup note: `#172` was created in parallel and closed as a duplicate of `#170`.

Execution note (2026-03-09):
- `#168` is closed in code: compositor GPU degradation now uses estimated per-tile byte accounting instead of a coarse pass-count cap, degradation receipts/diagnostics expose the budgeted failure mode explicitly, diagnostics snapshots export budget utilization and degraded-byte aggregates, and compositor frame samples carry per-tile estimated content bytes.
- `#169` is closed in code: command surfaces and tile chrome route backend changes through `SwapViewerBackend`, focused `PaneId` targeting is preserved when present, node-owned session state (`url/history`, `session_scroll`, `session_form_draft`) remains the swap contract, and lifecycle reconcile now tears down stale Servo/Wry runtime ownership before the alternate backend reattaches.
- `#170` is closed in code: diagnostics snapshots still export the internal `backend_telemetry` summary, and now also emit a versioned `backend_telemetry_report` envelope plus dedicated local JSON export so the current local-first artifact is stable and Verse-ready without reshaping.
- Remaining follow-through is intentionally narrower: richer live backend replay hooks may still improve parity beyond the current node-owned session contract, and backend telemetry remains local-first rather than actually Verse-published.

Each issue should explicitly reference Appendix subsection IDs (`A.1`, `A.3`, etc.) and include a **Foundation Done Gate**: "removes one concrete blocker for future capabilities without introducing new cross-lane hotspot conflicts."

### 0.11 Backend Bridge Contract Rollout (C+F)

Decision (2026-03-01): adopt **C+F** as the backend migration strategy for composited content paths.

- **C (Contract-first)**: all compositor/content pass invocation paths must consume a backend-agnostic bridge contract owned by `render_backend`, with backend-specific callback/context details hidden behind adapter implementations.
- **F (Fallback-safe)**: the wgpu path is primary, with capability-driven fallback for environments where interop path assumptions are unavailable or unstable.

Execution policy:

1. Land contract boundaries first (type/ownership isolation, call-site migration, diagnostics parity hooks).
2. Keep Glow only as a temporary benchmark and parity baseline during wgpu adapter bring-up.
3. Add explicit capability checks and fallback routing before removing Glow from production composition paths.
4. Retire Glow path when wgpu + fallback make it redundant for supported targets.

Acceptance gates for Glow retirement:

- Compositor replay diagnostics parity between Glow baseline and wgpu primary path.
- No open stabilization regressions tied to pass-order, callback-state isolation, or overlay affordance visibility.
- Capability fallback behavior validated in targeted non-interop environments.
- Receipt-linked evidence exists in issue tracker showing wgpu path covers all required pass-contract scenarios.

Issue linkage:

- `#183` is the implementation tracker for backend migration slices aligned to this C+F contract.
- Receipt: `2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`.

### 0.12 WebRender Readiness Gate + Feature Guardrails

Decision (2026-03-01): keep Glow active for current milestone delivery, but start WebRender/wgpu switch-readiness work now under explicit guardrails.

Policy:

- Runtime switch remains blocked until readiness gates are closed with evidence.
- Feature work continues only if it avoids new renderer-specific coupling and preserves backend bridge observability.
- Readiness work runs in parallel through bounded spikes and dependency-control validation.

Canonical reference:

- `2026-03-01_webrender_readiness_gate_feature_guardrails.md`.

Tracker linkage:

- Primary tracker: `#183`.
- Related lanes: `#88`, `#99`, `#92`, `#90`.

### 0.13 WebRender wgpu Renderer Implementation Plan

Decision (2026-03-01): convert WebRender wgpu renderer research into a phased, technically validatable implementation plan.

Phases P0–P12 cover dependency audit, upstream reconnaissance, fork/patch scaffold, trait extraction, shader translation, wgpu Device/Renderer implementation, compositor output contract, Graphshell integration spike, pixel parity validation, performance validation, platform matrix validation, and production cutover preparation.

Key properties:

- Every phase has explicit entry conditions, exit criteria, and rollback posture.
- GL path preserved throughout via C+F fallback-safe policy.
- `active_backend_content_bridge_policy()` remains `GlowBaseline` until P12 switch authorization.
- Readiness gates G1–G5 mapped to specific closing phases.

Canonical reference:

- `aspect_render/2026-03-01_webrender_wgpu_renderer_implementation_plan.md`.

Tracker linkage:

- Primary tracker: `#183`.
- Lane hub: `#245`.
- Spike evidence target: `#180`.
- Related lanes: `#88`, `#90`, `#92`, `#99`.

---

## 1. Immediate Priorities Register (10/10/10)

_Source file before consolidation: `2026-02-24_immediate_priorities.md`_


**Status**: Active / Execution (revised 2026-02-25)
**Context**: Consolidated execution register synthesized from current implementation strategy, research, architecture, and roadmap docs.

**Audit basis (2026-02-25 review)**:
- `2026-02-22_registry_layer_plan.md`
- `2026-02-22_multi_graph_pane_plan.md` (scope expanded in paired doc sync to pane-hosted multi-view architecture)
- `2026-02-24_layout_behaviors_plan.md`
- `2026-02-24_performance_tuning_plan.md`
- `2026-02-24_control_ui_ux_plan.md`
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-24_spatial_accessibility_plan.md` → superseded by `SUBSYSTEM_ACCESSIBILITY.md`
- `viewer/2026-02-24_universal_content_model_plan.md`
- `2026-02-23_wry_integration_strategy.md`
- `2026-02-20_edge_traversal_impl_plan.md`
- `2026-02-18_graph_ux_research_report.md`
- `2026-02-24_interaction_and_semantic_design_schemes.md`
- `2026-02-24_diagnostics_research.md`
- `2026-02-24_visual_tombstones_research.md`
- `2026-02-24_spatial_accessibility_research.md`
- `GRAPHSHELL_AS_BROWSER.md`
- `IMPLEMENTATION_ROADMAP.md`
- `design_docs/PROJECT_DESCRIPTION.md`

---

## 0. Latest Checkpoint Delta (Code + Doc Audit)

### Code checkpoint (2026-02-24)

- Registry Phase 6.2 boundary hardening advanced: workspace-only reducer path extracted and covered by boundary tests.
- Registry Phase 6.3 single-write-path slices closed for runtime/persistence: direct persistence topology writes were converged to graph-owned helpers, runtime contract coverage now includes persistence runtime sections, and targeted boundary tests are green.
- Registry Phase 6.4 started with a mechanical host subtree move: `running_app_state.rs` and `window.rs` are now canonical under `shell/desktop/host/` with root re-export shims retained during transition.
- Registry Phase 6.4 import canonicalization advanced beyond `shell/desktop/**`: remaining root-shim host imports in `egl/app.rs` and `webdriver.rs` were moved to canonical `shell/desktop/host/*` paths; shim files remain in place for transition compatibility.
- Phase 5 sync UI/action path advanced: pair-by-code decode, async discovery enqueue path, and Phase 5 diagnostics channel + invariant contracts are now in code with passing targeted tests.
- Compile baseline remains green (`cargo check`), warning baseline unchanged.

### Doc audit delta (2026-02-25)

- Immediate-priority list promoted from a loose synthesis into a source-linked 10/10/10 register.
- Multi-pane planning is now treated as a **pane-hosted multi-view problem** (graph + viewer + tool panes), not only "multi-graph."
- Several low-effort, high-impact items from UX and diagnostics research were missing from the active queue and are now explicitly tracked.
- **Cross-cutting subsystem consolidation**: Five runtime subsystems formalized with dedicated subsystem guides:
  - `SUBSYSTEM_ACCESSIBILITY.md` — consolidates prior archived accessibility planning/detail docs in `design_docs/archive_docs/checkpoint_2026-02-25/` (both now superseded)
  - `SUBSYSTEM_DIAGNOSTICS.md` — elevated from `2026-02-24_diagnostics_research.md`
  - `SUBSYSTEM_SECURITY.md` — new; consolidates security/trust material from Verse Tier 1 plan + registry layer plan Phase 5.5
  - `SUBSYSTEM_STORAGE.md` — new; consolidates persistence material from registry layer plan Phase 6 + `services/persistence/mod.rs`
  - `SUBSYSTEM_HISTORY.md` — new; consolidates traversal/archive/replay integrity guarantees and Stage F temporal navigation constraints
- Surface Capability Declarations adopt the **folded approach** (sub-fields on `ViewerRegistry`, `CanvasRegistry`, `WorkbenchSurfaceRegistry` entries — not a standalone registry). See `TERMINOLOGY.md`.

### Subsystem Implementation Order (Current Priority)

This section sequences subsystem work by architectural leverage and unblock status. It links to subsystem guides instead of repeating subsystem contracts.

| Order | Subsystem | Why Now | Best Next Slice | Key Blockers / Dependencies |
| --- | --- | --- | --- | --- |
| 1 | `diagnostics` | Enables confidence and regression detection across all other subsystems. | Expand invariant coverage + pane health/violation views; continue severity-driven surfacing. | Pane UX cleanup; cross-subsystem channel adoption. |
| 2 | `storage` | Data integrity and persistence correctness are hard failure domains and a dependency for reliable history. | Add `persistence.*` diagnostics, round-trip/recovery coverage, degradation wiring. | App-level read-only UX wiring; crypto overlap with `security`. |
| 3 | `history` | Temporal replay/preview and traversal correctness depend on `storage` guarantees and become a core user-facing integrity concern. | Add `history.*` diagnostics + traversal/archive correctness tests before Stage F replay UI. | Stage E history maturity; persistence diagnostics/archives. |
| 4 | `security` | High-priority trust guarantees, but some slices are tied to Verse Phase 5.4/5.5 closure sequencing. | Grant matrix coverage + denial-path diagnostics assertions + trust-store integrity tests. | Verse sync path closure and shared `GraphIntent` classification patterns. |
| 5 | `accessibility` | Project goal and major concern, but Graph Reader breadth should follow the immediate WebView bridge fix and diagnostics scaffolding. | WebView bridge compatibility fix (`accesskit` alignment/conversion) + anchor mapping + bridge invariants/tests. | `accesskit` version mismatch; pane/view lifecycle anchor registration; view model stabilization for Graph Reader. |

---

## 1A. Merge-Safe Lane Execution Reference (Canonical)

This section is the canonical sequencing reference for conflict-aware execution planning, aligned with `CONTRIBUTING.md` lane rules (one active mergeable PR per lane when touching shared hotspots).

### Lane sequencing rules

- Use one active mergeable PR per lane for hotspot files (`graph_app.rs`, `render/mod.rs`, workbench/gui integration paths).
- Use stacked PRs for dependent issue chains; merge bottom → top.
- Avoid cross-lane overlap on the same hotspot files within the same merge window.
- Treat this section as **active control-plane state**; treat detailed ticket stubs below as reference material.

### Recommended execution sequence (current)

Snapshot note (2026-02-26 queue execution audit + tracker reconciliation):
- The previously queued implementation chains below were audited and reconciled in issue state (closed):
  - `lane:p6`: `#76`, `#77`, `#63`-`#67`, `#79`
  - `lane:p7`: `#68`-`#71`, `#78`, `#80`, `#82`
  - `lane:p10`: `#74`, `#75`, `#73` and parent `#10`
  - `lane:runtime`: `#81`
  - `lane:quickwins`: `#21`, `#22`, `#27`, `#28`
  - `gap-remediation hub`: `#86`
- Evidence/receipt: `design_docs/archive_docs/checkpoint_2026-02-26/2026-02-26_planning_register_queue_execution_audit_receipt.md`

1. **lane:roadmap (docs/planning, merge-safe default lane)**
  - Queue reconciled (2026-02-26): `#11`, `#12`, `#13`, `#14`, `#18` closed as completed adoption/planning slices.
  - Remaining open roadmap queue item: `#19` (`TwoD↔ThreeD` `ViewDimension` hotswitch; still deferred/blocked).
  - Active docs-only execution guide for blocked-state parallel work: `design_docs/graphshell_docs/implementation_strategy/canvas/2026-02-27_roadmap_lane_19_readiness_plan.md`.
  - Low conflict risk with runtime/render hot files; preferred background lane while no critical hotfix override is active

  **Roadmap lane quick status (checklist style)**
  - `#19` remains **blocked** until prerequisites in `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md` are closed.
  - While blocked, roadmap work stays **docs-only** and confined to `design_docs/**`.
  - `R1` checklist is tracked below; `R2` acceptance contract draft: `design_docs/graphshell_docs/implementation_strategy/canvas/2026-02-27_viewdimension_acceptance_contract.md`.
  - `R3` terminology alignment is complete in `design_docs/TERMINOLOGY.md` (`ViewDimension`, `ThreeDMode`, `ZSource`, `Derived Z Positions`, `Dimension Degradation Rule`).
  - `R4` issue-stack seeding is complete at docs level in `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md` (`R4.1`..`R4.4` templates).
  - Move `#19` to implementation-ready only after explicit evidence links exist for each prerequisite closure.

  **`#19` prerequisite readiness checklist (R1)**

  | Prerequisite | Owner lane / tracker | Status (`open` / `partial` / `closed`) | Current evidence links | Closure criteria (for `closed`) |
  | --- | --- | --- | --- | --- |
  | Stabilization closure on camera/input/focus | `lane:stabilization` / `#88` | `partial` | Stabilization progress receipt updated at `implementation_strategy/2026-02-28_stabilization_progress_receipt.md` with landed evidence across `001a121` → `d67ffa9` (including replay-forensics `#166` completion, differential composition `#167`, iterative `#184` stabilization slices, `#185` selection ambiguity diagnostics hardening, `#186` deterministic selected-node pane/tab/split routing coverage, `#187` deterministic close-pane successor focus handoff coverage, and `#244` GUI decomposition boundary-contract test invariants). Camera regression closure evidence is recorded in `archive_docs/checkpoint_2026-03-05/2026-03-05_camera_navigation_fix_postmortem.md` and associated camera-lock regression tests. Remaining open stabilization items are focus/lasso related. | Linked issue/PR evidence confirms camera controls, focus ownership, and lasso regressions are closed with targeted tests/diagnostics and no active repro remains in the bug register. |
  | Surface composition pass contract + overlay affordance policy closure | `lane:stabilization` / `#88`; `lane:spec-code-parity` / `#99`; backend migration `#183` | `partial` | Gap analysis and architectural contract are documented in `§0`; C+F backend bridge-contract rollout is now explicitly defined in `§0.11` with receipt evidence at `implementation_strategy/2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`. | Compositor pass contract + overlay policy issue stack is linked with closure evidence showing Pass 3 ordering invariants and per-render-mode affordance behavior validated, and `#183` closure evidence confirms wgpu-primary + fallback-safe contract parity. |
  | Runtime-authoritative tile render mode behavior | `lane:viewer-platform` / `#92` | `partial` | Runtime render-mode projection and diagnostics-path hint plumbing were landed in recent work touching `tile_runtime.rs` + `tile_render_pass.rs`; full lane closure evidence not yet linked. | Linked issue/PR evidence confirms `TileRenderMode` is authoritative end-to-end (attach-time resolution, render dispatch, diagnostics projection) with acceptance tests and no known regressions. |
  | Persistence + degradation guarantees for dimension state | `lane:roadmap` (spec) then implementation lanes | `partial` | Blocker and requirements are defined in `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md`; acceptance contract drafted in `canvas/2026-02-27_viewdimension_acceptance_contract.md`; terminology alignment is complete in `design_docs/TERMINOLOGY.md` (`ViewDimension`, `ThreeDMode`, `ZSource`, `Derived Z Positions`, `Dimension Degradation Rule`); issue templates are seeded in readiness plan `R4.1`..`R4.4`. | Canonical docs align on persisted `ViewDimension` ownership and deterministic `TwoD` fallback semantics; linked implementation/test issues exist for restore/degradation behavior. |

  **Evidence-link rule for readiness transitions**
  - `open` → `partial`: add at least one concrete issue/PR/commit link showing active progress.
  - `partial` → `closed`: add closure proof links (tests/diagnostics/receipts) and verify closure criteria text is satisfied.
  - `#19` remains blocked until all four prerequisite rows are `closed`.

2. **lane:runtime-followon (new tickets required)**
  - `SYSTEM_REGISTER.md` SR2 (signal routing contract) before SR3 (`SignalBus`/equivalent fabric)
  - Hub: `#91` (SR2/SR3 signal routing contract + fabric tracker)
  - Create new child issues before execution; avoid reusing closed queue-cleanup issues (`#80/#81/#82/#86`)
  - Keep separate from high-churn UI hotspot work if touching `gui.rs` or registry runtime hotspots
3. **Core completion lanes (parallelized with merge-safe hotspots discipline)**
  - `lane:control-ui-settings` (`#89`), `lane:embedder-debt` (`#90`), `lane:viewer-platform` (`#92`), `lane:diagnostics` (`#94`), `lane:accessibility` (`#95`), `lane:subsystem-hardening` (`#96`), `lane:test-infra` (`#97`), `lane:knowledge-capture` (`#98`)
  - Goal: reach a coherent "flesh-and-bone" milestone body before hardening-first stabilization posture
4. **Inter-plan audit checkpoint (mandatory before stabilization lane promotion)**
  - Audit scope: acceptance-contract coverage, done-gate closure evidence, spec-code parity, diagnostics coverage, open blocker drift, and issue/doc synchronization.
  - Required artifact: timestamped receipt in `design_docs/archive_docs/checkpoint_YYYY-MM-DD/` documenting audit outcomes and explicit stabilization entry decision.
5. **lane:stabilization (bounded post-completion hardening pass; default after audit)**
  - Hub: `#88` (Controls/camera/focus correctness stabilization tracker)
  - Default role: harden integrated systems after completion lanes close at milestone level.
  - Exception: critical use-blocking regressions may trigger short pre-audit hotfix slices.
  - Rule: run as single focused PR slices, avoid mixing with unrelated feature/refactor changes in the same hotspots.

### Near-term PR stack plan (merge order)

- Completed (2026-02-26 audit/reconciliation): `lane:p6`, `lane:p7` phase-1, `lane:p10`, `lane:runtime`, `lane:quickwins` queues listed above
- Active merge-safe default stack: `lane:roadmap` docs/planning follow-on (`#19` only; blocked until prerequisites)
- Core implementation push: close priority completion lanes to milestone coherence before broad hardening
- Mandatory checkpoint: run inter-plan audit and publish receipt before broad stabilization promotion
- Conditional emergency override: critical use-blocking regressions may run as short stabilization hotfix PRs
- Parallel planning only (no code until ticketed): Register signal-routing roadmap slices (SR2/SR3)

### Stabilization Bug Register (Active)

Track active regressions here before they get folded into broader refactors. These are the only ad hoc slices allowed to preempt the default lane stack.

| Bug / Gap | Symptom | Likely Hotspots | Notes / Architectural Context | Done Gate |
| --- | --- | --- | --- | --- |
| Graph canvas camera controls fail globally *(closed 2026-03-05)* | `pan drag`, `wheel zoom`, `zoom in/out/reset`, and `zoom-to-fit` failed in the default graph pane (not just multi-pane) | `render/mod.rs`, `graph_app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs` | Closed with canonical postmortem evidence: `archive_docs/checkpoint_2026-03-05/2026-03-05_camera_navigation_fix_postmortem.md` (dead metadata slot + per-frame fit override root causes, final fix, verification). | Closed: default graph pane supports pan + wheel zoom + zoom commands again; closure evidence includes targeted camera-lock tests and postmortem verification. |
| Lasso metadata ID mismatch after multi-view | Selection/lasso behavior breaks or targets wrong graph metadata in multi-pane scenarios | `render/mod.rs` | Known hardcoded `egui_graphs_metadata_` path needs per-view metadata keying. | Lasso works across split graph panes; test covers second pane / non-default `GraphViewId`. |
| Tab/pane spawn focus activation race (blank viewport) | Newly opened tab/pane sometimes spawns visually blank until extra clicks/tab switches; graph pane can remain unfocused after pane deletion | `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `shell/desktop/workbench/*`, `shell/desktop/lifecycle/webview_controller.rs` | Looks like focus ownership + render activation ordering debt (likely overlaps `lane:embedder-debt` servoshell-era host/frame assumptions). | New focused panes render on first activation consistently; pane-deletion focus handoff is deterministic and renders immediately. |
| Selection deselect click-away inconsistency | Node selection works, but clicking background to deselect is "funky" and may hide state-transition edge cases | `render/mod.rs`, `input/mod.rs`, selection state in `graph_app.rs` | Deterministic plain-click deselect guard landed in `936073e`; keep monitoring for pane-focus interaction edge cases. | Deselect-on-background-click behavior is deterministic and covered by targeted selection tests. |
| Lasso boundary miss at selection edge | Lasso sometimes misses nodes at the edge of the box despite user expectation that center-in-box should count | `render/mod.rs`, `render/spatial_index.rs` | Correctness issue first; live lasso preview UX should be tracked separately under `lane:control-ui-settings`. | Lasso inclusion semantics are documented (center-inclusive minimum) and covered by edge-boundary tests. |
| Tile rearrange focus indicator hidden under document view | Blue focus ring does not render over document/web content while rearranging tile | `shell/desktop/workbench/tile_compositor.rs`, new `compositor_adapter.rs` | **Root cause identified in §0.3**: focus ring and web content both paint at `egui::Order::Middle` without guaranteed ordering; `render_to_parent` GL callback requires explicit post-content overlay sequencing and state restoration. Recent hardening (`b6b931b`) made pass-order violations render-mode-aware (composited-only), added native-overlay chrome diagnostics regressions, and fixed framebuffer binding restoration to use captured state. | Servo focus affordance visible during tile rearrange (Pass 3 over Pass 2 for `CompositedTexture` mode); Wry path has explicit chrome-region affordance and documented limitation; `CompositorAdapter` GL state isolation test passes; diagnostics prove pass ordering in compositor frame samples. |
| Legacy web content context menu / new-tab path bypasses node semantics | Right-click or ctrl-click link in webpage can use short legacy menu/path and may open tile/tab without creating mapped graph node | `shell/desktop/ui/gui.rs`, `shell/desktop/host/*`, `shell/desktop/lifecycle/webview_controller.rs`, `shell/desktop/workbench/tile_runtime.rs` | Graphshell command/pane semantics are bypassed by legacy webview path; cross-lane with `lane:embedder-debt` + `lane:control-ui-settings`. | Web content open-in-new-view flows route through Graphshell node/pane semantics or are explicitly bridged/deferred with limitations documented. |
| Command palette trigger parity + naming confusion | F2 summons `Edge Commands`; pointer/global trigger availability is context-biased and inconsistent | `render/mod.rs`, `render/command_palette.rs`, `input/mod.rs` | Keyboard trigger exists; command-surface model/naming/context policy lag behind plan. | Shared command-surface model backs F2 and contextual palette variants; naming reflects actual scope (not `Edge Commands` unless edge-specific). |

#### Known Rendering/Input Regressions (tracked under `lane:stabilization`)

- Global graph camera interaction failure is closed; see `archive_docs/checkpoint_2026-03-05/2026-03-05_camera_navigation_fix_postmortem.md` for root cause and verification evidence.
- Pane/tab focus activation and render timing are inconsistent (blank viewport until extra clicks/tab switches in some flows).
- Focus ring over composited web content remains a compositor pass/state-contract issue (Servo path), not an `egui` layer-count issue.
- Input consumption/focus ownership edge cases remain likely when graph pane and node pane coexist.
- Lasso correctness follow-ons remain: edge-boundary inclusion semantics and selection-state polish.

Use these as first-pass stabilization issue seeds when a dedicated issue does not yet exist.

#### Command Surface + Settings Parity Checklist (tracked under `lane:control-ui-settings`)

- Command palette must remain keyboard-triggerable and gain non-node pointer/global trigger parity (canvas, pane/workspace chrome, nodes/edges).
- F2/global command surface and right-click/contextual command surface should share one backend model while allowing different presentation sizes.
- `Edge Commands` labeling should be retired or narrowed to truly edge-specific UI.
- Contextual command categories should map to actionable entities (node/edge/tile/pane/workbench/canvas) with a clear disabled-state policy.
- Radial menu needs spacing/readability polish before primary-use promotion.
- Omnibar node-search iteration should retain input focus after Enter in search mode.
- Theme mode toggle (`System` / `Light` / `Dark`) should be added to settings and persisted.
- Settings IA must converge from transitional legacy booleans/bridge path to one page-backed settings surface.
- Settings tool pane must graduate from placeholder to scaffolded runtime surface.

Issue-ready intake stubs from the latest user report:
- `design_docs/graphshell_docs/implementation_strategy/2026-02-26_stabilization_control_ui_issue_stubs_from_user_report.md`

### Debt-Retirement Lanes (Current)

- `lane:embedder-debt` (servoshell inheritance retirement)
  - Hub: `#90` (Servoshell inheritance retirement tracker)
  - Scope: `shell/desktop/ui/gui.rs`/`gui_frame.rs` decomposition ✅ largely complete; next targets are `graph_app.rs` (14.7k lines) and `render/mod.rs` (5.6k lines); `RunningAppState` coupling reduction, host/UI boundary cleanup, misleading servoshell-era naming/comments removal
  - Important child slice: composited webview callback pass contract + GL state isolation (`tile_compositor.rs`) to fix Servo-path overlay affordance failures that are not Wry/native-overlay limitations
  - Primary guide: `design_docs/graphshell_docs/implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md`
  - Coordinator policy: treat `shell/desktop/ui/gui.rs` / `shell/desktop/ui/gui_frame.rs` / `shell/desktop/ui/gui_orchestration.rs` as orchestration façades with explicit authority boundaries; enforce via `CONTRIBUTING.md` coordinator checklist when these files are touched
  - Rule: pair mechanical moves with invariants/tests; avoid mixing with feature work in the same PR

### Incubation Lanes (Parallel / Non-blocking)

- `lane:verse-intelligence`
  - Hub: `#93` (Model slots + memory architecture implementation tracker)
  - Open a hub + child issue stack for the two design-ready plans (currently no implementation lane):
  - `design_docs/verse_docs/implementation_strategy/self_hosted_model_spec.md`
  - `design_docs/verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md`
  - First executable slices should be schemas/contracts + storage/index scaffolds (not model training)

### Spec/Code Mismatch Register (Active)

| Mismatch | Current Reality | Owner Lane | Done Gate |
| --- | --- | --- | --- |
| `viewer:settings` selected but not embedded | Viewer resolution can select `viewer:settings`, but node-pane renderer still falls back to non-embedded placeholder for non-web viewers. | `lane:viewer-platform` (`#92`), `lane:control-ui-settings` (`#89`) | Settings viewer path is renderable without placeholder fallback in node/tool contexts. |
| Browser viewer table vs implemented viewer surfaces | Spec/docs describe broader viewer matrix than runtime embedded implementations currently expose. | `lane:viewer-platform` (`#92`), `lane:spec-code-parity` (`#99`) | Viewer table claims are either implemented or explicitly downgraded with phased status. |
| Wry strategy/spec vs runtime registration/dependency path | Wry integration strategy exists, but runtime feature/dependency/registration path remains partial/transitional. | `lane:viewer-platform` (`#92`), `lane:spec-code-parity` (`#99`) | `viewer:wry` foundation is feature-gated and runtime-wired, or spec is marked deferred with constraints. |
| Overlay affordance policy unspecified by render mode | Wry strategy documents texture-vs-overlay z-order constraints but no canonical per-`TileRenderMode` affordance policy exists in spec or code. Focus/hover/selection ring behavior is single hard-coded path regardless of backend. | `lane:spec-code-parity` (`#99`), `lane:stabilization` (`#88`) | Overlay affordance policy table (§0.3.4) is implemented and validated per render mode; `NativeOverlay` limitations are documented. |
| `overlay_tiles: HashSet<TileId>` vs `TileRenderMode` enum | Wry strategy proposes `overlay_tiles` set on `TileCompositor`; Surface Composition Contract supersedes with authoritative `TileRenderMode` on `NodePaneState`. | `lane:viewer-platform` (`#92`) | Wry strategy updated to reference `TileRenderMode`; no `overlay_tiles` tracking exists independent of pane state render mode. |

---

## 1B. Register Size Guardrails + Archive Receipts

This register is intentionally large; to keep it operational for agents and contributors, apply the following:

- Keep **active sequencing + merge/conflict guidance** in Sections `1`, `1A`, and `1B`.
- Treat detailed issue stubs and long guidance sections as **reference payloads**.
- When sequencing decisions change materially, write a timestamped archive receipt in:
  - `design_docs/archive_docs/checkpoint_2026-02-25/`
- Archive receipt naming convention:
  - `YYYY-MM-DD_planning_register_<topic>_receipt.md`
- Archive receipts should include:
  - date/time window
  - lane order
  - issue stack order
  - hotspot conflict assumptions
  - closure/update criteria

Current receipt for this sequencing snapshot:
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_planning_register_lane_sequence_receipt.md`
- `design_docs/archive_docs/checkpoint_2026-02-26/2026-02-26_planning_register_queue_execution_audit_receipt.md` (queue execution audit + landed-status verification + `#70` lifecycle policy patch)

---

## 1C. Top 10 Active Execution Lanes (Strategic / Completion-Oriented)

This supersedes the earlier registry-closure-heavy priority table. The queue audit closed most of those slices in code/issue state; the remaining project risk is now concentrated in stabilization, architectural follow-ons, subsystem hardening, and design-to-code execution.

Execution order policy (2026-03-10): prioritize completion of core lanes first, run an inter-plan audit checkpoint, then execute stabilization as a bounded hardening pass (except critical hotfix regressions).

| Rank | Lane | Why Now | Primary Scope (Next Tasks) | Primary Sources / Hotspots | Lane Done Gate |
| --- | --- | --- | --- | --- | --- |
| 1 | **`lane:control-ui-settings` (`#89`)** | Command surfaces and settings IA are now clearly specified by user needs, but the runtime UI still exposes transitional/legacy command surfaces. | Unify F2 + contextual command surfaces, retire/rename `Edge Commands`, define contextual category/disabled-state policy, radial readability pass, omnibar focus retention, theme mode toggle, settings scaffold replacing placeholder pane. | `2026-02-24_control_ui_ux_plan.md`, `2026-02-20_settings_architecture_plan.md`, `render/command_palette.rs`, `render/mod.rs`, `shell/desktop/ui/toolbar/*`, `shell/desktop/workbench/tile_behavior.rs` | Command surfaces share one dispatch/context model across UI contexts; settings pane supports theme mode and is no longer placeholder-only for core settings paths. |
| 2 | **`lane:embedder-debt` (`#90`)** | Servoshell inheritance debt is the main source of host/UI focus/compositor friction and still leaks legacy behavior into user-facing flows. | Decompose `gui.rs`/`gui_frame.rs`, reduce `RunningAppState` coupling, narrow host/UI boundaries, fix legacy webview context-menu/new-tab bypass paths, retire misleading servoshell-era assumptions/comments. | `aspect_render/aspect_render/2026-02-20_embedder_decomposition_plan.md`, `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `shell/desktop/host/*`, `shell/desktop/lifecycle/webview_controller.rs` | One stage of decomposition lands with tests/receipts; legacy webview path bypasses are either bridged or retired; hotspot surface area is reduced. |
| 3 | **`lane:runtime-followon` (`#91`)** | Sector H is now implemented; remaining work in this lane is consumer adoption and doc cleanup, not missing signal-bus infrastructure. | Close out docs/issue receipts, then consume `SignalBus` from Sector G / future runtime workers where needed. | `SYSTEM_REGISTER.md`, `2026-03-08_sector_h_signal_infrastructure_plan.md`, `shell/desktop/runtime/registries/mod.rs`, `shell/desktop/runtime/registries/signal_routing.rs` | `SignalBus` remains testable/observable and downstream sectors consume it without reintroducing direct registry coupling. |
| 4 | **`lane:viewer-platform` (`#92`)** | Viewer selection/capability scaffolding is ahead of actual embedded viewers; Wry remains design-only; **`TileRenderMode` enum** (§0.3.3) needed for compositor pass dispatch and overlay policy. | Replace non-web viewer placeholders (`settings`/`pdf`/`csv` first), implement Wry feature gate + manager/viewer foundation, align Verso manifest/spec claims, **add `TileRenderMode` to `NodePaneState` with ViewerRegistry-driven resolution**. | `viewer/2026-02-24_universal_content_model_plan.md`, `2026-02-23_wry_integration_strategy.md`, `GRAPHSHELL_AS_BROWSER.md`, `mods/native/verso/mod.rs`, `Cargo.toml`, `shell/desktop/workbench/tile_behavior.rs`, `shell/desktop/workbench/tile_kind.rs` | At least one non-web native viewer is embedded; `viewer:wry` foundation exists behind feature gate or spec/docs are explicitly downgraded; `TileRenderMode` is set on every `NodePaneState` at viewer attachment time. |
| 5 | **`lane:diagnostics` (`#94`)** | Diagnostics remains the leverage multiplier for every other lane and still lacks analyzer/test harness execution surfaces. | Implement `AnalyzerRegistry` scaffold, in-pane `TestHarness`, expanded invariants, better violation/health views, orphan-channel surfacing. | `SUBSYSTEM_DIAGNOSTICS.md`, `shell/desktop/runtime/diagnostics/*`, diagnostics pane code paths | Analyzer/TestHarness scaffolds exist and can be run in-pane (feature-gated if needed). |
| 6 | **`lane:accessibility` (`#95`)** | Accessibility is a project-level requirement; phase-1 bridge work exists but Graph Reader/Inspector paths remain incomplete. | Finish bridge diagnostics/health surfacing, implement Graph Reader scaffolds, replace Accessibility Inspector placeholder pane, add focus/nav regression tests. | `SUBSYSTEM_ACCESSIBILITY.md`, `shell/desktop/workbench/tile_behavior.rs`, `shell/desktop/ui/gui.rs` | Accessibility Inspector is functional, bridge invariants/tests are green, and Graph Reader phase entry point exists. |
| 7 | **`lane:subsystem-hardening` (`#96`)** | Storage/history/security are documented but still missing closure slices that protect integrity and trust. | Add `persistence.*` / `history.*` / `security.identity.*` diagnostics, degradation wiring, traversal/archive correctness tests, grant matrix denial-path coverage. | `SUBSYSTEM_STORAGE.md`, `SUBSYSTEM_HISTORY.md`, `SUBSYSTEM_SECURITY.md`, persistence/history/security runtime code | Subsystem health summaries and critical integrity/denial-path tests are in CI or documented as explicit follow-ons. |
| 8 | **Inter-plan audit checkpoint (no lane id; required gate)** | Prevents false closure between plan slices and ensures a real milestone body exists before hardening-first work. | Run cross-lane acceptance audit: done-gate evidence, spec-code parity, diagnostics coverage, blocker drift, and tracker/doc sync; publish receipt and stabilization entry decision. | `PLANNING_REGISTER.md` (§1A + this table), subsystem guides, active lane issue hubs | Timestamped checkpoint receipt exists and explicitly authorizes stabilization promotion for the next cycle. |
| 9 | **`lane:stabilization` (`#88`)** | Most effective after system completion + audit, where hardening can target integrated behavior instead of moving partial substrates. | Execute bounded hardening pass over integrated camera/input/focus/render interaction; close remaining repro register items with tests/diagnostics receipts. | `render/mod.rs`, `app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs`, `shell/desktop/workbench/tile_compositor.rs`, `shell/desktop/workbench/*`; `SUBSYSTEM_DIAGNOSTICS.md` | Repros are tracked, fixed, and covered by targeted tests/receipts; normal graph interaction works reliably in default and split-pane contexts. |
| 10 | **`lane:test-infra` (`#97`) / `lane:knowledge-capture` (`#98`) split cadence** | Keeps both execution safety and product semantics moving while stabilization is bounded. | Alternate short slices: (`#97`) scenario/test harness scaling and CI split, (`#98`) import/clip -> tag/UDC visible path. | `2026-02-26_test_infrastructure_improvement_plan.md`, `2026-02-23_udc_semantic_tagging_plan.md`, `2026-02-24_layout_behaviors_plan.md` | Test infra debt no longer blocks refactors and at least one knowledge-capture E2E path ships with coverage. |

### Core vs Incubation Note

- `lane:verse-intelligence` is intentionally tracked in `1A` as an incubation lane (parallel / non-blocking for Graphshell core completion).
- It should still get a hub issue + child issues soon, but not ahead of core completion lanes and the audit->stabilization sequence.

---

## 1D. Prospective Lane Catalog (Comprehensive)

This is the complete lane catalog for near/mid-term planning. `§1C` is the prioritized execution board; this section is the fuller universe so good ideas do not disappear between audits.

### A. Active / Immediate Lanes (Execution Now)

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:stabilization` (`#88`) | User-visible regressions, control responsiveness, focus affordances, camera/lasso correctness | Active as bounded hardening pass after inter-plan audit (or short critical hotfix exception) | `render/mod.rs`, `graph_app.rs`, `shell/desktop/ui/gui.rs`, `input/mod.rs`, `tile_compositor.rs` | Default posture is post-completion hardening; critical use-blocking regressions may preempt briefly. |
| `lane:roadmap` | Merge-safe docs/planning follow-on: `#19` (`TwoD↔ThreeD` `ViewDimension` hotswitch, blocked) plus pre-wgpu spec conflict closure slices | Active merge-safe default (docs-only execution) | `PLANNING_REGISTER.md`, `2026-03-03_spec_conflict_resolution_register.md`, `canvas/2026-02-27_roadmap_lane_19_readiness_plan.md` | Use this lane for merge-safe canonical doc work, including P1–P4 spec conflict closure, without touching runtime hotspots. |
| `lane:control-ui-settings` (`#89`) | Command surfaces + settings IA/surface execution | Active planning / queued (high priority) | `2026-02-24_control_ui_ux_plan.md`, `2026-02-20_settings_architecture_plan.md`, `render/command_palette.rs` | User report now provides concrete issue-ready slices (palette/context unification, theme toggle, omnibar/radial polish). |
| `lane:embedder-debt` (`#90`) | Servoshell inheritance retirement / host-UI decomposition | Prospective (high priority, active child slices) | `aspect_render/aspect_render/2026-02-20_embedder_decomposition_plan.md`, `shell/desktop/ui/gui.rs`, `shell/desktop/ui/gui_frame.rs`, `host/*` | Includes compositor callback pass contract and legacy webview context-menu/new-tab path retirement/bridging. |
| `lane:runtime-followon` (`#91`) | SignalBus consumer adoption + follow-on signal policy cleanup | Active follow-on (core infrastructure landed) | `SYSTEM_REGISTER.md`, `2026-03-08_sector_h_signal_infrastructure_plan.md` | Sector H is implemented; remaining work is downstream consumption, not missing bus infrastructure. |

### B. Core Platform / Architecture Completion Lanes

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:viewer-platform` (`#92`) | Universal content execution + real embedded viewers + Wry foundation | Prospective | `viewer/2026-02-24_universal_content_model_plan.md`, `2026-02-23_wry_integration_strategy.md`, `tile_behavior.rs`, `mods/native/verso/mod.rs`, `Cargo.toml` | Closes spec/code drift around viewer support and `viewer:wry`. |
| `lane:diagnostics` (`#94`) | AnalyzerRegistry, in-pane TestHarness, invariant/health surfacing | Prospective | `SUBSYSTEM_DIAGNOSTICS.md`, diagnostics runtime/pane code | Leverage multiplier for all other lanes. |
| `lane:subsystem-hardening` (`#96`) | Storage/history/security closure slices | Prospective | `SUBSYSTEM_STORAGE.md`, `SUBSYSTEM_HISTORY.md`, `SUBSYSTEM_SECURITY.md` | Can be split into sublanes once issue volume grows. |
| `lane:test-infra` (`#97`) | T1/T2 scaling, `test-utils`, scenario binary, CI split | Prospective | `2026-02-26_test_infrastructure_improvement_plan.md`, `mod_loader.rs`, `Cargo.toml` | Prefer infra-only PRs to reduce merge risk. |
| `lane:accessibility` (`#95`) | WebView bridge closure + Graph Reader + inspector + focus/nav contracts | Prospective | `SUBSYSTEM_ACCESSIBILITY.md`, `tile_behavior.rs`, `shell/desktop/ui/gui.rs` | Includes placeholder inspector replacement. |

### C. UX / Interaction / Graph Capability Lanes

| Lane | Scope | Status | Primary Docs / Hotspots | Notes |
| --- | --- | --- | --- | --- |
| `lane:knowledge-capture` (`#98`) | UDC organization, import, clipping, badges/tags, visible graph effects | Prospective | `2026-02-23_udc_semantic_tagging_plan.md`, `2026-02-24_layout_behaviors_plan.md`, `2026-02-20_node_badge_and_tagging_plan.md`, `2026-02-11_*_plan.md` | Canonical “capture + classify + surface” lane. |
| `lane:layout-semantics` | Layout injection hook, frame-affinity organizational behavior prerequisites/execution (legacy alias: Magnetic Zones); workbench/workspace/tile semantic distinctions | Prospective (design pressure increasing) | `2026-02-24_layout_behaviors_plan.md`, `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` | User report surfaced unresolved tile vs workspace semantics and need for a root workbench overview UX. |
| `lane:performance-physics` | Culling, LOD, physics responsiveness/reheat, policy tuning | Partial / follow-on | `2026-02-24_performance_tuning_plan.md`, `2026-02-24_physics_engine_extensibility_plan.md` | Some slices landed; keep as follow-on lane for deeper performance + policy work. |
| `lane:command-surface-parity` | Omnibar/palette/radial/menu trigger parity and command discoverability | Prospective | `GRAPHSHELL_AS_BROWSER.md`, control UI UX docs, `render/command_palette.rs` | Can remain under `control-ui-settings` unless scope expands. |
| `lane:graph-ux-polish` | Multi-select, semantic tab titles, small high-leverage graph interactions | Prospective / quick-slice feeder | `2026-02-18_graph_ux_research_report.md`, `2026-02-23_graph_interaction_consistency_plan.md` | Good feeder lane for low-risk UX improvements between bigger slices. |

### D. Staged Feature / Roadmap Adoption Lanes (Post-Core Prereqs)

These are mostly sourced from the forgotten-concepts table and adopted strategy docs. They should be explicitly tracked as lanes once prerequisites are met.

| Lane | Scope | Trigger / Prereq | Primary Docs | Notes |
| --- | --- | --- | --- | --- |
| `lane:history-stage-f` | Temporal Navigation / Time-Travel Preview (Stage F) | Stage E history maturity + preview isolation hardening | `2026-02-20_edge_traversal_impl_plan.md`, `SUBSYSTEM_HISTORY.md` | Treat as staged backlog lane, not a quick feature. |
| `lane:presence-collaboration` | Collaborative presence (ghost cursors, follow mode, remote selection) | Verse sync + identity/presence semantics stable | `design_docs/verse_docs/implementation_strategy/2026-02-25_verse_presence_plan.md` | Crosses Graphshell + Verse; likely needs dedicated hub. |
| `lane:lens-physics` | Progressive lenses + lens/physics binding policy execution | Runtime lens resolution + distinct physics preset behavior | `2026-02-25_progressive_lens_and_physics_binding_plan.md`, interaction/physics docs | Can begin with policy wiring before full UX polish. |
| `lane:doi-fisheye` | Semantic fisheye / DOI implementation | Basic LOD + viewport culling stable | `2026-02-25_doi_fisheye_plan.md`, graph UX research | Visual ergonomics lane; pair with diagnostics/perf instrumentation. |
| `lane:ghost-nodes` | Ghost Nodes/edges after deletion (formerly `lane:visual-tombstones`) | Deletion/traversal/history UX stable | `2026-02-26_visual_tombstones_plan.md` | Adopted concept with strategy doc; candidate early roadmap lane. |
| `lane:omnibar` | Unified omnibar (URL + graph search + web search) | Command palette/input routing stabilized | `GRAPHSHELL_AS_BROWSER.md`, graph UX research | Core browser differentiator; keep distinct from palette cleanup. |
| `lane:view-dimension` | 2D↔3D hotswitch + position parity | Pane/view model + graph view state stable | `2026-02-24_physics_engine_extensibility_plan.md`, `PROJECT_DESCRIPTION.md` | Future-facing but should remain visible in planning. |
| `lane:html-export` | Interactive HTML export | Viewer/content model + snapshot/export shape defined | archived philosophy + browser docs | Strong shareability lane; non-core until model/export safety is defined. |

### E. Verse / Intelligence Incubation Lanes (Design-to-Code)

| Lane | Scope | Status | Primary Docs | Notes |
| --- | --- | --- | --- | --- |
| `lane:verse-intelligence` (`#93`) | Hub lane for self-hosted model contracts + adapters + conformance + portability + archetypes | Design-ready / issue hub open | `self_hosted_model_spec.md` | Start with schemas/contracts + runtime contract binding + diagnostics, not training. |
| `lane:intelligence-memory` | STM/LTM + engram memories + extractor/ingestor + ectoplasm interfaces | Design-ready / issue hub missing | `2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md` | May be tracked as a child lane under `lane:verse-intelligence`. |
| `lane:intelligence-privacy-boundary` | Distillation boundary between durable app state and intelligence providers | Design-ready / issue hub missing | `subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`, `self_hosted_model_spec.md` | Must land before remote-provider features that read graph/history/clip state. |
| `lane:agent-distillery` | Agent-owned WALs, experience units, and the distillery pipeline from graph/agent traces into typed artifacts and engrams | Design incubation / issue hub missing | `2026-03-09_agent_wal_and_distillery_architecture_plan.md`, `2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md` | Depends on `lane:intelligence-privacy-boundary` and Sector G agent supervision surfaces. |
| `lane:lineage-dag` | Shared lineage DAG, stream commitments, and traversal-policy primitives for Engrams and FLora checkpoints | Design-ready / issue hub missing | `lineage_dag_spec.md`, `engram_spec.md`, `flora_submission_checkpoint_spec.md` | Use one lineage structure across engram tuning and inter-verse federation rather than separate ancestry models. |
| `lane:model-index-verse` | Requirements/benchmarks/community reports evidence registry for model selection/diets | Conceptual / partially documented | model slots plan (Model Index sections), local intelligence research | Evidence substrate for archetypes and conformance decisions. |
| `lane:adapter-portability` | LoRA extraction/import/export, portability classes, reverse-LoRA tooling integration | Design-ready / issue hub missing | model slots plan (`TransferProfile`, portability classes) | Likely late-phase child lane after schemas + evals exist. |
| `lane:archetypes` | Archetype presets, nudging, “Design Your Archetype”, derivation from existing models | Design-ready / issue hub missing | model slots plan (`ArchetypeProfile`) | Keep modular and non-blocking to core Graphshell. |

### F. Maintenance / Quality Governance Lanes (Keep Explicit)

| Lane | Scope | Status | Notes |
| --- | --- | --- | --- |
| `lane:spec-code-parity` (`#99`) | Reconcile docs/spec claims vs code reality (viewers, Wry, placeholders, status flags) | Ongoing | Use this when mismatches pile up; often docs-only, sometimes tiny code fixes. |
| `lane:queue-hygiene` | Issue state reconciliation, closure receipts, register refreshes | Ad hoc (recently exercised) | Keep rare and bounded; should support execution, not replace it. |
| `lane:docs-canon` | Terminology/architecture canon cleanup across `TERMINOLOGY.md`, `SYSTEM_REGISTER.md`, subsystem guides | Ad hoc | Use when implementation changes invalidate routing/authority language. |

### Catalog Usage Rules

- Add new lanes here before or at the same time they appear in `§1A` sequencing.
- Promote a lane into `§1C` only when it has a clear execution window, owner hotspot set, and issue stack (or an explicit issue-creation slice).
- Do not remove future-facing lanes just because they are blocked; mark the blocker and trigger instead.

### G. NostrCore Tier 1 Baseline (Issue Seeding)

This issue-seeding block operationalizes the `NostrCore` native baseline defined in:

- `system/2026-03-05_nostr_mod_system.md`
- `system/register/nostr_core_registry_spec.md`

Positioning note:

- Treat this as a cross-lane stack, not a new standalone lane.
- Primary lane anchors: `lane:runtime-followon` (`#91`), `lane:subsystem-hardening` (`#96`), `lane:viewer-platform` (`#92`).

#### Issue: NostrCore Native Provider Registration + Manifest Gate

**Title**: Add first-party `NostrCore` native mod manifest and capability-provider registration

**Scope**:
1. Register `graphshell:nostr-core` native provider manifest (`provides`/`requires`) per `nostr_core_registry_spec.md`.
2. Wire capability keys into mod lifecycle validation (`namespace:name` checks + dependency resolution).
3. Emit explicit diagnostics on manifest/capability gate failures.

**Done gate**:
- `NostrCore` manifest is discoverable in runtime mod listings.
- Capability declarations validate and fail deterministically when malformed/missing.
- Diagnostics channel output includes manifest gate failure reasons.

**Lane**: `lane:runtime-followon` (`#91`)
**Labels**: `architecture`, `mods`, `nostr`, `lane:runtime-followon`

#### Issue: Nostr Signing Boundary (No-Raw-Secret Contract)

**Title**: Implement operation-level Nostr signing service with no raw key exposure

**Scope**:
1. Add `sign_event` service boundary under identity/security ownership.
2. Support local signer backend and NIP-46 delegated signer path.
3. Enforce explicit denial for key-export or raw-secret access attempts.
4. Add contract tests proving no raw-secret retrieval path exists.

**Done gate**:
- Authorized callers can request signatures; unauthorized callers are denied.
- No API path returns raw secret bytes.
- Contract tests cover allowed signing and denied key-export behavior.

**Lane**: `lane:subsystem-hardening` (`#96`)
**Labels**: `security`, `identity`, `nostr`, `lane:subsystem-hardening`

#### Issue: Host-Owned Relay Pool Capability Service

**Title**: Add shared Nostr relay subscribe/publish service with capability gates

**Scope**:
1. Implement host-owned relay pool service (`subscribe`, `unsubscribe`, `publish`).
2. Enforce per-caller capability checks and rate/usage policy.
3. Add diagnostics for publish/subscription failures and denied operations.

**Done gate**:
- Callers with granted capability can subscribe/publish through one shared host pool.
- Direct/unmanaged socket access path is absent for mods.
- Failure/denial channels are visible in diagnostics and testable.

**Lane**: `lane:subsystem-hardening` (`#96`)
**Labels**: `network`, `security`, `nostr`, `lane:subsystem-hardening`

#### Issue: Nostr Event -> Graph Intent Adapter Baseline

**Title**: Add baseline Nostr event-to-intent adapters for graph-native workflows

**Scope**:
1. Define adapter mappings for initial event kinds used by graph workflows (note/url/highlight/profile baseline).
2. Route adapters through existing reducer/workbench intent authorities.
3. Add payload validation and rejection diagnostics.

**Done gate**:
- At least one end-to-end mapping path is active and tested.
- Rejected payloads are diagnosable with explicit reason channels.
- No direct graph mutation path bypasses intent authorities.

**Lane**: `lane:runtime-followon` (`#91`)
**Labels**: `architecture`, `graph`, `nostr`, `lane:runtime-followon`

#### Issue: NIP-07 Bridge Capability Checks for App Nodes

**Title**: Add host-controlled NIP-07 bridge with per-origin capability enforcement

**Scope**:
1. Implement `window.nostr` bridge entrypoint for WebView app-node mode.
2. Gate NIP-07 methods on declared/granted Nostr capabilities.
3. Add per-origin denial diagnostics and permission-memory hooks.

**Done gate**:
- Eligible app nodes can execute approved NIP-07 methods.
- Non-granted methods are denied deterministically and logged.
- Bridge behavior is covered by at least one scenario-level test path.

**Lane**: `lane:viewer-platform` (`#92`)
**Labels**: `viewer`, `security`, `nostr`, `lane:viewer-platform`

---

## 2. Top 10 Forgotten Concepts for Adoption (Vision / Research Ideas Missing from Active Queue)

These are not "do now" items. They are concepts that should be explicitly adopted into planning so they do not disappear between migration and feature work. ✅ marks concepts that now have a strategy doc and a lane; they remain here as visibility anchors until their lane is promoted to `§1C`.

| Rank | Forgotten Concept | Adoption Value | Source Docs | Adoption Trigger |
| --- | --- | --- | --- | --- |
| 1 | ✅ **Ghost Nodes (nodes/edges preserved after deletion)** | Preserves structural memory and reduces disorientation after destructive edits. Previously "Visual Tombstones"; canonical term is now Ghost Node. Code-level state: `NodeLifecycle::Tombstone`. | `2026-02-24_visual_tombstones_research.md`, `2026-02-26_visual_tombstones_plan.md` (strategy adopted — `lane:ghost-nodes`) | After traversal/history UI and deletion UX are stable. |
| 2 | ✅ **Temporal Navigation / Time-Travel Preview** | Makes traversal history and deterministic intent log materially useful to users (not just diagnostics). | `2026-02-20_edge_traversal_impl_plan.md` Stage F (adopted as staged backlog — deferred until Stage E History Manager maturity), `GRAPHSHELL_AS_BROWSER.md`, `2026-02-18_graph_ux_research_report.md` | Stage E History Manager closure + preview-mode effect isolation hardening. Preview-mode effect isolation contract: no WAL writes, no webview/graph mutations, clean return-to-present; enforcement point: `desktop/gui_frame.rs`. Preserved non-goals: collaborative replay, undo/redo replacement, scrubber fidelity, timeline snapshot export. |
| 3 | ✅ **Collaborative Presence (ghost cursors, remote selection, follow mode)** | Turns Verse sync from data sync into shared work. | `2026-02-18_graph_ux_research_report.md` §15.2, `GRAPHSHELL_AS_BROWSER.md`, `2026-02-25_verse_presence_plan.md` (adopted — `lane:presence-collaboration`) | After Phase 5 done gates and identity/presence semantics are stable. |
| 4 | ✅ **Semantic Fisheye + DOI (focus+context without geometric distortion)** | High-value readability improvement for dense graphs; preserves mental map while surfacing relevance. | `2026-02-18_graph_ux_research_report.md` §§13.2, 14.8, 14.9, `2026-02-25_doi_fisheye_plan.md` (adopted — `lane:doi-fisheye`) | After basic LOD and viewport culling are in place. |
| 5 | **Frame-affinity organizational behavior / Group-in-a-Box / Query-to-Zone** (legacy alias: Magnetic Zones) | Adds spatial organization as a first-class workflow, not just emergent physics. | `2026-02-24_layout_behaviors_plan.md` Phase 3 (expanded with persistence scope, interaction model, and implementation sequence), `2026-02-18_graph_ux_research_report.md` §13.1 | **Prerequisites now documented** in `layout_behaviors_plan.md` §3.0–3.5. Implementation blocked on: (1) layout injection hook (Phase 2), (2) Canonical/Divergent scope settlement. Trigger: when both blockers are resolved, execute implementation sequence in §3.5. |
| 6 | **Graph Reader ("Room" + "Map" linearization) and list-view fallback** | Critical accessibility concept beyond the initial webview bridge; gives non-visual users graph comprehension. | `2026-02-24_spatial_accessibility_research.md`, `SUBSYSTEM_ACCESSIBILITY.md` §8 Phase 2 | After Phase 1 WebView Bridge lands. |
| 7 | **Unified Omnibar (URL + graph search + web search heuristics)** | Core browser differentiator; unifies navigation and retrieval. | `GRAPHSHELL_AS_BROWSER.md` §7, `2026-02-18_graph_ux_research_report.md` §15.4 | After command palette/input routing stabilization. |
| 8 | ✅ **Progressive Lenses + Lens/Physics binding policy** | Makes Lens abstraction feel native and semantic, not static presets. | `2026-02-24_interaction_and_semantic_design_schemes.md`, `2026-02-24_physics_engine_extensibility_plan.md` (lens-physics binding preference), `2026-02-25_progressive_lens_and_physics_binding_plan.md` (adopted — `lane:lens-physics`; implementation blocked on runtime prerequisites) | After Lens resolution is active runtime path and physics presets are distinct in behavior. |
| 9 | **2D↔3D Hotswitch with `ViewDimension` and position parity** | Named first-class vision feature; fits the new per-view architecture and future Rapier/3D work. | `2026-02-24_physics_engine_extensibility_plan.md`, `design_docs/PROJECT_DESCRIPTION.md` | After pane-hosted view model and `GraphViewState` are stable. |
| 10 | **Interactive HTML Export (self-contained graph artifact)** | Strong shareability and offline review workflow; distinctive output mode. | `design_docs/archive_docs/checkpoint_2026-01-29/PROJECT_PHILOSOPHY.md` (archived concept) | After viewer/content model and export-safe snapshot shape are defined. |
| 11 | **Focus Subsystem Unified Authority & Router** | Makes focus diagnostics and cross-surface handoff an explicit, testable contract instead of six distributed side-effect tracks. Currently no single authority object owns focus state. | `subsystem_focus/SUBSYSTEM_FOCUS.md` §1A (Runtime Reality Gap), `subsystem_focus/2026-03-08_unified_focus_architecture_plan.md` | After `lane:control-ui-settings` (`#89`) reaches implementation phase; depends on workbench/tile surface model stabilization. |
| 12 | **UX Semantics End-to-End Closure (Projection + Contracts + Bridge + Scenario Harness)** | Completes regression testing and accessibility mapping for all surfaces. UxTree build/publish and snapshot diff-gate are landed; full `UxProbeSet`, `UxBridge` command surface (`GetUxSnapshot`, `FindUxNode`, `InvokeUxAction`), and YAML scenario runner remain incomplete. | `subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` §0B–0C, §13–15, `subsystem_ux_semantics/2026-03-08_unified_ux_semantics_architecture_plan.md` | Track under `lane:diagnostics` (`#94`); trigger when diagnostics lane is promoted to `§1C` execution slot. |
| 13 | **Mod Lifecycle Integrity Spec (activation/sandboxing/unload contract)** | Without a canonical spec, mod activation, sandbox isolation, and unload lifecycle will drift into undocumented behavior as Phase 6+ mod work expands. Spec is explicitly deferred in the subsystem guide. | `subsystem_mods/SUBSYSTEM_MODS.md` §9 (Deferred Spec), `subsystem_mods/2026-03-08_unified_mods_architecture_plan.md` | After `mod_registry_spec.md`, `action_registry_spec.md`, and `input_registry_spec.md` are stable (listed as explicit blocking dependencies in `SUBSYSTEM_MODS.md` §9). |

---

## 3. Top 10 Quickest Improvements (Low-Effort / High-Leverage Slices)

These are intentionally scoped to small slices that can ship independently without waiting for larger architecture work.

| Rank | Quick Improvement | Why It Pays Off | Primary Source Docs |
| --- | --- | --- | --- |
| 1 | ✅ **Extract `desktop/radial_menu.rs` from `render/mod.rs`** | Done — `render/radial_menu.rs` exists as a standalone module. | `2026-02-24_control_ui_ux_plan.md` |
| 2 | ✅ **Extract `desktop/command_palette.rs` from `render/mod.rs`** | Done — `render/command_palette.rs` exists as a standalone module. | `2026-02-24_control_ui_ux_plan.md` |
| 3 | **Reheat physics on `AddNode` / `AddEdge`** | Fixes "dead graph" feel immediately when physics is paused. | `2026-02-24_layout_behaviors_plan.md` §1.1, `2026-02-18_graph_ux_research_report.md` §5.3 |
| 4 | **Spawn new nodes near semantic parent (parent + jitter)** | Improves mental-map preservation and reduces convergence churn. `KnowledgeRegistry::suggest_placement_anchor()` now exists; the remaining gap is a creation path that carries semantic tags at spawn time. | `2026-02-24_layout_behaviors_plan.md` §1.2, `2026-02-18_graph_ux_research_report.md` §§2.1, 2.6 |
| 5 | **Fix `WebViewUrlChanged` prior-URL ordering in traversal append path** | Prevents incorrect traversal records and future temporal-navigation corruption. | `2026-02-20_edge_traversal_impl_plan.md`, `2026-02-20_edge_traversal_model_research.md` |
| 6 | **Wire `Ctrl+Click` multi-select in graph pane** | Tiny code slice with immediate UX gain; unlocks group operations expectations. | `2026-02-18_graph_ux_research_report.md` §§1.3, 6.3 |
| 7 | **Add semantic container tab titles (`Split ↔`, `Split ↕`, `Tab Group`, `Grid`)** | Converts "looks broken" tile labels into teachable architecture UI. | `2026-02-23_graph_interaction_consistency_plan.md` Phase 4 |
| 8 | **Add zoom-adaptive label LOD thresholds (hide/domain/full)** | Immediate clarity and performance win at low zoom, low implementation risk. | `2026-02-24_performance_tuning_plan.md` Phase 2.1, `2026-02-18_graph_ux_research_report.md` §7.3 |
| 9 | ✅ **Add `ChannelSeverity` to diagnostics channel descriptors** | Done — `ChannelSeverity` is present on diagnostic channel descriptors in the diagnostics registry. | `2026-02-24_diagnostics_research.md` §4.6, §7 |
| 10 | **Add/confirm `CanvasRegistry` culling + LOD policy toggles** | Minimal schema/policy work that unblocks performance slices and keeps behavior policy-driven. | `2026-02-24_performance_tuning_plan.md`, `2026-02-22_registry_layer_plan.md` |
| 11 | ✅ **Wire `PresentationDomainRegistry` overlay affordance policy per `TileRenderMode`** | Done — focus/hover ring dispatch and degraded receipt styling now resolve through runtime-owned presentation profiles instead of hardcoded compositor colors. | `PLANNING_REGISTER.md §0.3.4`, `registries/domain/presentation/mod.rs`, `tile_compositor.rs` |
| 12 | **Add Node Audit Log event emission at mutation points** | Append-only event journal for node lifecycle and metadata changes; enables compliance/debugging audit trail without full replay surfaces. Spec is complete; code is a deferred stub only. | `subsystem_history/SUBSYSTEM_HISTORY.md` §2.3, `system/register/node_audit_log_spec.md` |
| 13 | **Implement Distillation Boundary Enforcement shim (intelligence privacy gate)** | Pre-emptive read-gate for all future model/intelligence-facing state access; prevents WAL/history/graph reads from bypassing the redaction/filtering layer before any provider exists. Spec is written; no code exists yet. | `subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`, `subsystem_security/SUBSYSTEM_SECURITY.md` |

### Quick Win Notes

- Items 1-2 are done (extraction already landed).
- Items 3-5 are correctness/feel fixes and should not wait for full layout/traversal phases.
- Item 9 is done (ChannelSeverity landed). Item 10 remains an infrastructure improvement target.
- Item 11 is now done (2026-03-10): overlay affordance policy is runtime-owned through `PresentationDomainRegistry`.
- Items 12-13 remain architecture/infra gaps with written specs and no code yet.

### Sector A Reality Note (2026-03-10)

Sector A is now complete in the repo at the runtime authority level:
- `ProtocolRegistry` now drives URI-aware MIME inference and cancellable HTTP content-type probes.
- `ViewerRegistry` now exposes capability descriptions and the canonical fallback floor.
- The existing layout-domain `ViewerSurfaceRegistry` is now the real viewer-surface authority,
  resolving viewer-specific surface profiles for web, document, embedded, and native-overlay paths.
- `LensRegistry` is no longer just an ID lookup; it now supports content-aware resolution,
  composition, and a built-in semantic-overlay lens for tagged semantic content.

### Sector D Reality Note (2026-03-10)

Sector D is now complete in the repo:
- `PhysicsProfileRegistry`, `CanvasRegistry`, `LayoutRegistry`, `LayoutDomainRegistry`, and
  `PresentationDomainRegistry` all exist in live runtime paths.
- Layout execution still uses `egui_graphs` as the widget substrate, but layout authority is now
  registry-owned through the extracted `app/graph_layout.rs` adapter layer and runtime
  resolution/apply ordering.

### Sector F Reality Note (2026-03-10)

Sector F is now complete in the repo at the registry/runtime level:
- `DiagnosticsRegistry` now carries schema/retention/sampling contract data and warns on orphan emits.
- `KnowledgeRegistry` is no longer a reconcile shim; it owns bundled UDC seed data, validation,
  query APIs, semantic-distance helpers, and semantic-index lifecycle signaling.
- `IndexRegistry` now exists as a runtime authority and backs the omnibox submit/action path with
  `index:local` + `index:history` + `index:knowledge` fanout.

Residual non-blockers that should stay explicit:
- the omnibar suggestion dropdown still uses a legacy candidate pipeline instead of `IndexRegistry`
- semantic placement-anchor consumption still needs a node-spawn caller that already knows semantic tags
- `index:timeline` remains a future history-coupled provider stub rather than a live index source

### Sector G Reality Note (2026-03-10)

Sector G is now implemented at the runtime authority level for `AgentRegistry` and `ThemeRegistry`:
- `ThemeRegistry` now exists as a runtime-owned authority with built-in default/light/dark/high-contrast
  themes, reducer-owned theme selection, persistence round-trip, and tokenized command/radial UI paths.
- `AgentRegistry` now exists as a real runtime registry, `ControlPanel` supervises spawned agent
  tasks, and the built-in `agent:tag_suggester` consumes Register navigation signals and emits the
  display-only `GraphIntent::SuggestNodeTags` path.
- GUI-owned runtime state and `phase3_*` helper surfaces now share one global `RegistryRuntime`
  authority instead of constructing competing runtime instances.

Residual blockers that keep Sector G and the registry master plan open:
- `WasmModHost` / intent-bridge support is still absent; `ModType::Wasm` is only a manifest/model
  marker today.
- `GraphIntent::ModDeactivated` still does not exist as the reducer-carried unload receipt from the
  original Sector G plan.
- Startup OS-theme detection and mod-provided theme activation remain follow-on work.
- Theme token migration is substantial but not absolute across all `render/` literals.

### Registry Plan Archive Note (2026-03-10)

Do not archive `2026-03-08_registry_development_plan.md` yet. The register body is materially
further along, but `RendererRegistry` (Sector B) and the remaining Sector G WASM/mod-theme
follow-ons still keep the master plan active.

---

## 4. Historical Tail (Archived)

Historical execution sequences, legacy closure backlog details, and preserved-numbering reference payload were moved out of the active register to keep this file operational as a control-plane document.

Archive receipt:
- `design_docs/archive_docs/checkpoint_2026-02-27/2026-02-27_planning_register_historical_tail_archive_receipt.md`

Canonical historical sources:
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_backlog_ticket_stubs.md`
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_copilot_implementation_guides.md`
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_planning_register_lane_sequence_receipt.md`
- `design_docs/archive_docs/checkpoint_2026-02-26/2026-02-26_planning_register_queue_execution_audit_receipt.md`

Active usage rule:
- Use `§1A`, `§1B`, `§1C`, and `§1D` in this file for current sequencing and execution decisions.
- Use archive docs for historical detail, superseded plans, and long-form receipts.

## 5. Suggested Tracker Labels (Operational Defaults)

- Priority tasks: `priority/top10`, `architecture`, `registry`, `viewer`, `ui`, `performance`, `a11y`
- Roadmap adoption: `concept/adoption`, `research-followup`, `future-roadmap`
- Quick wins: `quick-win`, `low-risk`, `refactor`, `ux-polish`, `diag`

## 6. Import Notes (Short Form)

- Keep `P#`, `F#`, `Q#` prefixes aligned between docs and tracker.
- Prefer one issue per mergeable slice in hotspot files.
- If a ticket body exceeds practical review size, move extended detail into a timestamped archive receipt and link it.
