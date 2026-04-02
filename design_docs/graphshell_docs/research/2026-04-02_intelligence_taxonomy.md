# Intelligence Taxonomy for Graphshell (2026-04-02)

**Status**: Research
**Purpose**: Define a clear vocabulary for different kinds of intelligence in Graphshell so planning does not collapse heuristics, semantic classification, machine learning, generation, and agents into one undifferentiated "AI" bucket.

**Related**:

- `2026-04-02_intelligence_capability_tiers_and_blockers.md`
- `../implementation_strategy/graph/2026-03-11_graph_enrichment_plan.md`
- `../technical_architecture/graphlet_model.md`
- `../implementation_strategy/aspect_projection/ASPECT_PROJECTION.md`
- `../implementation_strategy/aspect_distillery/ASPECT_DISTILLERY.md`
- `../../verse_docs/implementation_strategy/self_hosted_model_spec.md`
- `../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`

---

## 1. Why A Taxonomy Is Needed

Graphshell is likely to accumulate many different things that users and developers would casually call "intelligence":

- graph analysis,
- tag and UDC suggestion,
- semantic search,
- summarization,
- workspace digests,
- background agents,
- future transfer or collective learning.

If all of those are discussed as one bucket, planning quality drops quickly.

Important distinctions get blurred:

- deterministic graph reasoning versus learned similarity,
- symbolic classification versus generation,
- bounded suggestion versus autonomous action,
- local explanation versus transfer-ready artifact production.

Graphshell therefore needs a taxonomy that separates:

- **mechanism**,
- **output type**,
- **scope**,
- and **autonomy**.

---

## 2. The Four Main Axes

### 2.1 Mechanism axis

This answers: **how was the result produced?**

### 2.2 Output axis

This answers: **what kind of thing is being produced?**

### 2.3 Scope axis

This answers: **what world is the intelligence operating over?**

### 2.4 Autonomy axis

This answers: **how much freedom does the system have to act?**

These axes are intentionally independent.

Example:

- a graphlet explainer may be structural by mechanism, explanatory by output, graphlet-local by scope, and descriptive by autonomy,
- while a tag suggester may be statistical by mechanism, classificatory by output, node-local by scope, and suggestive by autonomy,
- while a future agent may be agentic by autonomy while still using structural, statistical, and generative mechanisms underneath.

---

## 3. Mechanism Categories

### 3.1 Structural intelligence

Structural intelligence is intelligence derived from graph truth, history truth, workbench truth, layout state, graphlets, and explicit heuristics without requiring a learned model.

Examples:

- bridge-node detection,
- graphlet anchor suggestion,
- frontier ranking,
- workbench correspondence explanation,
- graph diff summary from structural change,
- current-thread reconstruction from traversal history.

Structural intelligence is often:

- deterministic,
- inspectable,
- cheap,
- highly aligned with Graphshell's architecture.

Graphshell should treat this as first-class intelligence, not as a lesser substitute for ML.

### 3.2 Semantic or symbolic intelligence

Semantic intelligence is intelligence derived from explicit labels, taxonomies, classifications, ontologies, tags, or rule systems.

Examples:

- UDC-driven grouping,
- subject-based tab grouping,
- rule-based classification from existing metadata,
- tag inheritance or class narrowing,
- PMEST-aligned facet routing.

This may be manual, rule-based, or partially learned, but the core representation is symbolic and inspectable.

### 3.3 Statistical or learned intelligence

Statistical intelligence uses learned similarity, learned ranking, classifiers, embeddings, clustering models, or other statistical methods that go beyond explicit symbolic rules.

Examples:

- semantic neighbor search,
- concept deduplication,
- learned tag suggestion,
- latent topic clustering,
- retrieval ranking from embeddings.

This is the category most people mean when they say "machine learning," but it is not the whole space of intelligence.

### 3.4 Generative intelligence

Generative intelligence produces new derived content rather than only ranking, retrieving, or classifying existing content.

Examples:

- graphlet summary,
- workspace digest,
- edge-label suggestion,
- synthesis node generation,
- structured extraction from clips or pages,
- explanation text over a bounded local world.

Generative intelligence can be model-based or template-based, but in Graphshell planning it is most useful to reserve the term for systems that produce new textual or structured artifacts.

### 3.5 Agentic intelligence

Agentic intelligence is not primarily a mechanism category. It is orchestration over time.

It sequences:

- observation,
- planning,
- tool invocation,
- evaluation,
- retries,
- and sometimes promotion of derived artifacts.

Examples:

- a supervised graph summariser agent,
- a tag-suggestion agent that watches recent nodes,
- a workspace-diff agent that emits digests,
- a bounded retrieval-and-summarize pipeline over the current graphlet.

Agentic systems may use structural, symbolic, statistical, and generative methods underneath. "Agentic" should not be treated as just "more advanced ML." It is a distinct orchestration category.

### 3.6 Collective or transfer intelligence

Collective intelligence is intelligence that depends on reusable learned artifacts, community knowledge, transfer payloads, or federated/shared derived structures.

Examples:

- engrams,
- FLora submissions,
- shared retrieval memories,
- community-synced classification packs,
- future Verse-fed learned artifacts.

This category matters because some useful intelligence in Graphshell will not come from one local model run. It will come from durable, portable, policy-governed artifacts.

---

## 4. Output Categories

These categories are orthogonal to mechanism.

### 4.1 Explanatory intelligence

Answers why something is true or why the system is suggesting something.

Examples:

- why this graphlet is active,
- why these nodes are clustered,
- why this node got this tag,
- why this edge family is emphasized.

### 4.2 Ranking intelligence

Produces ordered candidates.

Examples:

- what to open next,
- best frontier nodes,
- best bridge node,
- likely anchor candidates.

### 4.3 Classificatory intelligence

Assigns a label, class, or facet state.

Examples:

- UDC suggestion,
- content-kind assignment,
- node-type or topic classification,
- confidence-bearing semantic labels.

### 4.4 Retrieval intelligence

Finds the most relevant existing items.

Examples:

- related nodes,
- current thread reconstruction,
- similar workspaces,
- relevant clips or summaries.

### 4.5 Synthetic intelligence

Produces a new artifact by combining or distilling other sources.

Examples:

- graphlet digest,
- workspace summary,
- synthesis node,
- structured extraction artifact,
- eval evidence bundle.

### 4.6 Actionable intelligence

Produces or executes a next action, often with bounded authority.

Examples:

- gather these nodes here,
- group tabs by subject,
- create this graphlet,
- propose this arrangement change,
- emit a tag or summary suggestion for review.

---

## 5. Scope Categories

### 5.1 Node-local intelligence

Acts on one node, page, clip, image, or audio item.

### 5.2 Graphlet-local intelligence

Acts on one bounded local world.

This is likely the most important scope for Graphshell because it is:

- bounded,
- explainable,
- cheaper than whole-workspace intelligence,
- and already central to the architecture.

### 5.3 Workspace intelligence

Acts on the currently open panes, frames, workbench state, and recent thread.

### 5.4 Whole-graph intelligence

Acts on graph-global structure and meaning.

Examples:

- duplicate detection,
- centrality and atlas analysis,
- broad clustering,
- stale or disconnected-region review.

### 5.5 Cross-workspace or collaborative intelligence

Acts across shared or repeated contexts.

Examples:

- what changed in this shared workspace,
- reused topic packs,
- future community-derived artifacts.

---

## 6. Autonomy Categories

### 6.1 Descriptive

Only observes, explains, or reveals.

### 6.2 Suggestive

Recommends an action, label, or interpretation, but does not change state on its own.

### 6.3 Assistive

Prepares a result, draft, or pending action for user confirmation.

### 6.4 Supervised agentic

Runs in the background with declared capability and bounded output surfaces, but still relies on policy and acceptance boundaries.

### 6.5 Delegated or autonomous

Can take actions with minimal intervention.

Graphshell should be cautious here. Given the architecture direction, supervised agentic behavior is a much more natural near-term target than broad autonomy.

---

## 7. A Practical Tier Model

The axes above are the real taxonomy. But a simpler ladder is still useful for roadmap talk.

### Tier 0 — Descriptive structural intelligence

No model required.

Examples:

- explain this graphlet,
- explain this cluster,
- graph diff from history,
- current-thread reconstruction.

### Tier 1 — Semantic assistance

Symbolic or rule-based assistance with user-visible provenance and acceptance.

Examples:

- tag and UDC suggestions,
- grouping by subject,
- explanation-driven filters,
- suggestive layout or arrangement actions.

### Tier 2 — Statistical retrieval intelligence

Embeddings, learned ranking, clustering, deduplication, semantic search.

Examples:

- semantic neighbor search,
- topic clustering,
- concept deduplication,
- better suggestion ranking.

### Tier 3 — Generative local intelligence

Bounded synthesis or extraction over nodes, graphlets, or workspaces.

Examples:

- graphlet digest,
- workspace summary,
- explain selection,
- structured extraction from clips.

### Tier 4 — Supervised agentic intelligence

Agents, `AWAL`, distillation, typed artifacts, bounded background workflows.

Examples:

- graph summariser agent,
- tag suggester agent,
- background digest builder,
- agent-assisted retrieval-memory construction.

### Tier 5 — Transfer and collective intelligence

Portable or shared learned artifacts.

Examples:

- engrams,
- FLora submissions,
- future community-derived packs or transfer-ready memories.

Important rule:

- this is a planning ladder, not an intrinsic value hierarchy.
- structural intelligence is not inferior to learned intelligence.
- agentic intelligence is not inherently smarter than retrieval intelligence.

---

## 8. The Most Useful Distinctions For Graphshell

If Graphshell needs a compact everyday vocabulary, the most useful distinctions are:

### 8.1 Structural intelligence

The graph thinking about itself.

### 8.2 Semantic intelligence

The graph understanding what kinds of things it contains.

### 8.3 Statistical intelligence

The graph noticing latent similarity beyond explicit labels.

### 8.4 Generative intelligence

The graph speaking or synthesizing.

### 8.5 Agentic intelligence

The graph working over time.

This shorthand is accurate enough for design discussions while still preserving the real distinctions above.

---

## 9. Architectural Implications

This taxonomy suggests a sequencing rule for Graphshell.

### 9.1 Structural and semantic intelligence should mature first

Graphshell already has the right substrate for these forms of intelligence.

These are the most aligned with:

- graph truth,
- history truth,
- graphlets,
- workbench structure,
- and the existing enrichment lane.

### 9.2 Statistical intelligence is the best first model-assisted jump

Embeddings improve many product surfaces at once with relatively low trust risk:

- semantic search,
- clustering,
- deduplication,
- suggestion quality,
- retrieval.

### 9.3 Generative intelligence should stay bounded

The best scope for generation is not "the whole world." It is:

- node-local,
- graphlet-local,
- or workspace-local.

This matches Graphshell's architecture and trust model.

### 9.4 Agentic intelligence requires real boundaries

Before agentic intelligence becomes a serious runtime layer, Graphshell needs:

- Distillation Boundary enforcement,
- typed artifact contracts,
- `AWAL` and episode durability,
- acceptance and promotion rules,
- and a clear separation between graph truth, agent truth, and transfer truth.

---

## 10. Bottom Line

Graphshell should not talk about intelligence as one monolithic thing.

The useful distinctions are:

- structural,
- semantic,
- statistical,
- generative,
- agentic,
- and collective or transfer intelligence.

And the useful planning ladder is:

- descriptive structural intelligence,
- semantic assistance,
- statistical retrieval,
- bounded generation,
- supervised agentic workflows,
- transfer and collective intelligence.

This taxonomy helps Graphshell stay precise about what kind of capability is being proposed, what architecture it depends on, and which kinds of trust and policy boundaries it actually needs.


---

Original response this taxononmy was based on:

For Graphshell, I’d separate intelligence along at least four axes.

1. By Mechanism
This answers: how was the result produced?

Structural intelligence
Uses graph structure, graphlets, history, layout, workbench state, tags, UDC, heuristics, and rules. No model required.
Example: “this node is probably a bridge,” “this graphlet’s likely anchor is X,” “these frames overlap semantically.”

Semantic or symbolic intelligence
Uses explicit labels, ontologies, taxonomies, UDC classes, user tags, and rule-based inference.
Example: “these nodes are all under udc:51,” “group tabs by subject,” “suggest a parent class from existing tags.”

Statistical or learned intelligence
Uses embeddings, clustering, classifiers, regressors, ranking models, or learned similarity.
Example: semantic neighbor search, deduplication by meaning, topic clustering, tag suggestions from content.

Generative intelligence
Produces new text, summaries, labels, extracted structure, or synthesis artifacts.
Example: graphlet digest, workspace summary, edge-label suggestion, clip extraction.

Agentic intelligence
Plans, sequences, monitors, retries, and decides when to invoke tools or transforms.
Example: an agent that notices recent work, builds a summary, proposes tags, and waits for acceptance.

That means pattern analysis and machine learning are not just “different tiers.” They are different mechanisms. Pattern analysis is often structural or symbolic. ML is statistical. Agentic is orchestration over one or more of the others.

2. By Output Type
This answers: what kind of thing does the intelligence produce?

Explain
“Why is this graphlet active?” “Why are these nodes clustered?”

Rank
“What should I open next?” “What is the best bridge node?”

Classify
“This is probably mathematics.” “This cluster is likely about Rust async.”

Retrieve
“Show me related nodes.” “Find the current thread.”

Summarize or synthesize
“What changed?” “What is the consensus of these nodes?”

Act
“Group these tabs,” “gather these nodes here,” “generate a digest node.”

This distinction matters because explainers and summarizers are not the same thing, even if they both feel “smart.”

3. By Scope
This answers: what world is the intelligence operating over?

Node-local intelligence
One node, one page, one clip, one image, one audio item.

Graphlet-local intelligence
The most natural Graphshell scope. A bounded local world.

Workspace intelligence
Open panes, current frames, current workbench state, recent thread.

Whole-graph intelligence
Global duplicate detection, topology analysis, semantic atlas, centrality.

Cross-workspace or collaborative intelligence
“What changed in the shared workspace?” “What patterns recur across projects?”

This is one of the most important distinctions for Graphshell. Bounded graphlet-local intelligence is much safer and more useful than immediately jumping to whole-workspace or whole-graph chat.

4. By Autonomy
This answers: how much freedom does the system have?

Descriptive
Only observes and explains.

Suggestive
Recommends, but never changes anything.

Assistive
Can prepare actions, drafts, filters, or arrangements for the user to confirm.

Supervised agentic
Can run background tasks and produce typed artifacts, but only within declared boundaries and acceptance flows.

Delegated or autonomous
Can take actions with minimal intervention.

This is where “agentic” actually belongs. It is mostly an autonomy and orchestration distinction, not a mechanism distinction.

So if I were naming the major categories for Graphshell, I’d use something like this:

A. Structural Intelligence
Graph-native, deterministic, explainable, no model required.
This is graph analysis, graphlets, history, layout, workbench correspondence, and heuristic ranking.

B. Semantic Intelligence
Tagging, UDC, ontology-like reasoning, classification, faceting, symbolic grouping.
This may be manual, rule-based, or learned, but the output is semantic structure.

C. Statistical Intelligence
Embeddings, similarity, clustering, deduplication, retrieval, learned ranking.
This is where “machine learning” starts mattering materially.

D. Generative Intelligence
Summaries, extraction, synthesis, explanation text, edge-label drafts, digests.

E. Agentic Intelligence
Supervised background workers that sequence the other kinds of intelligence into bounded workflows.

F. Collective Intelligence
Shared or federated learning, Verse-style transfer, community-synced knowledge, reusable artifacts, engrams.

If you want a simpler tiered model on top of that, I’d use this:

Tier 0: Descriptive Structural Intelligence
No models. Graph, history, tags, graphlets, heuristics.
This is where Graphshell should get excellent first.

Tier 1: Semantic Assistance
Classification, filters, explanation, suggestion, acceptance workflows.
Still mostly non-ML or light-ML.

Tier 2: Statistical Retrieval Intelligence
Embeddings, semantic neighbor search, deduplication, ranking by learned similarity.

Tier 3: Generative Local Intelligence
Bounded summaries, extraction, synthesis, graphlet chat, workspace digest.

Tier 4: Supervised Agentic Intelligence
AWAL, Distillery, typed artifacts, supervised background agents, bounded tool use.

Tier 5: Transfer and Collective Intelligence
Engrams, FLora, shared learned artifacts, collaborative or federated intelligence.

The important caveat is this:

Pattern analysis is not “lower” than ML in value.
Generative is not “higher” than retrieval in trustworthiness.
Agentic is not “smarter” by default. It is just more autonomous.
For Graphshell, graphlet-bounded structural intelligence may be more product-defining than a generic local chatbot.
If I were putting this into Graphshell’s own language, I’d say:

Structural intelligence is the graph thinking about itself.
Semantic intelligence is the graph understanding what kinds of things it contains.
Statistical intelligence is the graph noticing latent similarity beyond explicit labels.
Generative intelligence is the graph speaking.
Agentic intelligence is the graph working.
And the architecture implication is:

Structural and semantic intelligence should mature first.
Statistical intelligence is the best first model-assisted jump.
Generative intelligence should be bounded by graphlet or workspace scope.
Agentic intelligence should arrive only after Distillery, typed artifacts, and durable supervised boundaries are real.