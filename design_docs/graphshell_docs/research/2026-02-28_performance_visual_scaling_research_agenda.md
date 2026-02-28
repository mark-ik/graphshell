# Performance and Visual Scaling — Research Agenda

**Date**: 2026-02-28
**Status**: Research / Active
**Author**: Arc
**Feeds Into**:
- `implementation_strategy/subsystem_diagnostics/2026-02-24_performance_tuning_plan.md`
- `implementation_strategy/canvas/2026-02-25_doi_fisheye_plan.md`
- `implementation_strategy/canvas/2026-02-24_layout_behaviors_plan.md`
- `implementation_strategy/canvas/2026-02-25_progressive_lens_and_physics_binding_plan.md`
- `research/2026-02-18_graph_ux_research_report.md`

---

## Background

Performance and visual scaling is where the graph UI either succeeds as a daily-use tool or
becomes a demonstration. The current plans set hard engineering targets (500 nodes @ 60 FPS,
1000 nodes @ 30 FPS) and define a five-phase technical approach (viewport culling, LOD,
badge budget, physics budget, diagnostics guard). The DOI plan defines four rendering tiers
tied to recency/frequency/interest/distance scores. The layout plan adds semantic clustering,
magnetic zones, and physics micro-behaviors. Progressive Lens switching resolves the policy
for automatic LOD changes at zoom breakpoints.

What none of these plans address is the user side of scaling: at what graph size does the
tool stop feeling useful, at what LOD transition does the display stop communicating
structure, at what physics state does motion become a distraction rather than an aid, and
which camera control idioms feel stable vs. disorienting. These are empirical questions that
the engineering specs cannot answer from first principles.

The user framed the risk precisely: a graph UI lives or dies on this. A graph that is fast
but visually incoherent at 100 nodes is worse than a tree-view. A graph that is beautiful at
50 nodes but collapses into an unreadable hairball at 200 nodes fails the productivity case.

---

## Thread 1 — Usable Node/Edge Density: Where the Graph Breaks Down

### What the Docs Say

The performance tuning plan targets 500 nodes @ 60 FPS and 1000 nodes @ 30 FPS as
engineering correctness thresholds. These are frame-rate targets, not comprehension targets.
The graph could render 1000 nodes at 60 FPS and be completely unreadable to a user.

The graph UX research report identifies the five canonical aesthetic criteria in descending
priority for user task performance: minimize edge crossings, prevent node overlap, uniform
edge length, maximize angular resolution, expose symmetry. For Graphshell specifically, it
adds mental map preservation (incremental add-node model) and neighborhood preservation
(browsing sessions and topic clusters).

The research report identifies 50–150 nodes as the expected practical scale in graphshell's
browsing context. At >200 nodes, O(N²) FR degrades frame rate without a step budget. The
"Tight" physics preset (Preset C) is the designed response to 80+ node graphs — lower
repulsion, higher attraction, smaller k_scale.

The DOI plan defines a Ghost tier (DOI < 0.10) specifically for nodes that provide
structural context but should be visually minimized. This is the designed response to
density at the rendering layer. But the tier thresholds (0.10, 0.30, 0.65) are
analytically derived, not empirically validated against the density at which users find
the graph unreadable.

### What Is Not Known

1. **The comprehension cliff.** At what node count does a user stop being able to
   extract meaning from the graph without filtering or zooming? The engineering target
   (1000 nodes @ 30 FPS) may be well beyond the comprehension cliff. If the cliff is at
   150 nodes, the system needs progressive disclosure features at that threshold, not just
   the 500-node FPS target.

2. **Task-dependent density tolerance.** The density at which the graph becomes
   unreadable depends on the task. "Follow the path from A to B" fails at lower density
   than "identify clusters of related content." Does the task the user is performing
   correlate with how much graph density they can tolerate?

3. **Edge density vs. node density as the primary readability driver.** A 200-node
   graph with 400 edges (sparse) may be readable; a 100-node graph with 900 edges (dense)
   may not be. Edge crossing count — not node count — is the primary readability
   degrader according to Purchase (2002). The performance targets are framed in node
   count, but edge count may be the binding variable for comprehension.

4. **The DOI Ghost tier in practice.** When Ghost-tier nodes (DOI < 0.10) render as
   faint dots, do users perceive the resulting display as "graph with structure preserved"
   or as "cluttered background noise"? The design intent is structural context; the user
   experience might be visual soup.

5. **"Show all" vs. "show relevant" as the default.** At density thresholds where the
   full graph is unreadable, the designed response is DOI-tier dimming + semantic
   fisheye. But some users may prefer a hard filter (hide low-DOI nodes entirely) even
   though the DOI plan explicitly prohibits hiding nodes (filter is the search system's
   job, not DOI's). Does the DOI overlay approach satisfy the readability need, or do
   users reach for the filter control regardless?

### Research Methods

**Study 1.1 — Comprehension Cliff Calibration (within-subjects, n=20)**
Present participants with graphs of increasing node count: 20, 50, 100, 200, 400 nodes.
All graphs are randomly generated with a consistent edge density ratio (4 edges per node).
For each graph, give a task: (a) "Find the most-connected node," (b) "Identify three
clusters of related nodes," (c) "Trace the path from node X to node Y." Measure accuracy
and time on task. Record the node count at which accuracy first drops below 70% — that is
the comprehension cliff for each task type.

Produces: a task-stratified comprehension cliff table. Compare against the 500/1000 FPS
engineering targets. If the cliff is at 150 for path-tracing and 400 for cluster-finding,
the performance plan's scope may need a density-keyed feature gate (trigger DOI
progressively based on comprehension risk, not just FPS).

**Study 1.2 — Edge Density as Readability Driver (between-subjects, n=30)**
Present two graphs: Group A gets a 200-node / 400-edge graph (sparse). Group B gets a
100-node / 800-edge graph (dense). Same task: "How many distinct topic clusters do you
see?" Measure accuracy and confidence. If Group B performs worse despite fewer nodes,
edge density is the binding variable and the performance plan's node-count framing needs
to add an edge-count dimension.

**Study 1.3 — DOI Ghost Tier Perception (moderated, n=12)**
Enable DOI rendering with a 300-node graph. Ghost-tier nodes are faint dots (alpha 0.15,
radius 0.3× base). Ask: "Describe what you see in the background." Probe: "Does the
background feel like useful context or visual noise?" Record descriptions. If ≥50% describe
Ghost tier as "noise" or "clutter" rather than "structure," the Ghost tier rendering
(alpha, radius, or density cap) must be tuned before DOI ships.

**Study 1.4 — DOI vs. Hard Filter Preference at Density (moderated, n=10)**
Present a 400-node graph with DOI enabled (Ghost tier visible) and give a retrieval task.
After 3 minutes, offer the choice: "Would you like to hide everything except the top 20%
of nodes?" Record how quickly participants take the offer, and whether those who decline
still reach for the filter control independently. If ≥60% prefer hard filter, re-evaluate
whether the DOI plan's "no hiding" invariant is correct at high density.

### Deliverable

A **Density Scaling Profile**: a table mapping node/edge count ranges to recommended
rendering behavior (full DOI overlay / progressive DOI with Ghost tier / hard filter
trigger), with empirically-derived thresholds replacing the analytically-derived DOI tier
boundaries. Feeds into `2026-02-25_doi_fisheye_plan.md` tier thresholds and
`2026-02-24_performance_tuning_plan.md` Phase 2 LOD trigger points.

---

## Thread 2 — LOD Rules That Preserve Meaning vs. Becoming Visual Soup

### What the Docs Say

The performance tuning plan defines LOD at three levels:
- **Node label LOD**: zoom < 0.5 hide labels; 0.5–1.5 domain-only; > 1.5 full title.
- **Label occlusion culling**: greedy rect packing ranked by selection + graph importance.
- **Edge LOD**: zoom < 0.3 hide/policy; 0.3–0.8 reduced alpha/width, no arrowheads;
  ≥ 0.8 full styling.

The DOI plan adds a parallel rendering-emphasis layer on top of zoom-adaptive LOD: High
tier promotes a node to full label even at zoom levels where zoom-adaptive LOD would
suppress it; DOI never demotes below the zoom-LOD floor.

The progressive Lens plan defines zoom-threshold breakpoints for automatic Lens switching
(e.g., at zoom scale 0.4, switch to overview Lens with `physics:gas`). A ±10% hysteresis
band prevents oscillation at boundaries.

The graph UX research report lists three specific anti-patterns: "do not hide node
identity under zoom" (show at minimum a colored dot or favicon), "do not expose raw
physics parameters as primary UI" (named presets first, sliders second), and "do not use
the same gesture for pan and lasso."

The research report also defines zoom levels for typical use: 0.3–0.5 for full overview,
1.0 for daily use, 2.0–3.0 for reading full URLs and edge labels.

### What Is Not Known

1. **Where the zoom-label boundary feels natural.** The domain-only label at zoom 0.5
   is an engineering choice. Does losing the page title at that zoom level cause users to
   lose orientation, or is the domain name sufficient to maintain context? The answer
   depends heavily on the user's graph content — a researcher with 20 tabs from the same
   domain (arxiv.org) loses all label differentiation at the domain-only threshold.

2. **Edge LOD and structural legibility.** When edges reduce to low-alpha thin lines at
   zoom < 0.8, do users still perceive the graph topology, or does the graph read as a
   cloud of nodes with no structure? Edge crossings are the primary readability driver
   (Thread 1) — reducing edge opacity may help dense graphs by reducing visual noise, or
   it may hide the structure that makes the graph meaningful.

3. **DOI label promotion in practice.** The DOI plan says High-tier nodes get full labels
   even at low zoom where zoom-LOD would suppress them. In a 200-node graph at 0.4 zoom
   with 30 High-tier nodes and 170 Ghost-tier nodes, 30 labels float over a sea of faint
   dots. Is that a useful focus effect or a confusing anomaly where labels seem detached
   from node positions?

4. **Progressive Lens switch perception.** The progressive Lens plan defaults to `Ask`
   for both lens-physics binding and progressive auto-switch. The Ask mode shows a non-
   blocking toast. Does a toast at a zoom threshold feel like the app is offering help, or
   does it feel like an interruption during navigation? Does the ±10% hysteresis prevent
   perceived oscillation, or does the threshold transition still feel jerky?

5. **The right LOD signal for "I cannot read this."** When labels are suppressed, edges
   are thin, and nodes are colored dots, the information in the graph is still theoretically
   present — positions and colors. Is position-and-color at low LOD sufficient for users
   to navigate, or do they need an explicit affordance ("zoom in to read" or "switch to
   overview mode") to understand why the labels are gone?

### Research Methods

**Study 2.1 — Label LOD Orientation Retention (within-subjects, n=20)**
Build three graphs with content from a real browsing session (mix of domains): 30 nodes,
all from 5–6 domains. Present at zoom level 0.5 (domain-only threshold). Task: "Find the
tab you opened yesterday about machine learning." Measure time and accuracy. Then repeat at
zoom 0.4 (labels hidden). Measure time and accuracy difference. If the domain-only LOD
causes ≥30% accuracy drop for graphs where multiple nodes share a domain, the 0.5 threshold
must be tuned upward, or the label LOD must respect DOI even at low zoom.

**Study 2.2 — Edge LOD Topology Perception (between-subjects, n=24)**
Group A sees a 150-node graph at zoom 0.6 with full edge styling. Group B sees the same
graph at zoom 0.6 with reduced-alpha thin edges (the Phase 2 LOD behavior). Task: "How
many distinct clusters do you see? Roughly how many nodes are in the largest cluster?" Record
accuracy. If Group B's accuracy degrades by >20%, the edge LOD threshold (currently 0.8)
must be tuned or an alternative edge representation (e.g., bundled edges, cluster outline)
must be added at the low-zoom tier.

**Study 2.3 — DOI Label Promotion Perception (moderated, n=10)**
Show a 200-node graph with DOI active, at zoom 0.4. High-tier nodes (30 nodes) show full
labels; all others show no labels. Ask: "Do you find the highlighted labels helpful?
Distracting? Confusing?" Probe: "Do you understand why some nodes have labels and others
don't?" Record whether users spontaneously understand the DOI premise or attribute the
floating labels to a bug.

**Study 2.4 — Progressive Lens Transition UX (moderated, n=10)**
Zoom a graph slowly from 1.0 to 0.3 across a progressive Lens breakpoint at 0.4. Present
in two conditions: (a) `Ask` mode — toast appears at threshold; (b) `Always` mode — silent
switch. For `Ask` mode: measure whether users read and respond to the toast or dismiss it.
For `Always` mode: measure whether users notice the physics/visual change and what they
attribute it to. Produces a recommendation on whether `Always` is safe as a default or
`Ask` is required.

### Deliverable

A **LOD Threshold Calibration Report** specifying:
- Revised zoom thresholds for label LOD (replacing the 0.5 / 1.5 design values with
  empirically-validated breakpoints).
- Revised edge LOD alpha/visibility threshold (replacing the 0.3 / 0.8 design values).
- A recommendation on DOI label promotion behavior at low zoom (keep, remove, or gate by
  domain-diversity of the visible node set).
- A recommendation on progressive Lens auto-switch default (`Always` vs. `Ask` vs. `Never`).

Feeds into `2026-02-24_performance_tuning_plan.md` Phase 2 and
`2026-02-25_doi_fisheye_plan.md` tier thresholds and the progressive Lens plan §2.4.

---

## Thread 3 — Camera Controls and Zoom Semantics: Stability and Predictability

### What the Docs Say

The graph UX research report defines the current zoom range as `[0.1, 10.0]` and recommends
reducing the upper bound to 5.0 (labels become pixel-perfect below that; higher zoom adds no
information). It recommends zoom-to-selected (`Z` key with selection: fit viewport to
bounding box of selected nodes with 20% padding), keyboard zoom (`+`/`-`/`0`), and canvas
gravity (weak centering force to prevent nodes drifting off-screen).

The semantic fisheye pass (from the DOI plan) applies cursor-distance scaling to draw size
only — node `(x, y)` positions are never modified. The scale formula is
`max(1.0, 3.0 * (1.0 - dist / fisheye_radius))` where the default fisheye radius is
300.0 canvas units.

The interaction spec defines: scroll = zoom, pinch = zoom, background drag = pan, drag node
= move. The drag threshold (4–8 px before committing to either pan or node-drag) is standard
practice from D3 and Cytoscape.js defaults.

The progressive Lens plan adds zoom-threshold Lens switching as a distinct camera-driven
behavior: at zoom scale 0.4, the graph can automatically switch to a different physics/
visual profile. This is a camera-position-driven state change layered on top of
continuous zoom.

The multi-pane plan isolates culling and LOD per graph pane — each pane has its own
`MetadataFrame` camera oracle. Zoom state in one pane does not affect another.

### What Is Not Known

1. **Zoom cursor anchoring stability.** Scroll-to-zoom must zoom toward the cursor
   position, not the canvas center (cursor-anchored zoom). If the anchor point drifts or
   is computed from the wrong coordinate space during a pan+zoom simultaneous gesture,
   the graph appears to lurch. Have cursor-anchored zoom semantics been validated on the
   actual hardware targets (trackpad on macOS, scroll wheel on Windows)? Trackpad two-
   finger scroll events arrive differently than mouse wheel events on Windows 11.

2. **Smart-fit (`Z`) expected behavior with multiple selections.** Smart-fit computes
   the axis-aligned bounding box of selected nodes + 20% padding. If the selected nodes
   span the full canvas, smart-fit is equivalent to "fit all," which is a zoom-out. If
   the selected nodes are two adjacent nodes, smart-fit is a close zoom. Do users expect
   `Z` to always zoom in (fit the selection tightly) or to also zoom out if the selection
   is spread across the canvas?

3. **Semantic fisheye comfort threshold.** The fisheye scale formula produces a maximum
   scale factor of 3.0× at the cursor position when `fisheye_radius = 300.0`. At this
   magnification, a node under the cursor is 3× larger than its DOI-tier baseline size.
   Does this magnification feel like a useful loupe effect or an uncomfortable distortion?
   Does it interfere with the user's ability to click the intended target (the magnified
   node is larger, but adjacent nodes may be pushed visually)?

4. **Gravity and off-screen drift.** The research report notes that FR without boundary
   constraints allows nodes to drift off-screen at low density or high repulsion. Canvas
   gravity (weak centering force) is a mitigation. But a graph the user has carefully
   arranged spatially (with pinned anchors and custom positions) should not be pulled
   toward center by gravity. Is gravity safe as a default-on force, or does it need to
   be weaker than the user's manual arrangement force?

5. **Multi-pane zoom independence.** In a two-pane layout where both panes show the same
   graph (Canonical view), zooming one pane does not affect the other. This is the
   correct behavior for independent exploration. But when a user navigates to a node in
   Pane A and wants to see it in context in Pane B, the zoom states being independent
   means Pane B may not show the relevant area. Is there a "sync zoom" mode users want,
   or is independent zoom always the right default?

### Research Methods

**Study 3.1 — Cursor-Anchored Zoom Accuracy (moderated, n=8, multi-platform)**
Give participants a graph at zoom 1.0 with a target node at an off-center position. Task:
"Zoom in on the node in the upper-left corner." Measure: (a) how many scroll events before
the target node is in the viewport center, (b) whether the graph "lurches" during combined
trackpad pan+zoom. Test on both Windows trackpad and external mouse. Flag any platform
where cursor-anchored zoom fails or drifts.

**Study 3.2 — Smart-Fit Direction Expectation (unmoderated, n=30)**
Show two scenarios: (a) two nodes selected that are far apart, smart-fit zooms out to show
both; (b) two nodes selected that are close together, smart-fit zooms in to show them
larger. Ask: "Is this behavior what you expected?" and "If you selected two distant nodes
and pressed Z, what do you expect to happen?" Record preference: always-zoom-in (fit
tightly even if that means showing less), or fit-to-bounding-box (zoom out if needed).
Result determines whether smart-fit needs a zoom-direction constraint.

**Study 3.3 — Semantic Fisheye Comfort and Click Accuracy (moderated, n=12)**
Enable semantic fisheye at the default radius (300 canvas units). Give a task requiring
clicking on specific nodes in a dense area. Measure click accuracy (correct target hit rate)
with and without fisheye. Also ask: "Does the magnification feel helpful or disorienting?"
If click accuracy degrades with fisheye (missed clicks due to adjacent nodes being visually
displaced), reduce the default `fisheye_radius` or add a dead zone near the cursor where
fisheye does not apply to avoid occlusion of the target.

**Study 3.4 — Gravity vs. Manual Layout Interference (moderated, n=10)**
Give participants a graph, let them arrange it manually (pin 3–4 anchors, drag others to
preferred positions). Then enable canvas gravity (weak centering force) and observe over
2 minutes. Ask: "Does anything feel wrong?" and "Do you notice your nodes moving?" If
gravity interferes with manually-set positions, add a `#pin`-respecting invariant to the
gravity implementation: gravity applies only to unpinned nodes.

### Deliverable

A **Camera Controls Acceptance Report** specifying:
- Whether cursor-anchored zoom is stable on all target platforms (Windows trackpad, mouse,
  macOS trackpad), and which platforms need event-handling fixes.
- Smart-fit direction semantics: fit-to-bounding-box (current design) or always-zoom-in
  (if ≥60% of users expect the latter).
- Semantic fisheye default radius recommendation (increase, decrease, or add cursor dead
  zone) based on click accuracy results.
- Gravity safety: whether gravity can be default-on or must be gated to unpinned nodes.

Feeds into `2026-02-24_layout_behaviors_plan.md` §1.3 (gravity parameter consistency) and
`2026-02-25_doi_fisheye_plan.md` fisheye cost guardrails.

---

## Thread 4 — Physics: Help vs. Motion Tax

### What the Docs Say

The graph UX research report's anti-patterns section identifies the most important physics
rule: "Do not autorun physics indefinitely. A perpetually-running simulation prevents users
from creating stable spatial layouts. Auto-pause on convergence." The auto-pause target
(Phase 4.1 of the performance plan) pauses when average displacement < epsilon for N frames
and resumes on structural/interaction intent.

The research report also identifies "damping is the most important convergence parameter"
(0.92 default), the reheat-on-structural-change behavior (resuming physics on AddNode/
AddEdge without resetting velocity), and new-node placement near topological neighbors
(prevents global displacement when a connected node is added).

The layout behaviors plan extends this with: degree-dependent repulsion (de-clutter high-
degree hub topologies), domain clustering force (pull same-domain nodes into soft spatial
neighborhoods), magnetic zones (user-defined spatial constraints), and semantic gravity
(UDC-based clustering). All forces are gated by `CanvasRegistry` toggles.

The physics engine extensibility plan defines three named physics profiles that map to
the Lens metaphor: `physics:solid` (frozen, no forces), `physics:liquid` (gentle settling,
low repulsion), `physics:gas` (active, high repulsion, spreading). The progressive Lens plan
ties these to zoom breakpoints.

The performance plan Phase 4.2 adds a per-frame physics time budget (~5ms), carrying
remaining simulation work to subsequent frames to favor responsiveness over convergence
speed.

### What Is Not Known

1. **The subjective experience of convergence.** Auto-pause solves the "perpetually
   running" problem, but the transition from `Running` to `Settled` is a sudden stop. If
   nodes are still oscillating slightly at the auto-pause threshold (epsilon may be set
   too loosely), the stop feels abrupt. If epsilon is too tight, the simulation runs longer
   than necessary. What is the epsilon value at which users perceive the graph as "settled"
   vs. still moving?

2. **Reheat visibility.** When a new node is added (reheat), the existing graph shifts
   as physics integrates the new node. For small graphs this is a gentle adjustment. For
   graphs with 50+ nodes and a well-settled layout, even a small reheat disrupts the user's
   spatial memory. At what graph size does reheat become disorienting rather than helpful?

3. **Whether the physics profile metaphor (Solid/Liquid/Gas) is legible.** The three
   named profiles are designed to correspond to intuitive physical metaphors. But "Gas"
   means high-repulsion spreading in the physics model, while in everyday language "gas"
   might suggest anything dispersed or formless. Does the metaphor communicate the
   correct behavioral expectation to users who haven't read the documentation?

4. **Multi-force interference.** When domain clustering, semantic gravity (UDC), and
   degree-dependent repulsion are all enabled simultaneously, the combined force field may
   produce unstable equilibria (nodes oscillating between competing attractors) or
   unexpected layouts (semantic gravity pulling a node away from a domain cluster). Have
   these combined-force interactions been characterized even theoretically?

5. **The motion tax per task type.** Physics motion is helpful for layout discovery
   (exploring a new graph) but is a tax for structured work (annotating, clipping,
   building a deliberate spatial arrangement). The interaction schemes doc defines Lens
   states (Solid/Liquid/Gas) as the mechanism for switching between these modes. Does
   the user actually need three modes, or is the binary "physics on / physics off" with
   good convergence behavior sufficient for all task types?

### Research Methods

**Study 4.1 — Epsilon Calibration: Perceived Settle Threshold (n=20)**
Show participants a simulated graph animation slowing from active physics to convergence.
Vary the epsilon value (and thus the auto-pause point) across three conditions: (a) stops
at low epsilon (nodes clearly stopped but may have residual micro-oscillation), (b) stops
at medium epsilon (current default), (c) stops at high epsilon (pauses sooner, nodes may
still have visible slow drift). Ask: "Does this graph feel settled?" for each condition.
Find the epsilon value where ≥80% of users report "settled." That becomes the default.

**Study 4.2 — Reheat Disorientation by Graph Size (within-subjects, n=15)**
Participants work with graphs at three sizes: 20, 50, and 100 nodes in a settled layout.
Add a new connected node to each. Measure: (a) whether participants notice the existing
nodes moving, (b) whether they find the movement helpful ("the graph is organizing itself")
or disorienting ("everything moved and I lost track"). Record the graph size threshold at
which movement shifts from "helpful" to "disorienting." Above that threshold, constrain
reheat to only the local neighborhood of the new node (not a global physics resume).

**Study 4.3 — Physics Profile Metaphor Comprehension (unmoderated, n=30)**
Show three physics states without labeling them. Ask: "Describe what you see" for each.
Then show the names Solid / Liquid / Gas and ask: "Which name matches which state?" Record
match accuracy. If ≥70% correctly match all three, the metaphor is legible. If not, test
alternative naming (e.g., Frozen / Settling / Active; or Off / Gentle / Dynamic) in a
follow-up to find the highest-match naming.

**Study 4.4 — Multi-Force Stability Observation (instrumented observation, n=8)**
Enable all three extra forces simultaneously (domain clustering + semantic gravity + degree
repulsion) on a 100-node graph with mixed-domain, mixed-UDC content. Observe: (a) whether
the simulation converges or oscillates, (b) which force wins in cases of conflict (a node
belonging to a UDC cluster on one side and a domain cluster on the other), (c) whether
the resulting layout is interpretable to the user. Collect open-ended descriptions: "What
do you think is organizing this graph?"

This study is primarily for the engineering team (detecting instability before shipping
multi-force mode) but the user interpretability finding feeds the `CanvasRegistry` policy
for which forces should be default-on.

**Study 4.5 — Binary vs. Three-Mode Physics Task Match (within-subjects, n=15)**
Give three tasks: (a) exploring a new graph for the first time (layout discovery), (b)
annotating and clipping nodes from a settled graph (structured work), (c) manually
arranging a graph into a meaningful spatial layout (deliberate composition). For each task,
provide both a binary toggle (on/off) and the three-mode Lens control. After each task,
ask: "Did you change the physics during this task? Why or why not?" and "Was the mode
control helpful or irrelevant?" Record whether task type predicts mode usage.

If mode usage does not vary by task type (users stick to one mode for all tasks), the three-
mode system adds complexity without behavioral benefit. If mode usage does vary by task
type, the Lens switching design is validated.

### Deliverable

A **Physics UX Calibration Report** specifying:
- The empirically-validated auto-pause epsilon value (from Study 4.1).
- The graph size above which reheat must be constrained to local neighborhood rather than
  global (from Study 4.2).
- The recommended physics profile naming (Solid/Liquid/Gas or alternatives, from Study 4.3).
- A multi-force stability characterization: which force combinations are safe to enable
  simultaneously and which must be mutually exclusive or sequenced (from Study 4.4).
- A recommendation on whether three-mode physics switching is necessary or whether binary
  on/off with good convergence is sufficient (from Study 4.5).

Feeds into `2026-02-24_layout_behaviors_plan.md` Phase 1 (physics micro-behaviors),
`2026-02-24_physics_engine_extensibility_plan.md` (preset design), and
`2026-02-25_progressive_lens_and_physics_binding_plan.md` (Lens/physics binding policy).

---

## Cross-Thread Concerns

### The Graph Size vs. Feature Complexity Inversion

The thread-1 and thread-4 findings may conflict: large graphs (Thread 1) need more
aggressive LOD and filtering to be readable, but the features that make large graphs
readable (DOI rendering, semantic gravity, domain clustering, magnetic zones) add
computational cost and behavioral complexity. There is a risk that the features designed
to handle scale only become available after a large graph already exists — too late for
the user who built the graph without them.

Research should probe when users *discover* these features (before or after their graph
becomes unreadable) and whether a proactive density trigger (e.g., "Your graph has 100+
nodes — would you like to enable overview mode?") would be accepted.

### Mental Map Preservation as a First-Class Metric

The research report calls mental map preservation "critical for Graphshell." Position
stability during incremental adds, reheat locality, and magnetic zones all serve this
goal. But mental map preservation is not currently a measurable output of any validation
test in the plans. The physics calibration studies above should include a position-
stability measurement: after N node additions, what fraction of previously-settled nodes
have moved more than 10% of the canvas width? That fraction is a concrete mental map
preservation metric that can be tracked across implementations.

### Accessibility of Motion and Physics

Users with vestibular disorders may find continuous physics motion aversive. The
`prefers-reduced-motion` OS setting should gate physics animation (not just badge
animation). The auto-pause behavior (Thread 4) is a partial mitigation, but reduced-motion
users need a way to use Graphshell with physics fully off without losing layout features
(Zone forcing, domain clustering) that require physics to be running.

---

## Summary: Four Open Questions → Four Deliverables

| Thread | Core Question | Deliverable |
|--------|--------------|-------------|
| 1. Node/Edge Density | At what density does the graph stop being useful, and does DOI/Ghost-tier solve the readability problem or create visual soup? | Density Scaling Profile (empirical thresholds) |
| 2. LOD Rules | Do the zoom-threshold LOD breakpoints preserve orientation, or do they strip the information users need? | LOD Threshold Calibration Report |
| 3. Camera Controls | Are zoom semantics (cursor-anchored, smart-fit, fisheye) stable and predictable across hardware and graph sizes? | Camera Controls Acceptance Report |
| 4. Physics as Help vs. Tax | Does auto-pause, reheat, and multi-force physics aid understanding or become a motion tax? | Physics UX Calibration Report |

Each deliverable maps to a named implementation plan or spec. An optional cross-thread
deliverable — a **Mental Map Preservation Metric** — may be defined to make position
stability measurable across all physics-related implementations.

---

## References

- `implementation_strategy/subsystem_diagnostics/2026-02-24_performance_tuning_plan.md`
- `implementation_strategy/canvas/2026-02-25_doi_fisheye_plan.md`
- `implementation_strategy/canvas/2026-02-24_layout_behaviors_plan.md`
- `implementation_strategy/canvas/2026-02-25_progressive_lens_and_physics_binding_plan.md`
- `implementation_strategy/canvas/2026-02-24_physics_engine_extensibility_plan.md`
- `implementation_strategy/canvas/2026-02-22_multi_graph_pane_plan.md`
- `research/2026-02-18_graph_ux_research_report.md §§2–5, 7–8, 10`
- Purchase et al. (2002), "Metrics for Graph Drawing Aesthetics"
- Yoghourdjian et al. (2018), "Exploring the Limits of Complexity"
- Fruchterman & Reingold (1991), "Graph Drawing by Force-Directed Placement"
