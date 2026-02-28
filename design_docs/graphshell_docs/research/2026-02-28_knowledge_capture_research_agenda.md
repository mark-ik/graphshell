# Knowledge Capture Workflows — Research Agenda

**Date**: 2026-02-28
**Status**: Research / Active
**Author**: Arc
**Feeds Into**:
- `implementation_strategy/viewer/2026-02-11_clipping_dom_extraction_plan.md`
- `implementation_strategy/canvas/2026-02-23_udc_semantic_tagging_plan.md`
- `implementation_strategy/canvas/2026-02-20_node_badge_and_tagging_plan.md`
- `implementation_strategy/canvas/semantic_tagging_and_knowledge_spec.md`
- `technical_architecture/GRAPHSHELL_AS_BROWSER.md §§4–5`

---

## Background

This is where Graphshell either becomes a genuine productivity tool or stays a novelty.
Browsing with a graph UI is interesting; capturing, organizing, and re-surfacing knowledge in
that graph is where the differentiated value lives. The current implementation has the
foundations:

- **Clipping**: `GraphSemanticEvent::ContextMenu` → script injection → `data:` node + `#clip`
  tag + `UserGrouped` edge. Architecturally sound but not yet shipped; friction of use is
  unvalidated.
- **UDC semantic tagging**: `KnowledgeRegistry` routing, `UdcProvider`, reconciliation path,
  semantic physics force. Phase 1 in progress; assumes label-first (`"math" → udc:51`)
  inference works in practice. Unvalidated.
- **Badge/tag assignment UI**: `T`-key floating panel, nucleo fuzzy suggestions, icon picker.
  Designed but not started.
- **Import flows**: Firefox bookmark import → node tags. Declared in GRAPHSHELL_AS_BROWSER.md
  §5. No evidence of user validation.

The risk is the same as with any knowledge management tool: the system is more powerful in
theory than in practice, because classification and capture decisions have to fit naturally
into the browsing flow to happen at all. If they don't, users accumulate untagged, unlabeled
nodes and the graph becomes clutter instead of structure.

This agenda names the four empirical gaps the current plans cannot resolve from design alone.

---

## Thread 1 — Clipping and DOM Extraction: Friction Threshold

### What the Docs Say

The clipping plan defines a four-phase pipeline:

1. **Phase 1**: Extend `GraphSemanticEvent` with `ContextMenu`; implement `handle_context_menu`
   in `EmbedderWindow`; show an egui popup with "Clip Element" item.
2. **Phase 2**: `extract_element_at(webview_id, x, y)` via `webview.evaluate_script(...)` —
   heuristic element identification (smallest container with text/image), `outerHTML`, computed
   styles, bounding rect.
3. **Phase 3**: On "Clip Element": run extraction, generate `data:text/html;base64,...` URL,
   emit `GraphIntent::AddNode` + `GraphIntent::TagNode { tag: "#clip" }` +
   `GraphIntent::CreateUserGroupedEdge`.
4. **Phase 4**: Ensure `viewer:webview` handles `data:` URLs; render `#clip` nodes with
   distinct visual (dashed border or scissor badge).

The plan acknowledges that element selection is heuristic: "smallest container with text/
image." It does not specify how users select the granularity of what they want to clip
(paragraph vs. section vs. full page), nor how they know the clip succeeded.

The validation criteria include "Content Fidelity" (extracted HTML element is isolated from
original page) and "Persistence" (clip survives restart). Neither addresses friction.

### What Is Not Known

1. **Clip granularity mismatch.** The heuristic picks the "smallest container" at a point.
   A right-click in the middle of a long article might select a single sentence's `<p>` tag
   or a three-level nested `<div>`. Users expect to clip "this section" not "this element."
   Does the heuristic produce clips at the granularity users actually want, and does the
   failure mode (too-small or too-large) make users distrust the feature?

2. **Clip confirmation UX.** After "Clip Element" is selected, a new node is created in the
   graph. How does the user know? A toast? A flash on the graph? Nothing? The plan does not
   define a success signal, and the lack of one may cause duplicate clipping (user repeats
   because they're uncertain the action took effect).

3. **Clip-use-later vs. clip-and-annotate immediately.** Is the primary use pattern "clip
   it now, annotate it later" (fast path, no interruption to reading flow) or "clip and
   immediately add a note or tag" (slower, higher capture quality)? The current design does
   not offer the latter. If users want to annotate at clip time, the graph node creation
   must open an annotation affordance immediately.

4. **Clip-worthy element recognition.** Web pages vary wildly in DOM structure. On pages
   with flat structure (a Wikipedia article), the heuristic probably works. On pages with
   heavily componentized DOM (a React SPA with deeply nested shadow DOM), the smallest
   meaningful container might be three levels above the visible text. Has this been tested
   against the content types users actually clip?

5. **Frequency of clipping vs. opening a full node.** Is clipping a dominant workflow action
   or an occasional one? If users clip rarely, the right-click menu placement is acceptable.
   If users clip constantly, a keyboard shortcut or hover affordance (clip button on hovered
   selection) may be required for the feature to feel fast enough to use.

### Research Methods

**Study 1.1 — Clip Granularity Field Test (moderated, n=12)**
Give participants a research task requiring them to save 5 specific pieces of information from
3 different web pages. Ask them to use the clip feature. Observe:
- How often the heuristic selected the right element on first try.
- How often participants re-tried because the clip was too small or too large.
- Whether participants wanted to adjust the clip boundary.

Success threshold: heuristic correct on first try ≥75% of clips. Below that threshold,
interactive boundary selection (expand/contract the selection) must be added to Phase 2.

**Study 1.2 — Clip Confirmation Recognition (unmoderated, n=30)**
Show a screen recording of a clip action completing. Vary the success signal: (a) no signal,
(b) toast "Clipped to graph", (c) brief highlight on the new graph node, (d) both toast and
highlight. Ask: "What happened after you clicked 'Clip Element'?" and "Are you confident the
clip succeeded?" Record certainty score (1–5) per condition.

Target: condition (a) must score ≤ 2.0 certainty to confirm the gap. Recommended signal
design: whichever condition scores ≥4.0 at lowest intrusiveness.

**Study 1.3 — Annotate-at-Clip-Time Demand Probe (semi-structured, n=10)**
During a think-aloud browsing and capture session, after each successful clip ask: "Do you
want to add anything to this clip right now, or move on?" Count the proportion of clips where
the user wants to annotate immediately. If ≥40% want to annotate immediately, add an optional
inline annotation affordance to the Phase 3 clip flow.

**Study 1.4 — Clip Frequency Behavioral Log (diary/instrumentation, n=15, 2 weeks)**
Instrument the clip action with an event counter (no content logged — count only). At end of
study, aggregate clips per session. If median clips/session ≥ 5, add a keyboard shortcut
(`Ctrl+Shift+C` or similar) as a Phase 1 requirement, not a later enhancement.

### Deliverable

A **Clip UX Requirements Addendum** to `2026-02-11_clipping_dom_extraction_plan.md`
specifying:
- Whether interactive element boundary selection is required in Phase 2.
- The required success-signal design (toast / node highlight / both / neither).
- Whether an annotate-at-clip affordance is required for Phase 3.
- Whether a keyboard shortcut is required before Phase 1 ships.

---

## Thread 2 — UDC-Like Classification: Label-First Inference in Practice

### What the Docs Say

The UDC semantic tagging plan centers on **label-first inference**: users type natural
language ("math", "calc", "history of art") and `KnowledgeRegistry::search()` via nucleo
returns ranked UDC code suggestions ("Mathematics (udc:51)", "Calculus (udc:517)"). Selecting
a suggestion applies the `udc:` tag.

The semantic physics force (`SemanticGravity`) then clusters nodes with overlapping UDC
prefixes, making the graph self-organize into subject areas. "Group Tabs by Subject" creates
`UserGrouped` edges from the same clusters.

The plan acknowledges the multi-class problem (a node can belong to multiple UDC branches)
and the performance problem (centroid optimization for large graphs). Phase 1 (registry
parsing + reconciliation) is in progress; Phase 2 (semantic physics) has not started.

The `KnowledgeRegistry` spec defines `validate` returning `Valid` / `Warning` / `Invalid`.
Warning tags (unrecognized `#` prefix or unknown UDC depth) are accepted and emitted. Invalid
tags (malformed UDC code) are rejected at the UI layer.

The `semantic_tagging_and_knowledge_spec.md` defines the dirty-flag reconciliation path:
tag intents update `semantic_tags`, set `semantic_index_dirty`, and the frame loop reconciles.

### What Is Not Known

1. **Whether label-first inference produces accurate suggestions at the depth users need.**
   A user thinking about "machine learning" might query "ML" and get no UDC result, or get
   `udc:004.8` (Artificial intelligence) which is arguably correct but feels unfamiliar. Does
   the nucleo fuzzy search + UDC label dataset cover the vocabulary users actually use to
   describe their content?

2. **Whether users apply tags to nodes at all.** Many knowledge management systems include
   rich tagging features that users ignore in practice. The design assumes users will
   voluntarily tag nodes with UDC codes. Is that assumption valid for the target user
   (a researcher, a student, a developer)? Or do users want automated inference from page
   content instead of manual tagging?

3. **Whether semantic physics clustering is legible.** The graph self-organizes by UDC
   prefix — but does the resulting layout look like a meaningful map to users, or does it
   look like unexplained drift? Users who don't know UDC exists may find that nodes move
   unexpectedly as they add tags. They may attribute the movement to a physics bug rather
   than a semantic feature.

4. **Tag assignment panel friction.** The `T`-key panel with nucleo suggestions is the
   designed interaction. Is `T` key a discoverable trigger for tagging, or will users
   discover tagging only via the context menu? Does the panel's position (anchored to the
   node) work on small viewports or at high graph zoom?

5. **How many tags users apply per node.** If users apply 0–1 tags per node, semantic
   physics has no data to work with and the feature is invisible. If users apply 3–5 tags,
   the badge orbit overflows. The badge priority and overflow chip design assumes a
   distribution that hasn't been measured.

### Research Methods

**Study 2.1 — Label-First Query Accuracy Test (unmoderated, n=40)**
Present 20 natural-language topic descriptions drawn from real research/productivity contexts
(e.g., "statistical methods for surveys", "ancient Roman architecture", "React component
performance", "climate policy", "machine learning for image classification"). For each,
show the top-3 UDC suggestions returned by `KnowledgeRegistry::search()`. Ask: "Does any of
these describe what you meant?" Record hit rate per query.

Target: ≥70% of queries must produce at least one acceptable suggestion in the top 3. Below
that threshold, the UDC dataset must be supplemented with an alias/synonym layer before the
tag panel ships.

**Study 2.2 — Voluntary Tagging Behavior Diary (n=15, 2 weeks)**
Ask participants to use Graphshell for their normal browsing/research work. Provide the `T`-
key tag panel. At end of study, measure: (a) median tags per node, (b) % of nodes with any
tag, (c) % of tags that are UDC vs. user-defined, (d) whether users report tagging as useful.

Threshold to validate the manual-tagging assumption: ≥50% of nodes have ≥1 tag after 2
weeks. If fewer, add automated content-based tag suggestion (from page title/content via
`AgentRegistry` or heuristic keyword extraction) as a Phase 1 fallback.

**Study 2.3 — Semantic Physics Legibility (moderated, n=10)**
Enable semantic physics with a graph pre-populated with nodes across 3 UDC subject areas
(science, history, technology), but don't tell participants about semantic tagging. Ask: "Do
you notice anything happening to the graph layout? What do you think is causing it?" If ≥50%
attribute the clustering to something other than semantic tags (or to a bug), add a
first-run tooltip or legend explaining semantic physics before it is turned on by default.

**Study 2.4 — Tag Panel Discoverability and Friction (moderated, n=10)**
Give participants a node and ask them to tag it. Observe whether they discover `T` key, right-
click → Tags, or look for a tag button on the node. Measure time to first successful tag
assignment. Record what would have made discovery faster.

### Deliverable

A **Semantic Tagging Adoption Report** with:
- Query accuracy results and, if below threshold, a synonym/alias expansion list for the UDC
  dataset covering the most common missed queries.
- A go/no-go recommendation on shipping the manual-only tag UI vs. requiring automated
  content-based suggestion as a prerequisite.
- A recommendation on whether semantic physics should be on or off by default, and what
  first-run explanation (if any) is required.
- A recommended discoverability improvement for the tag panel if `T` key is insufficient.

Feeds into `2026-02-23_udc_semantic_tagging_plan.md` Phase 1 and Phase 2 sequencing.

---

## Thread 3 — Extracted Representations: What Is Useful in the Graph Later

### What the Docs Say

A clipped node stores its content as a `data:text/html;base64,...` URL — the full `outerHTML`
of the extracted element, self-contained. The clip gets a `#clip` tag, a distinct visual, and
a `UserGrouped` edge back to the source node. The title is inherited from the source node's
page title unless overridden.

The badge plan notes that `#clip` nodes get a "distinct node shape/border in graph view"
and a `✂️` badge. No detail on what the clip node renders when opened — it re-renders via
`viewer:webview` with the `data:` URL, which should display the extracted HTML fragment.

The edge traversal model (`EdgePayload`) preserves traversal history between source and clip.
If the source node is deleted, the clip node becomes an orphan with only the topology of
the tombstone edge to indicate its origin.

The UDC tagging plan notes that `AgentRegistry` could in the future automatically suggest UDC
tags from node content — but this is declared as a "future" item, not a Phase 1 target.

There is no specification of: what metadata is extracted alongside `outerHTML` (page URL,
page title, extraction timestamp, section heading context), whether clips support inline
annotations, or how clips surface in the graph's search/omnibar.

### What Is Not Known

1. **What users return to in a clip node.** When a user re-opens a clip they made yesterday,
   what are they looking for? The raw HTML fragment? The source URL so they can go back to
   the original? An annotation they made at clip time? The relationship to other clips from
   the same source? The current model only guarantees the `outerHTML` — which may be visually
   correct but informationally incomplete.

2. **Whether `UserGrouped` edge back to source is the right relationship type.** The clip is
   derived from the source, but it is also independent (that's the point — it survives source
   deletion). A `DerivedFrom` edge type might be more semantically correct than `UserGrouped`,
   and it would surface differently in traversal history. Has this distinction been evaluated?

3. **Title and context inheritance.** A clip node titled "Wikipedia — History" gives no hint
   about what was clipped. The useful title is "French Revolution causes — Wikipedia" or
   the first heading in the clipped fragment. How should clip node titles be derived — from
   the page title, from the nearest heading in the DOM, from a user-written annotation, or
   from automatic content summarization?

4. **How clips surface in omnibar search.** The omnibar `is:clip` predicate is mentioned in
   the tag plan. But when searching for a topic, should clips appear alongside full-page
   nodes, or should they be demoted (they are fragments, not canonical sources)? Should clip
   content be full-text indexed for search?

5. **Clip expiry and archival.** A clip from a page that later goes offline is more valuable
   than a full-page node (it preserved the content). But clips accumulate. Is there a natural
   lifecycle for clips (GC after N days, auto-archive when source node is cold-evicted) or
   do they persist indefinitely?

### Research Methods

**Study 3.1 — Clip Return Behavior Interview (semi-structured + think-aloud, n=10)**
Give participants a session where they clip 8–10 items during a research task. Two days later,
bring them back and ask: "Go back and find something you clipped on Tuesday." Observe: (a)
how they navigate to their clips (graph search? badge filter? edge traversal?), (b) what they
look at first when they open a clip (content? source URL? timestamp? origin edge?), (c) what
is missing that would help them.

Produces a ranked list of "what metadata must be present on a clip node at retrieval time."

**Study 3.2 — Edge Type Perception (unmoderated, n=20)**
Show two graph screenshots: one with a `UserGrouped` edge between source and clip node (same
visual as a user-drawn grouping edge), one with a hypothetical `DerivedFrom` edge (dashed
arrow with a distinct color). Ask: "Which relationship better describes 'this clip was
extracted from this page'?" Record preference and explanation. If ≥60% prefer `DerivedFrom`,
add it as a distinct edge type in the clipping plan.

**Study 3.3 — Clip Title Satisfaction (unmoderated, n=30)**
Show 6 clip nodes with different title derivation strategies: (a) page title only, (b) page
title + nearest `<h2>` heading, (c) user-defined annotation (no auto-title), (d) first 20
words of clipped content, (e) auto-generated summary (simulated), (f) source URL domain +
heading. Ask: "Which title would most help you remember what this clip is about?" Rank all
6 from most to least useful. Use results to define the Phase 3 title derivation algorithm.

**Study 3.4 — Clip Search and Retrieval Path Preference (unmoderated, n=25)**
Present 3 retrieval interfaces for a set of 15 clips: (a) omnibar `is:clip` filter with full-
text search, (b) graph view filtered to show only `#clip` nodes + their source connections,
(c) a "Clips Pane" sidebar showing clips as a flat list with timestamps. Ask: "Which would
you use to find a specific clip you made last week?" and "Which would you browse to rediscover
something you forgot you clipped?" Record preference split for targeted vs. serendipitous
retrieval separately — they may favor different surfaces.

### Deliverable

A **Clip Node Information Model** document specifying:
- The required metadata fields for a clip node (source URL, extraction timestamp, nearest
  heading context, user annotation, `outerHTML`).
- A recommended edge type for the source–clip relationship (retain `UserGrouped` or introduce
  `DerivedFrom`).
- The title derivation algorithm (which of the 6 candidates, or a combination).
- Retrieval surface requirements: whether a dedicated clips pane is justified, and what the
  `is:clip` omnibar behavior must produce.

Feeds into `2026-02-11_clipping_dom_extraction_plan.md` Phase 3 (Clip Node Creation) and
Phase 4 (Clip Rendering).

---

## Thread 4 — Import Flows: Which Paths Create Immediate Value vs. Noise

### What the Docs Say

GRAPHSHELL_AS_BROWSER.md §5 declares:
- Firefox `bookmarks.html` import creates nodes with tag metadata.
- Import/export via `graphshell://settings/bookmarks` settings page.
- Bookmarks are implemented as node tags (folder paths as tag strings like `bookmarks/work`).
- The omnibar `@b` scope and `is:starred` predicate surface bookmarked nodes.

The settings architecture plan (`2026-02-20`) and workbench manifest/persistence plan
(`2026-02-22`) are listed as the delivery vehicles for this UI. No specification of how
imported nodes are laid out in the graph, how many nodes a typical bookmark import produces,
or what happens when 500+ bookmarks are imported simultaneously.

There is no specification for browser history import, read-it-later import (Pocket, Instapaper,
Raindrop), or file system import (a folder of downloaded PDFs).

The UDC tagging plan notes automated classification from AgentRegistry as a "future" item.
There is no current plan for automatically organizing imported nodes beyond inheriting the
bookmark folder structure as tags.

### What Is Not Known

1. **Import volume and graph legibility.** A typical Firefox bookmark file contains 200–2000+
   bookmarks. Importing 500 bookmarks as 500 nodes creates a graph that is immediately
   overwhelming. Is there a pre-import filtering or sampling step? Should imports create
   nodes only for the bookmarks in specific folders, or let users select which folders to
   import?

2. **Immediate utility vs. deferred organization.** After a bookmark import, does the graph
   immediately feel useful, or does it feel like clutter that must be organized before it
   has value? The answer determines whether import should include an onboarding flow
   (cluster by folder, suggest UDC tags, remove duplicates) or whether raw import is
   acceptable as a starting point.

3. **History import: wanted or not?** The doc does not mention history import. Browser
   history can have thousands of entries per day. Is history import in scope? If so, what
   granularity (full history, last N days, visited ≥N times)? History-to-graph could be
   a powerful way to bootstrap a research graph, but it could also produce noise.

4. **Read-it-later service import (Pocket, Raindrop, Instapaper).** These services represent
   a curated subset of "things I wanted to return to" — arguably higher quality than raw
   bookmarks. Does the target user population use these services, and if so, is import from
   them higher leverage than Firefox bookmark import?

5. **File import (local PDFs, markdown files, exported HTML).** A user with a folder of
   downloaded research papers has structured content that could be imported as `file://`
   nodes. Does the target user want this? How should a folder of 50 PDFs be represented
   in the graph — 50 individual nodes, or a directory node that expands on demand?

6. **Duplicate and dead-link handling.** Bookmark files contain duplicates and links that
   have gone offline. Should import silently deduplicate (same URL → single node)? Should
   it flag or filter dead links at import time?

### Research Methods

**Study 4.1 — Import Intent Interview (semi-structured, n=12)**
Recruit research/productivity users who have accumulated browser bookmarks. Ask:
- "What do you keep in your bookmarks?" (taxonomy of content)
- "Do you ever go back and browse your bookmarks?" (retrieval pattern)
- "If you could import your bookmarks into Graphshell, what would you expect to see?"
- "What would make you NOT want to import them?" (concerns: volume, noise, effort to organize)

Produces a set of import personas (heavy bookmark users vs. light, organized vs. chaotic)
and the expected import flow per persona.

**Study 4.2 — Import Volume Legibility Threshold (in-person prototype, n=8)**
Import bookmark sets of varying sizes (10, 50, 200, 500 nodes) into a prototype graph. Ask
participants to: (a) find a specific bookmark, (b) identify clusters of related content.
Measure success rate and time-on-task per volume level. Identify the volume threshold at
which the graph becomes unusable without prior organization. That threshold becomes the
recommended default import batch size and the trigger for a post-import organization flow.

**Study 4.3 — Read-It-Later vs. Bookmark Import Preference (unmoderated, n=30)**
Present screenshots of a graph populated from (a) Firefox bookmarks import, (b) Pocket
import, (c) browser history last 30 days. Ask: "Which of these graphs would be most
immediately useful to you? Why?" Record which source produces highest perceived immediate
utility, and whether the answer varies by user type (researcher vs. developer vs. general
user).

**Study 4.4 — Automated Organization at Import (concept test, n=15)**
Show two import flows: (a) raw import — all bookmarks become nodes, unorganized; (b) guided
import — during import, system clusters by folder tag and suggests UDC codes for each
cluster, user confirms or adjusts. Ask: "Which would you prefer?" and "If guided import
added 30 seconds to the process, would you still choose it?" Record preference and
willingness-to-wait for organization quality.

### Deliverable

An **Import Flow Requirements Spec** defining:
- The supported import sources for v1 (Firefox bookmarks, Pocket/Raindrop, `file://` folder,
  browser history), ranked by user demand from Study 4.1 and 4.3.
- The default import batch size cap and the conditions triggering a post-import organization
  flow (from the legibility threshold in Study 4.2).
- Whether guided import (cluster + UDC suggestion) is required at the import step or can be
  deferred to a post-import organize command (from Study 4.4).
- Duplicate and dead-link handling policy.

Feeds into the bookmarks/import sections of GRAPHSHELL_AS_BROWSER.md and the settings
architecture plan (`2026-02-20_settings_architecture_plan.md`).

---

## Cross-Thread Concerns

### The Capture-to-Retrieval Loop

Knowledge capture only has value if users can retrieve what they captured. All four threads
have a retrieval dimension that the current specs underspecify:
- Clips need a retrieval path beyond "navigate back to the source node and find the edge."
- Tags need to be findable via omnibar predicates (`is:clip`, `is:starred`, `udc:51`).
- Imported nodes need to be distinguishable from natively-browsed nodes in search results.

The omnibar is the canonical search surface, but none of the plans above define what the
omnibar result set looks like when knowledge-capture artifacts are present. A cross-thread
deliverable — an **Omnibar Knowledge Capture Surface Spec** — may be needed after these four
agendas are complete.

### Automation vs. Manual Classification

All four threads touch the question of how much classification should be manual vs.
automated. The UDC plan defers automation to `AgentRegistry`. The clipping plan has no
automation. The import plan has no post-import organization automation. The badge plan has
manual-only tagging.

The diary studies in Threads 2 and 4 will produce empirical data on whether users actually
classify manually. If the answer is "rarely," the automation prerequisite moves forward on
the roadmap before the manual classification UI ships — there is no value in a sophisticated
tag assignment panel if the graph never has tags in it.

### First-Session Utility

Import (Thread 4) is the most important first-session signal. A user who imports their
bookmarks and immediately finds the graph useful is more likely to stay and use clipping and
tagging over time. A user who imports and finds noise is more likely to abandon the tool.
Thread 4 research should be prioritized first, as its results will set the context for
whether Threads 1–3 are high or medium urgency.

---

## Summary: Four Open Questions → Four Deliverables

| Thread | Core Question | Deliverable |
|--------|--------------|-------------|
| 1. Clipping Friction | Does the clip flow feel fast enough to use constantly, and does it produce the granularity users want? | Clip UX Requirements Addendum |
| 2. UDC Classification | Will users actually apply UDC tags, and does label-first inference work in practice? | Semantic Tagging Adoption Report |
| 3. Extracted Representations | What metadata and relationships make a clip node useful at retrieval time? | Clip Node Information Model |
| 4. Import Flows | Which import sources create immediate graph value, and at what volume does import become noise? | Import Flow Requirements Spec |

Each deliverable maps directly to a named implementation plan. This agenda is complete when
all four deliverables are available and have been incorporated as acceptance criteria into
those plans. An optional fifth deliverable — Omnibar Knowledge Capture Surface Spec — may
emerge from cross-thread findings.

---

## References

- `implementation_strategy/viewer/2026-02-11_clipping_dom_extraction_plan.md`
- `implementation_strategy/canvas/2026-02-23_udc_semantic_tagging_plan.md`
- `implementation_strategy/canvas/2026-02-20_node_badge_and_tagging_plan.md`
- `implementation_strategy/canvas/semantic_tagging_and_knowledge_spec.md`
- `implementation_strategy/system/register/knowledge_registry_spec.md`
- `technical_architecture/GRAPHSHELL_AS_BROWSER.md §§4–5`
- `research/2026-02-18_graph_ux_research_report.md`
- `research/2026-02-28_graphshell_ux_research_agenda.md §3`
