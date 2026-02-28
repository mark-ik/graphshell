# Navigation Semantics and User Mental Models Research Agenda (2026-02-28)

**Status**: Active research agenda
**Scope**: Four research threads on Graphshell's core differentiator claim — that navigation becomes semantic structure rather than tab history. Each section states the design bet being made, what the current docs assume, the open empirical questions, what methods would answer them, and what a useful deliverable looks like.

The organizing argument: Graphshell is only more valuable than a conventional browser if users can form a working mental model of nodes, edges, and history that is at least as reliable as their mental model of browser tabs. If the model is opaque or surprising, the graph representation adds confusion on top of the features users already understand. Each of the four threads below tests a specific part of that bet.

---

## 1. Node Identity vs. Mutable URL

### The design bet

A node is not a URL. It is a persistent, stable-identity workspace unit — identified by UUID — that happens to be currently browsing a particular URL. The URL is mutable: a node can navigate to github.com, then to github.com/servo, then back, and it is the same node throughout. This is deliberately different from the conventional browser mental model, where a tab's identity is strongly tied to its current page.

This bet enables several things: edges remain valid when a node browses away from its original URL, a node can be tagged and clustered based on intent rather than content, and graph structure survives within-tab navigation. It is architecturally sound. The question is whether users can hold this model.

### What the docs assume

`GRAPHSHELL_AS_BROWSER.md` states the invariants explicitly: "URLs are mutable. Within-tile navigation changes the node's current URL. The node persists." The node lifecycle (`Active/Warm/Cold`) is defined over node identity, not URL. The tile selector row shows the current URL as a label but the tile represents the node. The traversal model (`2026-02-20_edge_traversal_model_research.md`) is built on this: edges connect nodes, not URLs; a traversal record carries a `from_url` and `to_url` snapshot because the node's URL at the time of traversal may not be the same as its current URL when the user later inspects the edge.

The docs do not examine whether users understand this, or when they are surprised by it.

### Open questions

**Q1.1 — The identity shock moment**: When does the node-identity model first produce a result that surprises a user? Candidate moments:

- User adds a node for `amazon.com/product-A`, browses to `amazon.com/product-B` within the same node to compare, then returns to the graph. The node's title and URL now reflect product B, not product A. The edges and tags still apply to the node. Does the user understand that the node "was about" product A and is now "at" product B, or do they think product A is gone?
- User opens a node for a documentation page, follows 8 links within the same node across a documentation site. Returns to graph. The node's title is now the 8th page they visited, not the one they intended as the "documentation node." Do they understand why, and can they recover the original URL?
- User opens the same URL in two different nodes (allowed by design; duplicate URLs permitted). The two nodes look identical in the graph. User tries to delete one. Which one? This is particularly disorienting when both have the same title.

**Q1.2 — The mutable URL as feature vs. bug**: Some users will want node identity to be URL-pinned — "this node is for this specific page, and if I navigate away I've left it." Others will want the fluid identity the design provides. Are there user types or task types that predict which mental model users bring? Power users doing research may prefer fluid identity; users doing reference-keeping (bookmark-like use) may prefer URL-pinned identity.

**Q1.3 — Node title as the identity anchor**: In the absence of URL stability, the node title becomes the primary human-readable identity. But titles come from `<title>` tags that change as the user browses. A node starts as "Amazon — Product A", the user browses to "Amazon — Product B", the title changes. The user's own mental label ("my comparison node for laptops") is not persisted anywhere. Does the absence of a user-editable, URL-independent node label create confusion? The UDC tagging system provides semantic tags, but not a plain-language user-set name.

**Q1.4 — Intra-node history as the recovery mechanism**: The node carries a back/forward stack (`history_entries: Vec<NodeHistoryEntry>`). If a user is confused about where a node "is," the recovery path is to open the node and press Back. This is the same as a conventional browser Back button. Does this recovery path feel intuitive, or do users not know it exists? When a node is Cold (no active viewer), the history stack is preserved but not visible. Can a Cold node's history be inspected without activating it?

**Q1.5 — The duplicate-URL confusion**: Two nodes at the same URL are allowed and meaningful (two separate contexts, two separate browsing threads, two sets of edges). But they look identical in the graph by default. The only distinguishing features are their UUID, their edges, their tags, and their position. Is this enough for users to maintain the distinction, or do they experience it as a bug ("I have two copies of the same thing")?

### Methods

**Wizard-of-Oz identity probe** (key method):

Build a prototype or use a facilitator-operated live instance. Present participants with a pre-loaded graph containing:
- One node that has browsed away from its original URL.
- Two nodes at the same URL.
- One node with a title that no longer reflects its original purpose.

Ask participants: "Which node is the one you'd use to get back to [original page]?" and "Are these two nodes the same thing or different things?" Observe whether they use edges, position, or label to distinguish.

Do not explain the node identity model before the task. The goal is to surface naive mental models, not to test whether trained users understand the docs.

**In-session verbal annotation probe**:

During a real browsing task (comparison shopping, research), ask participants to narrate what they think is happening to their nodes as they navigate. Specifically: after within-tab navigation, ask "what is this node 'about' right now?" and "is this the same node as before?" Compare the verbal description to what actually happened in the graph.

**Unmoderated survey probe**:

Show a short screen recording of within-tab navigation that causes a node title to change. Ask: "What would you call this node now? Is this the same node as before or a new one?" This can be deployed at low cost to reach more participants.

### Research deliverable

`2026-xx-xx_node_identity_mental_model_findings.md` — a taxonomy of the mental models users bring (URL-as-identity vs. context-as-identity vs. label-as-identity), the specific moments that produce identity confusion, and design recommendations. Likely recommendations include: a user-editable node name field that persists independently of the URL-derived title; stronger visual differentiation between nodes at the same URL; and a Cold-node history preview affordance. Should feed directly into the node data model design and the graph node shape rendering spec.

---

## 2. Edge Expectations: Meaning, Labels, and Direction

### The design bet

Edges accumulate automatically from navigation history and represent real traversal behavior, not just user-declared relationships. Edge weight (stroke width) reflects traversal frequency. Edge direction reflects dominant traversal direction. Users can also assert edges manually. The combination of automatic and manual edges makes the graph a true navigation map — the topology reflects how the user actually moved through their research, not just how they intended to organize it.

This bet is architecturally specified in detail (`2026-02-20_edge_traversal_model_research.md`, §3). It has not been tested against user expectations.

### What the docs assume

The traversal research doc identifies three `NavigationTrigger` values that will eventually be classifiable: `ClickedLink`, `TypedUrl`, `GraphOpen`, `HistoryBack`, `HistoryForward`, plus `Unknown` for cases where Servo doesn't expose the navigation cause. It notes that `trigger` may default to `Unknown` in many cases.

The visual treatment is specified: dominant direction (>60% threshold for arrow direction, bidirectional below that), stroke width proportional to traversal count including archived count, full traversal history available in edge inspection panel. The `UserGrouped` edges (`user_asserted = true`) carry zero traversals when they are pure assertions, and carry traversal history when the user has also navigated the asserted relationship.

The docs do not address: what users expect when they see an edge they did not deliberately create; whether users understand what edge weight means; or whether direction arrows are read correctly.

### Open questions

**Q2.1 — The unexpected edge problem**: Most edges in a real session are traversal-derived, not user-asserted. A user who navigates from page A to page B to page C will find edges A→B and B→C in their graph after the session. Many users may not realize Graphshell is creating edges automatically. When they encounter an edge they did not create, possible reactions:
- Delight: "Oh, it remembered that I went from here to there."
- Confusion: "Why are these two nodes connected? I didn't link them."
- Distrust: "I can't tell which edges I made vs. which the system made."

Which reaction dominates, and does it depend on the task type? In a research task, automatically captured traversal paths may read as useful provenance. In a cleanup or curation task, auto-generated edges may feel like clutter the user did not authorize.

**Q2.2 — Edge weight as a readable signal**: The traversal research doc posits that stroke width proportional to traversal count is a "free feature" — visually meaningful at a glance. But is it? Graph visualization research (Tufte, Bertin, and the egui graph UX research doc's literature survey) suggests that quantitative channels like line width are less perceptually salient than categorical channels like color or position. Do users actually read edge weight as "I've traveled this path frequently" or do they ignore it as visual variation?

**Q2.3 — Direction arrows and reading habits**: The dominant-direction arrow (pointing A→B when >60% of traversals go A→B) is a subtle signal. In a dense graph where most edges are bidirectional (user went both ways roughly equally), most edges will show bidirectional arrows or no arrows. Does the user read a bidirectional arrow as "I went back and forth" or as "these two nodes are related but I don't know the direction"? These are semantically different, but the visual representation is the same.

**Q2.4 — User-asserted vs. traversal-derived distinguishability**: The model distinguishes `user_asserted = true` (explicitly created) from traversal-derived edges (created by navigation). Does the visual presentation make this distinction clear? An asserted edge with no traversals means "I declared this relationship but never navigated it." A traversal-derived edge with 20 traversals means "I went here constantly but never explicitly linked it." Users who curate their graph may need to know which is which to make sense of their topology.

**Q2.5 — Edge label expectations**: The docs describe edges by type (traversal-derived, user-asserted) and by trigger (ClickedLink, TypedUrl, etc.), but not by semantic label from the user's perspective. Users of tools like Roam Research, Obsidian, or knowledge graph tools have been trained to think of edges as typed relationships ("supports," "contradicts," "is an example of"). Does Graphshell's unlabeled edge model frustrate users who expect to label relationships? Or do users adapt to treating edge presence + direction as sufficient?

**Q2.6 — Edge inspection as a discovered affordance**: The edge traversal model enables edge inspection (click an edge → see its traversal history, timestamps, trigger breakdown). Is this an affordance users will discover and use, or does it feel too granular for most workflows? The hypothesis is that it becomes valuable in retrospect — "when did I last go from this page to that one?" — but this needs testing.

### Methods

**Expectation elicitation** (prior to any session):

Before showing participants Graphshell at all, ask: "In a graph browser where your navigation history creates connections between pages — what would you expect those connections to mean? What would you want to be able to do with them?" This surfaces the mental model users bring before they are shaped by the interface.

**Edge recognition task**:

After a 15-minute browsing session, show participants the resulting graph. For each visible edge, ask: "Do you remember creating this connection, or did it appear automatically?" and "What does the thickness of this line mean to you?" This directly measures edge surprise rate and weight readability.

**Edge curation observation**:

Ask participants to "clean up" a pre-populated graph (one built from a real research session, not theirs) to make it "meaningful." Observe whether they remove auto-generated edges they don't understand, whether they add labels to existing edges, and whether they seem confused by edge direction.

**Comparative study**:

Compare to Obsidian (labeled user-created links only) and a conventional browser history view (flat list). On the same research task, which representation do participants use to reconstruct what they were doing? Does the graph edge model outperform the flat list for the "what was I doing an hour ago" query?

### Research deliverable

`2026-xx-xx_edge_mental_model_findings.md` — the surprise rate for auto-generated edges (what fraction of edges do users not recognize as their own?), whether edge weight is a readable signal or ignored, whether direction arrows are interpreted correctly, and whether the user-asserted/traversal-derived distinction needs a more visible visual treatment. Should feed into the edge visual design spec and the edge traversal implementation plan's rendering section.

---

## 3. Traversal History and Timeline Views: Help or Confusion

### The design bet

A browsing session has a temporal dimension that linear history lists fail to represent. The graph's traversal records and the History Manager timeline together offer something better: a way to scrub through past sessions, see what was active when, preview the graph at a prior state, and restore the working context from any point in time. This transforms history from a flat back-button stack into a navigable temporal record.

This bet is architecturally well-specified. The traversal research doc specifies the tiered storage model (hot/cold), the History Manager's timeline view, dissolved record handling, and preview/replay isolation. The subsystem history doc (`SUBSYSTEM_HISTORY.md`) formalizes Stage F (temporal replay/preview) as a planned capability with strict isolation invariants. Stage D (basic History Panel showing last 50 traversals) is already implemented.

The question is whether users find this temporal view useful, confusing, or redundant with browser history.

### What the docs assume

The History Manager as described provides: a timeline sorted by timestamp descending, filterable by URL, domain, date range, dissolution status; row-click focusing on the source node; traversal trigger indicators; export to JSON/CSV. Stage F adds preview mode (non-destructive time travel that does not mutate live state) and replay. The traversal research doc acknowledges that the `NavigationTrigger` field may default to `Unknown` for many events because Servo does not always expose navigation cause.

The docs do not examine whether users actually have the "I want to go back to what I was doing at 2pm" query, how frequently they have it, or whether the graph traversal timeline is the right surface for answering it (vs. a flat history list, which users already know how to use).

### Open questions

**Q3.1 — The baseline frequency of temporal re-navigation**: How often do users actually need to return to a prior browsing state? The conventional browser Back button answers the local version of this (return to the previous page in this tab), but the History Manager is designed for the global version (return to what I was working on an hour ago, in a different context). Is this a pain point users experience regularly, or is it rare enough that the History Manager is solving an infrequent problem?

**Q3.2 — The "where was I" query shape**: When users want to return to prior work, what is the query they form? Candidate query shapes:
- Temporal: "I was working on this yesterday afternoon."
- Domain: "I was on some GitHub page — not sure which one."
- Topical: "I had some nodes about comparison shopping — which were they?"
- Structural: "I had a cluster of related nodes — what was in it?"

These correspond to different retrieval affordances. A timeline sorted by timestamp answers the temporal query. A domain filter answers the domain query. Graph structure answers the topical and structural queries. Does any single surface answer all four, or does each require a different entry point?

**Q3.3 — Preview mode orientation**: Stage F introduces a mode where the user can scrub to a prior graph state and preview it without mutating live state. This is a powerful capability that has no direct equivalent in conventional browsers. But it requires users to understand a key concept: "what you are seeing is not current." The spec requires explicit labeling ("Return to Present" exit control, labeled preview state). Is this enough to prevent users from treating preview state as live state and performing actions they expect to take effect?

**Q3.4 — Dissolution and "orphaned history"**: When a node is deleted, its traversal records move to the dissolved archive in the History Manager. The user can then see traversals to/from a node that no longer exists, with a tombstone placeholder in the timeline. The key question is whether users understand this distinction — "this traversal happened to a node that has since been deleted" — or whether it reads as broken history ("it's showing me a page I can't get to anymore").

**Q3.5 — Session reconstruction accuracy**: The practical use of the History Manager is reconstructing a prior working context. After a realistic 30-minute research session, a user returns the next day. Can they use the traversal timeline to reconstruct: (a) what topics they were researching, (b) which specific pages they read, (c) which connections they were tracing? Or does the traversal record contain too much noise (intra-node navigation, failed loads, exploratory branches they abandoned) to be useful for reconstruction?

**Q3.6 — Trigger-unknown entries**: Many traversal records will have `trigger: Unknown` because Servo does not expose the navigation cause. These entries appear in the History Manager without the "Back," "Forward," or "Link" labels that would make them interpretable. Does a timeline full of "Unknown" trigger entries undermine user confidence in the history view's reliability?

### Methods

**Diary study — temporal re-navigation frequency**:

Ask 6–8 participants to keep a 5-day diary logging: (a) every time they wanted to return to something they were browsing earlier in the same session, (b) every time they wanted to return to something from a previous session, and (c) how they accomplished it (Back button, browser history, bookmarks, search, gave up). This directly measures the baseline frequency of the problem the History Manager is designed to solve, without Graphshell involved.

**Task reconstruction study** (with Graphshell):

After a 20-minute research session in Graphshell, the participant closes the app. The next day they return to a cleared graph state and are shown only the History Manager timeline. Ask them: "Using only what you can see here, reconstruct what you were researching yesterday." Score: accuracy (did they identify the topics?), completeness (how many nodes could they identify?), and confidence ("how sure are you this is right?").

**Preview mode orientation test**:

Present a preview-mode prototype. Do not explain that they are in preview. Ask participants to "continue their research from this point." Measure: how many attempts a preview mode to take a "live" action (create a node, open a URL in a new tile, delete something) before discovering they are in a read-only state? What is their reaction when they discover it?

**Trigger-label degradation study**:

Show two versions of the History Manager timeline: one where all entries have correct trigger labels (ClickedLink, TypedUrl, Back, Forward), and one where 80% are "Unknown." Ask participants to rate usability and confidence. Measures whether Unknown-trigger entries meaningfully degrade the utility of the history view.

### Research deliverable

`2026-xx-xx_history_timeline_usability_findings.md` — baseline frequency data for the temporal re-navigation problem, task reconstruction accuracy scores, the orientation cost of preview mode, and the degradation impact of Unknown-trigger entries. Should directly inform the Stage F preview mode design (labeling, affordances for "you are in preview") and the History Manager's filter/search design priority. Should also answer whether the History Manager needs its own entry point in the main navigation, or whether it is a power-user feature that can live in a secondary panel.

---

## 4. Deletion, Ghost/Tombstone Behavior, and Restoration Feel

### The design bet

Deletion in Graphshell is not the same as deletion in a conventional browser (where closing a tab loses it immediately). A deleted node can become a tombstone: a structural placeholder that preserves edges, position, and label while dropping active viewer state. The user can show tombstones, inspect them, restore them to Active, or permanently delete them. The traversal archive preserves records for dissolved edges even after the node is gone. This gives the graph durability and recoverability that conventional browsers do not have.

The tombstone plan (`2026-02-26_visual_tombstones_plan.md`) specifies the implementation in three phases. Phase 1 (toggle + ghost rendering) adds `NodeState::Tombstone`, a "Show Deleted" toggle, and faint dashed-outline rendering. Phase 2 adds right-click restore/permanent-delete. Phase 3 adds GC policy. The plan defaults tombstones to hidden (toggle off), reasoning that they add visual noise if not handled carefully.

The question is whether these choices — hide by default, dashed outline, right-click restoration, GC — match what users expect and need.

### What the docs assume

The tombstone research doc describes three use cases: refactoring (deleted a hub node but want to remember its connections), history ("I know I had a link here yesterday"), and pruning (cleaning up without losing topology). The implementation plan translates these directly into the Phase 1 design.

Key design decisions embedded in the current plan that lack user validation:
- **Hidden by default.** Tombstones are invisible unless the user enables "Show Deleted." This respects the principle that they add visual noise, but it means users who would benefit from seeing them don't know they're there.
- **Dashed outline + × marker.** The visual treatment is distinct from active nodes but not described or tested further.
- **Context menu restoration.** Right-clicking a tombstone is the only gesture to restore it. This is invisible until you find the tombstone, which requires enabling the toggle first.
- **30-day GC by default.** Tombstones older than 30 days are silently garbage collected unless the user sets `max_age_days = ∞`.

None of these decisions have user input behind them.

### Open questions

**Q4.1 — The deletion expectation: permanent or soft?** When users press Delete in Graphshell, what do they expect to happen? Three possible expectations:
- **Permanent deletion**: the node disappears, nothing remains.
- **Reversible deletion (like a trash can)**: the node moves to a recoverable state, available for a limited time.
- **Archive/hide**: the node is hidden but not gone, like Gmail's Archive.

The tombstone model most closely resembles the "trash can" or "archive" pattern. But the default-hidden behavior means users who expect reversible deletion will not discover the restoration path until they explicitly look for it. Does the interaction feel like a trash can (intuitive recovery) or like silent permanent deletion (because nothing visible changes in the graph after delete)?

**Q4.2 — The visual noise calibration**: The tombstone plan acknowledges that "they add visual noise if not handled carefully" and defaults to hidden. But the use cases (refactoring, history, pruning) all involve users who want to see where structure used to be. Is hidden-by-default the right tradeoff, or should tombstones be visible by default in certain contexts — for example, visible for 24 hours after deletion, then hidden, to give users a chance to notice and restore?

**Q4.3 — Tombstone legibility**: The proposed visual treatment is a faint dashed-outline square with a × center mark. Does this read as "deleted node" or as "error state"? The × mark in particular has ambiguous meaning in UI conventions — it can mean "close," "error," or "deleted." Does the visual treatment need a more explicit label (e.g., a small "DELETED [date]" annotation on hover)?

**Q4.4 — Restoration workflow completeness**: The restoration path is: enable "Show Deleted" toggle → see tombstones → right-click a tombstone → click "Restore." This is four steps, and step one is a settings-panel toggle. For a user who deleted a node by accident and wants to undo immediately, this path is long. The tombstone plan notes that `U` (keyboard shortcut) could be added in Phase 2+, but it is not in Phase 1. Is the four-step path acceptable for the "accidental deletion" recovery scenario, or does it need a faster path?

**Q4.5 — Traversal history through tombstones**: The edge traversal model preserves traversal records for dissolved edges. The tombstone plan notes that "traversals past a tombstone show 'Node deleted on X; was titled Y.'" This means a user scrolling through the History Manager timeline will encounter entries pointing to deleted nodes. Does this feel like useful provenance ("I can see I was at this page even though I deleted the node") or like broken history ("it's showing me something I can't get to")?

**Q4.6 — The "restore target node still exists?" problem**: The tombstone plan (Phase 2, edge restoration) notes: "If target node is still tombstoned, show a warning: 'Linking to deleted node X; restore it first?'" This adds a dependency: to restore a node, the user may first need to restore the nodes it was connected to. Does this feel like a logical constraint or a confusing error? Users accustomed to flat bookmark restoration have no equivalent concept.

**Q4.7 — GC as a trust issue**: The 30-day GC default silently removes tombstones older than 30 days. The plan provides a notification ("Cleaned up X expired ghost nodes") but makes the GC opt-out rather than opt-in. Does silent GC erode user trust in the graph's durability? A user who relies on tombstones as long-term structural memory (use case 3: pruning) would be surprised to find them gone after 30 days without having explicitly chosen this.

### Methods

**Deletion expectation elicitation**:

Before any Graphshell exposure, ask participants: "In a graph browser, if you delete a node — what would you expect to happen? Would you expect to be able to get it back? For how long?" This establishes the baseline expectation before the design is encountered. Expected distribution: some users expect permanent deletion (no recovery), some expect a trash can (recoverable), some expect undo (short-window reversal). The tombstone design serves the trash-can expectation well; it may fail for the undo expectation.

**Accidental deletion recovery timing**:

Have participants complete a task in which they accidentally delete a node (design the task so accidental deletion is likely, e.g., Delete key near a selected node). Do not tell them the node is recoverable. Measure: time to discover the tombstone restore path (toggle → see → right-click → restore). At what time does the user give up and re-create the node from scratch instead?

**Tombstone visual recognition test**:

Show participants a screenshot of a graph with several active nodes and several tombstone nodes (dashed outline, × mark). Ask: "What are the dashed outlines?" and "What does the × mark mean?" Score the recognition rate without any prior explanation. If fewer than 70% of participants correctly identify the tombstone visual treatment as "deleted but recoverable," the visual treatment needs revision.

**GC awareness probe**:

After 15 minutes of using Graphshell with tombstones visible, tell participants: "In 30 days, some of the ghost nodes you've created will be automatically removed." Ask: "How do you feel about that? Does this change how you'd use ghost nodes?" This directly surfaces the trust implications of default GC.

**Orphaned history reaction**:

Show participants a History Manager timeline that includes traversal entries pointing to deleted (tombstoned) nodes. Ask: "What do you make of these entries? Can you tell me what happened here?" Measure whether users correctly interpret "this was a node that I deleted" vs. treating it as an error.

### Research deliverable

`2026-xx-xx_deletion_tombstone_ux_findings.md` — baseline deletion expectations (permanent vs. recoverable vs. undo), accidental deletion recovery timing for the current four-step path, tombstone visual recognition rate for the current dashed-outline design, GC trust implications, and orphaned history entry interpretability. Should directly update the tombstone implementation plan with: a faster recovery path for accidental deletion, revised GC defaults (opt-in vs. opt-out), any changes to the tombstone visual treatment, and the appropriate default state (hidden vs. visible after recent deletion).

---

## Cross-Cutting Notes

### Priority ordering

These four threads are interdependent in a specific way:

**Q1 (node identity)** is the most foundational. If users cannot hold the node-identity model, the rest of the design is built on a confused foundation. It should be studied first, with the Wizard-of-Oz identity probe deployable with any working prototype.

**Q2 (edge expectations)** is second. It depends on users having a stable node identity model — edge meaning is defined in terms of node relationships. The edge recognition task can be run in the same session as node identity probing.

**Q3 (history/timeline)** is third. It requires a working History Manager (Stage D is done; the timeline and basic filtering are available). The diary study component can run in parallel with Q1/Q2.

**Q4 (deletion/tombstones)** is fourth. It requires Phase 1 tombstone implementation before the visual recognition and recovery timing tests. The deletion expectation elicitation can run before implementation.

### Relationship to existing design decisions

Every design decision in the tombstone plan, the traversal research doc, and the history timeline spec was made without user input — they are architecturally principled but empirically grounded only in the general UX literature. The specific decisions most at risk:

- **Node identity model**: The design assumes users can form the UUID-identity mental model. Q1 directly tests this.
- **EdgePayload traversal frequency as weight**: The design assumes this is perceptually meaningful. Q2.2 directly tests this.
- **Tombstone hidden by default**: The design assumes visibility adds noise. Q4.2 tests whether this is the right default.
- **30-day GC opt-out**: The design assumes this is a safe default. Q4.7 tests whether it erodes trust.
- **Stage F preview mode**: The design assumes users can maintain the preview/live distinction. Q3.3 directly tests this.

### Methods that can be done before implementation is complete

These can run now with existing or near-complete implementation:
- Deletion expectation elicitation (Q4) — no Graphshell needed
- Traversal trigger expectation probe (Q2.1) — needs Stage D History Panel only
- Diary study for temporal re-navigation frequency (Q3.1) — no Graphshell needed
- Edge expectation elicitation (Q2.5) — verbal probe, no implementation needed

These require implementation to be further along:
- Tombstone visual recognition test (Q4.3) — needs Phase 1 tombstone render
- Preview mode orientation test (Q3.3) — needs Stage F
- Node identity probe (Q1.1) — needs a working prototype with within-tab navigation

### What this agenda does not cover

- Accessibility implications of the traversal model (tabbing through nodes when edges are auto-generated; screen reader linearization of edge history — covered by SUBSYSTEM_ACCESSIBILITY).
- P2P sync and conflict resolution for traversal records (Verse Tier 1 concern; out of scope until bilateral sync ships).
- Performance characteristics of the History Manager at large traversal counts (engineering concern; addressed by the traversal research doc's tiered storage model and the performance tuning plan).
