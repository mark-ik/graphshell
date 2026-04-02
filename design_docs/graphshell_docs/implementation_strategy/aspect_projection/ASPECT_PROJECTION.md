# PROJECTION — Aspect

**Date**: 2026-04-02
**Status**: Architectural aspect note
**Priority**: Architecture clarification and generalization pass

**Related**:

- `projection_interdomain_contract_spec.md`
- `projection_runtime_lifecycle_spec.md`
- `../../technical_architecture/unified_view_model.md`
- `../../technical_architecture/graphlet_model.md`
- `../workbench/graphlet_projection_binding_spec.md`
- `../shell/shell_overview_surface_spec.md`
- `../domain_interaction_acceptance_matrix.md`

**Policy authority**: This file is the canonical policy authority for the Projection aspect. Supporting projection docs refine contracts and implementation detail and must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

---

## 1. Purpose

This note defines the **Projection aspect** as the architectural owner of how domains represent themselves in each other without transferring truth ownership.

It exists to keep one boundary explicit:

- domains own truth,
- surfaces consume derived representations,
- and cross-domain local worlds, summaries, correspondences, and scoped subsets must not silently become second authorities.

Graphlets are the first strong canonical consumer of this aspect, but they are not the only target. The broader concern is interdomain representation itself.

---

## 2. What The Projection Aspect Owns

- generic projection descriptors and projection keys
- scope resolution for derived representations
- projection runtime contracts and invalidation rules
- cross-domain representation classes such as bounded local worlds, summaries, correspondences, and staged derived subsets
- projection-local ranking, frontier, and membership semantics when those are derived rather than truth-owning
- linked versus unlinked projection bindings where one domain hosts another domain's derived object
- diagnostics-ready projection lifecycle events (`derived`, `refreshed`, `linked`, `forked`, `invalidated`, `promoted`)

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

The Projection aspect does not own source truth.

- **Graph** still owns node/edge truth and graph algorithms.
- **Navigator** still owns navigation-oriented traversal and current local-world handling.
- **Workbench** still owns arrangement and hosted binding state.
- **Shell** still owns top-level summary exposure and orchestration.
- **Security**, **History**, and other subsystems may constrain or annotate projection behavior, but they do not turn projection into independent truth.

Projection is therefore a synthesized runtime concern that lets one domain present another domain's state in a bounded, reusable form.

---

## 4. Bridges

- Graph -> Navigator: graph truth becomes bounded local worlds, graphlets, frontiers, and scoped search spaces
- Graph -> Workbench: graph-backed subsets and correspondences become linked or unlinked arrangements
- Graph / Workbench / Runtime -> Shell: cross-domain state becomes summaries, overview projections, and re-entry targets
- History / Diagnostics / future domains -> other surfaces: temporal, health, or agent-oriented state may project as scoped representations without re-owning the source domain

---

## 5. Architectural Rule

If a behavior answers "how does one domain become a derived, scoped, or hosted representation inside another domain or surface without transferring semantic ownership?" it belongs to the **Projection aspect**.

---

## 6. Current Canonical First Consumer

`graphlet` is the first canonical projection object in Graphshell.

That means:

- graphlet semantics should remain the proving ground for the Projection aspect,
- graphlet-specific contracts must stay compatible with a broader projection model,
- future projections should reuse the same mental model when possible instead of inventing one-off local correspondence systems.

---

## 7. Generalization Direction

The Projection aspect is intentionally broader than graphlets.

Likely future projection classes include:

- graph-derived graphlets and frontier views
- workbench correspondence projections
- shell overview projections
- history slices and temporal local worlds
- diagnostics summaries and health projections
- future agent or distillery-facing derived views over durable local state

The important invariant is always the same: projection is derived representation, not truth migration.
