# Accessibility Interaction and Capability Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_ACCESSIBILITY.md`
- `../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical interaction and capability contract for the
**Accessibility subsystem**.

It governs:

- accessibility tree integrity
- focus and region accessibility behavior
- action routing through accessibility surfaces
- capability declarations for graph, workbench, and viewer surfaces
- degraded accessibility behavior

---

## 2. Canonical Capability Model

Accessibility in Graphshell is defined as a capability contract, not a best-effort add-on.

Every participating surface must declare:

- tree contribution capability
- focus synchronization capability
- action routing capability
- keyboard-navigation capability
- status/error announcement capability for user-visible outcomes
- degradation mode

The owning subsystem may render partial capability, but it must declare that state explicitly.

Shared-surface note:

- This applies equally to graph canvas, workbench chrome, Navigator rows,
  settings pages/rails, and diagnostics panes.
- Shell command surfaces (`CommandBar`, omnibar, command-palette entry points)
  must use the same capability/degradation model rather than relying on
  implicit widget behavior.
- Shared UI surfaces should reuse the same capability/degradation model instead
  of inventing bespoke accessibility assumptions.

---

## 3. Normative Core

### 3.1 Tree Integrity

- Semantic node identity must remain stable across non-semantic refreshes.
- No orphan accessibility subtree may be emitted.
- Closed or stale surfaces must not inject active nodes.

### 3.2 Focus and Navigation

- Focus preservation must be deterministic when a semantic target still exists.
- If a target disappears, fallback focus must follow a documented policy.
- Top-level region cycling must be deterministic and complete.
- Shared top-level surfaces such as graph-scoped and workbench-scoped
  Navigator hosts, and settings rails participate in the same deterministic
  region/focus policy.
- Omnibar capture exit and command-palette dismiss must expose the same return-path
  policy to accessibility consumers that Shell and UX Semantics use for diagnostics
  and trace output.

### 3.3 Action Routing

- Accessibility actions must route to the owning subsystem.
- No cross-surface misrouting is allowed.
- Unsupported actions must fail explicitly.

### 3.4 Degradation

- Missing capability must be observable and user-visible.
- Degradation must never be silent.
- Core app navigation must remain accessible to the maximum supported extent.

---

## 4. Planned Extensions

- richer Graph Reader support
- stronger viewer-surface capability declarations
- sonification integration
- more complete mod-surface accessibility declarations

---

## 5. Prospective Capabilities

- accessibility-specific graph navigation modes
- alternate non-visual graph exploration models
- accessibility health scoring by subsystem

---

## 6. Acceptance Criteria

- Tree integrity invariants are encoded as tests or explicit diagnostics.
- Core regions expose deterministic focus behavior.
- Capability declarations exist for core graph/workbench/viewer surfaces.
- Shell command surfaces declare capability, focus-return, and status-announcement behavior explicitly.
- Degraded accessibility states are explicit and observable.

