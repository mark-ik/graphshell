# Scene Mode UX Sketch (2026-04-02)

**Status**: Research / UX sketch
**Purpose**: Describe how a richer graph scene should feel and behave for the user when Graphshell evolves from a graph viewer into a spatial thinking surface.

**Related**:

- `scene_customization.md`
- `../implementation_strategy/graph/2026-04-02_parry2d_scene_enrichment_plan.md`
- `../implementation_strategy/graph/2026-02-24_physics_engine_extensibility_plan.md`
- `../implementation_strategy/graph/graph_node_edge_interaction_spec.md`
- `../implementation_strategy/graph/layout_behaviors_and_physics_spec.md`
- `../implementation_strategy/graph/multi_view_pane_spec.md`
- `2026-02-18_graph_ux_research_report.md`

---

## 1. Executive Summary

Graphshell's graph canvas should not be limited to "a graph that happens to be visible."

Its unique value is that the same underlying graph can support multiple spatial readings:

- a calm semantic browsing surface,
- an arrangement surface for deliberately shaping thought,
- a richer simulated scene where nodes become objects in a world.

Those are not separate products. They are escalating modes of the same graph model.

The graph remains canonical. The scene remains a projection. The user experience changes because the projection becomes richer, not because the graph stops being a graph.

The best version should feel halfway between:

- a knowledge map,
- a corkboard,
- a physics sandbox,
- and an information architecture tool.

The quality bar is:

- motion should settle, not jitter forever,
- collisions should clarify, not create chaos,
- regions should be soft and understandable,
- presets should have names that describe felt behavior,
- user actions should produce visible, explainable consequences.

---

## 2. Product Thesis

Classic browsers treat tabs and panes as the primary workspace.

Graphshell can differentiate itself by making the graph canvas itself a workspace:

- a place to browse semantic structure,
- a place to arrange information spatially,
- a place to simulate or inhabit a scene when that helps thinking.

This is valuable because graph semantics are not tied to one arrangement:

- edges may be visible or hidden without altering graph truth,
- graphlets can be foregrounded or suppressed depending on view policy,
- nodes can be represented as abstract graph markers or as scene objects,
- layout and physical projection can change while the underlying graph remains stable.

That means a richer scene mode is not "breaking" the graph. It is an alternative way of thinking with it.

---

## 3. The Three User Modes

### 3.1 Browse

**User intent**: explore the structure of information without heavy authoring.

The graph feels:

- calm,
- legible,
- mostly semantic,
- lightly animated,
- oriented around discovery rather than manipulation.

Expected characteristics:

- nodes drift or settle gently,
- edges are understated or hidden until useful,
- regions/backdrops are soft contextual cues,
- selected nodes can "peek" relationships without flooding the canvas,
- presets feel like browsing atmospheres rather than scene toys.

What the user should be able to do:

- select a node and reveal its immediate relationships,
- highlight its graph neighborhood,
- temporarily reveal relation families,
- switch scene/layout presets like `Liquid`, `Solid`, `Atlas`, `Constellation`,
- move between graph views without losing per-view scene interpretation.

### 3.2 Arrange

**User intent**: deliberately compose a space for thinking.

The graph feels:

- spatially meaningful,
- directly manipulable,
- organized by user intent rather than only by algorithmic layout.

Expected characteristics:

- users can draw or define regions,
- regions can mean "Current", "To Read", "Archive", "Math", "Work", "Inbox", etc.,
- nodes can be gathered, separated, pinned, or nudged into areas,
- tags, domains, and frames influence where things belong,
- arrangement remains soft and understandable rather than rigid and brittle.

What the user should be able to do:

- create a region and label it,
- pull all nodes of a tag/domain/frame into that region,
- add anchors or attractors,
- create soft walls or containment areas,
- pin structural nodes,
- save this arrangement as part of the view/workspace state.

### 3.3 Simulate

**User intent**: work with the graph as a living scene of objects.

The graph feels:

- more physical,
- more playful,
- but still semantically grounded.

Expected characteristics:

- nodes behave like objects in a world,
- regions can become basins, barriers, pens, lanes, or neighborhoods,
- edges are usually hidden by default but remain revealable on demand,
- relation peeking and "x-ray" overlays preserve graph legibility even in busy scenes,
- physical behavior reinforces semantic organization rather than competing with it.

What the user should be able to do:

- drag objects and let them settle,
- use region behaviors to sort and separate ideas,
- temporarily reveal graph structure over the scene,
- toggle "show all nodes clearly" when visual richness obscures object identity,
- work in a scene that behaves as a spatial cognitive tool rather than a toy.

---

## 4. Core Interaction Ideas

### 4.1 Peek Relations

In richer scenes, edges should not need to remain permanently visible.

`Peek Relations` is the core bridge between object-world UX and graph truth:

- select or hover a node-object,
- reveal its relevant relationships as a temporary overlay,
- fade those overlays back out when focus changes.

This allows scenes to stay visually rich without sacrificing graph explainability.

### 4.2 Reveal Nodes

When a scene is visually busy, users need a quick way to distinguish:

- graph node-objects,
- region/backdrop elements,
- scene-only affordances.

`Reveal Nodes` should temporarily:

- halo or outline every graph node-object,
- increase label clarity,
- reduce decorative scene emphasis.

This is a scene-comprehension aid, not a mode switch.

### 4.3 Gather Here

The most useful authoring interaction is not low-level manipulation. It is semantic spatial commands.

Examples:

- gather all nodes with this tag here,
- gather nodes related to the current selection here,
- pull all nodes in this frame into this area,
- separate domain groups into lanes,
- move the current graphlet toward this attractor.

This is what makes the scene a thinking tool rather than just a simulation surface.

### 4.4 Regions as workflow language

Regions should feel like understandable workflow categories:

- neighborhoods,
- rooms,
- basins,
- lanes,
- shelves,
- inboxes,
- archives,
- current-work zones.

The user should not need to think in physics jargon. Region names and behavior should reflect felt workflow meaning.

### 4.5 Semantic X-Ray

In simulate mode especially, the user needs a way to recover graph semantics without abandoning the scene.

`Semantic X-Ray` should:

- reveal hidden relation overlays,
- surface family-specific edges,
- accentuate graphlets or neighborhoods,
- optionally dim scene decoration while active.

This keeps the "object world" and the "graph world" composable rather than mutually exclusive.

---

## 5. User Journeys

### 5.1 First-time user

The user opens Graphshell and sees a calm graph.

They:

- click nodes,
- pan and zoom,
- select a scene preset,
- notice that selecting a node can reveal nearby relationships,
- gradually understand that the graph is not just a static diagram but a navigable space.

Success condition:

- the user feels the graph is expressive and calm, not chaotic or gimmicky.

### 5.2 Power user organizing a research session

The user has many nodes across several themes.

They:

- create regions,
- label them by task or subject,
- gather nodes into those areas,
- use attractors and walls to give the session structure,
- temporarily reveal relationships when needed,
- switch between calm browsing and stronger arrangement behaviors.

Success condition:

- the user feels like they are composing a space for thought, not merely sorting a list or tuning an algorithm.

### 5.3 Long-lived scene workspace

The user returns to a saved graph view / workspace and expects:

- regions to still mean what they meant,
- scene interpretation to still feel familiar,
- semantic groupings to still reinforce memory,
- scene behavior to help re-entry into prior work.

Success condition:

- the scene acts like spatial memory for the user's graph work.

---

## 6. UX Principles

### 6.1 Simulation must remain semantically useful

Every motion or physical behavior should help answer at least one of:

- what belongs together,
- what matters now,
- where should this go,
- what is this related to,
- what kind of thing is this,
- what space am I working in.

If a behavior does not improve one of those, it is decoration and should be treated skeptically.

### 6.2 Calm by default

The default graph experience should remain:

- stable,
- readable,
- settled,
- and welcoming.

`Simulate` should feel like an intentional escalation, not the default burden placed on every graph view.

### 6.3 Scene language over physics jargon

Users should encounter:

- rooms,
- neighborhoods,
- basins,
- gather,
- settle,
- reveal,
- emphasize,
- cluster,

before they encounter:

- colliders,
- damping factors,
- restitution,
- constraints.

Graphshell can still expose advanced controls later, but the first language should be experiential.

### 6.4 Relationship visibility should be demand-driven

Permanent edge visibility is often too noisy for scene-oriented work.

The better model is:

- hidden or subdued by default,
- revealable on demand,
- scoped to the current focus or question.

### 6.5 Space should reinforce memory

The scene should help users remember:

- task areas,
- semantic neighborhoods,
- graphlets of current interest,
- what they were doing when they last visited a workspace.

That is what turns "physics-oriented UI" into a real productivity feature.

---

## 7. Recommended UX Direction

The recommended UX direction is:

1. Keep `Browse` calm and semantically legible.
2. Make `Arrange` the first serious authoring mode.
3. Let `Simulate` evolve as a richer scene mode without making it the default burden on every graph view.
4. Treat hidden/revealable relationships as a first-class scene literacy tool.
5. Design presets and region behaviors in terms of felt meaning, not raw physics terms.

This supports the product thesis that Graphshell's canvas is not merely a graph visualization.
It is a spatial workspace for thought.
