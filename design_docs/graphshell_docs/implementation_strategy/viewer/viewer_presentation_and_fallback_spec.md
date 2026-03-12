# Viewer Presentation and Fallback Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:

- `../2026-02-28_ux_contract_register.md`
- `../subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../research/2026-02-27_viewer_state_matrix.md`
- `2026-02-26_composited_viewer_pass_contract.md`
- `wry_integration_spec.md`
- `node_lifecycle_and_runtime_reconcile_spec.md`
- `webview_lifecycle_and_crash_recovery_spec.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `../../TERMINOLOGY.md`

**Adopted standards** (see [standards report](../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7):

- **WCAG 2.2 Level AA** — SC 1.3.1 (info/structure accessible), SC 4.1.2 (name/role/value for viewer states), SC 1.4.1 (placeholder/degraded states not color-only)
- **OSGi R8** — viewer capability registration and selection vocabulary
- **OpenTelemetry Semantic Conventions** — viewer selection, fallback, and degradation diagnostics

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- file tree = graph-backed hierarchical projection.
- workbench = arrangement boundary.

This spec defines viewer-surface presentation semantics and must not redefine graph-truth or workbench-arrangement ownership.

## Contract template (inherits UX Contract Register §2A)

Normative viewer contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- File tree is not content truth authority.
- Physics presets are not camera modes.

---

## 1. Purpose and Scope

This spec defines how Graphshell chooses, presents, degrades, and explains viewer surfaces.

It explains:

- what viewer surfaces are for,
- what each viewer class means semantically,
- who owns viewer selection and render-mode policy,
- what state transitions viewer routing implies,
- what visual feedback must accompany loading, fallback, and degraded states,
- what fallback behavior must happen when the preferred viewer is unavailable,
- which viewer capabilities are core, planned, and exploratory.

---

## 2. Canonical Viewer Model

### 2.1 Canonical render modes

Graphshell currently reasons about these render modes:

1. **CompositedTexture**
2. **NativeOverlay**
3. **EmbeddedEgui**
4. **Placeholder**

### 2.2 Baseline viewer classes

The current baseline viewer set is:

- `viewer:webview`
- `viewer:plaintext`
- `viewer:markdown`

Special route:

- `viewer:settings` resolves to internal settings or tool-surface behavior rather than a generic content viewer.

### 2.3 Ownership model

- Graphshell owns viewer selection, render-mode policy, fallback order, and degraded-state meaning.
- The framework and runtime backends only implement the chosen rendering path.

---

## 3. Canonical Interaction Model

### 3.1 Viewer categories

1. **Select**
   - choose the best viewer for content or route
2. **Present**
   - render the chosen surface in the correct pane form
3. **Degrade**
   - reduce capability while preserving user understanding
4. **Fallback**
   - switch to a safe substitute when the preferred path cannot run
5. **Explain**
   - make the current viewer state legible to the user

### 3.2 Canonical guarantees

- viewer selection is app-owned and deterministic,
- users can tell what kind of surface they are looking at,
- degraded or fallback states are explicit,
- declared viewer IDs must not masquerade as fully operational when they are partial,
- render-mode policy must not silently change semantic content ownership.

---

## 4. Normative Core

### 4.1 Viewer Selection Policy

Choose the correct viewer identity for the current content or internal route.

**Core rules**: Viewer selection uses Graphshell-owned policy, not ad hoc widget decisions. Stable baseline viewers must resolve deterministically. Internal routes such as settings resolve through special app-owned paths.

**Owner**: Graphshell viewer registry and routing authority.

**State transitions**: Content open resolves a viewer ID. The resolved viewer ID maps to a render mode and concrete presentation path.

**Visual feedback**: The selected viewer class should be inferable from the pane's behavior and affordances.

**Fallback**: If the preferred viewer is unavailable, the fallback choice must be deterministic and visible.

### 4.2 Render-Mode Presentation Rules

Present the chosen viewer in a way that matches the active render mode.

**Core rules**: `CompositedTexture` tiles may render overlays over content. `NativeOverlay` tiles move Graphshell affordances into host chrome or gutters when direct overlay is constrained. `EmbeddedEgui` tiles use normal embedded affordance paths. `Placeholder` tiles are explicit non-content surfaces, not silent failures.

**Overlay affordance policy by render mode**:

| `TileRenderMode` | Focus/Hover affordance policy | Notes |
|---|---|---|
| `CompositedTexture` | Rect-stroke overlay rendered after content pass | Full overlay ring allowed over composited content |
| `NativeOverlay` | Chrome-only markers rendered in host chrome/border/gutter regions | Do not draw full-rect overlay inside native content region |
| `EmbeddedEgui` | Standard embedded rect-stroke overlay path | Uses normal embedded paint path |
| `Placeholder` | Standard embedded rect-stroke overlay path | Placeholder remains explicit, non-silent state |

**Owner**: Graphshell compositor and viewer policy. Backend renderers implement the path chosen by policy.

**State transitions**: Pane lifecycle changes may promote or demote render readiness while preserving viewer identity.

**Visual feedback**: The user must be able to distinguish content, overlay, placeholder, and degraded states.

**Fallback**: Native-overlay limitations must be documented in-surface. Placeholder state must explain why content is not currently rendered. Degraded/placeholder state must include a user-visible reason and a recovery affordance (e.g. switch viewer, retry/reactivate, or wait for cooldown). Degradation/fallback transitions should emit diagnostics receipts so parity can be audited over time.

### 4.3 Loading, Partial, and Deferred States

Make incomplete viewer paths explicit.

**Core rules**: Viewer state should be classifiable as `operational`, `partial`, or `deferred`. Declared but non-operational viewers must not be presented as complete.

**Owner**: Graphshell viewer-state authority and diagnostics policy.

**State transitions**: A viewer may move from loading to operational. A viewer may remain partial if some but not all capabilities are available. A deferred viewer remains non-baseline until explicitly implemented.

**Visual feedback**: Loading, partial, and deferred states must be labeled distinctly.

**Fallback**: Partial behavior must describe what is missing. Deferred behavior must surface as intentional absence, not as a broken pane.

### 4.4 Tool Surfaces vs Node Surfaces

Distinguish app tool pages from content-node viewers.

**Core rules**: Tool surfaces (settings, history, diagnostics) are real app surfaces with viewer-like presentation, but they do not erase Graphshell's authority over pane lifecycle. Internal routes should remain composable with the workbench model.

**Owner**: Graphshell routing and tool-surface controllers.

**State transitions**: Opening a tool route creates or focuses a pane destination with the correct tool surface.

**Visual feedback**: Tool surfaces must visibly read as app-owned surfaces, not anonymous placeholders.

**Fallback**: If a tool page cannot render in its preferred form, Graphshell must fall back to a usable host-owned surface.

### 4.5 Diagnostics, Accessibility, and Performance

Keep viewer behavior observable and trustworthy.

**Diagnostics**: Viewer selection, render-mode choice, fallback, and degradation reasons must be observable.

**Accessibility**: Viewer state must be explainable to non-visual users. Placeholder and blocked states must have accessible labels.

**Performance**: Degradation may reduce fidelity or warm/cold status, but it must not hide current viewer identity or reason.

---

## 5. Planned Extensions

- dedicated embedded PDF and CSV viewer paths,
- richer viewer-state badges and diagnostics summaries,
- stronger thumbnail and prewarm strategies,
- more explicit viewer override controls,
- per-domain Viewer settings page: default viewer overrides, placeholder explanation verbosity, prewarm strategy — exposed via the **General** settings category in `aspect_control/settings_and_control_surfaces_spec.md §4.2`.

---

## 6. Prospective Capabilities

- custom canvas-backed rich content viewers,
- viewer hot-swap previews,
- multi-view synchronized viewer comparisons,
- AI-assisted viewer recommendation overlays.

---

## 7. Acceptance Criteria

1. Viewer selection is deterministic and app-owned.
2. Baseline viewers are explicitly defined and distinguished from partial/deferred viewers.
3. Render-mode behavior is explicit for composited, native-overlay, embedded, and placeholder paths.
4. Fallback and degraded states are visible and explained.
5. Tool surfaces remain app-owned and composable with pane semantics.
6. Viewer state is diagnosable and accessible.
