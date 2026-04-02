# Semantic Scene Scaffolding Note

**Date**: 2026-04-02
**Status**: Design note / capability matrix
**Priority**: High-value exploratory direction for the Distillery aspect

**Related**:

- `ASPECT_DISTILLERY.md`
- `distillation_request_and_artifact_contract_spec.md`
- `../aspect_projection/ASPECT_PROJECTION.md`
- `../aspect_projection/projection_interdomain_contract_spec.md`
- `../graph/2026-04-02_scene_mode_ux_plan.md`
- `../graph/2026-04-02_parry2d_scene_enrichment_plan.md`
- `../../research/scene_customization.md`
- `../graph/2026-03-11_graph_enrichment_plan.md`
- `../graph/semantic_tagging_and_knowledge_spec.md`
- `../graph/node_badge_and_tagging_spec.md`
- `../../technical_architecture/graphlet_model.md`
- `../../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`

---

## 1. Purpose

This note documents how the Distillery aspect should think about automatic scene generation.

Primary recommendation:

- Graphshell should not pursue decorative "AI scene generation" as the first target.
- The correct first target is **semantic scene scaffolding**.

That means distillery-derived spatial output should answer questions like:

- what bounded local world is active,
- what regions or lanes would make this graph easier to think with,
- what scene mode fits the current task,
- what should be foregrounded or hidden,
- and why the system suggested that structure.

This keeps automatic scene generation from becoming gimmicky.

---

## 2. Canonical Framing

Automatic scene generation is valuable only when it acts as a projection and arrangement aid rather than a decorative style effect.

Distillery should therefore split the problem into two cooperating outputs rather than one merged "scene scaffold" pipe.

1. **Arrangement scaffolding**
   - structural proposal for bounded local worlds, regions, lanes, anchors, grouping, and placement logic
2. **Scene suggestion**
   - view-level proposal for scene mode, visibility policy, relation reveal policy, emphasis, and matching preset behavior

These two should work together, but they are not the same thing.

Arrangement scaffolding answers:

- what should be grouped,
- what regions or lanes should exist,
- what local world or graphlet scope is relevant,
- where the user's work should spatially cohere.

Scene suggestion answers:

- how the current view should behave,
- which scene mode fits the task,
- what should be revealed or suppressed,
- how much motion, density, and relation visibility is appropriate.

Good outputs:

- arrangement scaffold proposals
- graphlet-scoped local worlds
- region, lane, and anchor proposals
- scene-mode recommendations (`Browse`, `Arrange`, `Simulate`)
- visibility and relation-reveal policy suggestions
- layout/physics/presentation preset suggestions
- concise explanations for why the scaffold was proposed

Bad outputs:

- arbitrary mood-board decoration
- purely aesthetic theme swaps with no semantic explanation
- irreversible automatic rearrangement with no user acceptance step
- hidden semantics that users cannot inspect, reject, or tune

---

## 3. Distillery Role

Within the Distillery aspect, semantic scene work should be modeled as two nearby transform families that consume approved graph and workspace-derived source classes and emit typed spatial-intelligence artifacts.

The role of distillery here is not to own the final scene.

Instead it should:

1. read approved source classes,
2. derive candidate arrangement structure,
3. derive candidate scene behavior and view policy,
4. classify the result as a typed artifact,
5. provide explanation and provenance,
6. hand the proposal to Projection / Graph / View-owned runtime layers for acceptance, preview, or application.

That preserves authority:

- Distillery suggests,
- Projection and scene runtime host,
- Graph and view state remain authoritative.

---

## 4. Candidate Source Classes

The strongest first-pass scene scaffolding inputs are already present in Graphshell's own systems.

High-value existing source classes:

- graph topology and relation families
- active graphlet or graphlet candidate scope
- UDC classes and semantic tags
- user tags and reserved system tags
- frame membership and frame-affinity state
- workbench arrangement correspondence
- traversal history and recency
- active graph view and scene mode
- current scene runtime regions and bounds

Later model-facing inputs may add:

- embedding similarity neighborhoods
- generated summaries
- extracted structured facts
- episode and `AWAL` behavior traces

---

## 5. Capability Matrix

### 5.1 No new dependencies: existing systems only

Using only existing Graphshell systems and already-landed semantics, Graphshell can already support a meaningful first generation of arrangement scaffolding and scene suggestion.

Feasible now in principle:

1. **Graphlet arrangement scaffolds**
   - derive an arrangement scaffold around ego, corridor, component, frontier, or facet graphlets
   - choose `Browse` versus `Arrange` defaults based on graphlet shape and density

2. **Semantic district generation**
   - create labeled regions from UDC prefixes, tags, domains, or frame-affinity groupings
   - suggest names from known labels rather than model-generated copy

3. **Workbench correspondence arrangement scaffolds**
   - generate a scaffold from the current open panes, frames, and nearby graph context
   - propose regions like `Current`, `Reference`, `To Revisit`, or frame-derived neighborhoods

4. **Session and recency arrangements**
   - create temporal lanes or session slices from traversal history and current selection
   - use existing graphlet/session concepts rather than new ontology

5. **Scene suggestions**
   - suggest whether edges should be subdued, peek-only, or temporarily emphasized
   - recommend `Reveal Nodes` / `Semantic X-Ray` defaults by scene density and mode

6. **Preset recommendation**
   - choose among existing layout/physics/scene presets based on topology and semantic distribution

Why this is strong:

- it is explainable,
- it is local-first,
- it does not require any model,
- and it fits the current graph/view/scene/style layering.

### 5.2 Existing dependencies plus existing systems

Using Graphshell's current dependency surface together with the existing systems, the next tier gets geometrically and spatially smarter without needing new model dependencies.

Strong candidates:

1. **Automatic region fitting**
   - use current graph positions plus `parry2d`/geometry helpers to fit better bounds and region envelopes around semantic clusters or graphlets

2. **Spatially cleaner arrangement scaffolds**
   - use the scene runtime to place attractors, repulsors, dampeners, and containment regions automatically instead of only by hand

3. **Path and corridor arrangements**
   - use `petgraph` reachability, shortest-path, and frontier logic to build corridor and bridge scaffolds more intelligently

4. **Timeline and lane arrangements**
   - use traversal and recency information to generate lanes, queues, shelves, or archive strips

5. **Semantic-density moderation**
   - choose region density, label strategy, and relation visibility from graph size, cluster count, and frame/workbench context

This tier is probably the best near-term path because it makes the output feel more deliberate without introducing the privacy, runtime, and model-management costs of full local intelligence.

### 5.3 New dependencies

New dependencies become compelling when Graphshell wants to infer latent semantics that are not already explicit in the graph.

Useful classes:

1. **Embeddings / local semantic models**
   - infer topic neighborhoods beyond explicit tags and links
   - cluster mixed-content nodes that share meaning but not metadata

2. **Small local text models**
   - propose better region names
   - produce concise "why this scene" explanations
   - synthesize scene intent from a graphlet or workbench slice

3. **Vision or multimodal models**
   - derive scene scaffolds from image-heavy or screenshot-heavy workspaces
   - help classify visual clusters where text metadata is weak

4. **Full rigid-body scene dependencies**
   - `rapier2d` or later 3D physics only if Graphshell explicitly wants authored simulation worlds, joints, surfaces, or scene-editor workflows

This tier is powerful, but it should build on an already-legible non-model scene scaffolding system. Otherwise Graphshell risks adding expensive intelligence to a weak user-facing contract.

---

## 6. Proposed Distillery Outputs

Scene generation should enter distillery as typed artifacts, not as direct scene mutation.

Recommended first artifacts:

1. `ArrangementScaffold`
   - a proposal for regions, lanes, anchors, bounds, graphlet scope, and grouping logic

2. `SceneSuggestion`
   - a proposal for scene mode, visibility policy, relation reveal policy, emphasis, and matching preset behavior

3. `ProjectionRecommendation`
   - recommendation for graphlet shape, scope, or projection form that should precede or accompany arrangement scaffolding

4. `SceneExplanation`
   - concise explanation of why the scaffold or suggestion was proposed, tied to graph/topology/tag/workbench evidence

5. `SpatialHintSignal`
   - lightweight hints such as suggested primary anchor, likely basin center, density warning, or relation-peek recommendation

These outputs should remain proposals until a user or policy path accepts them into view-owned runtime state.

### 6.1 Cooperation rule

Arrangement scaffolding and scene suggestion should be allowed to run separately.

- Arrangement scaffolding may exist without any strong scene suggestion.
- Scene suggestion may refine an already-existing hand-authored arrangement.
- The highest-value path is usually: `ProjectionRecommendation` -> `ArrangementScaffold` -> `SceneSuggestion`.

This keeps the structural and behavioral parts of the feature decoupled.

---

## 7. Coolest Straightforward Feature

The coolest straightforward feature is not full automatic world-building.

It is arrangement scaffolding plus scene suggestion for the active graphlet or workbench slice.

Concrete user experience:

1. user selects a node, graphlet, or frame
2. Graphshell offers `Scaffold Scene`
3. the system proposes:
   - a graphlet scope if needed,
   - an arrangement scaffold,
   - labeled regions,
   - one or more anchors,
   - then a scene suggestion,
   - a scene mode,
   - a visibility and relation-reveal policy,
   - a matching layout/physics preset,
   - and a short explanation
4. user previews, accepts, forks, or rejects

That is cool because it makes the graph feel like a thinking surface, not because it is flashy.

---

## 8. Main Blockers

The main blockers are not model quality. They are architecture and UX maturity.

### 8.1 Explanation surfaces

Graphshell still needs stronger user-facing explanation for inferred semantics.

Without that, automatic scene scaffolding becomes opaque.

### 8.2 Provenance-bearing semantic records

The enrichment lane still calls out missing durable confidence/provenance/status storage for inferred metadata.

If scene scaffolds depend on weak or invisible semantic state, user trust will be low.

### 8.3 Projection runtime maturity

Graphlet and Projection are conceptually strong, but the generalized runtime contract is still mostly documentation rather than shared implementation.

Scene scaffolding wants that runtime badly.

### 8.4 Scene persistence boundary

Current scene runtime remains intentionally runtime-only.

That is fine for experimentation, but the memorable version of the feature wants a clean persistable view-overlay model.

### 8.5 Distillation boundary and local-intelligence plumbing

Once model-derived scene suggestions enter the system, the privacy boundary, source-class rules, and local-model contract all matter.

This is a good reason to keep the first generation model-free or model-light.

---

## 9. Non-Gimmick Rule

Automatic scene generation should be considered successful only if all of the following are true:

1. it improves thinking, navigation, or organization,
2. it is explainable,
3. it is previewable and rejectable,
4. it preserves graph and view authority boundaries,
5. it does not depend on decorative novelty for its value.

If those conditions fail, the feature is gimmicky even if the output looks impressive.

---

## 10. Recommended Execution Order

1. Build model-free arrangement scaffolding from graphlets, tags, frame affinity, workbench context, and current scene runtime.
2. Add model-free scene suggestion on top of those arrangements.
3. Add geometry-aware automatic region fitting and scene policy refinement using the current dependency surface.
4. Only then add model-derived naming, summarization, and latent clustering where they clearly improve the scaffold or suggestion.
5. Treat fully simulated or heavily styled scene generation as a later branch, not the first value proposition.

This keeps the feature cool for the right reason: it helps the user think with the graph.
