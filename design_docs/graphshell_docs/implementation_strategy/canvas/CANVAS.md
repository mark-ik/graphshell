# CANVAS — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `graph_node_edge_interaction_spec.md`
- `workbench_frame_tile_interaction_spec.md`
- `2026-02-28_ux_contract_register.md`
- `../../TERMINOLOGY.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.9)):
- **WCAG 2.2 Level AA** — graph-space interaction targets (nodes, edges, affordances) must meet SC 2.5.8 minimum target size; focus indicators on canvas elements must meet SC 2.4.11
- **Fruchterman-Reingold 1991** — force-directed layout algorithm; parameter semantics for `Liquid`/`Gas`/`Solid` presets must be documented against this model

---

## 1. Purpose

This note defines the **Canvas** as a Layout Domain feature area of Graphshell.

It exists to make one boundary explicit:

- the graph structure is a content and semantic domain feature area,
- not a presentation-layout area,
- and not a workbench arrangement area.

---

## 2. What The Canvas Domain Feature Area Owns

The Canvas owns graph truth and graph meaning:

- node identity
- edge identity
- graph topology / relationship structure
- graphlets (connected groups or explicitly meaningful graph subsets)
- graph selection truth
- graph traversal semantics
- graph activation semantics
- graph camera target semantics
- graph-space interaction meaning

The Canvas is the canonical owner of content relationships.

It is not the owner of tile layout, frame structure, or pane arrangement.

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` are not canvas-only settings.

They are a **cross-domain policy layer** that can influence:

- layout behavior
- physics behavior

Within the Canvas, these presets primarily affect:

- graph manipulation feel
- graph-space motion and convergence expectations
- force-parameter tuning and simulation stability

The Canvas must interpret these presets through Graphshell-owned policy, not through
implicit widget behavior.

That means:

- the framework may render and report events,
- but Graphshell decides what `Liquid`, `Gas`, and `Solid` mean for graph behavior.

---

## 4. Ownership Mapping

### 4.1 Canonical Graph-owned state

- graph data model
- node and edge identity
- graph selection set
- hovered or focused semantic graph target
- graph camera mode and camera target semantics
- graph interaction policy

### 4.2 State Graph does not own

- tile tree structure
- frame selection
- pane placement
- split geometry
- tab ordering

Those belong to the Workbench domain feature area.

---

## 5. Bridges To Other Areas

The Canvas interacts with other domains, aspects, and subsystems through explicit bridges.

### 5.1 Canvas -> Workbench bridge

Used when graph content must be shown somewhere.

Examples:

- open node
- focus existing presentation
- route a node into a pane, tile, or frame

The Canvas requests semantic intent.
The Workbench chooses and manages the destination arrangement.

### 5.2 Canvas -> Viewer bridge

Used when graph content needs a concrete presentation surface.

Examples:

- placeholder viewer
- embedded web content
- tool or document viewer

The Canvas identifies what should be shown.
The Viewer determines how it is rendered.

### 5.3 Policy -> Canvas bridge

Used when cross-subsystem presets influence graph behavior.

Examples:

- `Liquid`: more fluid node motion and convergence
- `Gas`: higher-dispersion node motion and exploration
- `Solid`: more damped, rigid node motion

The policy layer supplies defaults.
The Canvas applies them to canvas-specific behavior.

Camera policy is separate:

- camera locks, fit behavior, zoom policy, and pan bindings are independent camera controls
- physics preset changes must not implicitly mutate camera state or camera locks

---

## 6. Architectural Rules

- The Canvas must never derive graph truth from tile layout.
- Workbench arrangement must never redefine graph identity.
- Graph camera semantics must remain Graphshell-owned even when current rendering is driven by framework state.
- If canvas behavior is blocked or degraded, the Canvas must surface that explicitly rather than relying on silent widget fallthrough.

---

## 7. Practical Reading

If a behavior answers:

- what content exists,
- how content relates,
- what a node or edge means,
- what graph target the camera should care about,

it belongs primarily to the **Canvas**.

---

## 8. Deferred Spec: `canvas_render_pipeline_spec.md`

**Status**: Deferred — not yet written. Blocked on custom canvas paint callback stabilization.

A `canvas_render_pipeline_spec.md` should be created once the canvas rendering
architecture stabilizes past the `egui_graphs` custom canvas migration (see
`../aspect_render/2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`).

### Prerequisite

This spec is blocked on the custom canvas paint callback being established as the
stable draw entry point. Until that migration is complete, the canvas render
pipeline is partially owned by `egui_graphs` and cannot be fully specified.

### What the deferred spec must cover

When written, `canvas_render_pipeline_spec.md` must define the normative contract for:

- draw architecture: what primitives the canvas draws per frame and in what order,
- LOD (level of detail) tier thresholds: when nodes collapse to badges/thumbnails/icons
  based on camera zoom level — **must inherit and not redefine the canonical LOD tiers
  from `graph_node_edge_interaction_spec.md §4.8`** (Point / Compact / Expanded with
  `camera.scale` thresholds and hysteresis rules),
- batching policy: how draw calls for edges, node fills, badges, and labels are
  batched to minimize GPU command overhead,
- culling strategy: frustum and spatial-index culling rules for off-screen nodes/edges,
- frame-pass structure within the canvas render callback: what happens inside the
  canvas tile's composition pass (pre-pass, geometry pass, overlay pass),
- GPU resource lifecycle: buffer allocation, atlas management, texture upload policy
  for node thumbnails and badges,
- canvas-specific diagnostics channels for draw call counts, cull rates, and
  per-frame geometry budget.
