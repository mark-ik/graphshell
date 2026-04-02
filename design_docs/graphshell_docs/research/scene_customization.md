# Scene Customization Research Memo

**Date**: 2026-04-02
**Status**: Research / decision memo
**Purpose**: Determine how Graphshell should support richer canvas scene customization and organization, and when a heavier engine such as Rapier is actually justified.

**Related**:

- `../implementation_strategy/graph/2026-02-24_physics_engine_extensibility_plan.md`
- `../implementation_strategy/graph/2026-04-02_parry2d_scene_enrichment_plan.md`
- `../implementation_strategy/graph/2026-04-02_scene_mode_ux_plan.md`
- `../implementation_strategy/graph/layout_behaviors_and_physics_spec.md`
- `../implementation_strategy/graph/semantic_tagging_and_knowledge_spec.md`
- `../implementation_strategy/graph/node_badge_and_tagging_spec.md`
- `../implementation_strategy/graph/2026-02-25_doi_fisheye_plan.md`
- `2026-04-02_scene_mode_ux_sketch.md`
- `../research/2026-02-18_graph_ux_research_report.md`
- `../implementation_strategy/system/register/canvas_registry_spec.md`
- `../implementation_strategy/system/register/physics_profile_registry_spec.md`
- `../implementation_strategy/workbench/graph_first_frame_semantics_spec.md`
- `../implementation_strategy/2026-03-01_complete_feature_inventory.md`

---

## 1. Executive Summary

Graphshell does **not** need Rapier to unlock a meaningful scene-customization system.

The current stack already contains the beginnings of a scene language:

- per-view layout ownership,
- post-physics behavior injection,
- frame-affinity backdrops and derived organizational regions,
- physics/lens presets,
- semantic tags and label/color hints,
- planned zoom-adaptive labels, DOI, and decluttering overlays.

That is enough to support a strong first-generation scene system focused on:

- node and edge presentation overrides,
- labeled regions and backdrops,
- anchors and attractors,
- curated layout/physics presets,
- decluttering and label-density modes,
- saved per-view scene overlays.

**Primary recommendation**: build scene customization as a **layout-native, Graphshell-owned overlay system** that sits above graph-canonical data and below the render/layout policy layer.

**Fallback / escalation path**: if lightweight geometry or spatial indexing becomes necessary, add narrowly-scoped helpers such as `rstar` first. Only introduce Rapier later if Graphshell explicitly wants a separate authored canvas-editor mode with rigid bodies, collisions, joints, surfaces, or simulation-specific authoring workflows.

**Recommendation on persistence**: scene customization should not become graph-canonical authority. The preferred model is:

1. scene state is **per-view overlay state** at runtime,
2. frame/workspace snapshots may persist the active scene overlay for that view,
3. optional external `SceneFile` import/export can serialize the same view-scoped model,
4. graph-canonical state remains the source of truth for tags, memberships, relations, frames, and topology.

**Refined conceptual model**:

- **Graph**: canonical semantic structure. Durable identity, topology, relations, tags, memberships, provenance, and node metadata live here.
- **View / workspace state**: canonical non-graph state that defines how a pane or workbench context exists. This includes `GraphViewId`-scoped state, camera, active layout mode, lens selection, and whether a view is following default policy or carrying local divergent overrides.
- **Scene**: per-view spatial projection of graph plus view/workspace state plus optional scene overlay. A scene may include positions, regions, derived backdrops, attractors, colliders, labels, or simulation rules.
- **Style**: the visual/material representation of graph and scene elements. Shapes, colors, textures, edge treatments, badge systems, labels, and backdrop appearance live here.

This distinction matters because it prevents two common mistakes:

1. treating all visual customization as if it needed a physics engine, and
2. treating all scene behavior as if it must become graph-canonical truth.

---

## 2. Layered Authority Model

### 2.1 Graph

The graph remains the canonical semantic structure:

- node and edge identity,
- relation family and topology,
- frame/workbench membership semantics,
- canonical tags and semantic metadata,
- stable persistence and replay truth.

Graph answers:

- what exists,
- what it means,
- what is connected to what,
- which memberships and semantic carriers are authoritative.

### 2.2 View / Workspace State

View / workspace state is canonical non-graph state that shapes how a view exists.

This layer includes:

- `GraphViewId`-scoped state,
- camera and viewport interpretation,
- active layout selection,
- active lens and physics-profile selection,
- workbench placement and pane context,
- whether the view is following default policy or carrying local divergent overrides.

This matches the repo's existing distinction between graph truth and per-view state. It is important to keep this layer explicit rather than hiding it inside "scene," because layout mode, camera, and lens selection are not merely visual output.

### 2.3 Scene

The scene is a per-view spatial projection of graph plus view/workspace state plus optional scene overlay.

A scene may be:

- static or dynamic,
- layout-derived or manually-authored,
- ephemeral runtime state or persisted overlay state,
- lightweight and non-physical or backed by a richer simulation world.

The scene is not merely "derived from session state." It is derived from a concrete input set:

- graph state,
- view/workspace state,
- scene overlay or scene policy,
- selected layout or simulation algorithm.

Scene answers:

- where things are in this view,
- what spatial rules apply,
- what regions, attractors, colliders, or constraints are active,
- whether the view is running a true simulation or a lighter layout-derived projection.

Determinism, replay, and snapshot restoration are **capabilities of some scene modes**, not defining requirements for all scenes.

If this distinction is skipped, authority becomes hidden: camera and lens state, scene overlay state, and simulation policy collapse into one vague bucket. Graphshell should resist that.

### 2.4 Style

Style is the representational layer for graph and scene elements:

- node fills, shapes, halos, badges, icons, textures,
- edge stroke, color, thickness, dash, directional treatment,
- region/backdrop appearance,
- label, subtitle, annotation, and density policy.

Style answers:

- how graph and scene elements appear,
- how much information is shown,
- how emphasis is conveyed without changing canonical graph meaning.

### 2.5 Why this layered model is useful

This model gives Graphshell a cleaner path for both light and heavy scene work:

- Graph stays canonical.
- View / workspace state stays explicit.
- Scene stays view-owned.
- Style stays representational.

That means Graphshell can support:

- pure style overlays,
- lightweight scene organization,
- collision-aware layout,
- and eventually true simulated scene modes,

without collapsing all of those into one monolithic "physics" concept.

---

## 3. Current-State Grounding

### 3.1 Graphshell already has scene primitives

Current Graphshell architecture already supports several layers of scene expression without a second engine:

| Capability | Current source | What it already enables |
| --- | --- | --- |
| Post-physics injection seam | `graph/physics.rs`, `render/mod.rs` | Ordered behavioral passes after the main layout step |
| Derived organizational regions | `graph/frame_affinity.rs`, `layout_behaviors_and_physics_spec.md` | Backdrops, soft grouping, frame-based spatial organization |
| Built-in layout dispatch | `graph/layouts/active.rs` | View-selectable layout behavior without render-surface churn |
| Physics presets | `registries/atomic/lens/physics.rs`, `physics_profile_registry_spec.md` | Semantic layout behavior selection (`Liquid`, `Gas`, `Solid`) |
| Lens/physics binding | `layout_behaviors_and_physics_spec.md` | View-level policy and preset switching |
| Semantic clustering | `semantic_tagging_and_knowledge_spec.md` | Spatial grouping from canonical semantic tags |
| Tags and badges | `node_badge_and_tagging_spec.md` | Visual identity, label hints, semantic affordances |
| Label/LOD/decluttering research | `2026-02-18_graph_ux_research_report.md`, `2026-02-25_doi_fisheye_plan.md` | Progressive disclosure, label-density control, DOI emphasis |

This means Graphshell already has enough structure to support:

- graph-semantic styling,
- per-view presentation policy,
- organizational backdrops,
- layout-aware emphasis,
- soft regions and attractors,
- reusable preset scene templates.

### 3.2 Important invariants already established

The existing docs are unusually clear on a few boundaries, and any scene system should preserve them:

- **Graph is canonical**: semantic tags, relations, frames, memberships, and topology remain graph-owned.
- **View state is per-graph-view**: layout behavior and camera-like scene interpretation are not global.
- **Canvas policy is registry-owned**: canvas behavior should route through Graphshell policy surfaces rather than ad hoc widget-local logic.
- **No second hidden authority**: frame/workbench organization should not be silently replaced by a parallel scene model.
- **Derived regions are acceptable**: frame-affinity proves that rich visual organization can be derived from canonical graph/workbench state rather than stored separately.

### 3.3 Why the Rapier idea arises anyway

The appeal of Rapier is understandable because it offers capabilities the current layout-native stack does not:

- collisions,
- rigid-body constraints,
- authored regions and surfaces,
- springs and joints,
- deterministic simulation worlds,
- a path toward a true canvas editor rather than a mere layout policy surface.

Those are real differentiators. The question is whether Graphshell wants those **now**, or whether it first wants a lighter scene-customization layer that deepens the existing graph-view experience.

---

## 4. Problem Framing

The desired capability is not simply "more physics." It is a broader graph-scene problem:

1. Make graph views feel more alive and intentional without losing stability.
2. Let users spatially organize material with more nuance than one global layout preset.
3. Support visual storytelling and orientation: labels, backdrops, grouping, boundaries, emphasis.
4. Allow some durable scene authoring, including possible save/load of scene configurations.
5. Preserve graph-first semantics and per-view ownership.

Three different problem classes are easy to conflate and should be treated separately.

### 4.1 Presentation customization

This is about how existing graph state is drawn:

- node colors, fills, outlines, halos,
- edge color/weight/style,
- labels, subtitles, chips, annotations,
- badge density and visibility,
- backdrops and region labels,
- semantic or relation-family visualization modes.

This class does **not** require a separate physics engine.

### 4.2 Layout and force customization

This is about where things drift or settle:

- anchors and attractors,
- domain/semantic/frame grouping,
- region bias,
- label placement passes,
- soft exclusion zones,
- custom preset combinations.

This still does **not** inherently require Rapier. It fits naturally into Graphshell's current layout and post-physics stack.

### 4.3 Full simulation/editor behavior

This is where Rapier starts to make sense:

- authored regions with physical behaviors,
- collision or containment surfaces,
- spring/rope/rigid constraints,
- explicit scene editing with handles/tools,
- a simulation world that users are meaningfully editing as its own artifact.

This is not "better layout tuning." It is a distinct product mode.

---

## 5. Architectural Options

### 5.1 Option A: Layout-Native Scene Customization

Build scene customization on top of the current Graphshell stack:

- graph-canonical semantics,
- per-view scene overlay state,
- post-physics helper passes,
- egui render layers,
- canvas/physics/lens registry policy surfaces.

This option would support:

- node and edge style overrides,
- region backdrops and labels,
- anchors / attractors / soft zones,
- scene presets,
- lightweight save/load,
- label-density and decluttering policy,
- scene templates derived from graph semantics.

#### Strengths

- Best fit with current Graphshell architecture
- Lowest integration cost
- Preserves current graph-first and per-view invariants
- Easy to stage incrementally
- Best path for default shipped scene presets
- Minimal cognitive overhead for users and implementers

#### Weaknesses

- Cannot offer true rigid-body authoring semantics
- Authored region interactions remain custom-built
- Complex label packing or geometry may eventually need helper crates

### 5.2 Option B: Lightweight Geometry / Spatial Helpers

Retain Option A's Graphshell-owned model, but selectively add small helper crates when needed:

- `rstar` for spatial indexing and region queries
- `parry2d` for region geometry, intersection, and hit-testing
- `kiddo` or similar for nearest-neighbor and range queries

Important note: `rstar` is already present in Graphshell's dependency set, which lowers the cost of taking this path.

This option is appropriate for:

- label culling / overlap detection,
- region hit-testing,
- nearest attractor lookup,
- dynamic region membership queries,
- geometry-aware backdrop and hull logic.

#### Strengths

- Keeps authority and layout semantics Graphshell-owned
- Solves many practical scene problems without a second simulation substrate
- Better scaling than naive O(n²) helper passes in some cases
- Smaller conceptual jump than Rapier

#### Weaknesses

- Still requires custom scene logic
- Does not deliver a full physics-editor experience
- May accumulate bespoke geometry code if not disciplined

### 5.2A Collision-aware layout without Rapier

It is important not to frame "collision" as an all-or-nothing Rapier decision.

Graphshell can achieve a meaningful collision-aware scene layer without adopting a full rigid-body engine:

- stronger short-range separation,
- iterative overlap resolution after layout,
- circle/rect separation based on node bounds,
- wall or viewport containment,
- region hit-testing and exclusion,
- label-box packing or suppression,
- nearest-attractor and region-membership queries.

This path is sufficient for:

- "solid" presets that should feel packed and stable,
- no-overlap node behavior,
- simple walls and bounded canvases,
- collision-aware decluttering,
- soft region and attractor semantics.

What it does **not** naturally provide is the full authored-scene behavior set:

- bounce with believable contact response,
- sliding friction,
- durable joints/springs/ropes,
- a manipulable scene world with its own simulation identity.

### 5.3 Option C: Rapier-Backed Canvas Editor Mode

Use Rapier as a separate opt-in scene/editor mode, not as the default graph-layout substrate.

In this model:

- each node may correspond to a rigid body,
- regions become authored colliders or sensors,
- edges may optionally gain physical constraint meaning,
- users edit a scene world rather than only choosing graph presets,
- Graphshell provides a separate canvas-editor workflow.

This is the right option if the product goal includes:

- collisions or containment as first-class behavior,
- authored surfaces and boundaries,
- explicit spring/rope/rigid relationships,
- simulation-driven scene compositions,
- eventual 2D/3D physics-editor parity.

#### Strengths

- Richest authored-scene capability
- Natural substrate for a true scene editor
- Strong long-term path if Graphshell wants simulation as a first-class medium

#### Weaknesses

- Highest dependency and subsystem cost
- Highest persistence complexity
- Risks introducing a second spatial authority model
- Harder to scale for large graph browsing views
- Much more than most default customization use cases require

### 5.3A When Rapier wins

Rapier becomes the best path when Graphshell wants more than anti-overlap and region geometry:

- **bounce**: contact normals plus velocity response and restitution,
- **sliding friction**: tangential velocity damping against colliders and surfaces,
- **springs / joints**: stable constraint solving between bodies,
- **authored colliders / surfaces**: user-editable walls, basins, barriers, floors, and sensor regions,
- **true scene editing**: a physical world that is being manipulated directly, not just styled or softly biased.

If the target feature list prominently includes these behaviors, Rapier is likely the cleaner long-term substrate than continuing to grow a bespoke mini-physics layer.

---

## 6. Physics Engine Choice

### 6.1 Alternatives to Rapier

For Graphshell's purposes, the meaningful alternatives fall into three buckets:

| Option | Role | Fit |
| --- | --- | --- |
| `parry2d` | Collision geometry and spatial queries only | Excellent if Graphshell wants colliders/surfaces without a full physics world |
| `rstar` / `kiddo` | Spatial indexing / nearest-neighbor helpers | Good for scene-region queries and scaling light scene logic |
| `Rapier` | Full rigid-body physics engine with joints, contacts, queries, and optional determinism | Best fit for a true physical scene/editor mode |
| `salva2d` | Fluid simulation layer | Good future adjunct if Graphshell later wants liquid/particle scene behavior; not a substitute for the main scene substrate |
| `Avian` | ECS-driven Bevy-oriented physics engine | Poor fit for Graphshell's non-Bevy architecture |
| Box2D Rust crates | 2D rigid-body alternatives | Possible, but less natural and less obviously aligned with Graphshell's pure-Rust stack than Rapier |

### 6.2 Recommendation on engine choice

For Graphshell specifically:

- use **no full engine** for style-only and light scene overlays,
- use **`parry2d` + existing layout/runtime logic** if the next step is collision-aware layout, authored surfaces, and scene geometry without full physical world semantics,
- use **Rapier** once Graphshell clearly wants bounce, sliding, joints, authored colliders, and a real scene-editor mode,
- treat **`salva2d` as optional later augmentation** only if Graphshell explicitly wants fluid or particle-like scene behavior after the main scene substrate is established.

This yields a cleaner escalation path:

1. layout-native scene overlays,
2. `parry2d`-assisted collision/region/layout enrichment,
3. `rapier2d` scene mode only when the desired behaviors require it,
4. optional `salva2d` augmentation for fluid behaviors if the product later wants that specific class of scene simulation.

### 6.3 Repurposable pieces from the Rapier ecosystem

If Graphshell does **not** adopt Rapier immediately, the most valuable reusable piece is still `parry2d`.

`parry2d` is useful standalone for:

- intersection tests,
- collision shape definitions,
- hit-testing and overlap queries,
- contact and proximity checks,
- ray/shape casts,
- region geometry support for scene editing.

That makes `parry2d` a strong "middle tier" technology: significantly richer than bare force math, but much lighter than adopting a full rigid-body world.

---

## 7. Wiring Rapier Into Graphshell's Current Architecture

### 7.1 Current runtime shape

Today, Graphshell's graph canvas works broadly like this:

1. `render/mod.rs` sets `ActiveLayoutState` into `egui_graphs`
2. `GraphView<..., ActiveLayoutState, ActiveLayout>` executes one layout step
3. Graphshell reads back the updated layout state with `get_layout_state`
4. Graphshell applies post-physics helper passes such as frame-affinity or clustering
5. The resulting projected positions are reflected back into graph/runtime state

The important point is that `egui_graphs` is currently the render-facing graph widget, while layout behavior is already abstracted behind `ActiveLayout`.

### 7.2 Rapier should fit as another layout implementation, not a render rewrite

If Graphshell adopts Rapier, the cleanest integration is:

- keep `egui_graphs` for rendering and interaction,
- add a new `ActiveLayoutKind::RapierScene`,
- introduce `graph/layouts/rapier_scene.rs`,
- let that layout implementation own or reference a per-view `PhysicsWorld`,
- read body positions from the Rapier world and write them into the `egui_graphs` node locations during `Layout::next()`.

In other words:

- `egui_graphs` continues to draw the graph,
- `ActiveLayout` chooses the movement engine,
- Rapier becomes one engine among several, not a replacement for the graph widget layer.

### 7.3 Per-view scene ownership

The existing per-view architecture strongly suggests that a Rapier world should be **per graph view**, not global.

That means each view may hold:

- current scene mode,
- scene overlay data,
- optional Rapier world runtime,
- mapping from stable node ids to rigid bodies,
- optional authored regions/colliders/joints for that view.

This fits the existing "layout and camera are per view" rule and prevents one scene interpretation from leaking into sibling views.

### 7.4 Reconciliation between graph truth and Rapier world

The right model is not "Rapier becomes truth." It is "Rapier becomes a view-owned interpretation of graph truth."

Reconciliation rules should look like this:

- graph node added -> create a corresponding body in the view's world
- graph node removed -> remove body and any dependent joints
- graph pin/tag/frame-membership change -> update body constraints/properties or scene participation
- scene overlay changed -> rebuild only affected colliders/regions/joints
- body moved by simulation -> write projected position back into render-facing node location

This keeps graph semantics canonical while allowing the scene to be physically rich.

### 7.5 Persistence model for Rapier scenes

If Rapier scenes are persisted, they should still be treated as **view-owned scene state**, not graph-canonical truth.

The persisted scene snapshot would likely include:

- stable node-id to body bindings,
- body transforms and velocities,
- authored colliders/surfaces,
- optional joints and their parameters,
- scene-mode-specific settings.

That is compatible with the graph/scene/style layered model:

- graph = truth,
- scene = persisted view-owned physical projection,
- style = visual treatment over both.

### 7.6 Interaction with other layout algorithms

Rapier should not invalidate the plan to support multiple layout algorithms. It should simply occupy a different place in the portfolio:

- FR/Barnes-Hut -> browsing, exploration, topology legibility
- graph-native scene overlays -> semantic organization and storytelling
- Rapier scene mode -> authored physical scenes and richer behavioral canvases

That means `ActiveLayout` remains the dispatch seam:

- `ForceDirected`
- `BarnesHut`
- `RapierScene`
- future specialized layouts as needed

The key architectural rule is: **layout selection chooses the movement substrate, not the canonical graph model**.

---

## 8. Option Comparison

Scores are relative and directional: `5 = best fit`, `1 = weakest fit`.

| Dimension | Option A: Layout-native | Option B: Helper crates | Option C: Rapier mode |
| --- | --- | --- | --- |
| Product fit for default graph customization | 5 | 4 | 2 |
| Architectural fit with current semantics | 5 | 4 | 2 |
| Dependency weight | 5 | 4 | 1 |
| Persistence simplicity | 4 | 4 | 1 |
| Per-view runtime simplicity | 5 | 4 | 1 |
| Scalability for large browsing graphs | 4 | 4 | 2 |
| Value for shipped presets/templates | 5 | 4 | 2 |
| WASM / web portability | 5 | 4 | 3 |
| Long-term path to true scene editor | 2 | 3 | 5 |

### 8.1 Interpretation

- **Option A wins** for the problem Graphshell most immediately has: scene customization for graph browsing and organization.
- **Option B is the best tactical escalation path** when Option A begins to strain on geometry or performance.
- **Option C is compelling only if Graphshell explicitly wants a scene-editor product surface**, not simply a richer graph canvas.

---

## 9. Persistence and File-Model Analysis

The main risk here is accidental authority duplication. If scene customization becomes durable, Graphshell must avoid making scene files a parallel source of truth for graph/workbench semantics.

### 9.1 Model 1: Per-view overlay / projection file

`SceneFile` stores a scene overlay for one graph view:

- scene-specific style overrides,
- authored regions or labels,
- anchor/attractor configuration,
- scene preset references,
- view-scoped customization choices.

#### Pros

- Best match for per-view ownership
- Safest for graph-first semantics
- Easy to import/export/share
- Allows experimentation without mutating graph-canonical data

#### Cons

- Needs reconciliation if referenced nodes/edges are missing
- Must define stable keys and graceful degradation

### 9.2 Model 2: Embedded in frame/workspace snapshot state

Scene overlay is stored alongside graph-view state in snapshots.

#### Pros

- Natural UX for "this workspace/view opens with this scene"
- Keeps persistence near existing view/frame restore flows
- Avoids a separate always-external file requirement

#### Cons

- Less portable as a standalone artifact unless export is added later
- Requires disciplined snapshot ownership boundaries

### 9.3 Model 3: Graph-level canonical authority

Scene data is stored as graph truth.

#### Pros

- Strong consistency if the scene itself is the product

#### Cons

- Violates current graph/view separation for many customization concerns
- Risks a second meaning layer over frame/workbench semantics
- Makes stylistic or exploratory choices feel too authoritative
- Harder to support multiple views with different scene interpretations

### 9.4 Recommendation

Do **not** make scene customization graph-canonical by default.

Preferred narrowing:

1. **Runtime authority**: per-view scene overlay state
2. **Built-in persistence**: embedded in frame/workspace graph-view state when appropriate
3. **Portable artifact**: optional import/export via `SceneFile`
4. **Graph-canonical data stays canonical**: frames, memberships, tags, relations, and topology remain the underlying truth

This means an external scene file, if introduced, should serialize the same per-view overlay model. It should not establish a separate ontology of graph organization.

---

## 10. Conceptual Model to Evaluate

The following conceptual types are useful and small enough to reason about now, even though this memo is not an implementation spec.

### 10.1 Proposed conceptual vocabulary

| Type | Role | Likely ownership |
| --- | --- | --- |
| `SceneCustomizationSet` | Full per-view scene overlay | Per-view runtime; persistable in snapshot or file |
| `NodeStyleOverride` | Style override for specific nodes or selector-defined groups | Scene overlay |
| `EdgeStyleOverride` | Style override for edges or relation families | Scene overlay |
| `SceneRegion` | Labeled visual or behavioral region | Scene overlay; optionally editor-owned later |
| `SceneLabel` | Free or derived text/annotation object | Scene overlay |
| `SceneAnchor` / `SceneAttractor` | Soft spatial influence point/shape | Scene overlay |
| `ScenePreset` | Named reusable scene recipe/template | Registry or packaged asset |
| `SceneFile` / `SceneSnapshot` | Serialized representation of a scene overlay | Persisted artifact |

### 10.2 Ownership guidance

#### Graph-canonical

These should remain graph-owned:

- node identity,
- edge identity,
- tags,
- relation families,
- frames and frame membership,
- semantic metadata,
- topology and canonical addresses.

#### Per-view runtime state

These are best treated as view-owned:

- active scene overlay,
- active scene preset selection,
- per-view label density / decluttering policy,
- per-view visual emphasis choices,
- per-view attractors and region styling,
- scene-specific overrides that should not affect sibling views.

#### Saved scene artifacts

These are valid as serialized representations of per-view state:

- saved overlay definitions,
- authored region layouts,
- optional node/edge style overrides,
- scene template applications,
- editor-authored annotations.

#### Future editor-only state

Only if Graphshell later adopts a true scene editor:

- simulation world data,
- collision shapes,
- rigid-body parameters,
- springs / joints / surfaces,
- editor handles, tools, and non-graph geometry.

---

## 11. Default Graphshell Scene System

Even without Rapier, Graphshell could ship a compelling default scene-customization set.

### 11.1 Node appearance presets

Drive node appearance from existing graph semantics:

- by tag (`#starred`, `#focus`, `#archive`, user tags),
- by semantic class or UDC family,
- by domain / eTLD+1,
- by frame membership,
- by recency / DOI tier,
- by node type or provenance.

This builds naturally on `KnowledgeRegistry` label/color hints and the node badge/tagging contracts.

### 11.2 Edge appearance modes

Style edges by relation family or display mode:

- traversal emphasis,
- semantic-link emphasis,
- containment or arrangement emphasis,
- hidden-muted auxiliary edges,
- directional or weighted stroke variants.

This should remain presentation-level unless a later editor mode adds physical constraints as a separate concern.

### 11.3 Regions, backdrops, and labels

Graphshell can support a rich region system without full physics:

- derived frame-affinity regions,
- user-authored labeled backdrops,
- domain or semantic islands,
- per-region tint and border policy,
- region notes or title cards,
- region-as-attractor rather than region-as-collider.

### 11.4 Anchors and attractors

A light scene system should support:

- point attractors,
- region attractors,
- exclusion / repulsion regions,
- focus anchors,
- manual "gather these here" hints.

These fit the current layout-native extension model well.

### 11.5 Label density and decluttering

The existing graph UX research strongly suggests doing this before any heavy editor move:

- zoom-adaptive label policies,
- DOI-driven emphasis,
- overlap-aware label suppression,
- optional lightweight secondary label layout pass,
- region labels and node subtitles with bounded density rules.

This is high-value scene customization with much lower implementation risk than a full editor world.

### 11.6 Suggested shipped templates

Reasonable default scene templates include:

| Template | Intent |
| --- | --- |
| `Research Map` | semantic clustering + medium labels + highlighted citations/links |
| `Frame Atlas` | frame-affinity regions + strong backdrop labels + workbench organization emphasis |
| `Semantic Islands` | UDC/domain grouping with soft attractors and muted cross-cluster edges |
| `Archival Atlas` | calmer layout, decluttered labels, provenance- and time-aware emphasis |
| `Domain Constellation` | domain-colored nodes and stronger inter-domain separation |

These are immediately useful to Graphshell itself and do not require a second simulation substrate.

---

## 12. Critical Re-Examination of the Rapier Assumption

The foundational assumption worth challenging is:

> "If we want richer scenes, we probably need a physics engine."

The repo's current research and architecture suggest a different conclusion:

- most of the requested capability is about **scene semantics and presentation**, not rigid-body simulation,
- Graphshell already has a policy-driven layout and rendering surface that can carry much of this,
- the first valuable improvements are label, region, preset, and view-overlay improvements,
- the big risk is not lack of simulation power but lack of architectural discipline if a second authority model appears too early.

Rapier becomes justified only when at least one of the following is a real product requirement:

1. users need to author scene geometry, collisions, or surfaces directly,
2. edges need physical constraint semantics distinct from ordinary graph relations,
3. a "canvas editor" is intentionally a separate mode with its own mental model,
4. layout-native approximations become too bespoke, brittle, or expensive for the desired authored behaviors.

Until then, Rapier is likely overpowered for the default path.

---

## 13. Recommendation

### 13.1 Primary direction

Adopt **layout-native scene customization** as the default Graphshell direction.

That means:

- build a per-view `SceneCustomizationSet` overlay model,
- derive scene behavior from graph semantics when possible,
- keep scene authority outside graph-canonical topology,
- reuse the current layout and post-physics pipeline,
- add scene templates/presets before building a heavy editor substrate.

### 13.2 Fallback / escalation path

If this work hits geometry or scaling limits:

- first add targeted helpers such as `rstar` or `parry2d`,
- only then evaluate whether a separate scene-editor mode is needed,
- if yes, scope `rapier2d` as a **mode** with distinct persistence and runtime boundaries,
- if fluid behavior later becomes a concrete product need, evaluate `salva2d` as an additive layer on top of the scene-mode substrate rather than as a replacement for it.

### 13.3 Rapier verdict

Rapier is **not needed now for the default path**.

`rapier2d` remains a valid **later** choice for an opt-in canvas-editor/simulation mode if Graphshell decides it wants:

- authored simulation worlds,
- collision/surface semantics,
- physical constraints as first-class editable objects,
- or a future 2D/3D scene-editor surface.

`salva2d` is worth keeping in the research orbit for future fluid/particle scenes, but it should be treated as a specialized follow-on rather than part of the default Stage 1/2 substrate decision.

---

## 14. Staged Roadmap

### Stage 1 — Layout-native scene customization

Goal: deliver high-value scene expression without new heavy dependencies.

- Define a small per-view scene overlay model
- Add node/edge style override selectors
- Add labeled visual regions and anchors/attractors
- Add label-density and decluttering policy
- Add a small set of shipped scene templates
- Persist active scene overlays with graph-view/frame state

### Stage 2 — `parry2d`-assisted collision/region/layout enrichment

Goal: support more sophisticated scene behaviors without a full editor world.

- Use `rstar` more deliberately for region/index queries
- Add `parry2d`-backed geometry helpers for hit-testing, hulls, region queries, and collision-aware scene rules
- Improve performance and ergonomics for larger scenes
- Tighten import/export shape for optional `SceneFile`

### Stage 3 — `rapier2d` scene mode only if justified

Goal: support authored simulation scenes as a distinct product capability.

- Create a dedicated canvas-editor mode
- Define editor-only world/persistence boundaries
- Keep graph-canonical and view-scene semantics separate
- Add joints, surfaces, collisions, and physical scene authoring only where they are the actual product need

### Stage 3A — Optional `salva2d` fluid augmentation

Goal: support fluid or particle-like scene behavior only if Graphshell later wants that specific interaction class.

- Treat `salva2d` as an additive simulation layer, not as the main scene substrate
- Restrict it to explicit fluid/particle scenes or effects rather than general graph layout
- Keep the graph/scene/style authority model unchanged

---

## 15. Decision-Complete Conclusions

1. Graphshell should pursue scene customization now, but not by default through Rapier.
2. The smallest viable path is a Graphshell-owned, per-view scene overlay system built on the current layout/render stack.
3. Saved scenes are best understood as view-scoped overlays that may be embedded in snapshots and optionally exported as files.
4. Graph-canonical semantics must remain canonical; scene customization must not quietly replace frame/workbench authority.
5. Rapier should be revisited only when Graphshell deliberately chooses to build a true canvas editor, not merely richer graph organization and presentation.
