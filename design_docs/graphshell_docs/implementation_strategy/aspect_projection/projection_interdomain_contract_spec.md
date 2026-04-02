# Projection Interdomain Contract Spec

**Date**: 2026-04-02
**Status**: Canonical interaction and ownership contract
**Priority**: Architecture-shaping; graphlet-first but intentionally general

**Related**:

- `ASPECT_PROJECTION.md`
- `../../technical_architecture/unified_view_model.md`
- `../../technical_architecture/graphlet_model.md`
- `../workbench/graphlet_projection_binding_spec.md`
- `../shell/shell_overview_surface_spec.md`
- `../shell/SHELL.md`
- `../navigator/NAVIGATOR.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for cross-domain projection in Graphshell.

It governs:

- what a projection is,
- how projection objects differ from source truth,
- which ownership boundaries hold during projection,
- how linked and unlinked hosting behave,
- how graphlet fits as the first canonical projection object,
- and how future domains may reuse the same model.

It does not govern:

- graph truth mutation rules,
- workbench arrangement semantics,
- shell composition policy,
- or the privacy policy for intelligence-facing source access.

---

## 2. Canonical Role

A **projection** is a derived, scoped, reusable representation of source-domain truth for use in another domain or surface.

Normative rule:

- source domains keep truth ownership,
- projection objects carry derived representation,
- consuming domains may host, rank, bind, or summarize projections,
- but they must not silently become the owner of source membership or source semantics.

---

## 3. Projection Law

Every projection in Graphshell should obey these rules:

1. A projection has an explicit **source authority**.
2. A projection has an explicit **scope or derivation rule**.
3. A projection has an explicit **hosting or consuming context**.
4. A projection may be cached, ranked, linked, summarized, or promoted.
5. None of those operations transfer source truth ownership by default.

This is the general law behind graphlets, workbench correspondence views, and future non-graph projections.

---

## 4. Projection Object Model

Suggested generic shape:

```rust
pub struct ProjectionDescriptor {
    pub projection_id: ProjectionId,
    pub source_domain: ProjectionSourceDomain,
    pub source_authority: ProjectionSourceAuthority,
    pub projection_kind: ProjectionKind,
    pub derivation: ProjectionDerivation,
    pub scope: ProjectionScope,
    pub consumer_context: ProjectionConsumerContext,
    pub presentation_hints: ProjectionPresentationHints,
}
```

The important point is not the exact Rust shape. It is that projections have a first-class descriptor instead of being implicit side effects of whichever surface happens to show them.

### 4.1 Required descriptor fields

Every canonical projection must be able to answer:

- what source domain it came from,
- what source authority defines membership,
- what rule derived it,
- which consumer context is hosting it,
- whether it is linked, unlinked, ephemeral, or promoted.

---

## 5. Ownership Matrix

| Concern | Owner |
|---|---|
| source truth and mutation rules | source domain |
| projection descriptor and lifecycle contract | Projection aspect |
| current navigation-local projection handling | Navigator when the projection is navigation-oriented |
| hosted arrangement binding | Workbench |
| overview summarization | Shell |
| cache invalidation inputs and generation markers | Projection aspect in cooperation with the source domain |

---

## 6. Projection Classes

Graphshell should treat these as general projection classes rather than graph-only special cases.

### 6.1 Local-world projection

A bounded subset or local world derived from source truth.

First canonical example:

- graphlet

### 6.2 Correspondence projection

A representation that maps one domain's active structure into another domain's space.

Examples:

- workbench correspondence graphlet
- shell overview mapping open arrangements back to graph state

### 6.3 Summary projection

A condensed overview of source state intended for orientation rather than detailed manipulation.

Examples:

- shell summaries of active graphlets or workbench state
- future diagnostics or history summaries

### 6.4 Temporal or episodic projection

A time-scoped or event-scoped representation derived from historical or agent state.

Examples:

- session graphlets
- future history slices
- future agent episode views

---

## 7. Linked vs Unlinked Hosting

When one domain hosts a projection, the binding must be explicit.

### 7.1 Linked hosting

The host tracks a projection descriptor and expects recomputation or refresh when the source inputs change.

### 7.2 Unlinked hosting

The host keeps a session-local arrangement or representation that no longer follows the source projection definition.

### 7.3 Promotion

Promotion is explicit and domain-owned.

Examples:

- an ephemeral graphlet becomes a named pinned graphlet,
- a session arrangement becomes a saved workbench artifact,
- a summary projection becomes a durable reference object.

Promotion must never happen implicitly because a consuming domain hosted a derived representation for long enough.

---

## 8. Invalidation and Refresh

Every canonical projection must have explicit invalidation inputs.

Typical inputs include:

- source-truth mutation
- source-view scope change
- selector change
- relation-family emphasis change
- consumer-context override change
- policy-gated freshness or ranking updates

Normative rule:

- projections may be cached,
- but cache lifetime and refresh must be observable and keyed by explicit derivation inputs,
- not by incidental UI widget lifetime.

---

## 9. Graphlet as First Canonical Instantiation

`graphlet` is the current best proof that Graphshell needs Projection as a real aspect.

Graphlet already demonstrates the full pattern:

- source truth from Graph,
- current-local-world handling in Navigator,
- hosted linked or unlinked binding in Workbench,
- shell-level summary at the host layer,
- promotion and reuse without collapsing ownership.

The Projection aspect should therefore generalize from graphlet rather than replace it.

---

## 10. Future-Domain Reuse Rule

When a new domain needs to represent itself in another domain, the default question should be:

"Can this be expressed as a projection descriptor and projection lifecycle under the Projection aspect?"

Only if the answer is clearly no should Graphshell introduce a domain-local special-case correspondence system.

This keeps the architecture honest as more domains begin projecting themselves across the UI.

---

## 11. Acceptance Criteria

The Projection aspect is doing its job when:

1. Graphshell can describe cross-domain representations without inventing a second truth owner.
2. Graphlet remains canonical but is no longer treated as a one-off exception.
3. Linked and unlinked hosting are explicit.
4. Projection invalidation and refresh are keyed by derivation inputs, not widget lifetime.
5. Future domains can reuse the same projection model instead of creating ad hoc correspondence layers.