# Spec Conflict Resolution Register (Pre-WGPU Closure)

**Date**: 2026-03-03  
**Status**: Active  
**Purpose**: Convert identified spec/terminology conflicts into feature-closure and validation-gate work items.

**Input basis**:
- Conflict assessment across pane promotion/opening semantics, frame semantics, traversal triggers, storage invariants, and lifecycle reconcile behavior.
- Canonical terminology and control-plane constraints.

**Canonical references**:
- `design_docs/TERMINOLOGY.md`
- `design_docs/graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `design_docs/graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-02-28_ux_contract_register.md`
- `design_docs/graphshell_docs/implementation_strategy/aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`

---

## Why this closes real gaps

This register closes a specific failure mode: partial feature implementation with stale or conflicting semantics.

It contributes by forcing three things:
1. **Semantic authority cleanup** — one canonical meaning for Promotion, Demotion, Pane Opening Mode, and graph citizenship.
2. **Spec/code parity recovery** — remove contradictory definitions that let partial behavior masquerade as complete.
3. **Implementation preconditions** — surface prerequisites (especially the internal address scheme: original `graphshell://` plan basis, current `verso://` runtime canonical namespace) before claiming closure.

Without this, UI/UX regressions reappear because different docs authorize different behavior.

---

## Decisions (Critical)

### D1 — Pane Opening Mode work is a separate plan

Decision: create a **separate Pane Opening Mode + SimplificationSuppressed plan**, not a subsection inside `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`.

Reason:
- Tab-semantics plan is structural (`egui_tiles` semantics).
- Pane Opening Mode is graph-citizenship and lifecycle semantics (`QuarterPane/HalfPane/FullPane/Tile`) with explicit non-overlap and suppression behavior.
- Folding both into one doc risks repeating the old “structural and semantic are conflated” problem.

### D2 — “Promotion” term is reserved

Decision: reserve **Promotion** exclusively for graph-enrollment semantics (address write -> node creation / graph citizenship transition).

Structural hoist/unhoist operations remain structural terms (e.g., hoist, expand-to-tile-strip) and must not use Promotion/Demotion labels.

### D3 — Address-as-identity is a closure prerequisite

Decision: internal address scheme implementation is a precondition for fully closing promotion/citizenship semantics in runtime behavior (`graphshell://` as original plan basis, `verso://` as canonical runtime namespace with compatibility parsing).

---

## Priority-Ordered Resolution Backlog

## P1 — Direct semantic conflict (rewrite required)

1. `pane_chrome_and_promotion_spec.md`
   - Current path: `implementation_strategy/workbench/pane_chrome_and_promotion_spec.md`
   - Rewrite opening/promotion model sections to separate:
     - Pane Opening Mode (citizenship decision)
     - Pane Presentation / lock/chrome behavior (within already-open contexts)
   - Remove “promotion is chrome-only/no graph mutation” wording.

## P2 — Canonical model alignment

2. `graph_first_frame_semantics_spec.md`
   - Current path: `implementation_strategy/workbench/graph_first_frame_semantics_spec.md`
   - Add frame address semantics (`graphshell://frame/<FrameId>` original spec basis; `verso://frame/<FrameId>` runtime canonical alias).
   - ~~Reconcile MagneticZone vs frame-affinity wording~~ — **Resolved 2026-03-14**: `MagneticZone` is deprecated as a legacy alias; canonical model is `ArrangementRelation` / `frame-member` edges + frame-affinity backdrop rendering. See `canvas/2026-03-14_graph_relation_families.md §2.4` and updated TERMINOLOGY.md Legacy section.

3. `subsystem_history/edge_traversal_spec.md`
   - Current path: `implementation_strategy/subsystem_history/edge_traversal_spec.md`
   - Add `NavigationTrigger::PanePromotion` semantics and deferred-edge assertion path.

## P3 — Terminology and lifecycle consistency

4. `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`
   - Current path: `implementation_strategy/workbench/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`
   - Terminology cleanup: remove structural use of Promote/Demote.

5. `viewer/node_lifecycle_and_runtime_reconcile_spec.md`
   - Current path: `implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md`
   - Landed update: collapse-driven `Tombstone` path defined for graph-backed panes; ephemeral panes explicitly excluded from node-lifecycle `Tombstone` entry.
   - Add demotion-driven `Tombstone` entry path and reconcile expectations.

6. `subsystem_storage/storage_and_persistence_integrity_spec.md`
   - Current path: `implementation_strategy/subsystem_storage/storage_and_persistence_integrity_spec.md`
   - Landed update: single-write-path now explicitly covers address-as-identity and non-durable ephemeral pane-open behavior.
   - Clarify single-write-path impact of address-as-identity and ephemeral pane-open non-write behavior.

## P4 — New plans (missing implementation strategy)

7. New plan: Pane Opening Mode + SimplificationSuppressed
   - Landed doc: `implementation_strategy/workbench/2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md`
   - Define runtime contract, tile-tree constraints, dismissal semantics, and validation gates.

8. New plan: internal address scheme implementation
   - Landed doc: `implementation_strategy/system/2026-03-03_graphshell_address_scheme_implementation_plan.md`
   - Define address issuance for graph/tool/frame surfaces.
   - Define canonical graph citizenship query and integration points.

---

## Validation gates tied to this register

A backlog item closes only when:
1. Spec text updated and canonical-term compliant.
2. Cross-spec references updated (no contradictory legacy wording).
3. At least one scenario/integration validation pointer is attached to the behavior contract.
4. A parity note is added to control-plane artifacts where required.

---

## Next authoring sequence

1. Execute P1 rewrite (`pane_chrome_and_promotion_spec.md`).
2. Execute P2.2 and P2.3 (`graph_first_frame_semantics_spec.md`, `edge_traversal_spec.md`).
3. Execute P3 terminology/lifecycle/storage clarifications.
4. Create P4 new plan docs with explicit done gates.

This ordering minimizes semantic contradiction early and prevents new implementation slices from being built on conflicting contracts.
