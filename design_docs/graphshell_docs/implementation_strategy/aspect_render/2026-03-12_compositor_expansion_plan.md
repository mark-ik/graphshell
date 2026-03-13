# Compositor Expansion Plan — Glow-First Semantic Composition

**Date**: 2026-03-12
**Status**: Active strategy
**Author**: Arc + Mark
**Context**: The wgpu renderer migration is deferred indefinitely (2026-03-12). The egui_glow / Servo GL compositor is the production renderer. This plan identifies how to get substantially more out of the existing compositor architecture now that it is no longer a holding pattern for a migration.

**Relates to**:
- `frame_assembly_and_compositor_spec.md` — canonical pass contract (this plan extends, does not replace)
- `render_backend_contract_spec.md` — backend bridge boundary
- `ASPECT_RENDER.md` — policy authority
- `../PLANNING_REGISTER.md` §0, §0.10
- `../../../registries/atomic/lens/registry.rs` — LensRegistry, LensDefinition, LENS_ID_SEMANTIC_OVERLAY
- `../viewer/node_lifecycle_and_runtime_reconcile_spec.md` — NodeLifecycle four-state model

---

## 1. Framing

The three-pass composition contract (Chrome → Content → Overlay Affordance) is implemented, GL state isolation is hardened, TileRenderMode dispatch is authoritative end-to-end, and differential composition skip is in place. The pass contract is production-quality.

What the current compositor does **not** do: it is semantically blind. It knows a tile's geometry, render mode, focus state, and GPU pressure. It does not know whether the node behind a tile is Cold or Tombstone, what Lens is active, whether the node carries unread traversal events, or what UDC tags it holds. This plan connects the compositor to that semantic state to produce a richer visual output without leaving the Glow stack.

The opportunities below are ordered from highest-value/lowest-effort to longer-horizon.

---

## 2. Shared Contract — `TileSemanticOverlayInput`

Before taking individual opportunities, the compositor needs one stable semantic input contract rather than a growing list of ad hoc parameters.

### Proposed contract

Introduce a compositor-facing value, resolved once per visible tile before Pass 3 scheduling:

```
TileSemanticOverlayInput {
    node_key: GraphNodeKey,
    render_mode: TileRenderMode,
    lifecycle: NodeLifecycle,
    runtime_blocked: bool,
    semantic_generation: u64,
    active_lens_overlay: Option<LensOverlayDescriptor>,
    focus_delta: Option<FocusDelta>,
    selection_state: TileSelectionState,
    has_unread_traversal_activity: bool,
}

TileSelectionState =
  | NotSelected
  | Selected           -- included in the active multi-tile selection set
  | SelectionPrimary   -- the anchor/primary node in a multi-tile selection
```

This value is computed by graph/runtime/focus authority layers and consumed by the compositor. The compositor never derives semantic meaning on its own; it receives a render-ready semantic snapshot.

### Ownership boundary

- Graph/runtime owns lifecycle, runtime-blocked state, tags, traversal state, and semantic invalidation.
- LensRegistry owns lens visual descriptor resolution.
- Focus authority / Render aspect own `FocusDelta`.
- Workbench / multi-tile selection authority owns `TileSelectionState`.
- The compositor owns only **visual composition** of the resulting input.

### Future consumer note — Graph Reader

The planned Graph Reader (virtual accessibility tree) needs per-node lifecycle, blocked state, and selection context to build its virtual node descriptions. `TileSemanticOverlayInput` assembles exactly this semantic snapshot. When Graph Reader is implemented it should be able to consume this contract as a read-only input rather than inventing a parallel semantic aggregation path.

### Why this matters

Without a shared contract, O1/O2/O3/O5 each add their own cross-layer carrier and the compositor becomes another semantic aggregation monolith. `TileSemanticOverlayInput` keeps Pass 3 extensible while preserving aspect boundaries.

### Acceptance criteria

- Pass 3 scheduling consumes `TileSemanticOverlayInput` instead of independently querying lifecycle, focus, lens, and tag state.
- New semantic overlay features add fields to `TileSemanticOverlayInput` rather than bypassing it with ad hoc compositor parameters.
- Tests can construct `TileSemanticOverlayInput` directly for render-policy coverage without simulating full graph/runtime state.
- `TileSelectionState` is populated from multi-tile selection authority and is visible to the overlay precedence dispatcher.

---

## 3. Opportunity O1 — Content Signature Enrichment

### Problem

`content_signature_for_tile` currently hashes `(webview_id, tile_rect, pixels_per_point)`. This means the differential composition skip cannot detect:

- Node lifecycle state change (Active → Cold: the tile should visually update even if geometry is unchanged)
- Active Lens change (a different lens may drive a different overlay appearance for the same tile)
- Node tag or metadata change that should trigger a re-render of the overlay pass

A tile can sit with a stale-looking composition while semantic state has changed underneath it.

### Proposed change

Add a runtime-only `semantic_generation: u64` invalidation field for node panes. This may live either:

- on `NodePaneState` as a **non-persistent** runtime field, or
- in a compositor/runtime cache keyed by `PaneId` or `(NodeKey, GraphViewId)`

The generation counter increments whenever:
- `NodeLifecycle` state changes for the mapped node
- The active `lens_id` for the graph view changes
- Node tags change (if overlay-relevant tags are registered)

The compositor includes `semantic_generation` in the content signature hash. This forces recomposition on semantic change even when geometry is stable.

**Invariant**: The semantic generation counter is reducer-owned invalidation state. The compositor is a consumer, not a source of truth.

**Invariant**: If carried on `NodePaneState`, `semantic_generation` must be explicitly marked runtime-only and must not become part of the persisted pane payload schema.

### Acceptance criteria

- Content signature changes when `NodeLifecycle` transitions (e.g., Active → Cold) for a mapped node, even with identical tile geometry.
- Differential composition skip does not fire across a generation boundary.
- `CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP` channel includes `semantic_generation` in its payload.

---

## 4. Opportunity O2 — Node Lifecycle → Overlay Affordance

### Problem

`NodeLifecycle` has four states: `Active`, `Warm`, `Cold`, `Tombstone`. The compositor currently treats all of these identically for overlay rendering. A Cold tile and an Active tile look the same to the compositor.

This is a direct gap between the semantic model and the visual output.

### Proposed change

Extend `ScheduledOverlay` to carry `node_lifecycle: NodeLifecycle`. The compositor receives the resolved lifecycle state per tile from the graph/runtime layer and threads it into overlay scheduling. The overlay affordance dispatch translates lifecycle state into a visual treatment:

| `NodeLifecycle` | Border treatment | Additional indicator |
| --- | --- | --- |
| `Active` | Full-opacity solid border (current behavior) | None |
| `Warm` | ~70% opacity border | None |
| `Cold` | ~40% opacity border, slightly desaturated | Optional "cold" glyph in tile chrome |
| `Tombstone` | Dashed or absent border | Ghost styling via Pass 3 overlay |
| `RuntimeBlocked` | Warning-color border | Recovery affordance badge (already in S5 contract) |

The `ChromeOnly` restriction for `NativeOverlay` mode still applies — lifecycle indicators for native overlay tiles appear in the gutter, not over native content.

**Carrier note**: `NodeLifecycle` is not currently stored on `NodePaneState`; the implementation should keep lifecycle authority in graph/runtime state and pass the resolved value into Pass 3 scheduling rather than expanding the durable pane schema unless there is a broader pane-model reason to do so.

### O2 extension — thumbnail pipeline ghost-mode treatment

`ASPECT_RENDER.md` §2 lists the **thumbnail pipeline** (`thumbnail_pipeline.rs`) as Render-aspect-owned. The thumbnail pipeline captures a last-known-state image for each node pane. This creates an opportunity for `Cold` and `Tombstone` lifecycle states to go beyond border desaturation:

- When a node transitions `Active → Cold`, the thumbnail pipeline captures a last-known-state snapshot.
- The compositor can render this snapshot as a **faded ghost image** in the content area of a `Cold` or `Tombstone` tile, behind or as the backdrop for Pass 3 overlay affordances.
- This gives Cold/Tombstone tiles a recognizable visual identity (what was last loaded) rather than a blank tile with a desaturated border.

The thumbnail ghost is opt-in per `TileRenderMode`: it only applies to `CompositedTexture` and `Placeholder` modes. `NativeOverlay` and `EmbeddedEgui` tiles do not need a thumbnail ghost because their content is either live or managed by egui's own render tree.

**Authority boundary**: The thumbnail pipeline (Render aspect) supplies the ghost texture; the graph/runtime layer owns the lifecycle transition signal. The compositor draws the ghost only when lifecycle state in `TileSemanticOverlayInput` is `Cold` or `Tombstone` and a thumbnail is available. No ghost is rendered if no thumbnail exists (i.e., the node was cold before any content was ever loaded).

**Degradation note**: Thumbnail ghosts are subordinate to the semantic degradation policy in §12. Under pressure or degraded placeholder conditions, lifecycle legibility is the requirement; the ghost image may be dropped before required blocked/focus/selection affordances.

### Acceptance criteria

- `Cold` node tile border is visually distinct from `Active` node tile border in a test harness.
- `Tombstone` tile renders ghost/dashed border affordance via Pass 3.
- `Cold` and `Tombstone` tiles render a faded thumbnail ghost in the content area when a thumbnail is available; fall back to desaturated border treatment when no thumbnail exists.
- Thumbnail ghost is not applied to `NativeOverlay` or `EmbeddedEgui` tiles.
- Lifecycle indicators respect the `NativeOverlay` chrome-only constraint.
- Overlay affordance tests extended to cover each lifecycle state, including the with-thumbnail and no-thumbnail cases for `Cold`/`Tombstone`.

---

## 5. Opportunity O3 — Lens-Driven Pass 3 Descriptor

### Problem

The `LensRegistry` has a fully-defined `LENS_ID_SEMANTIC_OVERLAY` lens (`priority: 10`, `requires_knowledge: true`, `requires_graph_context: true`, `filters: ["semantic:overlay"]`). But the compositor never consults the active lens. The lens system and the compositor exist in completely separate worlds. This means:

- Lens theme tokens do not reach the overlay affordance colour/style computation.
- `LENS_ID_SEMANTIC_OVERLAY` has no path to actually inject anything visual into Pass 3.
- Mod-registered lenses can define a `filters` list and a `theme`, but neither is visible to composition.

The overlay affordance pass (Pass 3) is the natural compositor injection point for lens-driven visual semantics.

### Proposed change

Add an `overlay_descriptor: Option<LensOverlayDescriptor>` field to `LensDefinition`. `LensOverlayDescriptor` defines how the compositor should modify Pass 3 for tiles belonging to graph views with that lens active.

```
LensOverlayDescriptor {
    border_tint: Option<Color32>,      // tint multiplied onto the affordance stroke color
    glyph_overlays: Vec<GlyphOverlay>, // small icon/badge rendered in tile chrome
    opacity_scale: f32,                // multiplied onto overlay opacity (1.0 = no change)
    suppress_default_affordances: bool // true: lens fully replaces default border treatment
}

GlyphOverlay {
    glyph_id: String,     // registry key into glyph/icon registry
    anchor: GlyphAnchor,  // TopLeft, TopRight, BottomLeft, BottomRight, Center
    condition: Option<LensGlyphCondition>, // e.g., OnlyWhenTagged("udc:science")
}
```

The compositor receives the resolved `LensOverlayDescriptor` for the active graph view and applies it during Pass 3 dispatch. The compositor does not interpret lens semantics — it executes the visual contract the lens provides.

**Authority boundary**: Lens semantics are owned by the LensRegistry and graph layer. The `LensOverlayDescriptor` is a pure visual contract passed to the compositor. The compositor is not aware of what the lens *means*.

### Acceptance criteria

- A lens with a non-null `overlay_descriptor.border_tint` produces a tinted border in Pass 3.
- `LENS_ID_SEMANTIC_OVERLAY` can register a `LensOverlayDescriptor` that fires when `requires_knowledge` conditions are met.
- Mod-registered lenses can supply a `LensOverlayDescriptor` via the normal `LensRegistry::register_with_descriptor` path.
- `LensOverlayDescriptor` is part of the `LensDefinition` data model contract, and is serialized only if/when lens definitions are promoted into a persisted serde-facing contract.
- Overlay affordance tests extended to cover lens tint and glyph overlay cases.

---

## 6. Opportunity O4 — `compositor:tile_activity` Diagnostic Channel

### Problem

The differential composition decision — "this tile is actively recompositing vs. idle this frame" — is made per frame but never emitted as an observable signal. This information is a proxy for "this webview is live/animating" that other subsystems could use without JS instrumentation.

Specifically:
- The **History subsystem** wants to understand which nodes the user is actively engaging with. Active recomposition is a reliable signal that the webview is live (loading, animating, or being interacted with).
- The **UX Semantics subsystem** could use tile activity to drive test fixture timing (e.g., "wait for this tile to go idle before asserting").
- The **Diagnostics health summary** would benefit from a per-tile activity rate signal.

### Proposed change

Emit a `compositor:tile_activity` diagnostic sample for tiles where the differential decision was made. The event payload carries:

```
CompositorTileActivityEvent {
    node_key: GraphNodeKey,
    decision: DifferentialDecision,  // Recompose | Skip(reason) | GpuPressureDegraded
    signature_changed: bool,
    semantic_generation_changed: bool, // requires O1
    frame_index: u64,
}
```

This channel is emitted at `Info` severity (not `Warn`/`Error`). It does not affect composition logic — it is observability only.

**Volume guardrail**: This signal should be sampling-aware. It may be emitted as:

- a per-tile per-frame event in diagnostics/sampled builds, or
- an aggregated frame sample in production-oriented diagnostics paths

The implementation must avoid turning tile activity into a noisy always-on event stream.

The `DifferentialDecision` is already computed in `tile_compositor.rs`. This change threads it to the diagnostics emission path without any new computation.

### O4 extension — History subsystem consumption pattern

The History subsystem is the primary cross-subsystem consumer of `compositor:tile_activity`. The intended consumption model:

- The History subsystem subscribes to `compositor:tile_activity` via the `ChannelRegistry` at startup (the same subscription mechanism used by the Diagnostics Inspector pane).
- It receives **frame-level aggregated summaries** rather than per-tile per-frame events: one `CompositorFrameActivitySummary { active_tile_keys: Vec<GraphNodeKey>, idle_tile_keys: Vec<GraphNodeKey>, frame_index: u64 }` per frame in which any tile changed its differential decision.
- The History Manager uses active tile keys as a lightweight "node is alive/being-interacted-with" signal to annotate traversal events — no JS instrumentation, no separate polling.
- The `ChannelRegistry` ring buffer for `compositor:tile_activity` should be sized appropriately for this use (suggested: 256 frames, not unbounded). History reads from the ring asynchronously — it does not need to be in the render hot path.

**Authority boundary**: The compositor emits; the History subsystem consumes. The compositor does not know what History does with the signal. History does not reach into the compositor to query state directly.

### Acceptance criteria

- `compositor:tile_activity` channel events are emitted for every tile that enters differential decision evaluation.
- Events are `Info` severity and do not appear in error/warn health summaries.
- Frame-level aggregated summary variant is supported alongside per-tile events for consumers that prefer it.
- History subsystem spec updated to note `compositor:tile_activity` as an available signal, the intended consumption pattern (frame-level summary, ring buffer read), and the ring buffer sizing recommendation.
- Ring buffer for this channel is bounded; unbounded growth is a hard rejection criterion.

---

## 7. Opportunity O5 — Focus Ring Latched to Focus Authority Events

### Problem

The focus ring fade animation is currently derived each frame from render-pass state using `focus_ring_started_at`. This works but has a subtle flaw: if focus state arrives from the Focus authority boundary after the current frame's focus transition point, the ring animation may lag by one frame.

For the focus ring specifically, a one-frame lag is imperceptible. But the same pattern means there is no defined contract between the Focus subsystem and the Render aspect about *when* focus state transitions are observable to the render pipeline.

### Proposed change

Introduce a `FocusDelta` value computed once per frame at the start of the Render aspect's frame setup (before compositor dispatch). `FocusDelta` captures:

```
FocusDelta {
    changed_this_frame: bool,
    new_focused_node: Option<GraphNodeKey>,
    previous_focused_node: Option<GraphNodeKey>,
}
```

The `FocusDelta` is consumed at the render-pass/frame-setup seam and then passed into compositor dispatch as needed. It is used to:
1. Latch the `focus_ring_started_at` timestamp at a deterministic point (once per frame, not inside the overlay dispatch loop).
2. Enable the render/compositor pipeline to react to focus *transitions* rather than inferring them late from current state.
3. Provide a clean test seam: tests can inject a `FocusDelta` without simulating egui focus events.

**Invariant**: `FocusDelta` is computed by the Render aspect from the Focus subsystem's authoritative state. The compositor does not derive focus transitions inside the pass dispatch loop.

### Acceptance criteria

- `focus_ring_started_at` is latched exactly once per frame when `FocusDelta.changed_this_frame` is true.
- Focus ring animation tests can inject `FocusDelta` directly without egui event simulation.
- No regression in focus ring fade-out timing relative to current behavior.

---

## 8. Opportunity O6 — `EmbeddedEgui` Z-Order Debt Resolution

### Problem

For `EmbeddedEgui` tiles (settings pane, future native egui viewers), the focus ring and hover affordances are registered in Pass 3 using `RectStroke` style at `Order::Foreground`. However, the egui widget tree for that tile continues to render after the overlay pass is registered — meaning the focus ring is technically painted *before* the egui content for that tile finishes, placing it at a lower z-order than egui's own widget output.

This is currently harmless because `EmbeddedEgui` viewers do not render content that overdraws their tile border. But it is an architectural debt: if any future `EmbeddedEgui` viewer renders widgets that extend to the tile edge (e.g., a full-bleed header), the focus ring will appear behind that widget rather than over it.

### Proposed change

For `EmbeddedEgui` tiles, register the overlay affordance as an egui `Area` at a layer *above* the tile's widget layer, rather than via the existing `draw_overlay_stroke` path. Concretely:

- `EmbeddedEgui` overlay affordances use `egui::Area::new(overlay_layer_id).order(Order::Tooltip)` rather than `Order::Foreground`.
- This guarantees z-order correctness regardless of what the `EmbeddedEgui` viewer renders inside the tile rect.
- `CompositedTexture` and `NativeOverlay` modes are unaffected — they already have correct z-order via the `pending_overlay_passes` batch.

### Acceptance criteria

- `EmbeddedEgui` focus ring renders over full-bleed egui content (tested with a synthetic full-bleed widget in a settings pane test fixture).
- No z-order regression for `CompositedTexture` or `NativeOverlay` affordances.
- `OverlayAffordanceStyle` extended with `EguiArea` variant alongside existing `RectStroke` and `ChromeOnly`.

---

## 9. Opportunity O7 — Generic Viewer Callback Path

### Problem

The `CompositorAdapter` content pass is implemented exclusively for the Servo `render_to_parent` callback. The spec (`frame_assembly_and_compositor_spec.md` §4.2) defines a generic callback type:

```
fn render_content(tile_rect: Rect, clip_rect: Rect, gl_state: &mut GlStateGuard)
```

But no viewer other than Servo can currently register a content callback. Any future `CompositedTexture`-mode viewers — image viewer, PDF renderer, GPU canvas, future custom renderers — would need to re-implement the GL state save/restore dance themselves rather than going through the `CompositorAdapter`.

### Proposed change

Expose a `CompositorAdapter::register_content_callback(node_key, callback)` API that any viewer can use to register a `CompositedTexture`-mode content callback. This replaces the current Servo-specific path with a general dispatch table.

Status update (2026-03-13): the adapter-level seam is implemented and Servo now routes through the generic registration path. The remaining deferred work is the `ViewerRegistry` contract extension so non-Servo `CompositedTexture` viewers can declare and attach their callback factories through normal viewer selection/runtime wiring.

The `ViewerRegistry` registration contract is extended: viewers declaring `TileRenderMode::CompositedTexture` capability must provide a `ContentCallbackFactory` that `ViewerRegistry` hands to `CompositorAdapter` at viewer attachment time.

**This remains the lowest-urgency item** — the adapter seam can exist ahead of demand, but the `ViewerRegistry` rollout is still correctly deferred until a second `CompositedTexture` viewer exists. It is listed here to mark the remaining extension seam, not as near-term work.

### Acceptance criteria

- A synthetic test viewer can register a content callback via `CompositorAdapter::register_content_callback` and have it invoked in Pass 2.
- GL state isolation invariants apply equally to callbacks registered via the generic path.
- Servo's existing path migrates to use the generic registration API (no parallel paths).

---

## 10. Opportunity O8 — Pass 3 Affordance → Accessibility Tree Annotation

### Problem

Pass 3 overlay affordances (lifecycle borders, focus rings, selection rings, runtime-blocked badges) are computed by the compositor and drawn to the GL surface. The AccessKit bridge (`SUBSYSTEM_ACCESSIBILITY.md`) needs exactly the same semantic signals — focus state, lifecycle, blocked state, selection — to annotate the AccessKit node tree. Currently these signals are assembled independently in the accessibility layer, duplicating the aggregation work that `TileSemanticOverlayInput` already does for the compositor.

This means:

- Accessibility state and visual state can drift if they draw from different sources.
- The accessibility layer has no awareness of what the compositor actually drew (e.g., whether a focus ring was rendered, or whether it was suppressed by a RuntimeBlocked state).

### Proposed change

After Pass 3 dispatch completes for a tile, emit a lightweight `TileAffordanceAnnotation` alongside the GL draw calls:

```
TileAffordanceAnnotation {
    node_key: GraphNodeKey,
    focus_ring_rendered: bool,
    selection_ring_rendered: bool,
    lifecycle_treatment: LifecycleTreatment,   // Active | Cold | Tombstone | RuntimeBlocked
    lens_glyphs_rendered: Vec<String>,         // glyph_ids that were drawn
}
```

The canonical UX/accessibility projection layer consumes this output as a read-only enrichment signal (not `TileSemanticOverlayInput` directly, and not by having the compositor talk to AccessKit directly) and then maps the enriched result into AccessKit annotations. This ensures:

- The a11y tree reflects *what the compositor drew*, not what was planned.
- Focus, selection, and blocked state annotations are consistent between the visual and a11y representations.
- No redundant semantic aggregation in the accessibility layer.

**Authority boundary**: The compositor emits `TileAffordanceAnnotation` as an output of Pass 3 dispatch. The canonical UX/accessibility projection layer is the consumer and remains responsible for producing AccessKit-facing state. The compositor does not know what AccessKit does with the annotation.

**Scope note**: This does not replace AccessKit's own focus tracking for keyboard navigation, and it does not replace the canonical UxTree / UX semantics authority path. `TileAffordanceAnnotation` is an enrichment signal describing what Pass 3 actually rendered, not the primary a11y semantic input.

### Acceptance criteria

- `TileAffordanceAnnotation` is emitted for each tile after Pass 3 dispatch completes.
- The canonical UX/accessibility projection layer can consume `TileAffordanceAnnotation` to annotate node roles without independently re-querying lifecycle or focus state.
- `focus_ring_rendered: true` in `TileAffordanceAnnotation` implies the AccessKit node carries `aria-selected`/focus role annotation consistent with the visual state.
- `lifecycle_treatment: RuntimeBlocked` in the annotation triggers an AccessKit `aria-busy` or equivalent marker on the node.
- The a11y and compositor representations of focus, selection, and blocked state cannot diverge (test: inject a focus change, verify both GL draw output and AccessKit node annotation are consistent).

---

## 11. Overlay Precedence and Composition Rules

As Pass 3 becomes semantic rather than purely geometric, the compositor needs an explicit precedence contract for overlapping overlay intents.

### Proposed precedence order

Highest to lowest:

1. `RuntimeBlocked`
2. explicit focus transition / focus ring
3. selection ring (`SelectionPrimary` > `Selected`)
4. lens-driven overlay replacement (`suppress_default_affordances = true`)
5. lens-driven overlay modification (tint, glyph, opacity scale)
6. lifecycle base treatment
7. hover-only affordance

### Composition rules

| Overlay source | Default behavior |
| --- | --- |
| `RuntimeBlocked` | May replace border color/treatment and append recovery badge |
| Focus | Adds on top of lifecycle base unless explicitly suppressed by a blocking/error state |
| Selection | Adds a selection ring distinct from the focus ring; `SelectionPrimary` uses a bolder stroke and/or distinct color relative to `Selected`; both coexist with focus if the node is focused-and-selected |
| Lens replacement | Replaces default border treatment but must not suppress runtime-blocked recovery affordances |
| Lens tint/glyph | Multiplies/modifies lifecycle base; does not erase focus or selection unless explicitly documented |
| Lifecycle | Provides the default base border/ghost treatment |
| Hover | Lowest priority visual accent; may be omitted when higher-priority overlays are active |

### Native overlay rule

For `TileRenderMode::NativeOverlay`, the same precedence order applies, but all winning affordances are projected into chrome/gutter regions rather than over native content.

### Acceptance criteria

- The overlay dispatcher has one documented precedence rule for lifecycle, focus, selection, runtime-blocked, hover, and lens overlays.
- A lens with `suppress_default_affordances = true` cannot suppress `RuntimeBlocked` recovery affordances.
- A focused-and-selected tile renders both the focus ring and the selection ring simultaneously without one suppressing the other.
- Pairwise tests cover at least: `Focus × Lifecycle`, `Focus × Selection`, `Lens × Lifecycle`, `RuntimeBlocked × Lens`, and `NativeOverlay × RuntimeBlocked`.

---

## 11. Semantic Degradation and Performance Guardrails

The current compositor already degrades under GPU pressure and can fall back to placeholder rendering. The semantic overlay layer must define what survives degradation and what performance ceilings it must respect.

### Semantic degradation policy

| Condition | Required semantic behavior |
| --- | --- |
| `Placeholder` degraded content pass | RuntimeBlocked and lifecycle indicators still render in Pass 3 on the placeholder surface |
| `NativeOverlay` | Semantic affordances move to chrome/gutter only |
| GPU pressure degraded frame | Base lifecycle + RuntimeBlocked affordances survive; decorative lens glyphs may be omitted first |
| Overlay suppression / modal conflict | Recovery and focus-critical affordances win over decorative lens overlays |

### Performance guardrails

- Semantic overlay dispatch must remain `O(visible tiles)` per frame.
- No semantic overlay feature may introduce per-frame heap allocation in the steady state without explicit justification.
- Semantic invalidation must not trigger more than one recomposition per tile per reducer-visible semantic transition.
- Diagnostic sampling for `compositor:tile_activity` must not turn Pass 3 observability into an always-on high-volume stream.

### Test matrix minimum

Add a small matrix-oriented harness covering:

- `TileRenderMode × lifecycle`
- `TileRenderMode × RuntimeBlocked`
- `TileRenderMode × lens overlay`
- degraded placeholder × lifecycle/lens
- `EmbeddedEgui` full-bleed content × focus overlay

### Acceptance criteria

- Placeholder and GPU-pressure degraded paths retain required semantic overlays.
- Lens glyphs are the first semantic overlay class eligible for omission under pressure.
- Performance diagnostics show no unbounded overlay-path event volume or recomposition churn attributable to semantic invalidation.

---

## 12. Sequencing

These opportunities are independent and can be worked in any order, but the following sequencing is recommended:

| Phase | Items | Rationale |
| --- | --- | --- |
| **Phase 0** | Shared `TileSemanticOverlayInput` contract; overlay precedence table; degradation guardrails | Establishes the semantic/render boundary before feature growth |
| **Phase 1** | O1 (content signature), O2 (lifecycle → overlay) | Correctness improvements; low risk; directly visible to users |
| **Phase 2** | O3 (lens overlay descriptor), O4 (tile activity channel) | Connects compositor to registry/subsystem layer; enables semantic visualization |
| **Phase 3** | O5 (focus delta), O6 (EmbeddedEgui z-order), O8 (a11y annotation) | Polish/reliability; low user-visible impact now but prevents future debt |
| **Phase 4 / Deferred rollout** | O7 (viewer-registry callback rollout) | Adapter seam is landed; defer registry/factory rollout until a second CompositedTexture viewer is being built |

Phase 0 is deliberately architectural: it prevents O1–O6 from growing a patchwork of one-off compositor inputs. Phase 1 has low coupling once that contract exists. Phase 2 introduces the lens → compositor connection which is a new cross-system contract and should be designed carefully. Phase 3 is internal cleanup/polish. O8 is placed in Phase 3 because it depends on Pass 3 dispatch being stable (O1–O6 landed) before the output annotation path is worth formalizing.

---

## 13. New Diagnostics Channels

This plan introduces one new diagnostics channel and extends two existing ones.

| Channel | Severity | Phase | Description |
| --- | --- | --- | --- |
| `compositor:tile_activity` | Info | O4 | Per-tile differential decision per frame; recompose vs. skip vs. degraded |
| `compositor:overlay_lifecycle_indicator` | Info | O2 | Emitted when lifecycle-driven overlay style is applied (node_key, lifecycle_state) |
| `compositor:lens_overlay_applied` | Info | O3 | Emitted when a lens overlay descriptor modifies Pass 3 for a tile |

All new channels follow OpenTelemetry naming conventions (`component:event_name`) and use `Info` severity (observability only; no effect on health summaries unless explicitly elevated).

---

## 14. What This Enables (Product-Level)

Taken together, these changes transform Pass 3 from a static "draw focus ring" step into a **semantic visualization layer**:

- **Node lifecycle state is visually communicated** through tile border treatment without any UI label or tooltip.
- **Active lens shapes the visual language of the graph** — a diagnostic lens makes diagnostic information visible; a semantic overlay lens makes knowledge-graph annotations visible; a user-defined mod lens can drive entirely custom affordances.
- **The compositor becomes a first-class participant in graph semantics** rather than a dumb pass-through.

All of this is achievable on the Glow stack. None of it requires wgpu.

---

## 15. Document Cross-References

| Document | Relationship |
| --- | --- |
| `frame_assembly_and_compositor_spec.md` | Canonical pass contract — this plan extends it, does not replace it |
| `render_backend_contract_spec.md` | Backend bridge boundary — O1–O6 do not touch the backend layer |
| `ASPECT_RENDER.md` | Policy authority for the Render aspect |
| `../PLANNING_REGISTER.md` §0, §0.10 | Existing compositor work items; this plan adds new items |
| `../viewer/node_lifecycle_and_runtime_reconcile_spec.md` | Source of `NodeLifecycle` states used in O2 |
| `../../../registries/atomic/lens/registry.rs` | `LensDefinition`, `LensDescriptor`, `LENS_ID_SEMANTIC_OVERLAY` — extended in O3 |
| `../subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` | New channels (O4) must be registered here |
| `../subsystem_history/SUBSYSTEM_HISTORY.md` | History subsystem as consumer of `compositor:tile_activity` (O4) |
| `../subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` | AccessKit bridge as consumer of `TileAffordanceAnnotation` (O8); Graph Reader future consumer of `TileSemanticOverlayInput` |
