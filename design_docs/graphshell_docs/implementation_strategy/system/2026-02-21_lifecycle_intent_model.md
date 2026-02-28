# Lifecycle Intent Model v2

**Authoritative specification for lifecycle state transitions, reducer boundaries, and runtime reconciliation.**

This document is the single source of truth for lifecycle modeling. Where it conflicts with other design docs, this section takes precedence.

---

## 1) Two-layer lifecycle state

Use two explicit layers:

- **Desired lifecycle (reducer-owned):** what the app wants (`Active`, `Warm`, `Cold`)
- **Observed runtime (reconcile-owned):** what runtime currently has (`Mapped`, `PendingCreate`, `Unmapped`, `Blocked`)

`reconcile_runtime()` should only converge observed runtime toward desired lifecycle.

## 2) Transition causes (required metadata)

All lifecycle transition intents carry a cause:

```rust
enum LifecycleCause {
    UserSelect,
    ActiveTileVisible,
    SelectedPrewarm,
    WorkspaceRetention,
    ActiveLruEviction,
    WarmLruEviction,
    MemoryPressureWarning,
    MemoryPressureCritical,
    Crash,
    CreateRetryExhausted,
    ExplicitClose,
    NodeRemoval,
    Restore,
}
```

## 3) Intent schema (reducer input)

```rust
enum GraphIntent {
    PromoteNodeToActive {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToWarm {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToCold {
        key: NodeKey,
        cause: LifecycleCause,
    },
    MarkRuntimeCreatePending {
        key: NodeKey,
        webview_id: WebViewId,
    },
    MarkRuntimeCreateConfirmed {
        key: NodeKey,
        webview_id: WebViewId,
    },
    MarkRuntimeBlocked {
        key: NodeKey,
        reason: RuntimeBlockReason,
        retry_at: Option<std::time::Instant>,
    },
    ClearRuntimeBlocked {
        key: NodeKey,
        cause: LifecycleCause,
    },
}
```

`MapWebviewToNode` and `UnmapWebview` remain effect/reconcile-related intents but should not be the only signal of runtime truth.

## 4) State-machine rules

| Desired | Observed | Allowed reconcile action | Notes |
| --- | --- | --- | --- |
| Active | Unmapped | Start create → `MarkRuntimeCreatePending` | If blocked and backoff active, no create |
| Active | PendingCreate | Wait or timeout | Timeout may retry or block |
| Active | Mapped | No-op | Healthy steady state |
| Warm | Mapped | Keep mapped, deprioritize | Warm is cache-resident |
| Warm | Unmapped | Optional no-op or low-priority create | Policy-controlled (default no eager create) |
| Cold | Mapped/PendingCreate | Close/unmap | Must converge to Unmapped |
| Cold | Unmapped | No-op | Healthy steady state |
| Any | Blocked | Respect `retry_at` | Prevent hot retry loops |

## 5) Invariants (debug/test assertions)

1. No node may be `Active` + `Blocked` without an explicit pending retry policy.
2. `DemoteNodeToWarm` always sets desired lifecycle to Warm, independent of current mapping order.
3. `DemoteNodeToCold` always clears runtime mapping/creation state.
4. Direct lifecycle mutation helpers are not called from reconcile code; reconcile emits intents only.
5. Crash and retry-exhaustion paths produce explicit blocked/backoff state, not immediate re-promote loops.

## 6) Practical application to current code

1. Replace direct `graph_app.promote_node_to_active(...)` calls in `desktop/lifecycle_reconcile.rs` with `PromoteNodeToActive { cause: ... }`.
2. Change `DemoteNodeToWarm` semantics in reducer so it does not silently no-op when mapping is absent.
3. Extend `desktop/webview_backpressure.rs` timeout/retry-exhaustion path to emit `MarkRuntimeBlocked` (with backoff) instead of only demoting.
4. Treat `SelectNode` prewarm as intent emission (`PromoteNodeToActive { cause: SelectedPrewarm }`) instead of helper mutation.
5. Keep `WebViewCrashed` mapping to `DemoteNodeToCold { cause: Crash }` plus blocked state (optional retry gate).

## 7) Test additions (minimum)

- `test_lifecycle_reconcile_emits_promote_intents_not_direct_mutation`
- `test_demote_to_warm_sets_desired_state_without_mapping`
- `test_retry_exhaustion_sets_blocked_and_prevents_recreate_loop`
- `test_memory_pressure_demotion_includes_cause_and_order_is_stable`
- `test_crash_path_requires_explicit_clear_before_auto_reactivate`

---

## References

- [2026-02-20_embedder_decomposition_plan.md](../aspect_render/2026-02-20_embedder_decomposition_plan.md) — Main decomposition plan that uses this model
- [ARCHITECTURAL_OVERVIEW.md](../technical_architecture/ARCHITECTURAL_OVERVIEW.md) — Current architecture summary for lifecycle integration context
