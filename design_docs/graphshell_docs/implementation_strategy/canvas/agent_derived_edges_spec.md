# Agent-Derived Edges — Deferred Spec Stub

**Date**: 2026-02-28
**Status**: Deferred — not in current implementation scope
**Priority**: Prospective

**Related**:

- `../subsystem_history/edge_traversal_spec.md` — §2.2 (`AgentDerived` EdgeKind), §2.5 (decay and promotion rules)
- `graph_node_edge_interaction_spec.md` — §5.2 (richer relationship tooling)
- `../../TERMINOLOGY.md` — `AgentRegistry`, `EdgePayload`, `EdgeKind`, `Action`

---

## Why This Stub Exists

`edge_traversal_spec.md` defines the data-model contract for `AgentDerived` edges — their `EdgeKind` value, decay semantics, and promotion rules. That spec is intentionally narrow: it covers edge lifecycle and traversal history integrity.

This stub is the home for the **canvas interaction and AgentRegistry integration design** — the questions that fall outside the traversal subsystem's scope.

---

## Deferred Design Concept

An `AgentDerived` edge is created when an `AgentRegistry` agent emits a recommendation that two nodes are related. The agent does not navigate between them; it asserts a relationship based on observation (content similarity, co-access patterns, semantic proximity, etc.).

Key questions this spec must answer when designed:

### Agent assertion protocol
- What `GraphIntent` variant does an agent emit to assert an `AgentDerived` edge?
- What payload does the intent carry (confidence score, agent id, reasoning label)?
- Can the same agent re-assert an edge to reset its decay timer?
- Can multiple agents assert the same edge? If so, how are their scores composed?

### Canvas presentation
- How does the user distinguish an `AgentDerived` edge from other edge kinds?
- What opacity/fade schedule maps to elapsed time since assertion?
- Should the agent's confidence score map to any visual property (thickness, label)?
- How does the user accept, dismiss, or permanently remove an agent suggestion?

### Interaction model
- Accepting a suggestion: user navigates the edge (promotion is automatic per `edge_traversal_spec.md §2.5`).
- Explicitly dismissing a suggestion: what intent is emitted? Does dismissal create a suppression record to prevent re-assertion by the same agent?
- Accessing agent reasoning: can the user inspect why the agent suggested the edge?

### AgentRegistry coupling
- The `AgentRegistry` owns agent lifecycle (registration, activation, inference providers).
- Agent-derived edge assertion must route through the reducer via `GraphIntent`, not via direct mutation from agent code.
- The canvas must not know the specific agent that produced an edge; it only knows `EdgeKind::AgentDerived` plus `metrics.agent_confidence`.

### Physics and layout
- `AgentDerived`-only edges should exert weaker attractive forces than `TraversalDerived` edges to reflect their provisional nature. The physics profile for agent suggestions is a canvas policy decision, not a hard constraint.

---

## Required Constraints (from `edge_traversal_spec.md`)

These are already locked in the traversal spec and must not be contradicted here:

1. Decay rule: `AgentDerived`-only edges are evicted after the configured decay window (default 72 h) with no navigation.
2. Promotion rule: user navigation asserts `TraversalDerived` and halts decay.
3. Visual: `AgentDerived`-only edges render at reduced opacity, fading over time.
4. Multi-kind rendering: if `TraversalDerived` is added, `TraversalDerived` style takes over fully.
5. Rolling window and `EdgeMetrics` apply the same as for other edges.

---

## When to Design This

Design this spec when:

1. `AgentRegistry` has at least one implemented agent emitting real graph observations.
2. The `AgentDerived` `EdgeKind` is being wired into `push_traversal` / decay logic in the reducer.
3. There is a concrete product scenario for agent-suggested edge acceptance/dismissal UX.

Do not implement agent assertion `GraphIntent` variants, decay timers, or canvas affordances until this spec is written and reviewed.
