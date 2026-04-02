# Projection Runtime Lifecycle Spec

**Date**: 2026-04-02
**Status**: Canonical runtime contract
**Priority**: Graphlet-first implementation guidance; reusable by future domain projections

**Related**:

- `ASPECT_PROJECTION.md`
- `projection_interdomain_contract_spec.md`
- `../../technical_architecture/graphlet_model.md`
- `../../technical_architecture/unified_view_model.md`
- `../workbench/graphlet_projection_binding_spec.md`
- `../navigator/NAVIGATOR.md`

---

## 1. Purpose and Scope

This spec defines the runtime contract for projection descriptors, keys, invalidation, refresh, and lifecycle events in Graphshell.

It governs:

- projection descriptor identity,
- runtime cache keys,
- invalidation inputs,
- projection lifecycle events,
- linked and unlinked refresh behavior,
- graphlet as the first canonical runtime consumer.

It does not govern:

- graph truth mutation,
- workbench arrangement geometry,
- shell composition,
- or security policy for intelligence-facing source access.

---

## 2. Canonical Runtime Role

The Projection aspect needs a runtime contract because projection objects are not one-frame UI conveniences.

They are:

- derived from explicit source authority,
- recomputable under stable rules,
- consumable by more than one domain,
- and expected to survive handoff between Navigator, Workbench, and Shell without semantic drift.

Normative rule:

- projection runtime state is keyed by derivation inputs and consumer scope,
- not by whichever widget happened to render first.

---

## 3. Descriptor Contract

Every canonical projection must have a descriptor-level identity distinct from any one rendered instance.

Suggested shape:

```rust
pub struct ProjectionDescriptor {
    pub projection_id: ProjectionId,
    pub source_domain: ProjectionSourceDomain,
    pub projection_kind: ProjectionKind,
    pub derivation: ProjectionDerivation,
    pub scope: ProjectionScope,
    pub consumer_context: ProjectionConsumerContext,
    pub policy: ProjectionPolicy,
}
```

Required descriptor truths:

- what source authority the projection depends on,
- what kind of projection it is,
- what derivation rule defines membership,
- what scope or override path produced it,
- what consumer context is asking for it.

---

## 4. Runtime Key Contract

Descriptors are conceptual identity. Runtime instances need a cache key.

Suggested shape:

```rust
pub struct ProjectionRuntimeKey {
    pub source_revision: u64,
    pub descriptor_hash: u64,
    pub consumer_context_hash: u64,
    pub selector_hash: Option<u64>,
    pub anchor_hash: Option<u64>,
    pub scope_hash: u64,
}
```

The exact field set may vary by projection class, but the runtime key must always distinguish:

- source revision or equivalent invalidation generation,
- projection descriptor meaning,
- consumer scope,
- derivation-specific inputs such as selectors, anchors, seed nodes, or filter context.

Normative rule:

- two runtime instances are the same cached projection only if their source revision and derivation inputs match.

---

## 5. Runtime Instance Contract

The runtime should treat resolved projections as explicit instances rather than anonymous lists.

Suggested shape:

```rust
pub struct ProjectionRuntimeInstance {
    pub runtime_key: ProjectionRuntimeKey,
    pub descriptor: ProjectionDescriptor,
    pub membership: ProjectionMembership,
    pub frontier: Option<ProjectionFrontier>,
    pub ranking: Option<ProjectionRanking>,
    pub invalidation: ProjectionInvalidationState,
}
```

For graphlet, this maps naturally to:

- member nodes and edges,
- anchors and optional primary anchor,
- frontier candidates,
- backbone or local ranking hints.

Future domains should reuse the same pattern rather than returning ad hoc row sets.

---

## 6. Invalidation Contract

Projection invalidation must be explicit and observable.

### 6.1 Required invalidation families

Every projection class should classify invalidation sources under one or more of these families:

1. `SourceTruthChanged`
2. `ScopeChanged`
3. `SelectorChanged`
4. `AnchorSetChanged`
5. `ConsumerContextChanged`
6. `ProjectionPolicyChanged`
7. `PresentationHintChanged`

### 6.2 Runtime rule

- `SourceTruthChanged` invalidates membership correctness.
- `ScopeChanged`, `SelectorChanged`, and `AnchorSetChanged` invalidate derivation correctness.
- `ConsumerContextChanged` may preserve semantic membership while invalidating hosting suitability.
- `PresentationHintChanged` must not be allowed to masquerade as membership truth change.

### 6.3 Graphlet-specific guidance

For graphlet, the minimum invalidation inputs are:

- edge-family selector change,
- seed-node change,
- active graph-view override change,
- source graph mutation affecting reachable membership,
- graphlet-local ranking or primary-anchor change when it affects frontier order.

---

## 7. Refresh Contract

Projection refresh should distinguish recomputation from UI re-render.

### 7.1 Recompute required

Recompute is required when membership or frontier may have changed.

### 7.2 Rebind only

Rebind is sufficient when the host or consumer context changed but the underlying membership remains valid.

### 7.3 Warning path

If a linked host would change materially after recompute, the system must expose the change rather than silently rewriting the hosted structure.

This is already the right rule for graphlet-linked workbench groups and should generalize to future linked projections.

---

## 8. Lifecycle Events

Projection runtime needs first-class lifecycle events so linked hosts, diagnostics, and future history/provenance systems can observe projection transitions.

Suggested event family:

```rust
pub enum ProjectionLifecycleEventKind {
    Derived,
    Refreshed,
    Invalidated,
    Linked,
    Unlinked,
    Forked,
    Promoted,
    Discarded,
}
```

Suggested payload shape:

```rust
pub struct ProjectionLifecycleEvent {
    pub event_id: ProjectionEventId,
    pub projection_id: ProjectionId,
    pub runtime_key: Option<ProjectionRuntimeKey>,
    pub kind: ProjectionLifecycleEventKind,
    pub source_domain: ProjectionSourceDomain,
    pub consumer_context: ProjectionConsumerContext,
    pub causality: ProjectionCausality,
}
```

### 8.1 Event semantics

- `Derived`: a projection instance was computed for use.
- `Refreshed`: an existing projection instance was recomputed under the same descriptor identity.
- `Invalidated`: an existing runtime instance is no longer trustworthy under its prior key.
- `Linked`: a host now explicitly tracks this projection.
- `Unlinked`: a host detaches from source-driven updates.
- `Forked`: a new descriptor or host artifact was created from an existing projection without remaining fully linked.
- `Promoted`: the projection crossed into a more durable named or saved form.
- `Discarded`: the runtime instance or ephemeral projection is intentionally dropped.

---

## 9. Linked Host Contract

Linked hosts must keep two truths separate:

1. the projection descriptor and resolved membership they are following,
2. the host-local arrangement or presentation state they still own.

Normative rule:

- a linked host follows projection membership,
- but it does not surrender host-local geometry, focus state, tab order, or other arrangement-local carriers.

This remains the core graphlet-to-workbench rule and should generalize to future projection consumers.

---

## 10. Graphlet as Reference Implementation

Graphlet is the first reference implementation for this runtime contract.

That means Graphshell should use graphlet to validate:

- projection descriptors,
- runtime keys,
- invalidation families,
- linked versus unlinked refresh behavior,
- and lifecycle events.

If graphlet cannot be expressed cleanly under this model, the model is too weak.
If graphlet can, the model is strong enough to generalize to other domains.

---

## 11. Future-Domain Rule

When a new domain needs cross-domain representation, it should reuse this runtime contract unless it has a clearly different source-revision and derivation model.

The default goal is one projection runtime vocabulary across domains, not a fresh cache or event model for each new projection class.

---

## 12. Acceptance Criteria

The projection runtime contract is doing its job when:

1. derived representations can be keyed and refreshed without relying on UI widget lifetime,
2. invalidation reasons are explicit and classifiable,
3. linked and unlinked hosting use the same underlying lifecycle vocabulary,
4. graphlet can act as the first clean reference implementation,
5. future domains can reuse the same descriptor, key, invalidation, and lifecycle model.
