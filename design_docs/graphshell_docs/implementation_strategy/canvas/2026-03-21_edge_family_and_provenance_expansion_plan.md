# Edge Family And Provenance Expansion Plan

**Date**: 2026-03-21
**Status**: Active plan
**Purpose**: Expand the graph relation vocabulary beyond the current five-family model, with an explicit recommendation to add a dedicated **Provenance** family and to aggressively widen edge sub-kinds/types for prototype-era knowledge capture.

**Related**:

- `2026-03-14_graph_relation_families.md` - current family vocabulary and persistence/visibility/layout policy model
- `2026-03-21_edge_payload_type_sketch.md` - Rust-facing carrier-model sketch for family/sub-kind separation and explicit traversal events
- `2026-03-11_graph_enrichment_plan.md` - provenance-bearing enrichment pipeline and explanation requirements
- `graph_node_edge_interaction_spec.md` - richer relationship tooling and inspection affordances
- `../subsystem_history/edge_traversal_spec.md` - traversal-family semantics and rolling-window retention
- `../../research/2026-02-20_edge_traversal_model_research.md` - traversal-as-event research; useful counterweight against collapsing all navigation into semantic link types
- `../../TERMINOLOGY.md` - `Edge`, `EdgePayload`, `EdgeKind`, `Traversal`, `NavigationTrigger`

---

## 1. Why This Plan Exists

The current family vocabulary is directionally correct but too narrow for the way
Graphshell already wants to reason about knowledge:

- semantic relatedness,
- traversal history,
- containment,
- arrangement,
- imported structure.

That is enough to avoid chaos, but it is not enough to express several relation
classes the product already implies elsewhere in docs and UI discussion:

- provenance chains (`clipped from`, `summarized from`, `generated from`),
- epistemic structure (`supports`, `contradicts`, `questions`),
- identity/equivalence (`duplicate of`, `canonical mirror of`),
- workflow/process links (`depends on`, `next step`, `blocks`),
- collection/membership distinctions richer than the current minimal hierarchy.

Because this is still a prototype, the project should not optimize for minimum
schema churn. It should optimize for a vocabulary that makes graph meaning easy
to inspect, filter, and evolve.

This plan therefore adopts two explicit principles:

1. **Add edge types aggressively.** Prototype Graphshell benefits more from
   expressive relation capture than from premature schema minimalism.
2. **Add families only when policy truly differs.** A new family is justified
   when it needs materially different defaults for persistence, visibility,
   deletion, layout influence, or navigator precedence.

---

## 2. Recommendation Summary

### 2.1 Keep These Existing Families

- Semantic
- Traversal
- Containment
- Arrangement
- Imported

### 2.2 Add One New Family Now

- **Provenance**

### 2.3 Do Not Add Separate Families Yet For

- epistemic/argument relations,
- identity/equivalence relations,
- workflow/process relations.

These should begin life as **Semantic** sub-kinds first. If they later prove to
need different persistence or projection policy, they can split into their own
family in a later pass.

---

## 3. Family Test

Before creating a new family, ask five questions:

1. Does it want a different durability tier?
2. Does it want different default canvas visibility?
3. Does it want different deletion semantics?
4. Does it want different physics/layout force?
5. Does it want different navigator ownership or projection precedence?

If the answer is "no" to most of those, prefer a new edge type/sub-kind inside
an existing family.

Applied to current candidate areas:

| Candidate area | New family now? | Why |
| --- | --- | --- |
| Provenance / derivation | **Yes** | Provenance wants explanation-first UI, audit-preserving durability, low/default-zero layout force, and dedicated inspector treatment |
| Epistemic / argument | No | Fits Semantic defaults for now; primarily changes labels/sub-kinds and inspector actions |
| Identity / equivalence | No | Needs special actions, but not yet a distinct family-wide projection policy |
| Workflow / task | No | Can begin as Semantic sub-kinds until there is a dedicated task/workflow surface |

---

## 4. Proposed Family Vocabulary

### 4.1 Semantic Family (expanded)

Semantic remains the family for relations whose meaning is intrinsic: "these two
nodes are related in a conceptual sense."

Recommended sub-kinds/types to support:

- `hyperlink`
- `user-grouped`
- `agent-derived`
- `cites`
- `quotes`
- `summarizes`
- `elaborates`
- `example-of`
- `supports`
- `contradicts`
- `questions`
- `same-entity-as`
- `duplicate-of`
- `canonical-mirror-of`
- `depends-on`
- `blocks`
- `next-step`

Rationale:

- `supports` / `contradicts` / `questions` are argument structure, but still read
  naturally as semantic relations in the current UI model.
- `same-entity-as` / `duplicate-of` / `canonical-mirror-of` need special actions,
  but they do not yet justify a separate family.
- `depends-on` / `blocks` / `next-step` can remain semantic until Graphshell has
  a true workflow/task subsystem with distinct projection rules.

### 4.2 Traversal Family (expanded)

Traversal stays event-like and temporal. It captures that navigation happened,
not that a semantic relation exists.

Recommended trigger/sub-kind vocabulary:

- `link-click`
- `back`
- `forward`
- `address-entry`
- `pane-promotion`
- `programmatic`
- `redirect`
- `reopen-session`
- `jump-anchor`
- `in-page-search-jump`
- `unknown`

Normative rule:

- hyperlink-follow usually records both a semantic `hyperlink` relation and a
  traversal event with trigger `link-click`.
- back/forward are traversal-only by default.

### 4.3 Containment Family (widen planned sub-kinds)

The current containment design already anticipates a wider set than the current
code implements. This plan recommends adopting the full vocabulary intentionally:

- `domain`
- `url-path`
- `filesystem`
- `user-folder`
- `clip-source`
- `notebook-section`
- `collection-member`

`notebook-section` and `collection-member` are worth adding early because they
are user-legible and distinct from raw filesystem/path hierarchy.

### 4.4 Arrangement Family (clarify presentation-specific sub-kinds)

The arrangement family should remain presentation-rooted and graph-backed.
Recommended target vocabulary:

- `frame-member`
- `tile-group`
- `split-pair`
- `tab-neighbor`
- `active-tab`
- `pinned-in-frame`

These are not all equally important now, but adding them conceptually fixes an
existing ambiguity: group membership, spatial adjacency, ordering, and focus are
not the same relation.

### 4.5 Imported Family (widen source-bearing sub-kinds)

Imported should distinguish source and shape more explicitly:

- `bookmark-folder`
- `history-import`
- `rss-membership`
- `filesystem-import`
- `archive-membership`
- `shared-collection`

This gives import review and "keep this grouping" actions a much cleaner basis.

### 4.6 New Provenance Family

**What it captures**: Transformative or derivational lineage. Provenance answers
questions like:

- where did this node come from?
- what source produced it?
- was it clipped, summarized, translated, or extracted?
- is this artifact a derivative of another artifact?

Recommended sub-kinds:

- `clipped-from`
- `excerpted-from`
- `summarized-from`
- `translated-from`
- `rewritten-from`
- `generated-from`
- `extracted-from`
- `imported-from-source`

#### Persistence tier

- Usually **Durable**.
- Provenance should survive restart and sync because auditability is the whole
  point.

#### Visibility rule

- Hidden from canvas by default.
- Exposed strongly in inspector/popover and node sidecar views.
- Optional provenance lens can render them as faint directional chains.

#### Deletion behavior

- Explicit user action required.
- Deletion should preserve an audit event if possible; provenance removal is a
  semantic decision, not a cache clear.

#### Layout influence

- Zero or near-zero by default.
- Provenance should explain lineage, not drag the layout into pipeline diagrams
  unless the user explicitly activates a provenance lens.

#### Projection precedence

- Supplementary in navigator.
- Best surfaced in edge inspector, node inspector, "derived from" sections, and
  provenance-filtered views rather than owning the main tree.

This policy profile is distinct enough from existing families that Provenance
should not be squeezed into Semantic or Imported.

---

## 5. Recommended Data-Shape Direction

The current `EdgeType` enum is already carrying more than one axis:

- family,
- sub-kind,
- sometimes durability,
- sometimes provenance,
- sometimes event trigger.

That should be normalized into:

```rust
enum EdgeFamily {
    Semantic,
    Traversal,
    Containment,
    Arrangement,
    Imported,
    Provenance,
}

enum EdgeSubKind {
    Semantic(SemanticSubKind),
    Traversal(TraversalSubKind),
    Containment(ContainmentSubKind),
    Arrangement(ArrangementSubKind),
    Imported(ImportedSubKind),
    Provenance(ProvenanceSubKind),
}
```

Prototype recommendation:

- do not preserve the current enum shape just because it is already in code;
- prefer a clean split between family and sub-kind now rather than encoding more
  meaning into a monolithic enum.

---

## 6. Edge Types We Are Probably Ignoring Today

These are the high-value relation categories Graphshell currently under-models:

1. **Derivation**
   - summarized-from, translated-from, generated-from, excerpted-from
2. **Argument structure**
   - supports, contradicts, questions, rebuts
3. **Identity/equivalence**
   - duplicate-of, same-entity-as, canonical-mirror-of
4. **Workflow/process**
   - depends-on, blocks, next-step
5. **Collection semantics**
   - notebook-section, collection-member, archive-membership

Those are more valuable to prototype research and capture than adding still more
generic "related to" edges.

---

## 7. Prioritized Plan

### Phase A — Vocabulary reset

- Adopt Provenance as a sixth family.
- Split family and sub-kind in the conceptual model.
- Expand Semantic, Traversal, Containment, Arrangement, and Imported sub-kinds.

### Phase B — Inspector-first UI

- Edge inspector must always show:
  - family
  - sub-kind
  - durability
  - provenance/source
  - trigger/timestamp if traversal-backed
  - available actions

### Phase C — Behavior rules

- Default canvas visibility remains conservative.
- Provenance and Traversal stay hidden by default.
- Semantic remains visible by default.
- Containment/Arrangement/Imported remain lens- or navigator-oriented.

### Phase D — Prototype capture paths

- Clip creation writes `clipped-from`
- Summary generation writes `summarized-from`
- Import workflows write `imported-from-source`
- Duplicate detection writes `duplicate-of`
- Manual knowledge operations can create `supports` / `contradicts` / `questions`

---

## 8. Acceptance Criteria

- A dedicated Provenance family exists in the canonical family vocabulary.
- Family-vs-sub-kind separation is explicit in the design model.
- Hyperlink navigation and traversal events are no longer treated as a single
  undifferentiated concept.
- At least one prototype-ready vocabulary exists for derivation, argument,
  identity, workflow, and collection semantics.
- Inspector/presentation policy is defined for every family, including
  Provenance.
- The plan explicitly prefers clean replacement over legacy-compatibility
  preservation where the current model is too narrow.

---

## 9. Final Recommendation

Graphshell should resist both extremes:

- too few relation types, which collapses meaning into generic edges,
- too many top-level families, which makes policies incoherent.

The right next move is:

1. keep the five existing families,
2. add **Provenance** as the sixth,
3. aggressively expand edge sub-kinds within families,
4. make the edge inspector and filtering surfaces explain those distinctions.

That gives the prototype a much richer semantic graph without committing to a
premature explosion of family-specific UI.