<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Persistent Rapier Adapter Plan (2026-04-19)

**Status**: **Archived 2026-04-20** — Complete. Momentum-preserving
drag-release landed; every §5 validation item passes.
**Scope**: Upgrade [`RapierLayout`](../../../../../../crates/graph-canvas/src/layout/rapier_adapter.rs)
from its rebuild-per-step first revision to a persistent-world shape so
cross-frame momentum accumulates. The first revision (landed 2026-04-19)
builds a fresh `RapierSceneWorld` each step; this plan replaces that with a
single world kept alive across frames, with positions synced in and out
each step and a topology-hash-triggered rebuild.

**Parent**: [2026-02-24_physics_engine_extensibility_plan.md](2026-02-24_physics_engine_extensibility_plan.md) §"Canvas Editor Layer: rapier2d".

---

## 1. Why persistent

Rigid-body physics is stateful — velocities, angular momentum, joint
constraint violations, and collision islands all accumulate across steps.
The current adapter discards all of that every frame by rebuilding the
world from snapshot positions. The user-observable consequence is that
a nudged node doesn't coast; it stops the frame after you stop dragging.

For the Graphshell force-directed canvas this matches existing feel
(non-simulate mode), so the per-step rebuild was fine for the retirement
baseline. But the `simulate` feature exists specifically to enable
momentum, collision, and constraint behavior users can "feel". A
rebuild-per-step adapter in that mode is a semantic regression.

---

## 2. Persistence strategy

### 2.1 World ownership

Move the `RapierSceneWorld` off the `RapierLayout` temp allocation and
onto the layout itself:

```rust
pub struct RapierLayout<N: Clone + Eq + Hash> {
    pub config: RapierLayoutConfig,
    world: Option<RapierSceneWorld<N>>,
    /// Hash of the scene topology (node ids + edge set) the current world
    /// was built from. Rebuild if it drifts.
    world_topology_hash: u64,
}
```

`world` is constructed lazily on the first `step()` with non-empty scene.
Subsequent steps reuse it.

### 2.2 Topology-triggered rebuild

Each `step()` computes a topology hash from `scene.nodes[].id` and
`scene.edges[]`. If the hash differs from the stored one, the world is
rebuilt from scratch (discarding momentum — acceptable since the
underlying graph shape changed). Hash covers:

- Sorted node ids
- Sorted edges `(source, target)` pairs
- Pinned set (so pinning/unpinning rebuilds)

Hashing is cheap (`ahash::AHasher` or `fxhash::FxHasher` over a sort
buffer) compared to the rapier step itself; the sort is O(n log n) where
n is usually <10⁴.

### 2.3 Position sync in

On steps where topology is unchanged, sync positions from the scene into
the world before stepping:

```rust
for node in &scene.nodes {
    if extras.pinned.contains(&node.id) {
        world.set_kinematic_position(&node.id, node.position);
    } else {
        // Re-anchor the initial_positions so read_positions() returns
        // the per-step delta, not the cumulative delta since build.
        world.rebase_initial_position(&node.id, node.position);
    }
}
```

The second branch needs a small extension to `RapierSceneWorld`:

```rust
// crates/graph-canvas/src/simulate.rs
impl<N: Clone + Eq + Hash> RapierSceneWorld<N> {
    /// Reset the stored "initial position" for a node to a new anchor,
    /// without moving the body. Used by adapter callers to compute
    /// per-step deltas from the stored initial.
    pub fn rebase_initial_position(&mut self, node_id: &N, position: Point2D<f32>) {
        if let Some(initial) = self.initial_positions.get_mut(node_id) {
            *initial = position;
        }
    }
}
```

**But**: for a dragged-then-released node, we want the rapier body to
keep its momentum, not snap back to the scene position. The host's
dragged-node path already updates the scene position when a drag
concludes; between drag and release, the body's position should reflect
the user's drag target (kinematic), not be recomputed from rapier.

This is an interaction-flow design question that maps cleanly to:

- **Pinned nodes** → `set_kinematic_position` each step (user-controlled).
- **Dynamic nodes currently being dragged** → `set_kinematic_position`
  each step (user-controlled mid-drag).
- **Dynamic nodes, no drag** → rapier owns the position; adapter reads it
  back and writes it into the scene via deltas.

So the adapter also needs to know which nodes are mid-drag. Either add
`dragging: HashSet<N>` to `LayoutExtras`, or expect the host to mark
dragged nodes as `pinned` for the duration of the drag (simpler but
conflates two concepts).

Recommended: add `dragging: HashSet<N>` to `LayoutExtras`. Semantically
distinct from pinned (pinned = "user wants this node immovable forever",
dragging = "user has their finger on this one this frame").

### 2.4 Position sync out

After `world.step()`, read positions for dynamic non-dragged nodes and
emit deltas:

```rust
let deltas = world.read_positions();
deltas.retain(|key, _| !extras.pinned.contains(key) && !extras.dragging.contains(key));
```

Semantics of `read_positions`:

- After the rebase-in step (§2.3), the stored initial positions match the
  scene positions at the start of this step.
- `read_positions` returns current − initial = delta this step.
- Per-step delta emission matches the `Layout` trait contract.

---

## 3. API changes

### 3.1 `RapierSceneWorld` additions

- `rebase_initial_position(&mut self, node_id: &N, position: Point2D<f32>)`
  — already outlined in §2.3.
- Optionally: `topology_hash(&self) -> u64` for asserting consistency.

Non-breaking; adds one method to an existing public struct.

### 3.2 `LayoutExtras` additions

- `dragging: HashSet<N>` — new slot.

Existing code that builds `LayoutExtras` with `..Default::default()`
picks it up transparently. Explicit struct-literal uses require one new
field — small blast radius across graphshell proper.

### 3.3 `RapierLayout` internal state

- Add `world: Option<RapierSceneWorld<N>>` and `world_topology_hash: u64`
  fields to `RapierLayout`.
- Update `Default` / `new` to initialize empty.

No external API change on `RapierLayout`.

---

## 4. Performance envelope

The rebuild-per-step adapter measured at ~n + e body+joint constructions
plus one rapier step per frame. For n=200 / e=500 that was sub-ms on a
modern CPU.

Persistent variant:

- Happy path (no topology change): topology hash (O(n+e) sort + hash) +
  position-in sync (O(n)) + rapier step + position-out read (O(n)).
  Net: similar to the rebuild per step for small graphs, strictly faster
  for medium-to-large graphs where the build cost dominates.
- Rebuild path (topology changed): same cost as current rebuild plus the
  hash. Negligible overhead.

No perf regression expected; a real win at scale.

---

## 5. Validation

- **Momentum preserved across frames**: nudge a node via `apply_impulse`,
  verify it travels over multiple steps instead of stopping after one.
- **Topology change triggers rebuild**: add a node, verify the world
  actually grows (body count goes up).
- **Drag-release feel**: mark a node `dragging`, move it externally, then
  clear dragging; the body should carry velocity from the drag motion
  into free flight.
- **Pinned nodes immovable**: a pinned body stays put even when external
  forces (gravity, spring pulls from edges) would push it.
- **Per-step deltas match convention**: `read_positions` returns deltas
  that, summed with the previous step's positions, equal the current
  rapier translations. No cumulative drift.

---

## 6. Non-goals

- **Deterministic replay across machines.** Rapier is deterministic only
  with identical integration parameters and step counts; Graphshell's
  frame timing isn't tight enough to guarantee this. Verse-sync and
  similar cross-peer scenarios require periodic position snapshots, not
  this adapter.
- **Islands-aware LOD.** Future optimization: pause bodies far from the
  camera or in stable collision islands. Orthogonal to this plan.
- **Full rapier3d upgrade.** 2D only; rapier3d is a separate upgrade path
  tracked in [2026-02-24_physics_engine_extensibility_plan.md](2026-02-24_physics_engine_extensibility_plan.md).

---

## 7. Progress

### 2026-04-19

- Plan created alongside rapier adapter landing. Design is ready; no
  prerequisite decisions remain.

- **Landed** later the same day
  ([crates/graph-canvas/src/layout/rapier_adapter.rs](../../../../crates/graph-canvas/src/layout/rapier_adapter.rs),
  [crates/graph-canvas/src/simulate.rs](../../../../crates/graph-canvas/src/simulate.rs),
  [crates/graph-canvas/src/layout/mod.rs](../../../../crates/graph-canvas/src/layout/mod.rs)).
  Scope delivered:
  - `RapierSceneWorld::rebase_initial_position` and
    `RapierSceneWorld::set_dynamic_position` added. Non-breaking additions
    to the public API; persistent-world callers use the first for
    per-step delta re-anchoring and the second for external teleport
    (velocity reset) of dynamic bodies.
  - `LayoutExtras::dragging: HashSet<N>` slot added — transient
    per-frame flag, semantically distinct from `pinned`. Struct-update
    users (`..Default::default()`) picked it up transparently; no host
    changes required.
  - `RapierLayout` is now generic `RapierLayout<N>` and owns an
    `Option<RapierSceneWorld<N>>` plus a `world_topology_hash: u64`.
    Manual `Debug` impl because `RapierSceneWorld` does not derive
    `Debug`; manual `Default` because `#[derive(Default)]` would
    conflict with the `Option<RapierSceneWorld<N>>` field's `N` generic.
  - Topology hash is a `DefaultHasher` over a domain-tagged tuple of
    sorted node ids, sorted directed `(source, target)` edge pairs,
    sorted `pinned` set, and sorted `dragging` set. Rebuild fires when
    the hash drifts or when the world is `None`.
  - `effective_body_kind` now layers `dragging` over `pinned`: any
    dragging node is `KinematicPositionBased` regardless of policy.
    Pinned nodes follow `BodyKindPolicy` as before.
  - Per-step sync: for pinned **or** dragging nodes, the adapter calls
    `set_kinematic_position` + `rebase_initial_position` so the body
    tracks the host and emits no spurious delta; for dynamic non-dragged
    nodes, the adapter reads the current rapier translation and
    re-anchors the initial to it (per-step deltas, not cumulative).
  - Empty-scene path drops the persistent world and resets the hash so
    the next non-empty step rebuilds cleanly.
  - `Layout::step()` post-filter removes any delta for pinned or
    dragging nodes; defensive in addition to their kinematic body kind.
  - Seven new tests added on top of the five pre-existing rapier
    adapter tests (now twelve total): world-persistence across steps
    under unchanged topology, rebuild on added node, rebuild on pinned
    change, rebuild on dragging change, dragging-emits-no-delta,
    momentum accumulation across frames (the old rebuild-per-step
    variant fails this assertion), and empty-scene drops the world.

- **Deferred to follow-on** (single §5 validation gap):
  - **Drag-release momentum handoff.** The current implementation
    rebuilds the world when the dragging set changes, which discards
    the body's pre-rebuild velocity. The plan's §5 "drag-release feel"
    success criterion calls for the body to carry drag velocity into
    free flight on release. Landing that requires either runtime
    body-kind switching (`RigidBody::set_body_type(Dynamic, wake: true)`
    preserves linvel) or a pre-rebuild velocity capture that seeds the
    new body with `apply_impulse`. Both are small; neither is wired up
    today. Tracked here as a named follow-on.
  - Registry: `register_builtins<N>` now requires `N: Ord`
    unconditionally to keep the rapier topology-hash signature flat
    across features. All practical host node-id types (`u32`, `u64`,
    `NodeKey`, `String`) already satisfy this.

- **Receipts**: `cargo check -p graph-canvas --features simulate --lib`
  clean; `cargo test -p graph-canvas --lib` 195 passed / 0 failed;
  `cargo test -p graph-canvas --features simulate --lib` 228 passed / 0
  failed (12 of which are rapier adapter tests); `cargo check
  --workspace --exclude servoshell --exclude webdriver_server` clean
  (only pre-existing warnings in host crates).

### 2026-04-20 (drag-release momentum handoff)

The last deferred §5 validation item closed. Drag release now carries
the drag-motion velocity into free flight via in-place body-type
flipping, so a thrown node continues moving instead of coming to rest
the frame the pointer lifts.

**Portable layer** additions to
[crates/graph-canvas/src/simulate.rs](../../../../crates/graph-canvas/src/simulate.rs):

- `RapierSceneWorld::mark_body_kinematic(&N)` — flips a node's rigid
  body to `KinematicPositionBased` via rapier's `set_body_type`. No
  translation / velocity change. No-op when the body is already
  kinematic.
- `RapierSceneWorld::hand_off_kinematic_to_dynamic(&N, Vector2D<f32>)` —
  flips the body to `Dynamic` and seeds `set_linvel` with the supplied
  handoff velocity. Velocity is host-computed (from visible drag
  deltas) rather than read back from rapier's internal kinematic
  state, because the visible delta produces a more predictable "throw"
  feel than rapier's end-of-step linvel approximation.
- `RapierSceneWorld::body_type(&N) -> Option<RigidBodyType>` — lets
  tests and adapters observe the body kind.

**Adapter** changes in
[crates/graph-canvas/src/layout/rapier_adapter.rs](../../../../crates/graph-canvas/src/layout/rapier_adapter.rs):

- `dragging` is **removed from the topology hash**. Drag transitions
  no longer rebuild the world, they flip body types in place.
- New state on `RapierLayout<N>`: `prior_dragging: HashSet<N>`,
  `last_drag_position: HashMap<N, Point2D<f32>>`,
  `last_drag_velocity: HashMap<N, Vector2D<f32>>`. All three are
  cleared on world rebuild and empty-scene.
- Per-step drag-state matrix:
  - `(false, true)` drag start → `mark_body_kinematic`. Skipped for
    `PinnedStatic` anchors (preserves immovable-anchor semantics).
  - `(true, true)` ongoing drag → sample velocity from last drag
    position to now, cache in `last_drag_velocity`. Sampling during
    ongoing frames is what lets a motionless release frame still
    hand off the accumulated velocity from the drag.
  - `(true, false)` drag release → `hand_off_kinematic_to_dynamic`
    with the cached velocity. Pinned nodes skip the handoff so they
    remain anchored.
- End-of-step cleanup retains only currently-dragging entries in the
  two drag-caches; released nodes are purged so they can't interfere
  with a future drag of the same node.

**Tests** — two new rapier adapter tests on top of the existing
twelve:

- `rapier_adapter_drag_release_carries_velocity_into_free_flight` —
  drags node rightward across three frames (0,0) → (10,0) → (30,0),
  releases without motion on frame four. Asserts body flipped to
  `Dynamic` and the release-frame delta has a substantial +x carry
  (> 0.5 world units) rather than zero.
- `rapier_adapter_drag_start_flips_body_to_kinematic_without_rebuild`
  — drag start flips body to `KinematicPositionBased` without
  changing the topology hash.
- The old `rapier_adapter_rebuilds_on_dragging_change` test was
  replaced by `rapier_adapter_does_not_rebuild_on_dragging_change`
  to pin the new "in-place flip, no rebuild" contract.

**Receipts**: `cargo test -p graph-canvas --features simulate --lib`
— 257 pass (was 248 before this addition; 14 rapier adapter tests
total, +2 new). Full graph-canvas + graphshell suites clean at
257/257 + 2152/2152.

**Plan closed.**
