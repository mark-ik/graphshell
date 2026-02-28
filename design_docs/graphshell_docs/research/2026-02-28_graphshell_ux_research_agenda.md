# Graphshell UX Research Agenda (2026-02-28)

**Status**: Active research agenda
**Scope**: Five research threads that address whether Graphshell's graph/workbench model is a real productivity advantage or an interesting representation. Each section names the open question, what the docs already say, what must be learned, and what a useful deliverable looks like.

The single most important thread is §1 — task-based user studies. Without it, §§2–5 are engineering solutions in search of a validated problem. That said, §§2–5 each represent genuine open design decisions that will need answers regardless of how the user studies land.

---

## 1. Real-World Graph/Workbench Usage Patterns and Failure Modes

### The core question

Is graph-native browsing a real productivity advantage, or a representation that users find interesting but then work around?

The only reliable way to answer this is task-based studies: give participants actual work tasks and observe whether the graph model helps or hinders compared to a conventional browser. The five tasks that would produce the most diagnostic signal:

| Task | What it tests |
|------|---------------|
| **Comparison shopping** | Whether the graph helps hold multiple candidate nodes in active working set without losing them |
| **Source collection / note gathering** | Whether clipping, tagging, and edge-making are fast enough to become a real habit |
| **Revisiting older paths** | Whether traversal history is recoverable and legible — or whether users just start over |
| **Multi-pane reading and extraction** | Whether the tile-tree + graph split reduces or increases cognitive switching cost |
| **Research with evolving structure** | Whether the graph's organic growth is useful or produces spaghetti that users prune or abandon |

### What the docs already say

The graph UX research report (`2026-02-18_graph_ux_research_report.md`) surveys force-directed graph interaction literature and documents Graphshell's current interaction inventory. It focuses on parameters and physics behavior rather than task outcomes. The edge traversal research (`2026-02-20_edge_traversal_model_research.md`) identifies data loss in the traversal model (repeated navigations discarded, no timing data) and proposes a fix — but only on the grounds of data fidelity, not user behavior evidence. The interaction and semantic design schemes doc (`2026-02-24_interaction_and_semantic_design_schemes.md`) defines Lens/physics metaphors (Liquid/Gas/Solid states) as semantic channels, but does so from first principles, not from observed user mental models. `GRAPHSHELL_AS_BROWSER.md` specifies the graph-tile-viewer architecture and node lifecycle in detail.

None of these documents reports what users actually do, what they find confusing, or which graph affordances they ignore.

### Open questions

1. **Graph growth and abandonment**: In a real research or shopping session lasting 20–40 minutes, how large does a user's graph typically get? At what node count do users stop adding nodes and start closing them? Is there a natural working set size, or does it grow without bound?

2. **Edge mental model**: Do users understand edges as navigation history, or as semantic relationships they intend to create? The edge traversal model distinguishes `traversal-derived` edges from `user_asserted` edges — but do users understand or care about this distinction? Do they ever inspect edge direction or weight?

3. **Graph as spatial memory vs. graph as task output**: Users might benefit from the graph as a spatial memory aid (I can see where I've been) without benefiting from it as a task deliverable (I produce a graph that represents my research). These are different use cases with different success criteria. Which is actually happening?

4. **Pane management cognitive load**: The workbench tile-tree adds a layout dimension that conventional browsers do not have. Does managing pane arrangement help users work (by keeping related content co-visible) or does it add meta-work (I have to manage my panes on top of my research)?

5. **Failure mode catalogue**: What are the ways users get lost, confused, or stuck? Candidate failure modes: graph too dense to navigate; nodes misidentified (same URL, different context); edges accumulated that have no meaning; history not recoverable; clipping invoked but extracted content unusable. Which of these appear in real sessions?

6. **Comparison shopping specifically**: This is the highest-signal single task because it maps cleanly to a defined workflow (hold N candidates, compare attributes, eliminate). Does having candidates as nodes with spatial positions help? Or do users mentally compare using memory and open tabs in sequence regardless?

### Research methods

**Observational protocol** (think-aloud, screen recording):
- Recruit 6–10 participants who do regular web-based research (students, journalists, product researchers, comparison shoppers).
- Assign 2–3 of the five task types per participant.
- Think-aloud protocol with minimal facilitation.
- Record graph state at end of session (node count, edge count, edge types, tags applied).
- Post-session interview: what helped, what was confusing, what did you wish you could do.

**Metrics to capture**:
- Time-on-task vs. a conventional browser baseline (same task, incognito Chrome/Firefox).
- Number of nodes open at peak vs. at end (measures pruning behavior).
- Number of edges created by user vs. edges created by traversal (measures intentional vs. incidental graph use).
- Number of times user zoomed/panned to reorient vs. used keyboard search/filter (measures whether spatial layout provides value or not).
- Number of times user revisited a node via graph navigation vs. browser history / Ctrl+F.
- Task completion rate and subjective satisfaction.

**Minimum viable study**: Two sessions — one with Graphshell, one with a conventional browser — on a comparison shopping task (e.g., choosing a laptop, choosing a rental apartment). 8 participants. This would be enough to determine whether the graph model reduces or increases time-on-task, and whether users find the spatial layout meaningful or confusing.

### What to do with the results

The study should produce a **failure mode catalog** (ranked by frequency and severity) and a **benefit catalog** (ranked by how often users mentioned or relied on specific affordances). These two lists become the priority input for §§2–5 of this agenda.

If the study finds that the graph provides no meaningful advantage over a conventional browser for any of the tested tasks, the correct response is to revisit the core product hypothesis, not to iterate on physics parameters. If the study finds clear advantages for specific tasks and clear failure modes for others, those findings should be written up as a `2026-xx-xx_task_study_findings.md` and referenced by every subsequent design decision.

### Research deliverable

`2026-xx-xx_task_study_protocol.md` — the session protocol, task scripts, and metrics definition.

`2026-xx-xx_task_study_findings.md` — results, failure mode catalog, benefit catalog, and recommended design priorities.

---

## 2. Navigation/Traversal Semantics and Node-Edge Mental Models

### What the docs already say

The edge traversal research (`2026-02-20_edge_traversal_model_research.md`) establishes a detailed technical model: edges become `EdgePayload` structs with `Vec<Traversal>` records, the `!has_history_edge` deduplication guard is removed, and traversal frequency drives visual edge weight. The DOI/fisheye plan (`2026-02-25_doi_fisheye_plan.md`) defines a relevance score `DOI(n) = α·Recency + β·Frequency + γ·ExplicitInterest - δ·DistanceFromFocus` that drives rendering emphasis. The graph interaction spec (`graph_node_edge_interaction_spec.md`) defines the four-layer hierarchy (Graph Pane → Canvas → Node → Edge) and specifies ownership.

What's missing: evidence about what users expect navigation to mean, and whether the graph's traversal representation matches those expectations.

### Open questions

1. **"Back" as edge traversal vs. "Back" as stack pop**: In a conventional browser, Back means "return to the previous page in this tab's linear history." In Graphshell, Back means "traverse the History edge to the previous node." These are structurally different for users who have opened multiple nodes. When a user presses Back in a multi-node graph, which behavior do they expect? The current behavior (History edge traversal) may feel correct or deeply wrong depending on the user's mental model.

2. **Edge weight as salient signal**: The traversal model assigns visual weight (stroke width) based on traversal frequency. Is edge weight a signal users actually use to navigate? Or do they ignore edge thickness entirely and navigate by node proximity and label? This determines whether building the traversal frequency model is worth its implementation cost from a UX standpoint.

3. **User-asserted vs. traversal-derived edges**: The model distinguishes edges the user explicitly created (`user_asserted = true`) from edges created by navigation history. Do users understand or care about this distinction? Do they ever look at an edge and wonder "did I create this or did it appear automatically"? If they don't distinguish, the visual treatment needs to be clearer. If they do, the model is working.

4. **Edge inspection as a workflow**: The traversal model enables edge inspection (click an edge to see its traversal history, timing, dominant direction). Is this a workflow users would actually use, or is it too granular? Candidate use case: "I want to see whether I navigated from A to B more often than B to A, and when the last time I made that traversal was." Does that question arise naturally in real browsing tasks?

5. **Mental model of node identity**: Nodes are identified by a stable UUID, not by URL. URLs are mutable — the same node can browse to multiple URLs within its back/forward stack. This is architecturally correct but semantically complex. Do users think of a node as "this URL" (a conventional browser tab mental model) or "this topic/context" (the intended graph mental model)? Mismatches between these produce confusion when a node's URL changes and it "looks different" but is still connected by its original edges.

6. **Graph "Back" vs. workbench "Back"**: The workbench tile-tree has its own navigation history (which frame was active, in what order). Users may press Back expecting the workbench to restore a prior pane arrangement rather than the graph to traverse an edge. The two concepts are currently independent. This ambiguity is a known design gap.

### Research methods

**Card sorting / mental model probes** (remote, async):
- Show participants screenshots or recordings of a Graphshell session with 10–15 nodes and several edge types.
- Ask: "What does this line between nodes mean?" "If you pressed Back from this node, where would you expect to go?" "Which nodes are most important in this graph?"
- This surfaces naive mental models before participants are trained on Graphshell's actual model.

**Wizard-of-Oz traversal tests**:
- Have a facilitator operate Graphshell while the participant navigates verbally ("go back," "open that related node").
- Observe when verbal instructions and actual behavior diverge.

### Research deliverable

`2026-xx-xx_traversal_mental_model_findings.md` — a summary of observed mental models, specific divergences from the implemented model, and recommended interaction changes. Should feed directly into the edge traversal implementation plan (`2026-02-20_edge_traversal_impl_plan.md`) to validate or update its design assumptions.

---

## 3. Clipping/DOM Extraction and Knowledge-Capture Workflows

### What the docs already say

The clipping plan (`2026-02-11_clipping_dom_extraction_plan.md`) specifies a right-click → "Clip Element" → `document.elementFromPoint(x, y).outerHTML` pipeline that creates a new node with a `data:text/html;base64,...` URL, tagged `#clip`, linked to the source node via a `UserGrouped` edge. The implementation is split across four phases: context menu plumbing, content extraction, clip node creation, and clip rendering. The plan is marked Implementation-Ready.

The UDC tagging plan (`2026-02-23_udc_semantic_tagging_plan.md`) and the knowledge registry spec define a structured tagging layer. `VERSE.md` and the Tier 2 architecture describe how clipped content eventually feeds into engram extraction workflows.

What's missing is any investigation of how users actually want to capture knowledge — which of several possible workflows (clip element, clip page, annotate, extract structured data, save snapshot) they reach for, in what sequence, and what makes a capture "successful" to them.

### Open questions

1. **What do users mean when they say "I want to save this"?** There are at least four distinct intents: (a) I want to remember this URL; (b) I want to save the specific text/image I'm looking at right now; (c) I want to extract structured data from this page (price, date, author); (d) I want to annotate this with my own notes. The clipping plan implements (b). Do users reach for (b) first, or do they reach for (a) and only discover (b) later?

2. **Element selection accuracy**: `document.elementFromPoint(x, y)` returns the topmost element at the clicked coordinates. On real web pages, this is often a `<span>` or an icon, not the `<article>` or `<section>` the user intended. The plan acknowledges a heuristic ("smallest container with text/image") but doesn't specify it precisely. What should the heuristic be, and how often does the naive implementation fail on real-world pages?

3. **Clip node usability**: A clip node with a `data:text/html;base64,...` URL contains the extracted HTML fragment, stripped from its parent page's CSS. How often is this fragment readable and useful? Complex pages with heavily styled components may produce unreadable clips. How should the clip fall back when the extracted HTML is not self-contained?

4. **Readability extraction as an alternative or complement**: The aspirational protocols doc (`2026-02-22_aspirational_protocols_and_tools.md`) mentions readability extraction as a more reliable way to get clean article text. When would a user prefer readability extraction over element-level clipping? Are these two different tools, or two versions of the same action with different precision?

5. **Tagging as a capture step**: The UDC tagging system assigns semantic classification to nodes. Do users tag at the time of capture (as part of the save workflow) or after the fact (as a curation step on a collection they've built up)? If tagging is at capture time, the context menu needs to surface UDC suggestions inline. If it's a deferred curation step, the tag assignment UI can be separate.

6. **Extraction for Verse vs. extraction for personal use**: Clipping for personal knowledge retention and clipping as a contribution to a Verse FLora pipeline are different goals with different quality requirements. A personal clip can be messy and still useful. A Verse-contributing clip needs enough structure to produce useful engram derivations. Does the same capture workflow serve both goals, or do they need different entry points?

### Research methods

**Contextual inquiry** (observe users clipping in their natural habitat):
- Recruit 4–6 users who currently use browser bookmarks, read-later tools (Pocket, Instapaper), or note-taking apps (Notion, Obsidian) alongside their browser.
- Watch one session of their normal research or reading workflow.
- Note: when do they save something, what do they capture (URL vs. text vs. image), how do they annotate it, how do they find it later.

**Prototype test** (clipping workflow):
- Build a functional prototype of the four-phase clipping pipeline.
- Run think-aloud sessions on 3–4 task scenarios: clip a product listing, clip a code example from documentation, clip a quote from a news article.
- Measure: clip success rate on first try, number of attempts before usable clip, subjective confidence that the clip captures "what I meant."

### Research deliverable

`2026-xx-xx_clipping_workflow_findings.md` — answers to the six questions above, with recommendations for the element selection heuristic, fallback behavior, and tagging flow timing. Should update or extend the clipping plan (`2026-02-11_clipping_dom_extraction_plan.md`) with the tested interaction design.

---

## 4. Viewer/Fallback Behavior Across Web and Non-Web Content

### What the docs already say

The viewer state matrix (`2026-02-27_viewer_state_matrix.md`) documents which viewer IDs are actually operational today: `viewer:webview`, `viewer:plaintext`, and `viewer:markdown` are stable. `viewer:pdf` and `viewer:csv` are declared but not pane-embedded. `viewer:image`, `viewer:directory`, `viewer:audio` are documented targets, not yet active. The viewer presentation and fallback spec (`viewer_presentation_and_fallback_spec.md`) defines the normative model: viewer selection is app-owned and deterministic, degraded states are explicit, tool surfaces are distinguishable from content nodes. The universal content model plan (`2026-02-24_universal_content_model_plan.md`) specifies `mime_hint` and `AddressKind` as the routing signals for non-web content.

What's missing is an account of how users experience the current viewer gap — when they encounter a PDF, a local file, or an unsupported MIME type — and which fallback behaviors are acceptable vs. disorienting.

### Open questions

1. **PDF handling expectation gap**: `viewer:pdf` currently routes through `viewer:webview` (Servo renders the PDF via its built-in renderer). Users may not notice, or they may notice immediately that annotation, text selection, and printing behave differently from a native PDF viewer. Which PDF interactions do users attempt first, and which fail? Does the fallback to webview constitute an acceptable experience, or does it break the workflow?

2. **Local file node behavior**: A node pointing to a local file path (e.g., a downloaded document, a local codebase entry point) has no webview equivalent. The universal content model uses `AddressKind::LocalFile` to route to the appropriate viewer. From a user perspective: what does "opening a local file in Graphshell" mean, and is the current behavior (render in plaintext/markdown viewer for text files, placeholder for binary files) legible?

3. **The placeholder state as a failure signal**: The viewer spec requires that `Placeholder` tiles be explicit non-content surfaces, not silent failures. In practice, when a user opens a node that Graphshell cannot render, does the placeholder communicate why (unsupported format, loading in progress, network error)? Or does it look like a broken pane? This is both a UX research question and a diagnostics question.

4. **Multi-format nodes**: A user might clip a page that contains a mix of HTML, embedded PDF, and image assets. The resulting clip node is a single `data:text/html` node. If the user later opens it and finds that the embedded PDF is not rendered, is this a surprising failure or an expected limitation? How should the viewer badge communicate "partial render"?

5. **Audio and video node expectations**: `viewer:audio` is a documented target. When a user adds a YouTube URL or a podcast URL as a node, what do they expect? Play in-node (media player embedded in the tile), or open in the system browser? The answer affects whether the viewer registry needs media player capabilities or just a "hand off to external player" fallback.

6. **Viewer selection transparency**: The viewer spec says "the selected viewer class should be inferable from the pane's behavior and affordances." Is this actually true in practice? Can a user tell, at a glance, what kind of surface they're looking at — webview, plaintext, markdown, placeholder? Or do they have to probe the pane's behavior to find out?

### Research methods

**Behavioral probe** (semi-structured session):
- Give participants a pre-populated graph containing nodes with several content types: web URLs, a local PDF path, a local text file, a YouTube link, an unsupported binary file.
- Ask them to "explore and open" each node.
- Note: first action taken, interpretation of placeholder states, any expression of surprise or confusion.

**Fallback heuristic testing**:
- Systematically test the current fallback chain on 20–30 real-world URLs drawn from common workflows (academic papers, GitHub repos, documentation sites, product pages, news articles).
- Document viewer selection outcome, render quality, and any degraded/partial states.
- This is an engineering task, not a user study, but it directly informs what cases need better fallback design.

### Research deliverable

`2026-xx-xx_viewer_fallback_field_report.md` — documents observed failure cases by content type, current fallback behavior, user interpretation of placeholder/degraded states, and recommended fallback improvements. Should feed the viewer state matrix update process and the planned extension work (dedicated PDF/CSV viewers, richer viewer-state badges).

---

## 5. Performance and Visual Scaling Rules for Dense Graphs

### What the docs already say

The performance tuning plan (`2026-02-24_performance_tuning_plan.md`) sets targets of 500 nodes at 60 FPS and 1000 nodes at 30+ FPS, with a four-phase implementation: viewport culling → node/edge LOD → badge animation budget → physics budget. The DOI/fisheye plan (`2026-02-25_doi_fisheye_plan.md`) adds a relevance-based rendering layer on top of LOD that scales node size and opacity by a recency/frequency/interest score. The graph UX research report (`2026-02-18_graph_ux_research_report.md`) documents physics parameters and layout quality metrics from the literature.

What's missing is a UX-grounded answer to the question: at what node count does the graph become cognitively unnavigable regardless of FPS? And which visual interventions — LOD, DOI scaling, fisheye, clustering — are actually effective at restoring navigability versus just reducing render cost?

### Open questions

1. **Cognitive density threshold vs. render density threshold**: The performance targets (500/1000 nodes) are render targets, not usability targets. A graph with 200 nodes might already be cognitively unnavigable if they are uniformly distributed and unlabeled. What is the practical cognitive density threshold — the node count at which users stop being able to find what they are looking for without search/filter — and how does it relate to the render performance target?

2. **DOI score as a navigation aid**: The DOI score weights recency (0.30), frequency (0.20), explicit interest (0.30), and distance from focus (0.20). These weights are stated as defaults with no empirical basis. Do users actually navigate toward high-recency/high-frequency nodes in preference to low-recency nodes? Or do they navigate by label/URL, making DOI score irrelevant to their actual navigation decisions?

3. **Fisheye distortion vs. spatial stability**: The semantic fisheye plan preserves node positions (only draw-size changes) to avoid disrupting the user's spatial mental map. But the canonical argument for fisheye (Furnas 1986) is that it shows more detail near focus at the cost of spatial distortion. Does the non-distorting version (size-only scaling) provide meaningful benefit, or is it too subtle to be useful? Is spatial stability actually important to Graphshell users?

4. **LOD label reduction and reorientation cost**: When zoom decreases and labels are hidden (Phase 2 of the performance plan: zoom < 0.5 hides labels), users lose the primary way they identify nodes. Do users zoom back in to read labels (reorientation cost), or do they use search/filter instead? Is hiding labels at low zoom a net productivity win or a net productivity cost?

5. **Clustering vs. explicit layout for large graphs**: At 100+ nodes, physics-based layout produces clusters, but the cluster structure may not match the user's own topical organization. The "Brainstorm" and "Semantic Hierarchy" lens schemes in the interaction design doc suggest clustering by topic. Does automatic clustering help users find nodes faster, or does it feel like the graph is rearranging itself in ways the user didn't intend?

6. **Ghost tier nodes and structural context**: The DOI plan specifies that Ghost-tier nodes (DOI < 0.10) should remain visible as faint dots to provide structural context. Is this the right call? Do users actually use the faint structural context of Ghost nodes to maintain their mental map? Or do they perceive Ghost nodes as visual noise that increases apparent density without providing usable information?

### Research methods

**Controlled density experiment**:
- Create graphs of controlled sizes: 20, 50, 100, 200, 500 nodes, all using realistic browsing session content (not random graphs).
- Present each to 6–8 participants with a "find this node" task.
- Measure: time to find target, number of pan/zoom operations, search/filter usage rate.
- Vary LOD and DOI settings to isolate which visual interventions reduce search time.

**Think-aloud density navigation**:
- Give participants a dense graph (150–200 nodes) from a realistic research session.
- Ask them to reconstruct what the browsing session was about, and to find 3 specific nodes.
- Note: which navigational strategies they use (spatial scanning, zoom, search, edge traversal), and which graph features they ignore.

**Visual clarity rating**:
- Present screenshots of the same 100-node graph with: (a) all labels visible, (b) LOD-reduced labels, (c) DOI-scaled nodes + fisheye, (d) DOI-scaled nodes without fisheye.
- Ask participants to rate: "How clearly can you tell what's in this graph? How easy would it be to find a specific node?"
- Provides a subjective legibility baseline without requiring task completion.

### Research deliverable

`2026-xx-xx_dense_graph_usability_findings.md` — documents the cognitive density threshold observed empirically (node count at which find-a-node time increases sharply), effectiveness of each visual intervention (LOD, DOI, fisheye) measured against task time, and recommended default parameter settings for the DOI weights (α, β, γ, δ) and fisheye radius based on observed behavior rather than first principles. Should update the DOI/fisheye plan's weight defaults and the performance tuning plan's LOD thresholds.

---

## Cross-Cutting Notes

### Priority and dependency

§1 (task studies) must come first. Its findings determine which of §§2–5 are actually the right problems to solve. Running §§2–5 before §1 risks optimizing the wrong things.

§2 (traversal mental models) and §3 (clipping workflow) can run in parallel with each other, and their results are actionable immediately — they feed directly into implementation work (the edge traversal impl plan and the clipping plan) without requiring §1 to complete first.

§4 (viewer fallback) is partly an engineering task (the behavioral probe) and partly a design question. The behavioral probe of real-world URLs can be done immediately and informs the next viewer spec revision.

§5 (dense graph scaling) requires a working DOI/fisheye implementation before the controlled density experiment can be run. The think-aloud and visual clarity rating tasks can be done with the current implementation at lower node counts.

### Relationship to existing design decisions

The edge traversal model (`2026-02-20_edge_traversal_model_research.md`) makes specific design assumptions — particularly that traversal frequency is a meaningful signal and that edge weight is useful. §2 of this agenda directly tests those assumptions. If users never look at edge weight, the hot/cold tiered storage model is correct as a data fidelity measure but not as a UX feature.

The DOI weights (α = 0.30, β = 0.20, γ = 0.30, δ = 0.20) in the fisheye plan are stated as defaults to be refined. §5 of this agenda provides the empirical basis for that refinement.

The clipping plan's element selection heuristic ("smallest container with text/image") is acknowledged as simplified. §3 of this agenda should produce a tested heuristic that replaces the placeholder.

### What this agenda does not cover

- Formal accessibility testing (covered by the accessibility research doc and `SUBSYSTEM_ACCESSIBILITY.md`). Accessibility testing is a separate track that should run in parallel.
- Verse P2P collaboration UX (covered by the Verse research agenda). The social/collaborative dimension of browsing is out of scope until Tier 1 sync is shipped and being used.
- Settings and configuration UX (a separate surface; not task-driven in the same way as the browsing workflows studied here).
