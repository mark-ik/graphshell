# Edge Operability Matrix

**Date**: 2026-03-14
**Status**: Design — Gap Analysis
**Purpose**: For each relation family, answer the six user-operability questions
and identify exactly what is specified, what is partially specified, and what is
missing. This is a gap analysis, not a design spec — each gap entry is a work
item that needs its own contract before implementation.

**Related**:

- `2026-03-14_graph_relation_families.md` — family vocabulary and persistence tiers
- `2026-03-14_edge_visual_encoding_spec.md` — visual encoding (perceivability)
- `2026-03-14_canvas_behavior_contract.md` — physics scenarios (behavioral consequences)
- `graph_node_edge_interaction_spec.md` — interaction model authority
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — navigator projection

---

## The Six Questions

For any edge visible on screen, the user must be able to answer:

1. **What is this relation?** (Perceivability)
2. **Why does it exist?** (Inspectability — provenance)
3. **How was it created?** (Creation — intentionality)
4. **Can I change its kind or durability?** (Editability — promotion)
5. **What does it currently affect?** (Behavioral consequences)
6. **How do I hide or emphasize it?** (Projection control)

A family is **user-operable** when all six questions have clear, consistent answers
available to the user without guessing.

---

## Per-Family Contract Template

Each family section below uses this structure:

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Stroke style, color, opacity | Specified / Partial / Missing |
| Inspector surface | What the inspect popover shows | Specified / Partial / Missing |
| Creation gesture | How the user makes one | Specified / Partial / Missing |
| Editable fields | What the user can change after creation | Specified / Partial / Missing |
| Promotion path | How to change durability or kind | Specified / Partial / Missing |
| Deletion / removal | How to remove safely | Specified / Partial / Missing |
| Layout influence | What physics does with it | Specified / Partial / Missing |
| Navigator influence | How it affects sidebar projection | Specified / Partial / Missing |
| Lens / filter | How to show, hide, or isolate it | Specified / Partial / Missing |
| Persistence tier | Session / durable / derived / rolling | Specified / Partial / Missing |

**Status key:**
- **Specified** — a canonical doc covers this; implementation can proceed
- **Partial** — mentioned but not fully contracted; needs a follow-on spec
- **Missing** — no design exists; a gap that must be filled before implementation

---

## 1. Semantic / Hyperlink

*`EdgeKind::Hyperlink` — declared by document author on link-follow navigation.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Gray solid thin (1.4px); arrowhead on hover | Specified — `edge_visual_encoding_spec.md §3.2` |
| Inspector surface | Family: Semantic · Hyperlink; participants; navigation count | Partial — navigation count sourced from `TraversalDerived` co-edge; inspect popover defined but not fully specified for hyperlink-only edges |
| Creation gesture | Automatic on link-follow; no direct user gesture | Specified — `layout_behaviors_and_physics_spec.md §2.2` (semantic parent placement) |
| Editable fields | None — Hyperlink is derived from document structure, not user-authored | **Missing** — no explicit "read-only" affordance or explanation in the inspector |
| Promotion path | N/A — cannot be promoted to a different kind; it is what it is | **Missing** — no explicit statement that Hyperlinks are immutable; user may expect to be able to remove them |
| Deletion / removal | No removal — edge evicts only when both nodes are removed | **Missing** — user has no way to suppress a Hyperlink edge they don't want to see; should filter via lens, not deletion |
| Layout influence | Semantic weight (elastic association); `semantic_weight = 1.0` default | Specified — `graph_relation_families.md §6.1` |
| Navigator influence | Primary section "Workbench" when nodes are arranged; otherwise contributes to adjacency | Partial — adjacency-candidate derivation not fully specified |
| Lens / filter | Hidden: No — always visible by default; filterable via lens family toggle | Partial — lens chip family toggle specified in `edge_visual_encoding_spec.md §6` but per-sub-kind toggle not specified for Hyperlink vs UserGrouped |
| Persistence tier | Durable | Specified — `graph_relation_families.md §4` |

**Key gap:** The user has no way to understand that a Hyperlink edge cannot be
removed — or to suppress one they dislike. The correct answer is lens filtering
(hide Hyperlink family in this view), not deletion. This needs to be surfaced in
the inspector with a clear label: *"Derived from page link — use lens filter to
hide."*

---

## 2. Semantic / UserGrouped

*`EdgeKind::UserGrouped` — explicit user association.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Amber bold solid (3.0px); undirected | Specified — `edge_visual_encoding_spec.md §3.2` |
| Inspector surface | Family: Semantic · User grouped; label (if any); participants; creation date | Partial — label and date not yet in `EdgePayload`; creation date not stored |
| Creation gesture | Shift+drag from node to node; `G` keyboard shortcut | Specified — existing implementation |
| Editable fields | Label (rename the grouping) | **Missing** — `UserGroupedData.label` exists in model but no UI for editing it after creation |
| Promotion path | N/A — already durable; no further promotion | Specified — durable from creation |
| Deletion / removal | "Remove grouping" action in inspector; `Alt+G` keyboard shortcut | Specified — existing implementation |
| Layout influence | Semantic weight; elastic association; same as Hyperlink by default | Specified |
| Navigator influence | Connected nodes appear as adjacent candidates for "route to adjacent" | Partial — adjacency weighting between UserGrouped and Hyperlink not specified |
| Lens / filter | Toggle in lens chip; isolated in "Semantic" filter | Partial — per-sub-kind toggle not specified |
| Persistence tier | Durable | Specified |

**Key gap:** The label is stored but not editable post-creation. A user who
groups nodes and wants to name the relationship ("research cluster", "competitor
analysis") has no UI path to do so after the initial creation gesture. This
needs an inline label field in the inspector popover.

---

## 3. Semantic / AgentDerived

*`EdgeKind::AgentDerived` — inferred by an agent; provisional.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Muted violet thin; opacity decays with age | Specified — `edge_visual_encoding_spec.md §3.2, §3.3` |
| Inspector surface | Family: Semantic · Agent suggestion; agent name; confidence; decay progress; reasoning label | **Missing** — inspector content for AgentDerived not specified; agent identity and reasoning not yet in `EdgePayload` |
| Creation gesture | Agent-initiated; no user gesture | Specified — `agent_derived_edges_spec.md` |
| Editable fields | None while provisional; "Accept" promotes to durable UserGrouped | **Missing** — Accept/Dismiss flow only sketched in `agent_derived_edges_spec.md`; not fully specified as a UI contract |
| Promotion path | Accept → promotes to `UserGrouped` (durable); Dismiss → suppresses re-assertion | **Missing** — promotion intent (`AcceptAgentEdge`?) not yet defined |
| Deletion / removal | Dismiss in inspector; or auto-eviction at decay window | Partial — decay rule specified in `edge_traversal_spec.md §2.5`; dismiss UI not specified |
| Layout influence | Semantic weight but weaker by default (agent suggestions are provisional) | **Missing** — no per-kind weight differentiation within the Semantic family; `FamilyPhysicsPolicy` treats all Semantic equally |
| Navigator influence | Does not contribute to navigator tree structure | Partial — implied but not explicit |
| Lens / filter | Show/hide agent suggestions independently | **Missing** — no lens sub-filter for AgentDerived vs UserGrouped vs Hyperlink |
| Persistence tier | Rolling-window (72h decay) | Specified — `edge_traversal_spec.md §2.5` |

**Key gap:** `AgentDerived` is the least operably specified family. The inspector
needs agent identity, confidence, reasoning, and decay progress. The
Accept/Dismiss flow needs a defined intent (`AcceptAgentEdge`,
`DismissAgentEdge`). The physics weight needs to be weaker than UserGrouped
within the Semantic family — which currently `FamilyPhysicsPolicy` cannot
express (it weights the whole Semantic family uniformly). This may require a
per-kind weight refinement inside the Semantic family.

---

## 4. Traversal / TraversalDerived

*`EdgeKind::TraversalDerived` — navigation history trace.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Steel blue dashed; width scales with traversal count; directional arrow on dominant direction | Specified — `edge_visual_encoding_spec.md §3.2`; dominant direction cue already implemented |
| Inspector surface | Family: Traversal; total navigations; forward/backward count; last traversal date | Partial — `EdgeMetrics` has `total_navigations`, `forward_navigations`; last-date not stored |
| Creation gesture | Automatic on navigation; no user gesture | Specified |
| Editable fields | None — traversal is recorded, not authored | Specified (read-only) |
| Promotion path | Navigation promotes to Semantic if threshold crossed | Partial — promotion rule referenced in `edge_traversal_spec.md §2.5` but threshold and resulting kind not fully specified |
| Deletion / removal | "Clear traversal history for this pair" in inspector; bulk clear via History Manager | Partial — bulk clear specified; per-pair clear not specified as an intent |
| Layout influence | Traversal weight = 0.0 by default; active only when traversal lens enabled | Specified — `graph_relation_families.md §6.1` |
| Navigator influence | Contributes to "Recent" section sorted by last traversal | Specified — `graph_relation_families.md §5.1` |
| Lens / filter | Hidden by default; traversal-overlay lens reveals | Specified — `edge_visual_encoding_spec.md §3.1` |
| Persistence tier | Rolling-window; aggregate metrics durable | Specified — `edge_traversal_spec.md` |

**Key gap:** Last-traversal date is not stored in `EdgeMetrics` — only counts.
The inspector cannot answer "when did you last visit this?" without it.
Per-pair history clear needs a defined intent (`ClearPairTraversalHistory`).

---

## 5. Containment / ContainmentRelation

*`EdgeKind::ContainmentRelation` — hierarchical membership.*
*Sub-kinds: `url-path`, `domain`, `filesystem`, `user-folder`, `clip-source`.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Dotted/teal (derived), solid/teal (user-folder); arrowhead toward parent | Specified — `edge_visual_encoding_spec.md §3.2` |
| Inspector surface | Family: Containment · [sub-kind]; direction: A is contained in B; derived or user-authored label | **Missing** — inspector content not specified; distinction between derived (read-only) and user-folder (editable) not surfaced |
| Creation gesture | Derived: automatic from URL; user-folder: "Add to folder" command or drag-to-navigator | Partial — gesture named in `edge_visual_encoding_spec.md §5.4` but not fully specified; drag-to-navigator interaction not designed |
| Editable fields | user-folder: rename folder, move to different folder; derived: none | **Missing** — folder rename and re-nesting are common operations with no specified intent or gesture |
| Promotion path | Derived containment cannot be promoted; user-folder is already durable | **Missing** — no explicit statement in inspector that derived containment is immutable |
| Deletion / removal | user-folder: "Remove from folder" / "Unnest" in inspector; derived: no deletion (recomputed); clip-source: "Detach from source" | **Missing** — `Unnest` intent not defined; "Detach from source" not defined |
| Layout influence | Containment weight = 0.0 default; strong rigid containment when lens active | Specified — `graph_relation_families.md §6.1` |
| Navigator influence | Owns tree structure in containment-projection mode; Folders and Domain sections | Specified — `graph_relation_families.md §5.1` |
| Lens / filter | Hidden by default; containment lens reveals; per-sub-kind toggles in lens chip | Partial — sub-kind toggles defined in `edge_visual_encoding_spec.md §6` but toggle state persistence not specified |
| Persistence tier | Derived: derived-readonly; user-folder: durable; clip-source: derived-readonly | Specified — `graph_relation_families.md §4` |

**Key gap:** The largest gap in the Containment family is the **folder
management interaction model**. Creating, renaming, and re-nesting user-folder
containment relations needs a fully specified gesture and intent vocabulary.
"Add to folder" via the navigator is obvious; drag-to-nest on the canvas is
less so. The user also has no way to tell from the inspector whether a
containment edge is derived-readonly or user-authored — this must be surfaced
explicitly.

---

## 6. Arrangement / ArrangementRelation

*`EdgeKind::ArrangementRelation` — frame and tile group membership.*
*Sub-kinds: `frame-member`, `tile-group`, `split-pair`.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Double stroke indigo (frame-member); dotted indigo (tile-group); hidden by default | Specified — `edge_visual_encoding_spec.md §3.2` |
| Inspector surface | Family: Arrangement · [sub-kind]; frame name; session-only or durable; participants | **Missing** — inspector content not specified; frame name and durability label needed |
| Creation gesture | Automatic from tile tree (tile-group, split-pair); saving a frame persists frame-member edges | Partial — automatic creation implied by `graph_relation_families.md §2.4`; "save frame" gesture exists but the edge creation it implies is not specified as an intent |
| Editable fields | frame-member: rename frame; change frame membership; session-only tile-group: promote to frame | **Missing** — frame rename intent not defined; "Promote session group to named frame" flow not specified |
| Promotion path | Session-only tile-group/split-pair → promote to durable frame-member via "Save as Frame" | **Missing** — this is the most important promotion in the system and has no specified intent or UI path |
| Deletion / removal | frame-member: remove from frame; delete frame; session-only: evaporates on close | Partial — delete frame referenced in `layout_behaviors_and_physics_spec.md §4.5`; remove-from-frame intent exists; but session-only evaporation is not surfaced to the user |
| Layout influence | Arrangement weight = 0.5; local-arrangement soft bias; frame-affinity force | Specified — `graph_relation_families.md §6.1`, `layout_behaviors_and_physics_spec.md §4.3` |
| Navigator influence | Owns top-level tree structure in workbench mode; Workbench section | Specified — `graph_relation_families.md §5.1` |
| Lens / filter | Hidden from canvas by default; arrangement overlay lens reveals | Specified — `graph_relation_families.md §2.4` |
| Persistence tier | frame-member: durable (when saved); tile-group/split-pair: session-only | Specified |

**Key gap:** The session → durable promotion path for arrangement edges is the
most operably critical gap in the entire system. Right now a user can open nodes
in a tile group, have a useful workspace, and then lose it on close — with no
clear affordance for "save this arrangement as a named frame." This is the
"Pin workspace" button (W+) in the current toolbar, but it's not surfaced as
an arrangement-edge promotion and the user gets no confirmation of what was
saved. Needs a clearly specified intent (`SaveArrangementAsFrame { name }`),
a save flow, and a durability indicator visible in the sidebar row.

---

## 7. Imported / ImportedRelation

*`EdgeKind::ImportedRelation` — external system provenance.*

| Dimension | Contract | Status |
| --- | --- | --- |
| Visual encoding | Long-gap dashed warm gray; low opacity (0.35) | Specified — `edge_visual_encoding_spec.md §3.2` |
| Inspector surface | Family: Imported; import source name and date; import record ID | **Missing** — inspector content not specified; import record structure not defined |
| Creation gesture | Automatic from import record; no user gesture | Specified (implied) |
| Editable fields | None — imported relations are read-only; promote to UserGrouped to make editable | **Missing** — promotion path not specified; user has no way to "own" an imported relation |
| Promotion path | Import → accept/promote to UserGrouped (durable, user-authored) | **Missing** — `AcceptImportedRelation` intent not defined |
| Deletion / removal | Delete import record removes all its edges; individual edge suppression archives from re-import | **Missing** — suppression archive not specified; no intent defined |
| Layout influence | None; `imported_weight = 0.0` always | Specified — `graph_relation_families.md §6.1` |
| Navigator influence | Supplementary "Imported" section; collapsed by default | Specified — `graph_relation_families.md §5.1` |
| Lens / filter | Hidden by default; import review mode reveals | Partial — "import review mode" named but not specified |
| Persistence tier | Derived-readonly at import time; durable only if promoted | Specified |

**Key gap:** The entire "import review mode" is unspecified. What triggers it?
What does the UI look like? How does the user bulk-accept or bulk-reject imported
relations? This is the entry point for bookmarks import and filesystem ingest
integration, so it needs a full flow design before implementation.

---

## 8. Cross-Family Gaps

Gaps that apply across all families:

### 8.1 Per-Kind Weight Within Semantic Family

`FamilyPhysicsPolicy` treats all Semantic kinds (`Hyperlink`, `UserGrouped`,
`AgentDerived`) with the same weight. But operably, these should differ:
- `UserGrouped` should attract more strongly than `Hyperlink` (user explicitly
  asserted the relation)
- `AgentDerived` should attract less strongly than both (provisional)

**Gap**: `FamilyPhysicsPolicy` needs sub-weights within the Semantic family, or
the physics engine needs per-kind weight overrides for the Semantic group.

### 8.2 Edge Direction Convention

Directionality is inconsistently defined across families:
- Hyperlink: source → destination (arrowhead on hover)
- TraversalDerived: dominant direction (arrow on high-ratio direction)
- ContainmentRelation: child → parent (arrowhead toward parent)
- UserGrouped: undirected
- AgentDerived: undirected

**Gap**: The direction convention needs to be stated explicitly per family in
the inspector, not just rendered as an arrow. "A links to B" vs "A is contained
in B" vs "A and B are grouped" are very different statements.

### 8.3 Undo/Redo Scope for Edge Operations

User-authored edge operations (create UserGrouped, remove UserGrouped, accept
AgentDerived, promote session arrangement, add to folder) must be undoable.
Derived edges (Hyperlink, TraversalDerived, ContainmentRelation/derived) should
not be undoable because they are not user actions.

**Gap**: No explicit undo contract per edge family. The undo spec covers graph
mutations generally but does not distinguish which edge operations are
undoable.

### 8.4 "Make Another Like This" Affordance

The edge inspect popover should offer a "Create similar relation" action that
pre-fills the creation gesture for that family. Without this, the user cannot
easily create more relations of the same kind once they discover one.

**Gap**: Not specified anywhere; needs an intent and a UI entry point.

---

## 9. Operability Status Summary

| Family | Perceivable | Inspectable | Creatable | Editable | Promotable | Deletable | Layout | Navigator | Filterable | Persistent |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Hyperlink | ✅ | ⚠️ | ✅ | ✅ (read-only) | N/A | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ |
| UserGrouped | ✅ | ⚠️ | ✅ | ❌ (label) | N/A | ✅ | ✅ | ⚠️ | ⚠️ | ✅ |
| AgentDerived | ✅ | ❌ | ✅ | ❌ | ❌ | ⚠️ | ⚠️ | ⚠️ | ❌ | ✅ |
| TraversalDerived | ✅ | ⚠️ | ✅ | ✅ (read-only) | ⚠️ | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Containment (derived) | ✅ | ❌ | ✅ | ✅ (read-only) | N/A | ❌ | ✅ | ✅ | ⚠️ | ✅ |
| Containment (user-folder) | ✅ | ❌ | ⚠️ | ❌ | N/A | ❌ | ✅ | ✅ | ⚠️ | ✅ |
| Arrangement (session) | ✅ | ❌ | ✅ | ❌ | ❌ | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Arrangement (frame) | ✅ | ❌ | ⚠️ | ❌ | N/A | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| ImportedRelation | ✅ | ❌ | ✅ | ✅ (read-only) | ❌ | ❌ | ✅ | ✅ | ⚠️ | ✅ |

**Legend**: ✅ Specified · ⚠️ Partial · ❌ Missing

**Reading the table**: every ❌ is a gap that blocks the family from being
user-operable. Every ⚠️ is a gap that limits usability but doesn't completely
block operability.

---

## 10. Priority Order for Gap Closure

Ranked by: (operability impact) × (how many families the fix applies to)

1. **Inspector surface for all families** — every family has a ❌ or ⚠️ here.
   A single well-designed edge inspector popover contract covers most of these.
   Highest leverage.

2. **Session → durable promotion for Arrangement** — the most important single
   user-facing feature gap. Losing a workspace arrangement on close is a
   fundamental usability failure.

3. **AgentDerived Accept/Dismiss flow** — three ❌s in one family; needs
   `AcceptAgentEdge` and `DismissAgentEdge` intents and a minimal inspector.

4. **UserGrouped label editing** — one ❌ with high daily-use impact; small fix.

5. **Folder management interaction model** (Containment/user-folder) — creation
   and nesting gestures need a full spec.

6. **Import review mode** (ImportedRelation) — prerequisite for bookmarks and
   filesystem ingest; blocked on a flow design.

7. **Per-kind weight within Semantic family** — physics correctness gap; lower
   user-facing urgency than the above.

8. **Undo/redo scope per family** — correctness gap; can be handled as part of
   the intent definitions for items 2–5.
