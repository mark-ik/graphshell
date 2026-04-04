<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Layout Backend State Ownership Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the layout-backend and state-ownership lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan focused on widening `ActiveLayoutState`, stabilizing persisted layout IDs, and defining when Graphshell must move beyond its current wrapper around the upstream layout trait seam.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_layout_variant_follow_on_plan.md`
- `2026-04-03_twod_twopointfive_isometric_plan.md`
- `2026-04-03_wasm_layout_runtime_plan.md`
- `layout_algorithm_portfolio_spec.md`
- `multi_view_pane_spec.md`
- `view_dimension_spec.md`
- `2026-04-02_parry2d_scene_enrichment_plan.md`

---

## Context

Graphshell already owns more of the layout story than the original umbrella note implied:

- `ActiveLayout` is the built-in dispatcher
- `ActiveLayoutState` is the concrete render-facing persisted wrapper
- the render layer deals with one view-owned state carrier rather than per-layout concrete types
- built-in layout modules already live under Graphshell control

The current carrier is still narrow, though. It is shaped around `kind + physics`, which is enough for force-directed and Barnes-Hut variants, but it is too specific for:

- analytic layouts with non-FR state
- runtime-loaded WASM layouts
- richer scene-backed layout/runtime state
- future backend-neutral spatial or projection metadata

This plan exists to isolate the ownership question from the variant and projection lanes. Those lanes need a wider carrier, but they should not each invent their own persistence shape.

---

## Non-Goals

- replacing `egui_graphs` with a brand-new native layout trait in the first slice
- implementing the first new analytic variants directly in this doc
- delivering the WASM runtime itself
- turning scene enrichment into a mandatory prerequisite for state widening
- quietly expanding this lane into full free-camera 3D ownership

---

## Feature Target 1: Widen the Persisted State Carrier

### Target 1 Context

Today the persisted wrapper is effectively force-directed-shaped. Future layout families need a carrier that remains single and render-facing while allowing different backends to store different state.

### Target 1 Tasks

1. Replace the implicit `physics`-only assumption with a Graphshell-owned tagged carrier for per-backend state payloads.
2. Define stable serialized layout IDs independent of Rust enum variant names.
3. Keep one concrete per-view wrapper for `render/mod.rs` and snapshot persistence.
4. Preserve deterministic fallback behavior when the saved layout kind is unknown or unavailable.
5. Keep projection-specific derived data out of canonical 2D layout truth unless a later lane explicitly widens that boundary.

### Target 1 Validation Tests

- Existing force-directed and Barnes-Hut views roundtrip without state loss.
- A saved view with an unknown layout ID degrades through a documented fallback path.
- The widened carrier can hold at least one non-FR payload shape without changing render-facing APIs.

---

## Feature Target 2: Separate Persisted State from Runtime Derivation

### Target 2 Context

Not every backend-owned detail belongs in the snapshot. Some state is canonical and resumable; some state is runtime-only cache, derived scene data, or a transient stepping artifact.

### Target 2 Tasks

1. Draw an explicit line between persisted layout state, runtime caches, and scene/projection derivations.
2. Keep per-view ownership aligned with `multi_view_pane_spec.md` so one graph view can change backend without contaminating others.
3. Define where guest-owned opaque bytes live for runtime-loaded layouts without making them the default for all native layouts.
4. Keep scene-enrichment runtime data and projection overlays optional dependents of the layout carrier rather than stuffing them into every saved payload.
5. Ensure `TwoD` layout truth remains recoverable even when a view temporarily renders with derived depth or scene effects.

### Target 2 Validation Tests

- Snapshot payloads remain bounded and understandable after the carrier widens.
- Clearing runtime-only caches does not mutate saved layout truth.
- Per-view backend swaps do not leak runtime-only state across views.

---

## Feature Target 3: Define the Backend Family Model

### Target 3 Context

The carrier should describe what kinds of layout backends Graphshell intends to support, even if not all of them are implemented immediately.

### Target 3 Tasks

1. Define the initial backend families Graphshell intends to support through the shared carrier: built-in force-directed, built-in analytic, runtime-loaded WASM, and future scene-backed variants.
2. Keep layout selection, diagnostics, and persistence keyed by stable backend/layout IDs rather than ad hoc enum names.
3. Clarify whether `ActiveLayout` stays a bounded enum, gains an internal registration table, or becomes a mixed model with a stable external ID layer.
4. Make the ownership boundary explicit: Graphshell owns the persisted wrapper and backend selection policy even while the underlying trait seam still routes through `egui_graphs`.
5. Document the dependency edges: layout-variant, WASM runtime, and projection-mode work all consume this carrier.

### Target 3 Validation Tests

- Diagnostics can report both the requested layout ID and the resolved backend family.
- Snapshot serialization survives internal enum refactors because the external ID remains stable.
- At least one future backend family can be sketched without forcing immediate trait replacement.

---

## Feature Target 4: Set the Full Seam-Ownership Escalation Gate

### Target 4 Context

The main architectural question is not whether Graphshell owns some layout state already. It does. The real question is when the imported `Layout<S>` / `LayoutState` seam becomes the bottleneck.

### Target 4 Tasks

1. Define the triggers that justify replacing the imported trait boundary: unbounded backend families, backend-neutral stepping, broader spatial-query ownership, or non-`egui_graphs` render paths.
2. Record the cases that do not require full replacement yet: bounded built-in variants, projected 2.5D views, and post-physics helper composition.
3. Ensure follow-on plans can proceed against the widened wrapper without prematurely forcing a wholesale trait migration.
4. Keep the migration path explicit if full seam ownership becomes necessary later.
5. Tie that escalation decision to real dependent work rather than abstract purity.

### Target 4 Validation Tests

- The plan can admit new built-in variants without immediate trait replacement.
- The plan identifies concrete criteria for when the current wrapper is no longer sufficient.
- Follow-on plans can reference this document as the ownership authority instead of restating the tradeoff.

---

## Exit Condition

This plan is complete when Graphshell has a widened, snapshot-safe, per-view layout state carrier with stable external layout IDs, explicit separation between persisted and runtime-only state, and a documented decision gate for when full native ownership of the layout trait seam is actually required.
