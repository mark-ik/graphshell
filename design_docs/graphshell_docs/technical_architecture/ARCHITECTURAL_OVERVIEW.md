# Graphshell Architectural Overview

**Last Updated**: March 6, 2026
**Status**: Thin summary doc
**Purpose**: High-level orientation only. Canonical model details live in subsystem and implementation-strategy specs.

---

## 1. What Graphshell Is

Graphshell is a spatial browser/workbench with three authority domains:

- **Graph Tree**: semantic identity, node/edge truth, traversal/history truth, lifecycle truth.
- **Workbench Tree**: panes, splits, tabs, focus regions, frame/workbench arrangement.
- **Viewer Runtime**: live rendering attachments reconciled from graph/workbench intent.

This document intentionally does **not** restate low-level data models. Use it to
find the canonical source for a concern, not to derive reducer behavior.

---

## 2. Canonical Authority Map

| Concern | Canonical doc |
|---|---|
| System authority boundaries and registries | `implementation_strategy/system/system_architecture_spec.md` |
| Graph/canvas interaction semantics | `implementation_strategy/canvas/graph_node_edge_interaction_spec.md` |
| Traversal model, edge payloads, history manager behavior | `implementation_strategy/subsystem_history/edge_traversal_spec.md` |
| History subsystem policy and diagnostics expectations | `implementation_strategy/subsystem_history/SUBSYSTEM_HISTORY.md` |
| Node lifecycle and runtime reconcile | `implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| Viewer selection, presentation, fallback | `implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md` |
| Workbench/frame/tile semantics | `implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md` |
| Graph-first frame semantics | `implementation_strategy/workbench/graph_first_frame_semantics_spec.md` |
| Input routing and modal ownership | `implementation_strategy/aspect_input/input_interaction_spec.md` |
| Command surfaces and omnibar/radial/context parity | `implementation_strategy/aspect_command/command_surface_interaction_spec.md` |
| UX semantic projection, probes, scenarios | `implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` |
| Diagnostics contracts and health summaries | `implementation_strategy/subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` |
| Render/compositor pass ownership | `implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md` |

---

## 3. Current Product Summary

- Core browsing graph is functional.
- Workbench tile tree is functional.
- Traversal-aware edge/history model is the canonical runtime model.
- Four-state lifecycle (`Active`, `Warm`, `Cold`, `Tombstone`) is the canonical lifecycle contract.
- History Manager timeline/dissolved surface is active.
- Temporal preview/replay hardening remains backlog work.
- Faceted filtering and facet-pane routing now have canonical specs but remain runtime-pending.
- WGPU/WebRender migration remains planned.

For status-by-feature, use
`implementation_strategy/2026-03-01_complete_feature_inventory.md`.

---

## 4. Read This Next

- If you are changing reducer/model behavior, start in the relevant subsystem spec.
- If you are changing pane/open/focus behavior, start in workbench and focus specs.
- If you are changing user-visible interaction, start in the UX coverage matrix and the relevant canonical interaction spec.
- If you are changing observability or test gates, start in diagnostics and UxScenario specs.

---

## 5. Anti-Pattern

Do not treat this document as authority for:

- concrete Rust type shapes,
- lifecycle transition tables,
- traversal append rules,
- route naming policy,
- diagnostics channel lists,
- acceptance criteria.

Those belong in canonical subsystem/spec docs and must be changed there first.
