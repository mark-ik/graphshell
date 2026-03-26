---
**Reconciliation status** (as of 2026-03-18):
See status column at the end of each Wave 1 item.
Codes: ✅ Closed | ⚠️ Partially met — needs explicit doc | 🔲 Open

---

Wave 1

G01 Graph Core Boundary. Depends: none. Done gate: one canonical doc defines graph truth vs graph presentation vs workbench/session state.
  **Status ⚠️ Effectively met, needs explicit doc.** `model/graph/mod.rs` carries a module-level docblock ("Graph data structures — boundary: direct mutation methods are `pub(crate)` and reserved for trusted writers only") plus "Trusted writers" enumeration. `TERMINOLOGY.md` and `PLANNING_REGISTER.md §1A` reinforce the boundary. No single doc names all three layers (graph truth / graph presentation / workbench-session state) in one place. *Action: add a short `design_docs/graphshell_docs/implementation_strategy/graph/graph_truth_boundary_note.md` to close the done-gate formally.*

G02 Mutation Entry Audit. Depends: G01. Done gate: every graph mutation callsite is inventoried and tagged reducer-owned, workbench-owned, or legacy violation.
  **Status 🔲 Open.** The four-phase `apply_reducer_intent_internal` dispatcher (`app/intent_phases.rs`) is in place, tagging phases 1–4 in code comments. However no inventory *document* exists enumerating mutation callsites by tag. `add_node_and_sync` is public (`pub fn`) on `GraphBrowserApp`, bypassing the intent path directly. This is the canonical open item for this gate. *Action: write mutation callsite inventory; audit `pub fn` surface on `GraphBrowserApp` for non-intent mutation entry points.*

G03 Graph Glossary Lock. Depends: G01. Done gate: Node, Edge, Relation, GraphView, Frame, Tile, and ArrangementObject each have one canonical definition.
  **Status ✅ Closed.** `design_docs/TERMINOLOGY.md` defines all seven canonical terms. `GraphViewId`, `GraphViewState`, `EdgeType` with all five relation families, `NodeKey`, and `ArrangementRelation` sub-kinds are live in code and consistent with the glossary. No deprecated aliases detected in active code.

G04 Graph Truth vs Presentation Contract. Depends: G01. Done gate: one doc lists which fields are durable graph truth and which are per-view/per-session projection only.
  **Status ⚠️ Partially met.** The distinction is enforced in code structure (graph fields in `Node`/`EdgePayload` vs `GraphViewState`, `NavigatorProjectionState` as projection-only) but no single doc makes the field-level assignment explicit. *Action: one-page doc listing durable fields vs view/session projection fields, linked from G01 note.*

G05 Durable Graph Identity Note. Depends: G01. Done gate: NodeKey today vs future durable NodeId semantics are documented with migration implications.
  **Status 🔲 Open.** `NodeKey = NodeIndex` (petgraph stable index) exists in code. `Node.id: Uuid` exists as the durable identity carrier. The gap between `NodeKey` (session-stable handle) and `Node.id` (durable UUID) is not documented anywhere with migration implications. *Action: short doc or inline note in `model/graph/mod.rs` explaining NodeKey vs Node.id semantics and the expected migration path to a durable `NodeId` type.*

G06 Intent Carrier Classification. Depends: G02. Done gate: every active GraphIntent variant is marked graph mutation, view action, runtime event, or workbench bridge.
  **Status ⚠️ Partially met.** `app/intent_phases.rs` enumerates four phases in its module docblock; `app/intents.rs` has a supporting `GraphMutation` sub-enum, a `ViewAction` sub-enum, and a `RuntimeEvent` variant. The phase-4 handler (`handle_domain_graph_intent`) is the fallthrough for graph mutations. However no exhaustive per-variant classification table exists as a doc or as code attributes. *Action: add a classification table to the intent_phases module doc, or emit it as a doc note in the strategy docs.*

G07 Legacy Mutation Diagnostics. Depends: G02, G06. Done gate: reducer/runtime emits explicit warnings when legacy non-reducer graph mutation paths are hit.
  **Status 🔲 Open.** No evidence of `warn!` or diagnostic emissions at legacy bypass callsites. `add_node_and_sync` is called from multiple non-intent paths without a warning. *Action: instrument known non-intent mutation entry points with `Warn`-severity diagnostics or `tracing::warn!`.*

G08 Reducer-Only Enforcement Plan. Depends: G02, G06. Done gate: one execution note names remaining direct mutations and the carrier each must migrate to.
  **Status 🔲 Open.** Depends on G02 mutation inventory being completed first. No such execution note exists. *Action: write enforcement note after G02 inventory is complete, naming each remaining `pub fn` graph-mutating method and its target intent carrier.*

G09 Graph Contract Test Harness Scaffold. Depends: G02. Done gate: there is a test seam for reducer-only graph mutation and impossible-state assertions.
  **Status ✅ Closed.** `shell/desktop/tests/harness.rs` provides `TestRegistry` with `GraphBrowserApp::new_for_testing()`, `add_node()` (routes through `add_node_and_sync`), and a full scenario test suite (`undo_redo.rs`, `grouping.rs`, `navigation.rs`, `graph_parity.rs`). The seam exists. Impossible-state assertion coverage is thin but the scaffold is present and used.

G10 Single-Write-Path Closure Slice. Depends: G07, G09. Done gate: at least one legacy graph mutation cluster is removed and proven to flow only through reducer carriers.
  **Status 🔲 Open, blocked on G07/G08.** The harness (G09) is in place. G07 and G08 are the blockers — without the mutation inventory and diagnostic instrumentation, we cannot prove a cluster has been fully closed. *Action: unblock by completing G02 → G07/G08 → G10 in order.*
Wave 2
11. G11 Canonical Node Shape. Depends: G03, G05. Done gate: node payload contract covers identity, address, title, mime, provenance, and content references.
12. G12 Canonical Edge Shape. Depends: G03. Done gate: edge payload contract covers endpoints, relation family, label, provenance, and visibility hooks.
13. G13 Relation Family Registry. Depends: G11, G12. Done gate: one shared relation-family vocabulary exists for navigator, arrangement, history, and copy provenance.
14. G14 Edge/Relation Inventory Collapse. Depends: G13. Done gate: duplicate or ambiguous relation types are merged or explicitly justified.
15. G15 Ontology vs Presentation Separation. Depends: G13. Done gate: each relation family is marked truth, derived projection, or presentation-only.
16. G16 Copy Provenance Contract. Depends: G13, G15. Done gate: copy edges/events, delete semantics, and visibility policy are written and testable.
17. G17 Traversal vs Semantic Edge Split. Depends: G13. Done gate: traversal/history carriers are no longer described as the same thing as durable semantic relations.
18. G18 Arrangement Relation Contract. Depends: G13, G15. Done gate: frame/tile/group membership truth is defined as graph-backed relation families.
19. G19 Node Residency Semantics. Depends: G11, G18. Done gate: cold/live/recent/exiled states are defined in graph terms, not just UI terms.
20. G20 Graph Validation Pass. Depends: G11–G19. Done gate: a validation spec rejects malformed nodes, impossible endpoints, and invalid relation payloads.

Wave 3
21. G21 GraphView First-Class Contract. Depends: G04, G18. Done gate: GraphView has explicit identity, ownership, and persistence semantics.
22. G22 GraphView State Ownership. Depends: G21. Done gate: edge policy, dismissal state, layout-affecting presentation, filters, and selection memory are assigned to GraphView.
23. G23 View-Local Dismissal Rule. Depends: G21, G22. Done gate: dismissing an edge/node in a view is formally separated from deleting graph truth.
24. G24 GraphView Copy Contract. Depends: G22, G23. Done gate: graph-view clone semantics are specified and linked to focused-view behavior.
25. G25 GraphView Routing Contract. Depends: G21. Done gate: open/restore/copy/focus routes use explicit workbench carriers rather than ad hoc helpers.
26. G26 Arrangement Object Projection in GraphView. Depends: G18, G21. Done gate: frames/graphlets as expandable graph objects are defined per view.
27. G27 GraphView Persistence Shape. Depends: G22. Done gate: persisted per-view policy and layout state has a concrete schema note.
28. G28 Mixed Selection Model. Depends: G03, G21. Done gate: nodes, edges, frames, tiles, and arrangement objects can coexist in one selected target set.
29. G29 Command Applicability Rule. Depends: G28. Done gate: commands are available only when valid for every selected object; no implicit fallback target remains.
30. G30 Selection Reveal Contract. Depends: G28, G29. Done gate: reveal-on-select behavior is specified for visible, offscreen, and hidden graph contexts.

Wave 4
31. G31 Selection Lifecycle Contract. Depends: G28, G30. Done gate: hidden/offscreen objects may retain memory but not live selection, with tests specified.
32. G32 Edge Selection Semantics. Depends: G12, G28. Done gate: edge single-click/double-click/select behavior is reducer-owned and distinct from node selection.
33. G33 Recent/Cold Catchall Semantics. Depends: G19, G31. Done gate: recent is defined as a graph-side recency category with entry/exit rules.
34. G34 Dismiss Node Lifecycle. Depends: G19, G23, G33. Done gate: dismiss-from-container vs delete-node behavior is specified with undo/history implications.
35. G35 Move/Associate/Copy Carrier Set. Depends: G16, G18, G34. Done gate: these three cross-context actions have explicit graph carriers and semantics.
36. G36 History/Traversal Family Integration. Depends: G17, G33. Done gate: history manager categories map cleanly onto graph relation families and events.
37. G37 Persistence Schema Audit. Depends: G11–G36. Done gate: graph truth, graph-view state, and session-only state are assigned to distinct persistence lanes.
38. G38 WAL Coverage Audit. Depends: G06, G37. Done gate: every durable graph mutation path is either WAL-logged or explicitly marked non-durable.
39. G39 Layout-Family Contract. Depends: G18, G21, G26. Done gate: relation families that affect layout are enumerated and linked to view/layout policy.
40. G40 Family Visibility vs Layout Tests. Depends: G39. Done gate: tests/spec clauses cover hidden relations, arrangement objects, and layout stability.

Wave 5
41. G41 Navigator Projection Mapping. Depends: G13, G21, G33. Done gate: each navigator section is mapped to graph truth, derived projection, or view-local state.
42. G42 Graph Command Targeting Audit. Depends: G29, G41. Done gate: graph commands consume the selected target set uniformly across navigator and graph surfaces.
43. G43 Invalid-State Diagnostics Pack. Depends: G20, G31, G42. Done gate: diagnostics channels exist for impossible selection, invalid relations, and misrouted graph mutations.
44. G44 Reducer/Workbench Boundary Cleanup. Depends: G25, G35, G42. Done gate: graph mutations and tile-tree mutations no longer share misleading carriers without explicit bridge notes.
45. G45 Graph Import/Export Boundary. Depends: G21, G37. Done gate: importable graph truth is separated from non-portable view/session state.
46. G46 Graph Scenario Test Matrix. Depends: G28–G45. Done gate: scenarios cover mixed selection, dismiss/delete, copy provenance, relation visibility, and graph-view copy.
47. G47 Graph Invariant Assertions in Runtime. Depends: G20, G43. Done gate: debug assertions or equivalent runtime checks exist for core graph invariants.
48. G48 Doc/Tracker Sync Pass. Depends: G01–G47. Done gate: active graph docs, planning register, and issue labels agree on current ownership and open blockers.
49. G49 Hardening Slice for Highest-Risk Graph Paths. Depends: G46, G47. Done gate: the top 3 regression-prone graph behaviors have targeted tests and diagnostics.
50. G50 Graph Milestone Closure Receipt. Depends: G01–G49. Done gate: one closure doc summarizes what is now canonical, what remains transitional, and what future lanes can safely build on.