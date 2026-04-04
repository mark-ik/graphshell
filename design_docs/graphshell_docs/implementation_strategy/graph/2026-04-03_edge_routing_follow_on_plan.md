<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Edge Routing Follow-On Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the edge-routing lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan for post-layout edge path selection, bounded bundling strategy, and readability-driven routing policy.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-03-14_edge_visual_encoding_spec.md`
- `graph_node_edge_interaction_spec.md`
- `2026-03-01_ux_migration_design_spec.md`
- `2026-04-03_layout_transition_and_history_plan.md`
- `2026-04-03_layout_variant_follow_on_plan.md`

---

## Context

The umbrella physics note already identifies edge routing as a separate post-layout concern:

- it operates on final node positions after layout converges
- it is intended to reduce overlap and crossings rather than redefine graph semantics
- it may use orthogonal routing, low-cost curve shaping, or bounded bundling approaches

Other active docs already reinforce the same boundary:

- `2026-03-14_edge_visual_encoding_spec.md` explicitly defers edge bundling as a separate layout concern
- `2026-03-01_ux_migration_design_spec.md` treats high crossing count as a readability condition that should produce a suggestion, not a hidden automatic mutation
- the UX research report rejects full real-time CPU force-directed edge bundling as too expensive for sustained 60 FPS interaction

This plan exists to own the missing middle: what routing modes Graphshell should support, when routing runs, how it is diagnosed, and what the first shippable bounded path actually is.

---

## Non-Goals

- making edge routing part of graph truth or edge identity
- hiding major routing-mode switches behind silent heuristics
- shipping full real-time force-directed edge bundling on the CPU as the baseline path
- redefining edge color, family encoding, or edge interaction semantics already owned elsewhere

---

## Feature Target 1: Define the Routing Mode Contract

### Target 1 Context

Graphshell needs explicit routing modes rather than a vague future promise of "better edge paths." The first step is naming the modes and defining their scope.

### Target 1 Tasks

1. Define the initial routing-mode set for graph views: straight, curved, and deferred bundled/step-bundled as separate modes.
2. Keep routing as a view/layout/readability policy rather than an edge-level semantic property.
3. Define stable mode IDs so diagnostics and user-visible controls can report the active routing path.
4. Ensure routing policy composes with the current layout result instead of replacing it.

### Target 1 Validation Tests

- The active routing mode is inspectable and diagnosable.
- Changing routing mode does not mutate graph truth or edge families.
- Saved view state restores the requested routing mode through a documented fallback path.

---

## Feature Target 2: Ship the Low-Cost First Slice

### Target 2 Context

The research report is clear: full real-time FDEB is too expensive as an always-on interactive CPU path. The first slice has to start with cheaper alternatives.

### Target 2 Tasks

1. Treat low-cost curve shaping as the first shippable readability path for dense edge regions.
2. Keep full bundling, if pursued, as a deferred or idle-time step that runs only after the graph settles.
3. Ensure the first slice can degrade cleanly to straight edges when the routing pass is unavailable or too expensive.
4. Keep the initial implementation bounded to post-layout path generation rather than a second live simulation.

### Target 2 Validation Tests

- The first-slice routing path improves readability without destabilizing live interaction.
- Idle-time or settled-state routing never blocks active drag/zoom interactions.
- Fallback to straight edges is explicit and diagnosable.

---

## Feature Target 3: Integrate With Readability Metrics And Suggestions

### Target 3 Context

The UX migration spec already names high edge crossings as a condition that should suggest bundling or a different layout. This plan should formalize that dependency rather than invent a parallel trigger system.

### Target 3 Tasks

1. Define which readability metrics can trigger routing suggestions, especially edge crossings and dense overlap.
2. Keep routing changes suggestion-driven by default, not silent automatic mode flips.
3. Expose requested vs resolved routing mode through diagnostics.
4. Coordinate routing suggestions with layout suggestions so the user can understand which adaptation is being recommended and why.

### Target 3 Validation Tests

- High crossing count can emit a routing suggestion without forcing a mode change.
- Diagnostics distinguish metric evidence from accepted routing policy.
- Routing suggestions can be accepted or dismissed without mutating graph truth.

---

## Feature Target 4: Preserve Edge Interaction And Visual Contracts

### Target 4 Context

Routing changes how edges are drawn, but it must not quietly break hit targets, hover semantics, or the encoding rules already owned by edge interaction and visual encoding specs.

### Target 4 Tasks

1. Ensure routed paths still satisfy the hit-target and hover expectations from the edge interaction surface.
2. Keep routing independent from family color/dash/width encoding rules in the visual encoding spec.
3. Define how routed edges participate in selection, hover, and focus overlays.
4. Coordinate routed-path transitions with `2026-04-03_layout_transition_and_history_plan.md` so path changes during layout/view transitions are not abrupt or misleading.

### Target 4 Validation Tests

- Routed edges preserve selection and hover continuity.
- Visual encoding remains family-driven even when the path geometry changes.
- Path changes across layout transitions are bounded and diagnosable.

---

## Exit Condition

This plan is complete when Graphshell has a documented, diagnosable edge-routing policy that can select among bounded routing modes, ship a low-cost readability-first path, integrate with readability suggestions, and preserve the existing edge interaction and visual encoding contracts.
