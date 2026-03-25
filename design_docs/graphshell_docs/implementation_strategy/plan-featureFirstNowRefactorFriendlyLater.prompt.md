# Graphshell Foundation Contract

**Replaces**: "Feature-First Now, Refactor-Friendly Later" and the Greenfield Blueprint
**Posture**: This is a prototype. Breaking things today to avoid catastrophes later is acceptable.
**Not acceptable**: Getting three categories of things wrong that require data migration or a trust model rebuild to fix.

---

## 0. The Vertical Slice — The Only Real Test of Architecture

Before any contract, define the one thing that must work end-to-end:

```
User opens app
→ types a URL in the omnibar
→ Servo renders the page in a pane
→ a traversal edge is written to the WAL
→ the node appears in the graph canvas
→ user closes the pane
→ node transitions to Warm state
→ diagnostics show the state transition
→ WAL replay restores the node correctly
```

Every contract below either enables this slice or prevents it from breaking as we add
features. If a proposed contract cannot be explained in terms of this slice, it is
deferred. This test is also the acceptance gate for Phase 0.

---

## 1. What We Are Questioning

The previous plan inherited some assumptions that need to be examined before we freeze
anything.

### Assumption 1: We need 11 crates now.

No. VSCode does not derive its modularity from package count. Its modularity comes from
one thing: the extension API — the stable public surface that extension authors target.
Internal packages change constantly; the extension API does not.

Graphshell's equivalent of the extension API is the registry system and policy traits:
`CanvasTopologyPolicy`, `ViewerProvider`, `ActionRegistry` key schemas, `ModRegistry`.
These must be stable. The internal module structure is free to change. Two crates now;
extract more when natural seams emerge from real feature pressure, not from anticipation.

### Assumption 2: History is a peer domain alongside graph and workbench.

No. History is a projection of graph traversal edges — it is a query layer over the
graph, not an independent authority. Giving it a domain crate creates a dependency back
into the graph domain before the graph model is even stable. History stays as a module
inside the graph domain until the query surface has earned its own crate.

### Assumption 3: All contracts are equally urgent.

No. Three categories of mistakes are genuinely catastrophic — they require data migration
or a trust model rebuild if wrong:

1. **Node identity, operation ordering, and address format.** If `NodeId`, `OpId`, or
   `Address` are wrong, future sync is impossible without migrating every persisted node.
2. **Content process isolation.** Verso runs Servo as a subprocess. That IS the trust
   boundary. If we design as if everything is in one process, we cannot retrofit
   meaningful security later.
3. **Extension API surface.** If mods target internal types that we later reorganize,
   we break mod compatibility. Lock the extension API; reorganize internals freely.

Everything else — render resolution ownership, focus capture model, diagnostics
uniformity — is recoverable with normal refactoring.

### Assumption 4: "Temporary adapter + deletion ticket" scales.

No. The `pending_*` fields in `graph_app.rs` started as intentional temporary state.
Without a mechanical cap, the "deletion ticket" pattern becomes a graveyard. Every
temporary adapter must be importable only from within a declared boundary — it should
not compile from outside. The deletion ticket must have a milestone gate attached, not
just an issue number.

### Assumption 5: The process boundary is an implementation detail.

No. Firefox's lesson: the content process boundary is a first-class design decision that
determines what security guarantees are possible at all. Verso runs Servo in a subprocess.
That subprocess is the untrusted web execution context. The IPC channel between the main
process and Verso IS the trust boundary. What the previous plan called "renderer authority
contract" is really: what crosses this IPC boundary, in what shape, and who validates it.

---

## 2. Three Process Boundaries (The Firefox Lesson)

Graphshell has three trust and isolation zones today. Name them explicitly:

```
┌──────────────────────────────────────────────┐
│  Main Process  (trusted orchestrator)        │
│  graphshell-core + graphshell host code      │
│  graph model / workbench / registries        │
│  egui / wgpu compositor / iroh               │
└──────────────────┬───────────────────────────┘
                   │  IPC: compositor callback + navigation commands
┌──────────────────▼───────────────────────────┐
│  Content Process  (untrusted web)            │
│  Verso / Servo                               │
│  WebRender / JS / CSS / cookies / storage    │
└──────────────────┬───────────────────────────┘
                   │  wgpu surface sharing / texture handle
┌──────────────────▼───────────────────────────┐
│  GPU Process  (isolated for stability)       │
│  wgpu / WebRender compositor                 │
│  (in progress: CompositorAdapter)            │
└──────────────────────────────────────────────┘
```

**Consequence for design**: The "renderer authority contract" is not a Rust type hierarchy
question — it is an IPC message design question. `ViewerRequest` and `ContentEvent` are
IPC crossing types. `ViewerResolution` is host-local resolution derived from IPC results.
This is the correct framing for that contract.

**Consequence for the compositor**: the three-pass composition (UI Chrome → Content →
Overlay) maps directly onto this process topology. The compositor's job is to assemble
outputs from all three zones into one frame. The CompositorAdapter GL isolation effort is
the right direction.

---

## 3. Two Module Boundaries That Actually Matter (The VSCode Lesson)

### Boundary 1: WASM-clean core vs platform host

```
graphshell-core          (WASM-clean identity and mutation kernel)
  Graph, NodeId, OpId, GraphId, Address
  GraphMutation, GraphDelta, apply()
  GraphSemanticEvent  (the only type that crosses core → host)
  CoopSession authority rules
  NIP-84 / clip publication schema
  Compiles to wasm32-unknown-unknown: ZERO ERRORS — this is the enforcement

graphshell (current monolith, being internally reorganized)
  egui / wgpu / Servo / iroh / egui_tiles / egui_graphs
  WorkbenchState, RuntimeState, FocusRouter
  ViewerRegistry, CanvasTopologyPolicy, ModRegistry
  Diagnostics, UxTree, ActionRegistry
```

The WASM compilation constraint is mechanical enforcement. It is better than any code
review: if the `graphshell-core` boundary compiles to `wasm32-unknown-unknown`, it
definitionally has no platform entanglement. Nothing else guarantees this.

The enforcement can begin before the extraction is fully complete. A minimal
`graphshell-core` crate or shim target is enough to start compiling the intended kernel
boundary early, then Phase 2 completes the move of the real implementation behind that
already-existing boundary.

Extract more crates only when: (a) a module has been stable across at least one feature
wave, AND (b) its internal dependency direction is already clean, AND (c) a second
deployment target (mobile, extension) will actually consume it.

### Boundary 2: Extension API (semver-stable, never break)

These are the VSCode extension API equivalents. Once a mod targets them, they are frozen:

```rust
pub trait CanvasTopologyPolicy: Send + Sync { ... }  // physics/layout customization
pub trait ViewerProvider: Send + Sync { ... }         // new content viewer types
// ActionRegistry: namespace:name key schema — stable
// ModRegistry: inventory::submit! native mod pattern — stable
// PhysicsProfileRegistry: preset key schema — stable
```

The internal module layout of the host crate can be reorganized at will. Mod authors
never touch internal types. The only mod-facing surface is the registry system and these
traits.

---

## 4. Non-Negotiable Contracts

### Contract 1: Identity and Ordering

These three types must be correct now. If they are wrong, WAL replay, undo, and future
sync all require a data migration.

```rust
pub struct NodeId(Uuid);        // v4 — stable durable identity; never a session handle
pub struct OpId(Uuid);          // v7 — monotone ordering; sortable WAL, undo, sync log
pub struct GraphId(Uuid);       // v4 — one per Workbench; the graph root anchor
pub struct PaneId(Uuid);        // v4 — workbench-local only; never persisted as node truth
pub struct FrameId(Uuid);       // v4 — workbench-local only
pub struct GraphViewId(Uuid);   // v4 — per-view instance; workbench-local

pub enum Address {
    Http(Url),
    File(PathBuf),              // host-resolved only; valid in WASM as data
    Ipfs(Cid),                  // Verse — lock the variant NOW even if resolver is unimplemented
    Onion(OnionAddr),           // Verse — same
    Internal(InternalPath),     // verso:// internal scheme
    Custom(String),             // mod escape hatch
}
```

**UUID v7 for `OpId` is not optional.** UUID v7 is time-ordered, making WAL entries
sortable by `OpId` without a separate timestamp column. When sync arrives, two peers
merge by replaying `OpId`-ordered event logs. UUID v4 here makes sync painful to retrofit.

**`Address::Ipfs` and `Address::Onion` must be locked now.** If added later, every
existing node with `Address::Custom("ipfs://...")` needs a migration. Lock the variants;
leave the resolution logic `unimplemented!()`.

**Enforcement**: newtypes with private inner fields. The compiler prevents confusing
`NodeId` with `OpId`. `#[non_exhaustive]` on `Address` prevents external exhaustive
matches from breaking when new variants land.

### Contract 2: Mutation Authority

The overloaded `GraphIntent` blends durable mutations, workbench actions, and runtime
events into one enum. This is the reason `graph_app.rs` is 13K lines — everything looks
like a graph operation. The fix is an enum split, not a full `AppPlan` pipeline.

**Minimum viable version — do this now:**

```rust
// Three honest enums, one for each authority domain
pub enum GraphMutation { /* durable graph writes only */ }
pub enum WorkbenchAction { /* pane/tile/frame arrangement only — no WAL */ }
pub enum RuntimeEffect {
    AttachViewer { pane_id: PaneId, resolution: ViewerResolution },
    DetachViewer { pane_id: PaneId },
    EmitDiagnostic { event: DiagnosticEvent },
    ReconcileFocus { command: FocusCommand },
}

// GraphIntent becomes an adapter surface only
#[deprecated = "use GraphMutation, WorkbenchAction, or RuntimeEffect"]
pub enum GraphIntent { ... }
```

**Enforcement**: `#[deprecated]` on `GraphIntent` immediately. CI check: count of
non-deprecated `GraphIntent` usages must trend downward — add a test that fails if
the count increases. New code that constructs `GraphIntent` directly is a review block.

The `AppCommand → AppPlan → AppTransaction` pipeline from the previous plan is the
correct long-horizon target. Do not implement it yet. The enum split makes the authority
visible now without the implementation cost.

### Contract 3: IPC Crossing Types

Everything that crosses from main process → Verso, or back, must be serializable.
No egui types. No `NodePaneState` references. No interior mutability. Serialize or don't
cross.

```rust
// main → content process
pub struct ViewerRequest {
    pub pane_id: PaneId,
    pub address: Address,
    pub renderer_hint: Option<String>,
}

// main-side resolution (never sent over IPC, but derived from IPC response)
pub struct ViewerResolution {
    pub viewer_id: String,
    pub render_mode: RenderMode,
    pub fallback_reason: Option<ViewerFallbackReason>,
}

// content process → main
pub struct ContentEvent {
    pub pane_id: PaneId,
    pub kind: ContentEventKind,  // Ready, Failed, NavigationRequested, ...
}
```

**Enforcement**: all IPC-crossing types live in a module annotated with
`#[deny(missing_debug_implementations)]` and derive `serde::Serialize, serde::Deserialize`.
A type that cannot be serialized cannot be in that module. The compiler enforces the
boundary.

### Contract 4: Extension API Stability

The traits and registry key schemas listed in §3 Boundary 2 are documented
`// SEMVER-STABLE: do not change without a version bump` and reviewed as API changes,
not internal refactors.

No test enforces this today — it is a code review gate. But it must be called out
explicitly in the review template so it is never accidentally treated as internal churn.

### Contract 5: Focus — One Enforceable Rule Now

The full six-track focus architecture is correct and in progress. But trying to unify
all six tracks simultaneously is too much scope for one gate. One rule that is immediately
testable and immediately high-value:

**`Esc` is never ambiguous.**

Every modal surface has exactly one `Esc` handler in scope, and that handler must return
focus to a named `FocusReturnAnchor`. No modal surface ships without a scenario test
that opens it, presses `Esc`, and asserts focus returned to the declared anchor.

```rust
pub enum FocusReturnAnchor {
    GraphView(GraphViewId),
    Pane(PaneId),
    Toolbar,
    LastActive,
}
```

The six-track unification can follow. `Esc` correctness is the gate for this contract
today.

### Contract 6: Observability — Structural, Not Cultural

"Every state machine registers named transition points" is a culture rule. Culture rules
do not survive delivery pressure.

The structural version: every degraded or fallback branch must include an
`emit_diagnostic!()` call at the point of divergence. This is still a code review gate,
but it is bounded to one pattern rather than an abstract culture goal.

The long-horizon target: a `#[derive(DiagnosticState)]` proc-macro that forces transition
registration at compile time. Not now. Name it as the target; implement it in Phase 2.

---

## 5. What Gets Torn Up Today

### `GraphIntent` is deprecated immediately.

Not "gradually phased." Not "bridged for now." Marked `#[deprecated]` today. Every new
feature routes through `GraphMutation`, `WorkbenchAction`, or `RuntimeEffect`. The count
of remaining `GraphIntent` usages is a CI metric that must decrease, not increase.

### The 11-crate map is deferred indefinitely.

Two crates now. Additional crates are earned by stable seams under real feature pressure,
not designed in advance. Premature crate extraction adds friction before we know where
the seams actually are.

### History as a peer domain is rejected.

History is a query layer over graph traversal edges. It lives in a module inside the
graph domain. It earns its own crate when the query surface is stable and a second
consumer (mobile, extension) needs it. Not before.

### The temporary-adapter graveyard is capped.

Maximum 5 open temporary-adapter deletion tickets at any time. New cross-boundary
shortcuts are blocked at review if the cap is exceeded. Tickets must name a milestone
gate, not just an issue number. A ticket without a milestone is a rejected ticket.

---

## 6. Phase Sequence

### Phase 0 — Lock the foundations (before the next feature lane starts)

- [ ] `NodeId`, `OpId`, `GraphId`, `Address` newtypes with correct UUID versions
- [ ] `Address::Ipfs`, `Address::Onion` variants locked; resolvers `unimplemented!()`
- [ ] Minimal `graphshell-core` crate or shim target exists and compiles to `wasm32-unknown-unknown`
- [ ] `GraphMutation / WorkbenchAction / RuntimeEffect` enum split; `GraphIntent` deprecated
- [ ] `ViewerRequest`, `ContentEvent` as serializable IPC-crossing types; `ViewerResolution` as local resolution output
- [ ] `CanvasTopologyPolicy`, `ViewerProvider` documented as semver-stable
- [ ] Scenario test: every existing modal surface → `Esc` → named `FocusReturnAnchor`
- [ ] Vertical slice test passes: navigate → render → WAL write → node in graph → close → Warm → WAL replay

### Phase 1 — Build features against the contracts

- All new durable mutations use `GraphMutation`, not `GraphIntent`
- All new viewer features dispatch through `ViewerResolution`
- All new modal surfaces ship with an `Esc` → named anchor scenario test
- All new degraded states include an `emit_diagnostic!()` call
- CI: banned-construction check fails if new code constructs `GraphIntent` directly
- Temporary-adapter cap: 5 open tickets maximum; new shortcuts blocked if exceeded

### Phase 2 — Eliminate legacy paths

- Remove all `GraphIntent` usages (migrate to the three honest enums)
- Delete `Deref/DerefMut` on `GraphWorkspace` (already planned in foundational reset)
- Collapse `pending_*` staging fields into one explicit command queue with one drain point
- Complete `graphshell-core` extraction behind the already-existing crate / shim boundary
- Move `DomainState` into its own module with restricted visibility
- Begin `AppCommand → AppPlan` pipeline for one narrow domain (routing/pane-open is the right first domain)

### Phase 3 — Extract workspace crates (when seams are earned)

- `graphshell-domain-graph` when `GraphMutation` and `apply()` are stable
- `graphshell-domain-workbench` when `WorkbenchAction` and tile tree are stable
- `graphshell-core-wasm` when a browser extension deployment is actively planned
- `graphshell-core-uniffi` when a mobile deployment is actively planned
- Do not extract crates speculatively

---

## 7. Explicit Deferrals

These have architecture implications that ARE addressed by the locked foundations above.
No further code decisions needed until these features are actively built.

| Feature | Architecture implication | Addressed by |
|---|---|---|
| Verse / IPFS hosting | `Address::Ipfs` must be a variant | `Address` enum locked in Phase 0 |
| Nostr relay sync | `OpId` must be globally orderable | UUID v7 locked in Phase 0 |
| Matrix-backed rooms | `GraphId` is the room anchor | `GraphId` locked in Phase 0 |
| iOS / Android | Core must be WASM-clean | `graphshell-core` WASM constraint |
| Browser extension | Core must be WASM-clean | `graphshell-core` WASM constraint |
| FLora / federated mods | Mod API must be stable | Extension API semver-stable lock |
| Verse async peer sync | WAL ordering alone is insufficient for divergent-state merge; CRDT layer or explicit merge policy required when two peers mutate offline and reconnect | Deferred — WAL + `OpId` is the correct Phase 0 foundation; merge semantics must be decided before Verse sync ships, not before |

These are deferred without any current architecture implications:

- `AppPlan` / `AppTransaction` full pipeline — `GraphMutation` split is sufficient now
- `#[derive(DiagnosticState)]` proc macro — define the pattern, implement in Phase 2
- Full six-track focus unification — `Esc` correctness is the gate for now
- UxTree scenario runner — add probes per-lane as lanes ship
- GPU process full isolation — CompositorAdapter is in progress; let it land
- Multiple view projections (list, timeline, kanban), surface handoff flows, and Navigator
  as explicit graph view — see `technical_architecture/unified_view_model.md` for the
  architecture backing; no code decisions needed until each view type is actively built

---

## 8. Verification Gates

### Phase 0 gate
- `cargo check -p graphshell-core --target wasm32-unknown-unknown` passes with zero errors
- Vertical slice test passes end-to-end (§0)
- `Esc` scenario test passes for all existing modal surfaces
- `GraphIntent` is `#[deprecated]`; CI fails if new usages are added

### Phase 1 gate (per feature lane)
- Lane uses `GraphMutation`, `WorkbenchAction`, or `RuntimeEffect` — not `GraphIntent`
- Lane ships with at least one scenario test
- Lane's degraded states each have an `emit_diagnostic!()` call
- Temporary-adapter count is at or below cap

### Phase 2 gate
- Zero non-adapter uses of `GraphIntent` remain
- `graphshell-core` compiles to `wasm32-unknown-unknown`
- `pending_*` staging fields replaced by one explicit command queue

### Phase 3 gate (per crate extraction)
- Crate has been stable across one full feature wave
- Second deployment target actively needs the crate
- Internal dependency direction was already clean before extraction

---

## 9. Relevant Files

- `design_docs/TERMINOLOGY.md` — canonical concept naming
- `design_docs/DOC_POLICY.md` — no-legacy-friction and scaffold constraints
- `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md` — lane sequencing
- `design_docs/graphshell_docs/implementation_strategy/system/2026-03-06_foundational_reset_implementation_plan.md` — CLAT pattern and current state baseline
- `design_docs/graphshell_docs/technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` — `graphshell-core` scope and WASM target
- `design_docs/graphshell_docs/implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md` — three-pass compositor model
- `design_docs/graphshell_docs/implementation_strategy/subsystem_focus/focus_state_machine_spec.md` — focus authority
- `graph_app.rs` — the monolith; Phase 2 extraction source
- `app/intents.rs` — `GraphIntent`; deprecated in Phase 0
- `app/graph_mutations.rs` — target home for `GraphMutation`
- `app/workbench_commands.rs` — target home for `WorkbenchAction`
- `shell/desktop/workbench/` — tile tree and pane lifecycle
- `registries/` — extension API surfaces; semver-stable
