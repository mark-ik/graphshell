<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Execution Isolation and Worker Runtime Plan (2026-04-23)

**Status**: Active / cross-cutting system plan
**Scope**: Define a Graphshell-wide execution taxonomy and follow-on implementation plan for host workers, guest runtimes, Servo multiprocess integration, and wasm/web-worker realization without collapsing those concerns into one "multiprocess" model.

**Related docs**:

- `system_architecture_spec.md`
- `control_panel_spec.md`
- `signal_bus_spec.md`
- `../subsystem_mods/2026-03-08_unified_mods_architecture_plan.md`
- `../../../archive_docs/checkpoint_2026-04-23/graphshell_docs/implementation_strategy/shell/2026-04-22_m5b_trait_extraction_plan.md`
- `../graph/2026-04-03_wasm_layout_runtime_plan.md`
- `../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md`
- `../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md`
- `../../technical_architecture/2026-04-09_browser_envelope_coop_and_degradation_policy.md`
- `../../../verso_docs/technical_architecture/VERSO_AS_PEER.md`

---

## 1. Why This Plan Exists

Graphshell now has enough real execution machinery that "parallelization" can no
longer be treated as a vague future concern.

The codebase already contains at least four distinct execution stories:

1. reducer-owned synchronous host authority
2. host-supervised background workers
3. Servo's engine-owned thread/process model
4. planned WASM guest execution

Those are not interchangeable. If Graphshell keeps speaking about them as though
they are all just "workers" or all just "multiprocess," the host/runtime
boundary will stay underspecified and each subsystem will drift toward its own
ad hoc lifecycle, diagnostics, and failure model.

This plan exists to define the missing shared vocabulary and to order the next
work so:

- Servo can keep owning browser-content process topology,
- host services can stay host-supervised instead of pretending to be browser
  renderers,
- WASM guests can target a worker-safe runtime instead of inheriting today's
  in-process bring-up shape,
- browser-hosted Graphshell builds can map the same concepts onto Web Workers
  rather than OS threads/processes.

This is an implementation-strategy document beneath `system_architecture_spec.md`.
It does **not** replace that file as canonical system policy authority.

---

## 2. Current Reality

### 2.1 Code-verified facts

1. **Servo multiprocess is already wired at the host entry point.**
   - `prefs.rs` exposes `-M/--multiprocess`.
   - `prefs.rs` also parses `--content-process <token>`.
   - `shell/desktop/runtime/cli.rs` dispatches content-process launches to
     `servo::run_content_process(token)`.
2. **Graphshell already names a portable async host seam.**
   - `graphshell-core::async_host::AsyncSpawner` is object-safe and explicitly
     exists so native and wasm hosts can choose different executor models.
   - The archived M5b trait-extraction plan recorded that a follow-on
     `WorkerPool`-class seam may be required when wasm32 commits.
3. **Current WASM mods are not worker-backed yet.**
   - `mods/wasm/mod.rs` is a headless in-process Extism runtime.
   - Active plugins live in a global `OnceLock<Mutex<HashMap<...>>>`.
   - The runtime shape is direct host calls into a loaded plugin, not a
     message-based worker boundary.
4. **Graphshell already has real host-side worker usage.**
   - `ControlPanel`-owned work such as Verse sync, Nostr relay coordination,
     memory-monitoring, prefetch, and other runtime services are host-supervised
     worker concerns rather than browser-engine isolation concerns.

### 2.2 Immediate architectural implication

Graphshell needs a taxonomy that can describe all of those execution modes
without forcing them into one process model.

The right question is **not**:

- "Should everything become multiprocess like Servo?"

The right question is:

- "What execution/isolation class does each component belong to, and what host
  contract does that class require?"

---

## 3. Non-Goals

This plan does **not** do the following:

- replace Servo's own internal process architecture with a Graphshell-owned one
- require Chromium-style browser/renderer/GPU process splits
- force every background service into an OS process
- declare today's in-process Extism runtime an acceptable long-term WASM model
- decide every platform-specific enablement detail up front

---

## Feature Target 1: Define the Execution Classes

### Target 1 Context

Graphshell needs one execution taxonomy that works across desktop-native,
mobile, and wasm-hosted envelopes. Without that, host/runtime APIs will keep
mixing synchronous authority, background work, and guest isolation into one
undifferentiated bucket.

### Target 1 Tasks

1. Define the primary execution classes used by the host/runtime boundary.
2. State what authority, lifecycle, and communication shape each class allows.
3. Make stable handles and typed request/result contracts the default host
   language across class boundaries.
4. Keep trust/admission policy separate from execution class; a trusted native
   component and an untrusted guest may still use different execution classes.

### Target 1 Execution Classes

| Execution class | What it is for | Communication shape | Typical examples |
| --- | --- | --- | --- |
| **Main-thread host authority** | Reducer-owned truth and frame-coupled host logic that must remain synchronous with UI/compositor state | direct in-process calls inside host authority boundaries | graph truth, workbench authority, compositor, view-model routing |
| **Supervised host worker** | Host-owned async or blocking work that may run independently of the frame loop but is still trusted host code | host-owned task spawn + typed messages/receipts | Verse sync, Nostr relay pool, prefetch, indexing, memory monitor |
| **Guest worker / sandbox runtime** | Narrow-authority guest compute or content logic that should not receive direct mutable access to host truth | message/ABI boundary with watchdog, cancellation, and fallback | WASM layouts, future WASM applets, other sandboxed guests |
| **Engine-owned process model** | A subsystem that owns its own thread/process topology and IPC internally | stable host IDs + engine API/IPC boundary, not host-managed internal topology | Servo single-process threads or Servo multiprocess content processes |
| **External helper / service process** | Explicit OS-process isolation for heavyweight or privileged helpers that do not fit the engine-owned case | process boundary with explicit startup/shutdown and health receipts | future converters, media helpers, privileged bridges |

### Target 1 Validation Tests

- Every existing runtime component can be assigned a primary execution class.
- No class requires the host to assume direct mutable access across a process or
  guest boundary.
- Browser-hosted Graphshell can express the same class model even where the
  concrete backend is a Web Worker instead of an OS thread/process.

### Target 1 Outputs

- A canonical execution-class vocabulary for implementation planning.
- A host/runtime rule: stable IDs and typed messages cross boundaries; direct
  in-process access is the exception, not the default.

---

## Feature Target 2: Classify the Current Runtime

### Target 2 Context

The taxonomy only matters if it maps onto the code we already have. This target
classifies the major current components and marks where reality is still
transitional.

### Target 2 Current Classification

| Component family | Current class | Current reality | Follow-on note |
| --- | --- | --- | --- |
| Reducer, graph/workbench truth, compositor, host UI routing | **Main-thread host authority** | Already the authoritative synchronous host path | Should stay host-owned across all envelopes |
| Verse sync, Nostr relay coordination, prefetch, memory monitor, similar runtime services | **Supervised host worker** | Already mediated through host worker/runtime infrastructure | `AsyncSpawner` is the current seam; richer worker contracts remain follow-on |
| Servo web runtime | **Engine-owned process model** | Single-process threads by default; multiprocess launch path already exists behind `-M` | Graphshell should address Servo through stable handles, not invent a second content-process model |
| Current WASM mods | **Transitional: in-process guest runtime** | Headless Extism runtime stored in a global mutexed plugin map | Not an acceptable steady state for browser-hosted Graphshell or sandbox-oriented guest policy |
| Runtime-loaded WASM layouts | **Planned guest worker / sandbox runtime** | ABI/watchdog/fallback work is already planned, but runtime substrate is not worker-backed yet | Must align with this plan instead of freezing the current in-process runtime shape |
| Future browser-hosted background services | **Guest worker / sandbox runtime** or **supervised host worker**, depending host envelope | No unified worker substrate yet | On wasm hosts, the concrete realization is usually a Web Worker |
| Future privileged or heavyweight helpers | **External helper / service process** | Not generally used today | Reserve for cases that genuinely need OS-process isolation |

### Target 2 Validation Tests

- The current runtime no longer reads as "single-process except Servo."
- The in-process Extism runtime is explicitly marked as transitional rather than
  implied to be the future WASM architecture.
- Trust boundary and execution boundary are distinguishable in the docs.

### Target 2 Outputs

- A code-truthful classification matrix for current components.
- A clear statement that Servo's multiprocess capability is **one** execution
  class realization, not the universal template.

---

## Feature Target 3: Design the Follow-On Worker/Guest Runtime Seam

### Target 3 Context

`AsyncSpawner` is the right minimal seam for host-owned tasks, but it is not
enough by itself for sandboxed guests, browser-hosted workers, or external
helpers. Graphshell needs a second abstraction for message-based worker/guest
execution.

### Target 3 Tasks

1. Keep `AsyncSpawner` as the host-owned task-spawn seam.
2. Introduce a follow-on `WorkerPool` or `GuestRuntime` concept for:
   - long-lived message-based workers
   - sandboxed guests
   - watchdog/cancellation/liveness tracking
   - fallback/degradation receipts
3. Require that the follow-on seam be **worker-safe first**:
   - native hosts may back it with Tokio workers, threads, or process helpers
   - wasm hosts must be able to back it with Web Workers
4. Reuse the same typed request/result shape across native and browser-hosted
   realizations whenever practical.

### Target 3 Archived M5b Findings Carried Forward

The now-archived M5b trait-extraction plan established the most useful near-term
worker-runtime constraints and those findings remain active here:

1. **`AsyncSpawner` should stay minimal.**
   It is the host-owned task-spawn seam, not the full long-lived worker/guest
   runtime contract. Keep raw spawn/cancel on `AsyncSpawner`; put message-based
   worker supervision, watchdogs, and guest lifecycle on the follow-on
   `WorkerPool` / `GuestRuntime` seam.
2. **Wasm-hosted Graphshell needs multiple worker realizations, not one blanket model.**
   The useful split from M5b is:
   - **dedicated Web Workers** for heavyweight long-lived I/O services such as
     iroh sync and the Nostr relay pool
   - **cooperative / frame-driven execution** for lighter host-owned services
     such as mod discovery, prefetch scheduling, and shell signal draining
   - **stub / unsupported mode** for host capabilities that do not have a
     browser-safe equivalent, such as the current memory-monitor path
3. **Dedicated Web Workers are the preferred browser-safe analogue for long-lived services.**
   They preserve message-based isolation without forcing Graphshell onto the
   unstable shared-memory threading path.
4. **`wasm_bindgen_rayon` is a separate data-parallel concern.**
   It may matter later for bursty layout/physics work, but it should not be
   treated as the answer for long-lived I/O services or general guest/runtime
   supervision.

### Target 3 Browser-Host Worker Posture

The M5b archival findings imply the first practical browser-host posture:

| Runtime concern | Preferred browser-host realization | Why |
| --- | --- | --- |
| Long-lived network/service worker (`iroh`, `nostr`) | **Dedicated Web Worker** | preserves isolation and background progress without pretending the main thread is concurrent |
| Lightweight scheduler/relay work (prefetch, signal drain, simple discovery) | **Cooperative / frame-driven** | low throughput sensitivity; easier to keep portable |
| Host-only diagnostics with no good browser analogue | **Stub / unsupported** | better than silently inventing fake parity |
| Data-parallel compute bursts | **Separate future evaluation** | do not couple this to the long-lived worker-runtime design |

This means the follow-on `WorkerPool` / `GuestRuntime` seam must support at
least three outcomes on wasm hosts:

1. a real worker-backed realization
2. a cooperative host-loop realization
3. an explicit unsupported/stub path

### Target 3 Minimum Contract

The follow-on worker/guest seam should eventually answer:

1. how a worker/guest is created and identified
2. how manifest capabilities and ABI versions are negotiated before work starts
3. how requests/events are delivered and correlated
4. how results, diagnostics, and health receipts come back
5. how cancellation, timeout, and shutdown work
6. how fallback is triggered when the worker/guest becomes unhealthy
7. how unsupported/stub realizations are reported without pretending to be healthy

### Target 3 Proposed Worker-Backed Guest Runtime Shape

The most useful next abstraction is a host-owned **`GuestRuntime` supervisor**
layered above `AsyncSpawner`, not a raw executor replacement.

`AsyncSpawner` remains the substrate for trusted host-owned task launch. The new
seam owns **message-based guest lifecycle** and can be backed by a dedicated
native worker, a browser `DedicatedWorker`, or a stricter helper/process-backed
runtime later without changing the caller-facing contract.

| Concept | Responsibility | Why it matters |
| --- | --- | --- |
| **`GuestProgramSpec`** | Declares guest identity, execution class, capability set, ABI/version string, and preferred backend policy | startup must validate what is being launched before a worker is spawned |
| **`GuestRuntimeHandle`** | Stable host-side handle for one supervised runtime backend | callers need stable IDs instead of thread/process references |
| **`GuestSessionHandle`** | Logical session within that runtime for one guest instance or stateful workflow | lets one runtime backend host multiple independent guests without exposing transport details |
| **Typed request/result envelopes** | Correlated message transport for `request_id`, payload, and completion/failure | keeps native and browser realizations semantically aligned |
| **Health and degradation receipts** | Reports `Starting`, `Ready`, `Busy`, `TimedOut`, `Crashed`, `Restarted`, `Unsupported`, `DegradedFallback` | the host needs explicit liveness/fallback state instead of silent failure |

### Target 3 Initial Lifecycle

1. The host resolves a `GuestProgramSpec` and validates capability/ABI claims.
2. The host asks `GuestRuntime` to provision or reuse a worker-backed backend.
3. The runtime performs a handshake and returns a stable `GuestRuntimeHandle`.
4. The caller opens a `GuestSessionHandle` for one stateful guest workflow.
5. Requests cross the boundary as typed envelopes with correlation IDs.
6. Results return alongside diagnostics receipts rather than through direct host
   mutation.
7. Timeout, crash, or policy failure moves the session/runtime into an explicit
   degraded state so the caller can trigger documented fallback behavior.

### Target 3 Initial Backend Policy

| Host envelope | Preferred guest-runtime backend | Notes |
| --- | --- | --- |
| **Desktop native** | dedicated worker thread or sandbox runtime behind the same message contract | do not freeze today's direct in-process Extism calls in place as the public seam |
| **Wasm-hosted / browser** | `DedicatedWorker` / Web Worker | this is the required portability target for sandboxed guests |
| **Mobile native** | selective worker-backed realization | only for guests whose lifecycle and capability posture are validated |

`WorkerPool` may still exist as an implementation detail for provisioning
workers, but downstream plans should target **`GuestRuntime` semantics**:
supervised worker-backed guest execution, typed envelopes, and explicit health
receipts.

### Target 3 Validation Tests

- A guest/runtime contract can be implemented with native workers **and** Web
  Workers without changing the caller-facing semantics.
- Runtime-loaded layouts and other WASM guests can use a message-based host ABI
  instead of direct mutable host access.
- The worker/guest seam stays distinct from Servo's internal process model.
- A downstream guest plan can name `GuestProgramSpec`, session handles, and
  health receipts without inventing a parallel worker vocabulary.

### Target 3 Outputs

- A follow-on design target above `AsyncSpawner`, not a replacement for it.
- A shared substrate direction for WASM guests, browser-hosted workers, and any
  future process-backed helper runtime.
- An initial contract sketch for `GuestRuntime`, `GuestSessionHandle`, typed
  request/result envelopes, and degradation receipts.

---

## Feature Target 4: Align Diagnostics and Host Policy

### Target 4 Context

Execution classes are only operationally useful if the host can explain them in
diagnostics and degrade them honestly per host envelope.

### Target 4 Tasks

1. Add diagnostics receipts for execution class, health, timeout/crash, and
   fallback cause.
2. Define host-envelope policy for which classes are supported where.
3. Keep degraded or unsupported execution models explicit in routing and UI.
4. Make downstream plans use this taxonomy instead of silently inventing local
   worker/process language.

### Target 4 Initial Host Policy Matrix

| Host envelope | Main-thread authority | Supervised host worker | Guest worker / sandbox runtime | Engine-owned process model | External helper process |
| --- | --- | --- | --- | --- | --- |
| **Desktop native** | yes | yes | yes | yes | yes, when justified |
| **Mobile native** | yes | selective | selective, only with validated lifecycle/capability limits | selective | rare / platform-specific |
| **Wasm-hosted / browser** | yes | limited by browser event model; often maps to cooperative or Worker-backed execution | yes, via Web Workers / browser-safe guest substrate | no OS-process assumption; only engine realizations that fit browser envelope | no OS-process assumption |

### Target 4 Validation Tests

- Diagnostics can distinguish host worker failure, guest watchdog timeout, and
  engine-owned process failure.
- Browser-hosted Graphshell does not advertise desktop-native execution powers
  that it cannot realize.
- Downstream plans can cite one execution-policy source instead of restating the
  same boundary differently.

### Target 4 Outputs

- A host-envelope execution-policy baseline.
- A diagnostics plan for execution-boundary health and degradation.

---

## 4. Near-Term Sequencing

1. **Adopt this taxonomy in planning docs.**
    - M5b should remain responsible for trait extraction and local wasm bring-up
      mechanics.
    - This plan should become the cross-cutting reference for execution classes
      and worker-runtime direction.
2. **Design the follow-on worker/guest contract.**
   - Specify the `GuestRuntime` seam, stable handles, request/result envelopes,
     and the health/degradation receipts it must expose.
   - Keep `AsyncSpawner` as the smaller host-owned spawn substrate beneath it.
3. **Align the WASM runtime plans.**
   - `2026-04-03_wasm_layout_runtime_plan.md` should target a worker-safe guest
     runtime instead of implying permanent in-process Extism execution.
4. **Truth the runtime assumptions.**
   - Audit Graphshell-local multiprocess assumptions and validate Servo `-M`
     behavior in the live host before locking process policy language.
5. **Decide platform policy.**
   - Desktop, mobile, and wasm-hosted Graphshell should each have an explicit
     support matrix for the execution classes above.

---

## 5. Exit Condition

This plan is complete when Graphshell has:

1. one shared execution taxonomy used across system, shell, mods, and WASM
   runtime planning,
2. an explicit classification of current engines/components against that
   taxonomy,
3. a defined follow-on worker/guest runtime seam above `AsyncSpawner`,
4. the active WASM runtime plans aligned to that seam instead of freezing the
   current in-process guest substrate,
5. runtime-truthed assumptions for Servo multiprocess integration where process
   policy depends on observed behavior,
6. and host-aware diagnostics/policy describing how those classes degrade across
   desktop, mobile, and browser-hosted envelopes.
