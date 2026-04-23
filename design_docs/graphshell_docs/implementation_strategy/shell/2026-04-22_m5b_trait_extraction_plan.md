<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# M5b — Runtime-host trait extraction (Async + SignalRouter + ViewerSurfaceHost)

**Date**: 2026-04-22
**Status**: Slice plan — ready to execute
**Audience**: Contributors bringing up M5 iced-native + preparing for
the eventual wasm32-unknown-unknown bring-up.

**Related docs**:

- [`2026-04-14_iced_host_migration_execution_plan.md`](2026-04-14_iced_host_migration_execution_plan.md)
- [`../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md`](../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md)
- [`../../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md`](../../technical_architecture/2026-03-29_portable_web_core_host_envelopes.md)

---

## 1. Context

M4 (see the portable-shell-state technical-architecture doc) extracted
every portable runtime/host-boundary type into `graphshell-core`.
`GraphshellRuntime` still lives in the shell crate because three of
its fields are genuinely host-coupled: **async scheduling**, **signal
routing**, and **viewer-surface lifecycle**. Those aren't "move the
type"; they're concrete integrations with tokio, the registry signal
bus, and servo's rendering contexts.

The M5 iced-host execution plan (2026-04-14) assumes iced inherits
tokio the same way egui does, which is fine for iced-native. It does
not address the host-coupled seams, which block:

- iced hosts that want to drop tokio (lighter footprint).
- `wasm32-unknown-unknown` bring-up (no tokio runtime, no threads,
  different worker-supervision model entirely).

M5b sits alongside M5a (iced-native wiring) and extracts three host
traits so both hosts talk to the same portable interface. M5b does
**not** redesign the worker-supervision model — that's a separate
architectural question tracked in §6 below.

---

## 2. Scope

Three traits land in `graphshell-core`:

1. **`AsyncSpawner`** — spawning supervised tasks.
2. **`SignalRouter`** — subscribing to the signal bus as a
   host-neutral `Stream<Item = SignalEnvelope>`.
3. **`ViewerSurfaceHost`** — viewer surface allocation/retirement
   lifecycle (decoupled from servo-specific GL context creation).

Each trait replaces a leak where the runtime currently reaches for a
concrete tokio / servo / shell type. Egui and iced-native hosts both
implement the traits with their existing tokio + servo plumbing; the
runtime talks to the trait exclusively.

---

## 3. Non-goals

- **Worker supervision redesign** (cooperative task driving, frame-loop
  polling, etc.). The `AsyncSpawner` trait accepts the current "spawn
  and forget" model — it just parameterises who does the spawning.
  A follow-on slice can introduce a `WorkerPool` trait with a
  different semantic model when wasm32 commits.
- **Removing tokio from the shell crate**. Tokio stays as an
  implementation detail of the egui + iced-native `AsyncSpawner`
  impls. Only the runtime's *dependency on tokio's concrete types*
  goes away.
- **WASM bring-up itself**. This slice unblocks wasm32 but does not
  provide a wasm host. Worker-model decisions (see §6) must be made
  before wasm32 actually ships.
- **iroh / Nostr / sysinfo wasm-compat**. Those are upstream
  dependency concerns, independent of the trait shape.

---

## 4. Trait shapes

### 4.1 `AsyncSpawner`

Location: `graphshell-core::async_host` (new top-level module).

```rust
/// Host-provided async task spawner.
///
/// The runtime calls into this trait instead of `tokio::spawn` /
/// `tokio::task::JoinSet` directly so each host picks its own
/// execution model. Native hosts wrap a `tokio::runtime::Handle`;
/// future wasm hosts wrap `wasm_bindgen_futures::spawn_local` (with
/// the caveat that `Send` + `'static` bounds narrow the set of
/// futures they can accept).
pub trait AsyncSpawner: Send + Sync {
    /// Spawn a supervised task. The host is responsible for task
    /// lifetime, cancellation on drop, and panic isolation.
    fn spawn_supervised(
        &self,
        label: &'static str,
        task: BoxFuture<'static, ()>,
    );

    /// Spawn a blocking (CPU-bound or fs/sync I/O) task. Not
    /// available on all hosts — returns `Err(SpawnError::Unsupported)`
    /// on targets with no blocking executor (notably
    /// `wasm32-unknown-unknown`).
    fn spawn_blocking<T: Send + 'static>(
        &self,
        label: &'static str,
        work: Box<dyn FnOnce() -> T + Send + 'static>,
    ) -> Result<crossbeam_channel::Receiver<T>, SpawnError>;

    /// Broadcast a graceful-shutdown signal to all supervised tasks.
    /// Idempotent; returns immediately (does not await task
    /// completion).
    fn request_cancel(&self);

    /// Has cancellation been requested?
    fn is_cancelled(&self) -> bool;
}

pub enum SpawnError {
    /// This host does not support blocking tasks (e.g. wasm32).
    Unsupported,
    /// The runtime is shutting down and the task was rejected.
    ShuttingDown,
}
```

**Why `BoxFuture` instead of a generic `F: Future`?** Object safety.
The runtime stores `Arc<dyn AsyncSpawner>`, so methods can't be
generic. `BoxFuture<'static, ()>` is the standard workaround. The
small allocation overhead per-spawn is negligible vs. the task's own
cost.

**Why `crossbeam_channel::Receiver<T>` as the blocking-task result?**
Matches the existing `spawn_blocking_host_request_rx` return type —
callers already thread this through `AsyncRequestState<T>` host-side
drivers. No new vocabulary.

### 4.2 `SignalRouter`

Location: `graphshell-core::signal_router` (new top-level module).

```rust
/// Host-provided subscription handle for the registry signal bus.
///
/// The runtime's `GuiFrameInbox` subscribes to shell-facing signals
/// (semantic index updates, workbench projection refreshes, settings
/// routes, profile invalidations) via this trait instead of calling
/// `phase3_subscribe_signal_async` directly.
pub trait SignalRouter: Send + Sync {
    /// Subscribe to a signal topic. Returns a boxed `Stream` the
    /// caller drains each frame. On native hosts this is backed by a
    /// tokio channel; on wasm32 it can be a `futures::channel::mpsc`.
    fn subscribe(
        &self,
        topic: SignalTopic,
    ) -> BoxStream<'static, SignalEnvelope>;
}

/// Identifier for a signal topic — keeps the subscription API
/// portable without importing concrete registry types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignalTopic(pub String);

/// Opaque signal payload. Hosts that produce signals pack domain
/// data into the `payload` field; consumers downcast or
/// pattern-match on the string tag.
#[derive(Clone, Debug)]
pub struct SignalEnvelope {
    pub tag: String,
    pub payload: serde_json::Value,
}
```

**Why not carry the concrete `SignalBus` type?** The bus lives in
`shell::desktop::runtime::registries` and pulls in tokio + registry
internals. A thin envelope + topic-string is enough for the
subscription seam. Producers of signals (the registries themselves)
keep their concrete bus type; the consumer-side (the runtime's frame
inbox) uses the trait.

### 4.3 `ViewerSurfaceHost`

Location: `graphshell-core::viewer_host` (new top-level module).

```rust
/// Host-provided viewer surface lifecycle.
///
/// The runtime tracks *which* panes should have a live viewer
/// surface (via `ViewerSurfaceRegistry` data — already portable);
/// the host owns *how* surfaces are allocated and retired.
pub trait ViewerSurfaceHost: Send + Sync {
    /// Allocate a surface for a node. Idempotent — calling twice for
    /// the same node is a no-op.
    fn allocate_surface(
        &mut self,
        node_key: NodeKey,
        viewer_id: &ViewerInstanceId,
    ) -> Result<(), ViewerSurfaceError>;

    /// Retire a surface for a node. Idempotent — calling for a
    /// node that has no allocated surface is a no-op.
    fn retire_surface(&mut self, node_key: NodeKey);

    /// Query whether a surface is currently allocated.
    fn has_surface(&self, node_key: NodeKey) -> bool;
}

pub enum ViewerSurfaceError {
    ResourceExhausted,
    InvalidViewer,
    HostShuttingDown,
}
```

**What about GL contexts, texture handles, servo WebView objects?**
Those stay shell-internal. The `ViewerSurfaceHost` impl on the egui
side holds `Rc<OffscreenRenderingContext>` and `egui::TextureHandle`s;
the runtime doesn't see them. The iced-native impl will hold iced's
equivalent types. The wasm32 impl provides a no-op (surfaces exist
in the bookkeeping but never materialise as textures — iced-web
renders via HTML5 Canvas).

---

## 5. Call-site migration

### 5.1 `AsyncSpawner`

**Writers** (sites that spawn tasks):

- `control_panel.rs`: `spawn_supervised_task`, `spawn_blocking_host_request_rx`,
  `spawn_registered_agent`, `spawn_agent`, `spawn_shell_signal_relay`.
  All internal to `ControlPanel`; refactor `ControlPanel` to hold
  `Arc<dyn AsyncSpawner>` instead of a `tokio::runtime::Handle`.
- `GuiFrameInbox::spawn()` — calls `control_panel.spawn_shell_signal_relay()`.
  No change needed; `ControlPanel` internally dispatches through the
  trait.

**Runtime field**:

- `GraphshellRuntime.tokio_runtime: Option<tokio::runtime::Runtime>`
  → `GraphshellRuntime.async_spawner: Arc<dyn AsyncSpawner>`.
- The concrete `tokio::runtime::Runtime` moves to the egui host's
  `TokioAsyncSpawner` struct, held alongside `EguiHost`'s other
  host-adjacent state.

**Shell-side impl** (~200 lines):

```rust
// shell/desktop/runtime/tokio_async_spawner.rs
pub struct TokioAsyncSpawner {
    handle: tokio::runtime::Handle,
    cancel: CancellationToken,
    join_set: Mutex<JoinSet<()>>,
}

impl AsyncSpawner for TokioAsyncSpawner { /* ... */ }
```

The existing `ControlPanel::spawn_supervised_task` logic (panic
isolation, cancellation coordination, tier-aware concurrency) moves
into `TokioAsyncSpawner` or stays in `ControlPanel` operating on the
trait — whichever reads cleaner. **Prefer to keep tier/concurrency
policy in `ControlPanel`** (portable, testable) and push only the raw
spawn+cancel mechanism into the trait.

### 5.2 `SignalRouter`

**Writers** (sites that produce signals): stay shell-side. Domain
registries publish to their concrete `SignalBus`; no API change.

**Readers** (sites that subscribe): one call site —
`GuiFrameInbox::spawn()` in `shell/desktop/ui/gui_state.rs`
(approximately, subject to the current M4 layout). The `await`-on-
receive loop moves behind a `while let Some(envelope) = stream.next().await`
pattern driven by the `AsyncSpawner` (same loop body, different
channel type).

**Shell-side impl** (~50 lines):

```rust
// shell/desktop/runtime/registries/signal_router_adapter.rs
pub struct RegistrySignalRouter {
    registry: Arc<RegistryRuntime>,
}

impl SignalRouter for RegistrySignalRouter {
    fn subscribe(&self, topic: SignalTopic) -> BoxStream<'static, SignalEnvelope> {
        let receiver = self.registry.subscribe_to_signal_bus(&topic.0);
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(receiver).map(|...|...))
    }
}
```

### 5.3 `ViewerSurfaceHost`

**Writers** (sites that allocate surfaces):

- `tile_runtime.rs`: creates per-pane surfaces on webview mapping.
- `tile_compositor.rs`: queries surface existence for composited
  rendering.
- `webview_lifecycle.rs` (if it exists under that name): retires
  surfaces on webview destroy.

**Runtime field**:

- `GraphshellRuntime.viewer_surfaces: ViewerSurfaceRegistry` stays as
  *bookkeeping data* (already portable). Adjacent field:
  `GraphshellRuntime.viewer_surface_host: Arc<Mutex<dyn ViewerSurfaceHost>>`
  (or unwrapped Arc if the trait bounds allow), supplying the live
  servo / iced / wasm lifecycle.
- Callers that today do `viewer_surfaces.insert_gl_context(...)` call
  `viewer_surface_host.lock().allocate_surface(...)` and let the
  host impl update its internal state. The registry data is updated
  via a trait method that takes `&mut ViewerSurfaceRegistry` as a
  parameter, OR the host holds the registry internally and exposes
  query methods — **decide during implementation** based on which
  locking story reads cleaner.

**Shell-side impl** (~150 lines):

```rust
// shell/desktop/workbench/servo_viewer_surface_host.rs
pub struct ServoViewerSurfaceHost {
    rendering_context: Rc<OffscreenRenderingContext>,
    window_rendering_context: Rc<WindowRenderingContext>,
    // ... the concrete servo types that compositor_adapter.rs holds today
}

impl ViewerSurfaceHost for ServoViewerSurfaceHost { /* ... */ }
```

---

## 6. The worker-supervision question (out of scope, but decide before wasm)

`ControlPanel` today supervises **long-lived concurrent workers** —
P2P sync (iroh), Nostr relay, memory monitor, mod loader, prefetch
scheduler. These run independently of the frame loop and spawn
tokio tasks via `JoinSet`. The `AsyncSpawner` trait preserves this
model: on native hosts, `spawn_supervised` kicks off a concurrent
task that runs until it completes or is cancelled.

**On wasm32, "concurrent background task" doesn't exist** (single
threaded event loop, no preemption). Four product-shape options:

**(a) Cooperative / frame-driven workers.** Rewrite workers from
"spawn and forget" to "poll from the frame loop each tick."
Progress only happens when UI is live. Degrades P2P throughput
significantly; acceptable for a browser demo, not for real P2P use.
**~2–3 weeks** for the ControlPanel refactor.

**(b) Gate Tier-1 workers off wasm.** Provide no-op impls for
P2P/Nostr/memory-monitor on wasm; browser Graphshell has no
background services, only in-frame intent processing. Clean scope,
fastest path; costs product functionality on browser.
**~1 week.**

**(c) Dedicated Web Workers per service.** Each Tier-1 worker
(iroh, Nostr) runs in its own `Worker` with its own wasm instance;
main thread messages through `postMessage` or SharedArrayBuffer.
Architecturally close to what native does. Requires COOP/COEP
headers on the hosting site (`Cross-Origin-Opener-Policy: same-origin`
plus `Cross-Origin-Embedder-Policy: require-corp`) — real deployment
commitment, breaks embedded iframes and some third-party resources.
Requires upstream wasm32 support from each worker's crate stack.
`wasm_bindgen_rayon` is a **different tool** — suitable for data-
parallel bursts (physics, layout), NOT for long-lived I/O services.
Rayon-on-wasm needs `SharedArrayBuffer`, nightly Rust,
`-Z build-std`, target-feature flags; evaluate independently if
layout/physics becomes a CPU bottleneck.

**(a+c) Hybrid — cooperative for simple workers, dedicated Web
Worker for heavyweight I/O services (recommended).** See §6a for the
per-worker classification. This is the realistic path now that
iroh 0.33 + nostr-sdk both have browser-compatible builds.

### 6a. Per-worker wasm classification (April 2026 ecosystem status)

| Worker | Current stack | Wasm path | Notes |
|--------|---------------|-----------|-------|
| **P2P sync (iroh)** | `iroh` 0.33+ | **(c) dedicated Web Worker** | [iroh 0.33 browser support](https://docs.iroh.computer/deployment/wasm-browser-support) — relay-based connections only (no hole-punching, no DHT, no local discovery — browser sandbox prohibits raw UDP). E2E encryption intact. Build with `default-features = false`. 0.34 planned to expand capabilities. |
| **Nostr relay pool** | `nostr-sdk` | **(c) dedicated Web Worker** | [`nostr-sdk-wasm-example`](https://github.com/rust-nostr/nostr-sdk-wasm-example) demonstrates wasm32 build. NIP-03 gated off on wasm. `WebSocket` is worker-available natively, no proxy needed. Library is ALPHA — expect breaking API changes. |
| **Memory monitor** | `sysinfo` | **stub to `Normal`** | [`sysinfo` doesn't target wasm32](https://crates.io/crates/sysinfo). Browser memory pressure is unreliable (`performance.memory` is Chromium-only and coarse). The memory-pressure level drives tier throttling for workers we won't have in the browser anyway — stubbing to `Normal` preserves semantics cleanly. |
| **Mod loader** | Filesystem walk via `fs::read_dir` | **(a) cooperative + remote fetch** | No wasm filesystem. Mod discovery in browser routes through `fetch()` to a remote manifest or through `OPFS` for user-imported mods. Cooperative (main-thread) fetch is fine — mod discovery isn't performance-critical. |
| **Prefetch scheduler** | Tokio timer-based | **(a) cooperative** | Schedule-driven, not throughput-critical. Poll from frame loop on wasm; lose sub-frame precision but prefetch is heuristic anyway. |
| **Nostr relay worker (NostrRelayWorker)** | Same as Nostr relay pool | **(c) dedicated Web Worker** | Shares the Nostr Web Worker instance. |
| **Shell signal relay** | Tokio mpsc bridge | **(a) cooperative** | Already coalesced into `GuiFrameInbox`; drain per-frame is the natural model on wasm. |

**Cost breakdown for (a+c) hybrid**:

- Trait extraction (this M5b slice): **~1 week**.
- Cooperative refactor for mod loader + prefetch + shell signal
  relay: **~1 week** (small, well-defined worker surfaces).
- iroh Web Worker bring-up: **~1–2 weeks** (wire postMessage bridge,
  handle `Send` bounds, graceful shutdown).
- Nostr Web Worker bring-up: **~1–2 weeks** (shares infrastructure
  with iroh worker; faster if done second).
- `sysinfo` wasm stub: **~1 day**.
- COOP/COEP deployment: **hosting-configuration work**, not code.

**Total (a+c) estimate**: ~4–6 weeks beyond M5b, spread across the
wasm32 bring-up milestone. Compared to option (b) "gate everything
off" at ~1 week, the hybrid preserves P2P + Nostr functionality on
browser at a 3–5 week premium.

### 6b. Rust wasm threading toolchain status

As of April 2026 (Rust ≈ 1.95): **wasm threading still requires
nightly**. [Tracking issue #77839](https://github.com/rust-lang/rust/issues/77839)
for WebAssembly atomics is open; `target_feature = "atomics"` and
`-Z build-std` are both unstable. The limiting factor here is Rust
stabilisation rather than a missing wasm ecosystem piece.
[`wasm32-wasip1-threads`](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1-threads.html)
is an experimental target.

Consequences for Graphshell:

- If we want `wasm_bindgen_rayon` (shared-memory threading) for
  data-parallel layout/physics: **nightly toolchain required** for
  the wasm build. Acceptable for a sub-target; not ideal.
- If we stick to dedicated Web Workers + `postMessage` (separate
  wasm instances per worker, no shared memory): **stable toolchain
  works**. This is another reason option (c) dedicated Web Workers
  is a better fit than the rayon path — it avoids the
  SharedArrayBuffer + nightly-Rust complexity entirely.

### 6c. Recommended order

1. **M5b traits** (this slice, ~1 week).
2. **M5a iced-native** (already planned, 2–3 weeks).
3. **Wasm-prep**: cooperative refactor for mod loader + prefetch +
   signal relay. Lands in native first (testable), then applies on
   wasm unchanged. ~1 week.
4. **Sysinfo wasm stub + iroh + nostr Web Worker bring-up**.
   Parallel-izable across contributors. ~3–5 weeks aggregate.
5. **M6 wasm bring-up proper** — iced-web host, Canvas rendering,
   surface-registry no-ops.

Traits (step 1) alone don't commit to any option — they just remove
the hard blockers so the decision can land when wasm32 commits.

---

## 7. Acceptance criteria

Slice lands as follows:

- **Three trait modules** in graphshell-core
  (`async_host`, `signal_router`, `viewer_host`) with full docstrings
  and unit tests for the trait contracts where applicable (mostly
  consists of type-check tests + a `MockAsyncSpawner` fixture).
- **`GraphshellRuntime`** holds `Arc<dyn AsyncSpawner>`,
  `Arc<dyn SignalRouter>`, `Arc<Mutex<dyn ViewerSurfaceHost>>` (or
  equivalent) instead of concrete tokio / registry-bus / servo types.
- **Shell-side impls** under
  `shell/desktop/runtime/{tokio_async_spawner,registry_signal_router}.rs`
  and `shell/desktop/workbench/servo_viewer_surface_host.rs`. All
  existing behaviour preserved — no regressions in the egui host.
- **Iced-native impls** added alongside (if M5a is concurrent) or
  stubbed as `todo!()` hooks if M5a lands later. The iced impls mirror
  the egui impls (same tokio handle, same servo contexts) except for
  the rendering-side conversions iced requires.
- **`cargo test -p graphshell-core --lib`** keeps passing with new
  trait-contract tests added.
- **`cargo build -p graphshell-core --target wasm32-unknown-unknown`**
  still builds clean (the new trait modules must be WASM-compatible
  — no platform syscalls in the trait definitions).
- **No blocking-task usage** reaches wasm through the runtime: any
  `spawn_blocking` call site that still wants to work on wasm needs
  a fallback plan OR be gated via the `SpawnError::Unsupported`
  path.
- **Design doc progress log** in
  [`../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md`](../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md)
  gets an "M5b complete" entry.

---

## 8. Estimate

- **M5b core** (three traits + egui impls + runtime wiring): ~1 week.
- **Concurrent M5a iced-native** (already scoped elsewhere): 2–3
  weeks per its execution plan.
- **M5b + M5a in parallel**: adds roughly a half-week to M5a for the
  iced-native trait impls (they're thin wrappers over tokio + servo).

If M5b lands *before* M5a starts, the iced-native host comes up
through the traits from day one — no egui-style leakage to unwind
later.

---

## 9. What this unblocks

- **Iced-native hosts** that want to drop tokio: now possible (swap
  `TokioAsyncSpawner` for a `PollAsyncSpawner` or
  `SmolAsyncSpawner`).
- **wasm32-unknown-unknown** architecture: the trait shape is the
  hard part; providing a wasm impl is ~1 week once the worker-model
  decision (§6) is made.
- **Headless / CI test runtime**: a `MockAsyncSpawner` that executes
  tasks synchronously becomes viable for integration tests that
  currently require a real tokio runtime.
- **Future runtime splits**: if the runtime is ever hoisted to its
  own crate (graphshell-runtime?), the host-adjacent trait bounds
  are already in place — no concrete tokio leakage to clean up.
