# Viewer Presentation and Fallback Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `../2026-02-28_ux_contract_register.md`
- `../2026-02-27_ux_baseline_done_definition.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../research/2026-02-27_viewer_state_matrix.md`
- `../2026-02-26_composited_viewer_pass_contract.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`

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

**What this domain is for**

- Choose the correct viewer identity for the current content or internal route.

**Core rules**

- Viewer selection uses Graphshell-owned policy, not ad hoc widget decisions.
- Stable baseline viewers must resolve deterministically.
- Internal routes such as settings resolve through special app-owned paths.

**Who owns it**

- Graphshell viewer registry and routing authority.

**State transitions**

- Content open resolves a viewer ID.
- The resolved viewer ID maps to a render mode and concrete presentation path.

**Visual feedback**

- The selected viewer class should be inferable from the pane's behavior and affordances.

**Fallback / degraded behavior**

- If the preferred viewer is unavailable, the fallback choice must be deterministic and visible.

### 4.2 Render-Mode Presentation Rules

**What this domain is for**

- Present the chosen viewer in a way that matches the active render mode.

**Core rules**

- `CompositedTexture` tiles may render overlays over content.
- `NativeOverlay` tiles move Graphshell affordances into host chrome or gutters when direct overlay is constrained.
- `EmbeddedEgui` tiles use normal embedded affordance paths.
- `Placeholder` tiles are explicit non-content surfaces, not silent failures.

**Who owns it**

- Graphshell compositor and viewer policy.
- Backend renderers implement the path chosen by policy.

**State transitions**

- Pane lifecycle changes may promote or demote render readiness while preserving viewer identity.

**Visual feedback**

- The user must be able to distinguish content, overlay, placeholder, and degraded states.

**Fallback / degraded behavior**

- Native-overlay limitations must be documented in-surface.
- Placeholder state must explain why content is not currently rendered.

### 4.3 Loading, Partial, and Deferred States

**What this domain is for**

- Make incomplete viewer paths explicit.

**Core rules**

- Viewer state should be classifiable as `operational`, `partial`, or `deferred`.
- Declared but non-operational viewers must not be presented as complete.

**Who owns it**

- Graphshell viewer-state authority and diagnostics policy.

**State transitions**

- A viewer may move from loading to operational.
- A viewer may remain partial if some but not all capabilities are available.
- A deferred viewer remains non-baseline until explicitly implemented.

**Visual feedback**

- Loading, partial, and deferred states must be labeled distinctly.

**Fallback / degraded behavior**

- Partial behavior must describe what is missing.
- Deferred behavior must surface as intentional absence, not as a broken pane.

### 4.4 Tool Surfaces vs Node Surfaces

**What this domain is for**

- Distinguish app tool pages from content-node viewers.

**Core rules**

- Tool surfaces (settings, history, diagnostics) are real app surfaces with viewer-like presentation, but they do not erase Graphshell's authority over pane lifecycle.
- Internal routes should remain composable with the workbench model.

**Who owns it**

- Graphshell routing and tool-surface controllers.

**State transitions**

- Opening a tool route creates or focuses a pane destination with the correct tool surface.

**Visual feedback**

- Tool surfaces must visibly read as app-owned surfaces, not anonymous placeholders.

**Fallback / degraded behavior**

- If a tool page cannot render in its preferred form, Graphshell must fall back to a usable host-owned surface.

### 4.5 Diagnostics, Accessibility, and Performance

**What this domain is for**

- Keep viewer behavior observable and trustworthy.

**Diagnostics**

- Viewer selection, render-mode choice, fallback, and degradation reasons must be observable.

**Accessibility**

- Viewer state must be explainable to non-visual users.
- Placeholder and blocked states must have accessible labels.

**Performance**

- Degradation may reduce fidelity or warm/cold status, but it must not hide current viewer identity or reason.

---

## 5. Planned Extensions

- dedicated embedded PDF and CSV viewer paths,
- richer viewer-state badges and diagnostics summaries,
- stronger thumbnail and prewarm strategies,
- more explicit viewer override controls.

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


