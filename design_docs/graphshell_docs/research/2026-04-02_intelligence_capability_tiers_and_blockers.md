# Intelligence Capability Tiers and Blockers (2026-04-02)

**Status**: Research
**Purpose**: Assess what Graphshell intelligence can look like across three capability tiers: using only current systems, using current dependencies plus system extensions, and using new dependencies. Identify the most immediately compelling product directions and the real blockers that stand between current architecture and trustworthy intelligence features.

**Related**:

- `../implementation_strategy/graph/2026-03-11_graph_enrichment_plan.md`
- `../implementation_strategy/graph/semantic_tagging_and_knowledge_spec.md`
- `../implementation_strategy/graph/node_badge_and_tagging_spec.md`
- `../implementation_strategy/graph/faceted_filter_surface_spec.md`
- `../implementation_strategy/subsystem_history/SUBSYSTEM_HISTORY.md`
- `../technical_architecture/graphlet_model.md`
- `../implementation_strategy/aspect_projection/ASPECT_PROJECTION.md`
- `../implementation_strategy/aspect_distillery/ASPECT_DISTILLERY.md`
- `../../verse_docs/research/2026-02-24_local_intelligence_research.md`
- `../../verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`
- `../implementation_strategy/subsystem_security/2026-03-09_intelligence_distillation_privacy_boundary_plan.md`
- `../implementation_strategy/system/register/2026-03-08_sector_g_mod_agent_plan.md`

---

## 1. Executive Summary

Graphshell is already much closer to intelligence features than a normal browser because it has:

- graph truth,
- history truth,
- workbench truth,
- semantic tagging and UDC classification,
- graphlet-local worlds,
- layout and scene interpretation,
- and a growing projection model for how one domain represents itself in another.

This means Graphshell can ship a meaningful first intelligence layer without leading with models.

The most important current distinction is:

- **product intelligence**: features that make the graph feel aware, explainable, and assistive using current graph/history/tagging/projection systems,
- **model intelligence**: features that require embeddings, generation, vision, audio, or an agent runtime.

Graphshell should treat product intelligence as the first milestone, not as a fallback.

The strongest immediate opportunities are:

1. graphlet and cluster explanation,
2. bounded workspace or graphlet digest,
3. next-best-node and next-best-expansion guidance,
4. accept/reject semantic suggestions with provenance,
5. graph diff and activity summary,
6. bounded "chat with current graphlet" once a local model path is real.

The main blockers are not mostly about model quality. They are:

- missing explanation surfaces,
- missing provenance-bearing semantic records,
- missing faceted payoff for captured semantics,
- missing distillation boundary and typed artifact pipeline,
- missing agent durability and promotion boundaries,
- missing local model/runtime management if Graphshell moves beyond heuristics.

---

## 2. Framing: What Intelligence Means in Graphshell

In Graphshell, intelligence should not be reduced to "attach an LLM to the graph."

The better framing is:

- intelligence helps the user understand the graph,
- intelligence helps the user navigate and shape local worlds,
- intelligence helps the system derive useful structure from history, content, and behavior,
- intelligence stays bounded, inspectable, and provenance-aware.

Graphshell's strongest native intelligence substrate is the combination of:

- graph structure,
- graphlets as bounded local worlds,
- traversal and temporal truth,
- semantic tags and UDC,
- workbench arrangements and frame semantics,
- current layout and scene interpretation.

That substrate is already enough to make the product feel smart before any local-model stack is added.

---

## 3. Tier One: No New Dependencies

This tier uses only Graphshell's current systems and current dependency graph as already exercised in the product.

### 3.1 What becomes possible

#### A. Explain the current graph state

Graphshell can already explain a surprising amount with no model support:

- why a node is clustered where it is,
- why a graphlet is active,
- why a frontier node is ranked highly,
- why a frame or tile group corresponds to a local world,
- why a node is tagged or classified the way it is,
- why two nodes are considered related structurally.

This explanation can be derived from:

- graph edges and relation families,
- graphlet anchors and backbone candidates,
- history recency and traversal families,
- UDC and user/system tags,
- frame affinity and layout heuristics,
- current view and workbench bindings.

This is not fake intelligence. It is product intelligence.

#### B. Recommend structural next steps

Without models, Graphshell can still surface:

- next node to open,
- next frontier expansion,
- best bridge between two regions,
- likely current thread,
- strong candidate primary anchor,
- likely duplicate or overlap clusters,
- graphlets worth pinning.

These recommendations can come from graph analysis, recency, tag overlap, UDC hierarchy, and workbench locality.

#### C. Summarize structurally instead of generatively

Graphshell can already produce useful structural summaries:

- "5 nodes added to the current math thread"
- "2 frames now overlap in UDC 51 / mathematics"
- "most recent traversal corridor is A -> B -> C -> D"
- "this graphlet is mostly recent traversal plus one imported reference node"

These are not language-model summaries. They are graph-native summaries.

#### D. Improve graph hygiene

No-dependency intelligence can flag:

- likely duplicate nodes by title/address/domain heuristics,
- under-tagged or unclassified nodes,
- stale archived nodes still acting as hubs,
- disconnected islands that probably belong together,
- frames whose members no longer match their actual semantic center,
- graphlets missing an obvious anchor,
- nodes that repeatedly co-occur in the same workbench contexts.

#### E. Spatial and workbench assistance

Because Graphshell already has layout, frame affinity, graphlets, and workbench semantics, it can offer:

- gather nodes by tag/domain/UDC here,
- suggest a better frame or graphlet binding,
- open this result as a corridor graphlet,
- arrange this local world by temporal, radial, or component logic,
- highlight the likely semantic region of current work.

### 3.2 Straightforwardly cool features in this tier

The most compelling no-new-dependency features are:

1. **Explain this graphlet**
2. **Explain this cluster**
3. **What changed here?**
4. **What should I open next?**
5. **Gather related things here**
6. **Show me why this node is tagged/classified this way**

These are all directly aligned with current architecture.

### 3.3 Blockers in this tier

The blockers are mostly UX and data-model blockers, not algorithm blockers.

#### Missing explanation surfaces

The graph enrichment plan is explicit: Graphshell is ahead in semantic plumbing and behind in user-facing payoff.

Current missing pieces include:

- selected-node enrichment inspector,
- graphlet explanation surface,
- filter/explain integration,
- durable visible provenance.

#### Missing durable semantic records

The current runtime semantic transport is not enough for trustworthy explanation. Graphshell still needs durable classification records with:

- provenance,
- confidence,
- status,
- acceptance or rejection state.

#### Missing filter and navigation payoff

If the metadata cannot drive real filtering, grouping, and navigation, then more derived semantics only increase hidden complexity.

---

## 4. Tier Two: Current Dependencies Plus System Extensions

This tier assumes Graphshell keeps its current dependency set and extends existing systems more aggressively.

Important current dependencies and system assets include:

- `petgraph`
- `nucleo`
- `parry2d`
- `rstar`
- local storage/index crates already in the repo
- current graph/layout/workbench/history/projection systems

### 4.1 What becomes possible

#### A. Stronger graph analysis and projection intelligence

Using current graph tooling, Graphshell can derive much richer local-world intelligence:

- corridor and bridge scoring,
- articulation points and chokepoints,
- SCC and loop diagnostics,
- hub and authority heuristics,
- candidate graphlet shapes,
- better frontier ranking,
- workbench correspondence graphlets,
- recent-thread reconstruction.

This is a serious upgrade to Navigator and graphlet behavior without requiring a model.

#### B. Better semantic routing and search

With current tagging, UDC, and fuzzy matching infrastructure, Graphshell can do more than literal string search:

- label-first subject search,
- concept-guided UDC narrowing,
- better related-node suggestions,
- semantic filter suggestions,
- route into the right graphlet shape from a query.

This is still not embedding search, but it is significantly smarter than keyword matching.

#### C. Spatial intelligence

The new scene/runtime layer and current layout work make spatial assistance possible:

- region-aware suggestions,
- current-work vs archive spatial split,
- semantic or temporal attraction basins,
- scene-informed graph explanations,
- region membership summaries,
- spatially explainable gather/sort commands.

#### D. Persistent local derived indices

Without adding a model runtime, Graphshell can still persist:

- structural summaries,
- graphlet descriptors,
- heuristic cluster signatures,
- node co-occurrence records,
- history-derived recency slices,
- explanation cache artifacts.

That allows more intelligence-like product behavior without recomputing everything from scratch every frame.

### 4.2 Straightforwardly cool features in this tier

The most compelling current-dependency extensions are:

1. **Graph diff summary for a workspace or graphlet**
2. **Smarter frontier and bridge suggestions**
3. **Workbench correspondence graphlet and arrangement explanation**
4. **Semantic gather/sort actions with explainable reasoning**
5. **Structural duplicate and overlap detection**
6. **Graph-native "why is this related?" answers**

### 4.3 Blockers in this tier

#### Missing projection-runtime reuse

Graphshell has the right ideas in graphlet and projection architecture, but product intelligence features will become messy if each one invents its own cache, summary object, or local-world carrier.

The Projection aspect needs to become a real reuse layer in code, not only in docs.

#### Missing durable inspector-facing artifacts

The system needs stable explanation objects that a selected node, graphlet, or workspace can surface. Right now too much derived meaning risks living as ephemeral runtime state.

#### Missing acceptance and reversal flows

As soon as Graphshell starts suggesting groupings, anchors, or classifications, it needs:

- accept,
- reject,
- ignore,
- revert,
- and "why".

Without that, a smart suggestion becomes a suspicious surprise.

---

## 5. Tier Three: New Dependencies

This tier is where Graphshell becomes model-assisted or model-driven in a serious way.

The most plausible dependency families are:

- local embedding models,
- small local text models,
- vector index or ANN search,
- OCR or document extraction,
- vision models,
- audio transcription,
- optional local agent runtime extensions.

### 5.1 What becomes possible

#### A. Semantic neighbor search that is actually semantic

Embeddings are the first major jump. They enable:

- related nodes by meaning rather than shared tags,
- semantic clustering,
- deduplication by concept,
- much better suggestion quality,
- bounded RAG over graphlets or workspaces,
- semantic physics that uses content similarity instead of only UDC and tags.

This is the single most strategically useful new-dependency tier.

#### B. Local text intelligence

Small local text models enable:

- graphlet digest,
- workspace digest,
- explain selection,
- edge-label suggestions,
- structured extraction from pages and clips,
- local chat with current graphlet,
- synthesis nodes from a bounded set of current materials.

These are some of the most straightforwardly cool features because they feel immediately legible to users.

#### C. Vision and screenshot intelligence

Vision models enable:

- image tagging,
- screenshot search,
- smart icon or thumbnail generation,
- scene-aware or content-aware visual labeling,
- better treatment of image-rich and multimodal workspaces.

#### D. Audio intelligence

Audio models enable:

- speech-to-text for recordings,
- audio-node indexing,
- transcript-based graph linking,
- multimodal workspace search.

#### E. Agentic background intelligence

Once bounded model execution, typed artifacts, and agent durability exist, Graphshell can support supervised agents that:

- summarize recent work,
- suggest tags or classifications,
- build retrieval memories,
- watch for workspace changes,
- generate graph-diff summaries,
- produce local-only behavior profiles.

### 5.2 Straightforwardly cool features in this tier

The strongest "wow, that is obviously useful" features are:

1. **Chat with current graphlet**
2. **Workspace digest and graph-diff summary**
3. **Semantic neighbor search**
4. **Accept/reject tag and classification suggestions**
5. **Explain this selection / synthesize this thread**
6. **Screenshot and audio search**

### 5.3 Blockers in this tier

#### Missing Distillation Boundary in runtime practice

This is the biggest blocker.

Graphshell already knows, architecturally, that:

- durable app state must cross a Distillation Boundary,
- source classes need policy evaluation,
- remote and local execution are not equivalent,
- raw persistence is not model input.

Until that becomes real runtime machinery, model features are easy to demo and hard to trust.

#### Missing Distillery and typed artifact pipeline

Graphshell also already knows that the real output of intelligence should be typed artifacts, not a generic blob.

The missing parts are:

- request objects,
- transform-family execution,
- artifact classes,
- promotion boundaries,
- provenance links across graph truth, agent truth, and transfer truth.

#### Missing `AWAL` and episode durability

For agentic or semi-agentic intelligence, Graphshell still lacks a real durable carrier for:

- what an agent observed,
- what it tried,
- what was accepted or rejected,
- what should become reusable experience.

Without that, agents either stay toy-like or pollute graph truth.

#### Missing model-runtime management

If Graphshell adds local models, it must also own:

- manifests,
- download sources,
- cache and disk policy,
- hardware and memory budget checks,
- feature gating,
- failure and fallback behavior,
- provider trust class and privacy enforcement.

This is a product and runtime burden, not just a technical dependency choice.

#### Missing evaluation discipline

As soon as features become model-driven, Graphshell needs to measure:

- whether summaries are useful,
- whether suggestions are accepted,
- whether retrieval actually improves navigation,
- whether false positives or hallucinated structure are eroding trust.

---

## 6. The Most Interesting Near-Term Product Directions

The most interesting directions are not necessarily the fanciest architectures.

The most interesting ones are those that make Graphshell feel uniquely itself.

### 6.1 Explain this graphlet

This is the best immediate feature.

It is:

- graph-native,
- bounded,
- aligned with the graphlet model,
- useful without a model,
- and even better with one later.

It can answer:

- what holds this local world together,
- what its primary anchor probably is,
- what its frontier is,
- why these nodes are here.

### 6.2 Graphlet or workspace digest

This is the strongest bridge feature between current product intelligence and future model intelligence.

First version:

- structural digest from graph/history/tagging.

Later version:

- local text-model summary over the same bounded local world.

### 6.3 Semantic neighbor search

This is probably the highest-leverage first model-assisted feature once embeddings exist.

It improves:

- navigation,
- clustering,
- search,
- suggestion quality,
- and semantic physics.

### 6.4 Accept/reject semantic suggestions

This is the right productization path for intelligence in Graphshell because it preserves user authorship.

It is also the feature most obviously blocked by missing provenance and explanation surfaces.

### 6.5 Graph diff summary

This is a straightforwardly cool feature because it maps directly onto Graphshell's graph/history/workbench identity.

It is especially strong for:

- re-entry,
- shared workspaces,
- current-thread review,
- and agent-assisted summarization later.

---

## 7. The Biggest Real Blockers

These are the blockers that matter most regardless of whether models are added.

### 7.1 Missing explanation surfaces

Graphshell is ahead in capture and behind in explanation.

This is the most immediate blocker to making intelligence feel trustworthy.

### 7.2 Missing provenance-bearing semantic storage

Current transport and cache structures are not enough for a real accept/reject intelligence layer.

Graphshell needs durable records that say:

- where a classification came from,
- how confident it is,
- whether it was accepted,
- whether it is imported, derived, suggested, or verified.

### 7.3 Missing bounded intelligence read path

The Distillation Boundary is not optional once graph, history, clips, and workbench state feed intelligence features.

### 7.4 Missing typed intelligence outputs

If everything becomes "assistant output," the architecture gets blurry fast.

Graphshell needs typed artifacts and promotion rules.

### 7.5 Missing local model/runtime policy if dependencies are added

Even the best small-model plan fails if Graphshell cannot say:

- where models come from,
- how they are loaded,
- when they are allowed to read local state,
- what they emit,
- how failures are surfaced.

### 7.6 Missing evaluation and trust instrumentation

Useful intelligence is not only about capability. It is about whether users keep trusting it.

---

## 8. Recommended Sequence

The strongest sequence for Graphshell is:

1. **Land explanation and inspection for current enrichment and graphlet state.**
2. **Ship one bounded intelligence feature using only existing systems.**
3. **Add embeddings before a general local chat feature.**
4. **Add a small local text model for bounded graphlet/workspace summarization.**
5. **Only then expand toward broader distillery and agent flows.**

This sequence is correct because it matches the product and architecture reality:

- Graphshell already has enough structure to feel smart without a model.
- Embeddings provide a sharp upgrade with less trust risk than generation-first UX.
- Generation should arrive on top of bounded local worlds, not before them.
- Distillery and agent durability should become real before Graphshell pretends intelligence is a background utility.

---

## 9. Bottom Line

Graphshell does not need to wait for a full local-model stack to become intelligent.

Right now, the best intelligence work is:

- graph-native,
- bounded,
- explainable,
- filterable,
- provenance-aware.

The coolest near-term features are not "a chatbot attached to everything."

They are:

- explain this graphlet,
- summarize this local world,
- show what changed,
- suggest the next move,
- and make semantics legible enough that later model assistance feels like a natural extension instead of a foreign layer.

When Graphshell eventually adds embeddings, local text models, and distillery-backed agents, those features should slot into an already intelligent product rather than trying to invent intelligence from scratch.
