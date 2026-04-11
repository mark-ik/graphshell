# Node Object Query Model

**Date**: 2026-04-11
**Status**: Architecture design — pre-implementation
**Scope**: Define the canonical model for examining a node as a queryable object
surface rather than only viewing a node through a renderer. This model unifies
facets, node inspection panes, browser-derived artifacts, history, tags,
memberships, and future agent-readable object access.

**Related**:

- `2026-02-18_universal_node_content_model.md`
- `2026-04-09_graph_object_classification_model.md`
- `unified_view_model.md`
- `graph_canvas_spec.md`
- `../implementation_strategy/2026-03-01_ux_migration_design_spec.md`
- `../implementation_strategy/graph/faceted_filter_surface_spec.md`
- `../implementation_strategy/graph/facet_pane_routing_spec.md`
- `../implementation_strategy/graph/2026-03-11_graph_enrichment_plan.md`
- `../implementation_strategy/subsystem_history/edge_traversal_spec.md`
- `../implementation_strategy/viewer/clipping_and_dom_extraction_spec.md`
- `../research/2026-04-11_linked_data_over_middlenet_relevance_note.md`
- `../research/2026-04-11_tabfs_tablab_graphshell_relevance_note.md`

---

## 1. Why This Model Exists

Graphshell already distinguishes between:

- the **node** as a durable graph object, and
- the **viewer** as one way to render current content.

But several active/planned surfaces still describe this indirectly:

- PMEST facet routing,
- node details panes,
- history/timeline inspection,
- clip facets,
- graph enrichment inspectors,
- browser-state and archive ideas.

This document makes the missing architectural idea explicit:

**viewing a node** and **examining a node** are different operations.

### Viewing a node

Viewing is renderer-first.

Examples:

- open the node in Servo,
- show the PDF,
- render an image,
- show plaintext content.

Viewing answers:

- what is this content like to read/use right now?

### Examining a node

Examining is object/query-first.

Examples:

- inspect identity, MIME, and viewer binding,
- inspect tags, classifications, and memberships,
- inspect history and traversal evidence,
- inspect clips, resources, logs, requests, or imported artifacts,
- inspect runtime/browser-side evidence related to the node.

Examining answers:

- what does Graphshell know about this node,
- what evidence is attached to it,
- what object families can be queried from it,
- and what can a human or agent do with those objects?

---

## 2. Core Principle

A node should be examinable as a **queryable object hub**, not only as a
renderer target.

The viewer is one projection over node state.

The examination surface is another projection over node-adjacent object
families.

This means:

- the viewer is not the canonical inspection model,
- facets are not merely UI tabs,
- and browser-derived/runtime artifacts should be representable as queryable
  objects rather than trapped in a debug-only surface.

This model is intentionally about **node-adjacent object access**, not about
replacing the broader graph object classification model.

- the **graph object classification model** answers what classes of durable or
  inspectable things Graphshell can contain,
- the **node object query model** answers how a chosen node exposes related
  evidence and sub-objects for examination.

---

## 3. Queryable Object Families

The node examination surface should organize node-adjacent data into typed
object families.

### 3.1 Identity objects

Identity answers "what node is this?"

Examples:

- node id
- current address
- address history
- title
- MIME hint
- viewer binding / render mode
- provenance/import source

### 3.2 Structure objects

Structure answers "where does this node belong?"

Examples:

- tags
- semantic classifications
- graphlet memberships
- frame/domain memberships
- relation-family summaries
- ownership/scope membership

### 3.3 History objects

History answers "what happened over time?"

Examples:

- node navigation history
- edge traversal summaries
- timeline events
- audit records
- dissolved/archive history references

### 3.4 Content objects

Content answers "what material or extracted representation exists?"

Examples:

- clip content facets
- extracted metadata
- summaries/previews
- thumbnails/favicons
- document/resource manifests

### 3.5 Runtime/browser objects

Runtime answers "what live browser/runtime evidence exists around this node?"

Examples:

- active tab/session binding
- network requests/responses
- console output
- cookies/session state
- runtime resources
- viewer-specific diagnostics

This family is especially relevant to future browser-surface bridge work.

### 3.6 Scene objects

Scene answers "what scene/presentation interpretation is attached?"

Examples:

- node-avatar binding
- scene-object linkage
- physics material/preset
- route/path linkage
- scene-script binding

This family remains view-owned and non-canonical unless explicitly promoted.

---

## 4. Canonical Query Shape

The examination model should expose a typed query surface rather than a bundle
of ad hoc per-pane conditionals.

Conceptually:

```rust
enum NodeObjectFamily {
    Identity,
    Structure,
    History,
    Content,
    Runtime,
    Scene,
}

struct NodeObjectQuery {
    node: NodeKey,
    family: NodeObjectFamily,
    facet: Option<NodeFacet>,
    filter: Option<NodeObjectFilter>,
}

enum NodeObject {
    Identity(NodeIdentityObject),
    Tag(NodeTagObject),
    Membership(NodeMembershipObject),
    HistoryEvent(NodeHistoryObject),
    ClipFacet(NodeClipObject),
    RuntimeRequest(NodeRuntimeRequestObject),
    RuntimeConsoleMessage(NodeRuntimeConsoleObject),
    SceneBinding(NodeSceneBindingObject),
}
```

This does **not** mean Graphshell must expose one public runtime API exactly in
this shape on day one. It means the architecture should normalize around typed
object families instead of UI-local inspection code.

It also implies that a node examination surface may return both:

- durable graph-owned objects, and
- ephemeral runtime/view-owned objects

as long as the provenance and authority boundary of each object are explicit.

---

## 5. Relationship To PMEST Facets

PMEST should be the **human-facing organization layer** for examination, not
the only storage/query model.

### Personality

Best maps to identity objects:

- address
- title
- renderer binding
- content identity

### Matter

Best maps to content and metadata objects:

- MIME/content metadata
- extracted content descriptors
- clip/resource details

### Energy

Best maps to process and relation activity:

- traversals
- edge/process summaries
- runtime activity traces

### Space

Best maps to structure:

- tags
- memberships
- graphlets
- domain/frame containment

### Time

Best maps to temporal objects:

- navigation history
- timeline events
- audit records

PMEST is therefore the navigation grammar for examination, while the
underlying query model remains typed and extensible.

---

## 6. Examination Surface Rules

### 6.1 Read-first by default

Node examination should be read-first by default.

Inspection must not silently mutate:

- graph topology,
- traversal truth,
- scene state,
- or viewer state.

This matches the existing history guardrail that inspection is not traversal.

### 6.2 Viewer is an adapter, not the authority

Viewer/runtime/browser-derived context can supply examinable objects, but the
viewer must remain an adapter into Graphshell-owned inspection surfaces.

Examples:

- DOM extraction inspector,
- browser request log surface,
- console/resource inspection,
- cookie/session evidence.

These should flow into Graphshell's node-object query model, not remain
viewer-specific islands forever.

### 6.3 Facet panes are query projections

Node-specific panes opened from the facet rail should be treated as query
projections over object families, not one-off bespoke panes.

For example:

- identity/address mode = projection over identity objects
- details mode = projection over content/metadata objects
- membership pane = projection over structure objects
- timeline pane = projection over history objects

---

## 7. Browser-Surface Relevance

The TabFS/TabLab-style idea is most relevant here.

The important takeaway is not "mount the browser as files" as Graphshell's core
truth model.

The important takeaway is:

- browser state can be treated as queryable objects,
- those objects can be grouped into families,
- ordinary tools and agents can inspect them,
- and Graphshell can choose to expose filesystem/REST/query adapters later.

That future browser-surface bridge should feed the `Runtime` object family of
this model.

---

## 8. Agent And Automation Relevance

This model also gives agents a better substrate than raw viewer handles.

An agent should be able to query:

- node identity objects,
- tags and memberships,
- history/timeline objects,
- clip/content artifacts,
- browser/runtime evidence,
- and scene bindings

through a typed object-query interface rather than through a renderer-specific
bridge alone.

This makes examination:

- more explainable,
- more scriptable,
- more auditable,
- and less tightly coupled to one viewer backend.

---

## 9. Recommended Follow-On Surfaces

This model should guide future work in:

- facet pane routing
- selected-node inspector / graph enrichment sidecar
- browser-surface bridge
- clip/resource inspection
- agent-readable node inspection APIs

The likely implementation direction is:

- keep PMEST as the UX routing vocabulary,
- add typed node-object families under it,
- and let viewers, history, enrichment, and browser-state bridges publish into
  the same examination model.

---

## 10. Acceptance Shape

This model is useful when Graphshell can truthfully say:

- opening a node viewer and examining a node are different operations,
- facet panes are projections over typed object families,
- history inspection does not imply traversal mutation,
- browser/runtime artifacts can become queryable node-adjacent objects,
- and agents can inspect node-related evidence without depending on one viewer
  backend's private shape.

---

## 11. Naming And Boundary Guidance

Use this document when the question is:

- how should a user or agent examine a node,
- how should facet panes organize node-adjacent evidence,
- how should browser/runtime/scene artifacts surface as queryable objects.

Do **not** use this document as the authority for:

- graph-wide object classification,
- viewer backend implementation policy,
- graph-canvas rendering architecture,
- or scene-package/runtime physics design.

Those concerns remain owned by their respective technical and implementation
specs.
