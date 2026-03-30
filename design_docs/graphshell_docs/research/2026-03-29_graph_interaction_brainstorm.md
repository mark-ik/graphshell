<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Interaction Brainstorm

**Date**: 2026-03-29
**Status**: Research / Brainstorm Capture
**Purpose**: Collect validated interaction ideas from brainstorming, with user feedback and architectural notes. These are candidates for future exploration, not committed plans.

**Related**:

- [`../technical_architecture/graphlet_model.md`](../technical_architecture/graphlet_model.md) — Graphlet kinds and derivation model
- [`../implementation_strategy/graph/2026-02-24_physics_engine_extensibility_plan.md`](../implementation_strategy/graph/2026-02-24_physics_engine_extensibility_plan.md) — Physics engine extension points
- [`../implementation_strategy/graph/2026-03-14_graph_relation_families.md`](../implementation_strategy/graph/2026-03-14_graph_relation_families.md) — Edge family taxonomy
- [`../../verso_docs/research/2026-03-28_rss_feed_graph_model.md`](../../verso_docs/research/2026-03-28_rss_feed_graph_model.md) — Feed graphlet chain topology (related: emission/eviction/harvest as interaction patterns)

---

## Validated Ideas (User-Endorsed)

### 1. Gravity Wells / Interest Clustering

**What**: Node physics mass increases with visit frequency and dwell time. Heavily-used nodes pull their neighbors closer, creating visible clusters of "where you spend time."

**User feedback**: Good if the effect is weak-by-default and accumulative. Minimum threshold: 3 visits before any effect. Then weak tiered gravity levels, not a continuous function. Concern: will users notice it at first?

**Architecture notes**:
- Pure physics extension — no new node types or edge families.
- Tiered mass values, not continuous, to keep the effect legible.
- Physics engine extensibility plan already supports per-node mass overrides.
- The subtlety concern is a design feature, not a bug: the canvas silently self-organizes around usage. Users notice when a cluster forms, not when a single node gets heavier.

---

### 2. Reading River

**What**: A user-curated queue node. Nodes dropped into the river flow through it in order. Configurable drain speed. Priority floats items forward.

**User feedback**: Loved it. Suggested a reader renderer, or a mode for `SimpleDocument` or Servo.

**Architecture notes**:
- A new graphlet kind (§5 of graphlet_model.md): `Queue` or `River`. Anchor is the river node; members flow through it.
- Same chain topology pattern as the RSS feed graphlet, but manually populated instead of poll-driven.
- Capacity could be unlimited (no eviction unless user drains) or bounded (user sets a read-later budget).
- Viewer integration: a "reader mode" surface that presents the current river member as a cleaned-up `SimpleDocument` or Servo reader view. Could be a dedicated viewer or a mode of the existing `SimpleDocument` viewer.
- Drain speed is a presentation config, not graph truth — the graph just has the ordered membership; the river UI paces how fast items move through.

---

### 3. Knowledge Decay and Revival

**What**: Nodes not visited in a configurable window lose physics mass and drift toward the periphery, visually fading. Visiting a node revives it.

**User feedback**: Liked it. Suggested decay starts when a node "goes cold" (configurable inactivity threshold).

**Architecture notes**:
- Complementary to gravity wells (§1): gravity wells accumulate mass, decay reduces it. They balance each other.
- "Cold" threshold is a graph-level setting (per-frame or global). When a node's `last_visited` exceeds the threshold, it enters decay.
- Decay reduces physics mass progressively. At minimum mass, the node drifts to the graph periphery but is never deleted.
- Revival: any interaction (open, hover-dwell, tag, link) resets the decay clock and restores mass.
- Visual: opacity fades with decay. A fully decayed node is still there but ghostly. RSS eviction (rss_feed_graph_model.md §4.2) is the hard version; decay is the soft ambient version.

---

### 4. Constellation Mode / User-Defined Graphlet Templates

**What**: Users save and name graph shapes. A named constellation is a reusable template — instantiate it with new nodes snapping into the same positions.

**User feedback**: Loved it. "User-defined graphlet templates."

**Architecture notes**:
- Extension of pinned graphlets (graphlet_model.md §4). A template is a pinned graphlet spec with placeholder anchor positions, saved as a reusable artifact.
- Template stores: relative node positions, edge topology pattern, role labels for each position.
- Instantiation: user selects nodes to fill template positions, or creates new nodes for unfilled slots.
- Relates to the general "pinned graphlet → promoted to graph truth" pathway.

---

### 5. Merge / Collision Detection

**What**: Duplicate nodes (same URL, similar content) slowly drift toward each other in physics. At a threshold, a merge proposal appears.

**User feedback**: Good for standalone nodes. If nodes are in a graphlet, they exist as duplicates with purpose in distinct contexts — don't merge those.

**Architecture notes**:
- Duplicate detection runs as a background analysis pass (same URL, fuzzy title match, content hash match).
- Physics: duplicates get a weak attractive force. Only applies if **neither** node is an active member of a graphlet.
- At proximity threshold, the merge proposal renders as a dashed bridge edge with a merge icon. Accept → nodes collapse; dismiss → attractive force removed permanently for that pair.
- Graphlet membership check is the critical guard: `if either_node.active_graphlet_membership().is_some() { skip }`

---

### 6. Breadcrumb Trails / Recency Accent

**What**: The user has an accent color that appears in decreasing luminosity around the last few selected/opened nodes.

**User feedback**: Liked it. Suggested accent color appears around the last ~3 (configurable) nodes selected/opened, fading in luminosity.

**Architecture notes**:
- Pure canvas rendering concern — no graph truth mutation.
- A short FIFO of recently-interacted node IDs (3–5 entries, configurable). Each entry renders a highlight ring at decreasing luminosity.
- The accent color could be the user's chosen profile color (already exists in the co-op presence model).
- Co-op synergy: in a co-op session, each participant's breadcrumbs show in their accent color. You see where each person just was.

---

### 7. Orbital Mode for Ego Graphlets

**What**: Ego graphlet members orbit the anchor at radii proportional to relationship strength.

**User feedback**: "Would be a nice feature for ego graphlets."

**Architecture notes**:
- A layout policy extension for ego graphlets (graphlet_model.md §5.1), not a new graphlet kind.
- Orbital radius = f(edge weight, edge family, hop distance). Higher relevance = closer orbit.
- Physics: orbital constraint force (tangential velocity + radial spring to target radius). Nodes don't stack — they spread around their orbit ring.
- Click an orbit ring to expand/collapse it (zoom into that relevance tier).

---

### 8. Portal Node / Aperture

**What**: Click a node to expand it into a spatial aperture revealing its interior graphlet. The node grows to fill a region of the canvas and its subgraph unfolds within its boundary.

**User feedback**: Loved it. "If it's not specced, it should be."

**Architecture notes**:
- This is a canvas-side interaction model — the node renders as a collapsible container that expands to show its children/graphlet.
- Similar to the workbench-correspondence graphlet (§5.9) but rendered inline on the canvas rather than as a separate workbench binding.
- Recursive: a portal inside a portal. Depth-limited to prevent performance issues (configurable max depth, default 2–3).
- The portal boundary is a canvas clip region. Nodes inside the portal respond to their own local physics (separate simulation scope or nested Barnes-Hut region).
- Related to containment edges and the directory node filesystem projection.

---

### 9. Filesystem as Spatial Graph

**What**: A directory node can be "exploded" into a spatial arrangement mirroring its directory structure.

**User feedback**: "Love it! Think it's already specced. Would love for it to be specced if it's not."

**Architecture notes**:
- The containment edge family already models filesystem hierarchy.
- "Explode" is a portal-node variant (§8 above): expand a directory node to reveal its contents spatially.
- Children positioned in a grid/tree layout around the parent.
- Cross-reference edges to non-filesystem nodes would show up as edges leaving the portal boundary — visible connections between files and the rest of the graph.
- Relates to the `filesystem-import` provenance marker in the edge family taxonomy.

---

### 10. Git-Like Event Log Branching

**What**: The event log should be branchable and forkable like git. Manage derivations of a given graph ID like a GitHub repo.

**User feedback**: Loved the fork/hypothesis concept but reframed it as git-style event log branching. "You should be able to manage the resulting derivations of a given graphID like a github repo."

**Architecture notes**:
- The WAL-based event log (fjall) is already an append-only sequence of `GraphIntent`s. A branch is a named fork point in that sequence.
- Branch semantics: fork from a specific WAL position. New intents on the branch don't affect the parent log. Merge = replay branch intents onto the parent log with conflict resolution.
- This is fundamentally what Device Sync version vectors already model (each peer has its own log position). Git-style branching generalizes it to single-user hypothetical branches.
- Heavy lift: merge conflict UI, branch management UI, storage implications of maintaining multiple branches. Worth speccing as a standalone exploration.

---

### 11. Variable Node Size as UI Primitive

**What**: Node canvas size as a function of a configurable variable (citation count, child count, visit frequency, etc.).

**User feedback**: Noticed this pattern recurs — "representing the size of a node with a given variable/range is a UI primitive we should look into implementing, because it seems to come up a lot."

**Architecture notes**:
- A `CanvasStylePolicy` extension: `node_size_source: Option<SizeSource>` where `SizeSource` is an enum of available metrics (child count, visit count, edge count, custom metadata field, etc.).
- The size computation runs as part of the style policy evaluation, not the physics engine (physics uses mass; rendering uses visual size; they're related but not identical).
- Configurable per lens, per frame, or globally.
- This is the foundation for citation graph visualization, hierarchy visualization, and many other use cases.

---

### 12. Citation Overlap and Link Extraction

**What**: Extract links from a node's document, match them against other nodes' links. Where citations overlap, generate cross-reference edges and potential graphlets.

**User feedback**: Generalized from citation graphs — "extract links from node's address or document, then see if links match any of a selection of nodes' links."

**Architecture notes**:
- Link extraction is a content analysis pass — run on node content (HTML, PDF, Gemini text) to extract outbound references.
- Matching: compare extracted links against known node addresses in the graph. Matches generate `cites` or `links-to` semantic edges.
- Graphlet generation: when a cluster of mutual citations is detected, suggest a new graphlet (or promote it to the frontier graphlet's expansion candidates).
- This is a background analysis task — computationally non-trivial for large graphs. Bounded by running on active/recent content, not the entire graph.

---

### 13. Graph Sonification

**What**: The physics state of the graph generates ambient sound — spring tensions become resonant frequencies, velocity becomes rhythm.

**User feedback**: "I love this! What a nice thing to do for people."

**Architecture notes**:
- Pure output concern — no graph truth involvement.
- Input signals: spring tension across edges (pitch), node velocity (rhythm/activity), cluster density (timbre/richness), decay state (volume fade).
- Implementation: a sonification module that reads physics state each frame and maps it to audio parameters. Could use `cpal` (cross-platform audio output) with a simple synthesizer, or drive MIDI output for external synths.
- Accessibility angle: sonification provides a non-visual channel for graph state. Users with visual impairments could navigate by sound. Dense, tight clusters sound different from sparse periphery.
- Opt-in, obviously. Default: off.

---

## Ideas Deferred or Declined

### Tension Topology (supports/contradicts edges with opposing physics)

User feedback: depends on context — "what content reliably has these edges? seems like something that might not get a lot of use." Deferred until edge frequency for `supports`/`contradicts` is observed in real usage.

### Problem Tracking / "Web of Worry"

User feedback: "I don't see the difference between this and an ego graphlet except for styling." Declined as a distinct concept — it's an ego graphlet with a semantic filter on causal edges.

### Time-Based Graphlets

User feedback: needs refinement to distinguish from the timeline feature. Reframed as: "pick graphlets from a given day/time range." Deferred pending timeline spec review.

### Collaborative Graphlet Handoff (co-op throw)

User feedback: "nah." Declined.

### Memory Palace Mode

User feedback: "isn't that just pinning?" Declined as a distinct mode — pinning already serves this purpose.

### Hot Path Highlighting

User feedback: "the edges are the desire path, plus just traversing the graph isn't much of a desire path signal." Acknowledged — traversal alone is a weak signal. Deferred pending a richer activity model that would make desire paths genuinely informative (edit frequency, dwell time, re-visits, not just traversal count).

### Graph Diff / Before-After

User feedback: "sounds enriching for the timeline spec when you want to compare two dates/times." Reframed as a timeline feature rather than a standalone interaction. Deferred to timeline spec integration.
