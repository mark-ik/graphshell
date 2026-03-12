# Boa Scripting Engine Plan

**Date**: 2026-03-11
**Status**: Planning
**Priority**: Incubation — depends on no active lane; can begin as a standalone slice

**Related**:

- `subsystem_mods/SUBSYSTEM_MODS.md` — JS mods are a Mod tier; follow manifest/capability policies
- `subsystem_mods/2026-03-08_unified_mods_architecture_plan.md` — JS mod tier sits alongside WASM mod tier
- `system/register/action_registry_spec.md` — `registerAction` populates ActionRegistry; must follow `namespace:name` convention
- `subsystem_security/SUBSYSTEM_SECURITY.md` — scripting sandbox policy is a security boundary
- `TERMINOLOGY.md` — `GraphIntent`, `GraphSemanticEvent`, `ActionRegistry`, `NodeKey`

---

## Context

Graphshell is built around clean serialization boundaries: `ActionRegistry` uses string `namespace:name`
keys, `GraphSemanticEvent` crosses subsystem boundaries, `GraphIntent` is pure state mutation with no
side effects. These properties make scripting integration unusually tractable — scripts interact through
string-keyed action names and plain-object payloads, never holding live Rust borrows.

`boa_engine` is a pure-Rust ECMAScript 2024+ engine (94% test262 coverage as of v0.21). It offers ES
modules, async/await, a clean embedding API with `js_class!` macros, and synthetic modules that expose
Rust APIs as `import { ... } from "graphshell:core"`. No C FFI, no build toolchain complexity.

The target use case is **user-authored automation and query scripts** — not web page rendering and not
a replacement for Servo/SpiderMonkey. This is Graphshell-native JS: graph queries, reactive automation,
custom action registration, and eventually a JS-backed mod tier.

**Performance note**: Boa has no JIT (interpreted bytecode). It is ~28x slower than QuickJS on
compute-heavy benchmarks. For graph queries over hundreds to low-thousands of nodes and action
dispatch, this is irrelevant. The performance ceiling matters only for CPU-bound algorithms, which
are not the target workload.

---

## Engine Choice Rationale

| Engine | Language | Pure Rust | Async | ES Modules | Sandbox | Notes |
|---|---|---|---|---|---|---|
| **Boa** | JS | ✅ | ✅ (host-drained) | ✅ | Adequate | Best fit |
| Rhai | Custom | ✅ | ❌ | ❌ | Excellent | Faster; unknown language |
| rquickjs | JS | ❌ C FFI | ✅ | Partial | Moderate | 28x faster; C build pain |
| mlua | Lua | ❌ C FFI | ✅ | N/A | Moderate | Windows C FFI pain |

Boa wins on: JS familiarity (zero user learning curve for web developers), pure-Rust build, ES module
support enabling `import { ... } from "graphshell:core"`, and adequate sandboxing for the use case.

**Rhai secondary role**: Rhai is appropriate for lightweight config expressions (physics profile
tuning, filter predicates in settings). Two engines with distinct scopes is not unusual. This plan
covers Boa only; Rhai config expressions are a separate, smaller, independent decision.

---

## Scripting Architecture

### Boundary model

Scripts interact with Graphshell exclusively through:

1. **`dispatch(action_name, payload)`** — calls `ActionRegistry` by string key; queues `GraphIntent`
   mutations. Scripts never call mutation APIs directly.
2. **`graph.nodes()` / `graph.edges()` / `graph.selected()`** — return copied plain-object snapshots.
   No live Rust borrows cross the JS boundary.
3. **`graphshell.on(event_name, callback)`** — registers a JS callback for `GraphSemanticEvent`
   notifications. Rust fires events; JS handles them by dispatching actions.
4. **`graphshell.registerAction(name, fn)`** — injects a named action into `ActionRegistry`.

This model means:

- No Boa GC integration with Graphshell's internal types (copies only).
- No `Trace`/`Finalize` required on `Node`, `Edge`, or any graph struct.
- Scripts are isolated: a script crash does not corrupt Graphshell state.
- The sandbox surface is minimal and explicit — nothing is reachable unless explicitly registered.

### Context lifecycle

One `boa_engine::Context` per scripting session (startup). Module-bearing mods each get their own
`Context` to prevent cross-mod namespace pollution. A lightweight query context (no modules, no
callbacks) can be instantiated on demand for one-shot query execution.

### Sandboxing

A fresh `Context::default()` has no I/O surface. The scripting engine exposes only what is explicitly
registered:

- `graphshell:core` synthetic module: `dispatch`, `on`, `registerAction`, `graph`
- No `fetch`, no `setTimeout`, no filesystem — unless explicitly added in Phase 4.
- Instruction budget via `ContextBuilder::instructions_remaining(n)` for untrusted scripts.
- `HostHooks::can_compile_strings` override disables `eval()` and `Function()` constructors.

Module loader whitelist: only `graphshell:*` specifiers resolve; all others return an error. User mod
files are loaded by path from a trusted directory, not by arbitrary URL.

---

## Implementation Plan

### Phase 1 — Read-Only Queries + Dispatch

**Goal**: Enable graph queries and action dispatch. No callbacks, no modules, no async.

**Feature target**: A user can open a script pane, type JS, and execute it to filter nodes or trigger
actions.

**Work**:

1. Add `boa_engine` to `Cargo.toml` under an `scripting` feature flag (off by default initially).
2. Create `scripting/mod.rs` and `scripting/context.rs`:
   - `ScriptingContext` wraps `boa_engine::Context`
   - Registers `dispatch(name, payload)` as a global native function routing to `ActionRegistry`
   - Registers `graph` global object with `nodes()`, `edges()`, `selected()` methods that copy from
     graph state
3. `graph.nodes()` returns a `JsArray` of plain objects: `{ key, title, url, kind, tags }` — a
   serialized snapshot, not live references.
4. Wire `ScriptingContext` into `GraphBrowserApp` as an optional field.
5. Expose a "Run Script" action in the command palette for testing.

**Done gate**:
- `graph.nodes().filter(n => n.kind === "web").length` returns correct count.
- `dispatch("graph:node_new", { url: "https://example.com" })` opens a node.
- Feature-gated: `cargo build` without `scripting` is clean.

### Phase 2 — Reactive Event Hooks + Action Registration

**Goal**: Scripts can respond to graph events and inject custom named actions.

**Work**:

1. Add `graphshell.on(event_name, callback)` — stores `JsFunction` in a `ScriptCallbackRegistry`
   keyed by `GraphSemanticEvent` variant name.
2. When a `GraphSemanticEvent` fires in the main loop, `ScriptingContext::dispatch_event` serializes
   the event to a plain JS object and calls all registered callbacks.
3. Add `graphshell.registerAction(name, fn)` — inserts a JS-backed `ActionHandler` into
   `ActionRegistry`. Action names must match `user:*` namespace prefix to avoid collisions with
   system actions.
4. Handle script errors in callbacks: log to diagnostics channel, do not panic or crash the host.

**Done gate**:
- `graphshell.on("node:opened", e => dispatch("graph:node_pin_toggle", { nodeKey: e.nodeKey }))`
  correctly fires when a node is opened.
- `graphshell.registerAction("user:hello", () => dispatch("ui:notify", { message: "hello" }))`
  appears in command palette and executes.
- A script runtime error in a callback emits a diagnostics warning, does not crash.

### Phase 3 — ES Module Mods

**Goal**: JS files in a `mods/` directory are loaded as ES modules at startup; they can import from
`graphshell:core` and export an `activate()` function.

**Work**:

1. Implement `GraphshellModuleLoader` implementing `boa_engine::module::ModuleLoader`:
   - Resolves `graphshell:core` → synthetic module exporting `{ dispatch, on, registerAction, graph }`
   - Resolves `graphshell:nostr`, `graphshell:verse` as further synthetic modules when those
     subsystems are active
   - Resolves file paths under `mods/` directory relative to user data dir
   - All other specifiers → error
2. At startup, scan `{user_data}/mods/*.js`; for each, evaluate as an ES module and call `activate()`.
3. Integrate with `SUBSYSTEM_MODS`: JS mods are a new mod tier alongside native and WASM mods.
   A JS mod's `activate()` is its equivalent of `inventory::submit!`.
4. Mod manifest for JS mods: a `manifest.json` alongside `main.js` declaring `provides`/`requires`.

**Done gate**:
- A `mods/hello/main.js` with `import { on } from "graphshell:core"; export function activate() { on("node:opened", e => console.log(e.url)); }` loads and fires.
- Unknown `import` specifiers return an error, not a panic.
- Mod load failures emit diagnostics and do not prevent other mods from loading.

### Phase 4 — Async + Runtime Utilities (Deferred)

**Goal**: Scripts can use `setTimeout`, `setInterval`, `fetch` (optional), and `async`/`await`.

**Work**:

1. Add `boa_runtime` crate. Implement a `JobExecutor` that integrates with Graphshell's main loop tick.
2. On each frame (or at a lower-frequency tick), drain the JS job queue: `context.run_jobs()`.
3. `fetch` integration: optional, disabled by default, requires `network` capability in mod manifest.
4. `setTimeout`/`setInterval` mapped to the host tick scheduler.

**Done gate**:
- `await Promise.resolve(42)` works in a script.
- `setInterval(() => dispatch("user:tick", {}), 5000)` fires approximately every 5 seconds.
- `fetch` is absent unless the mod manifest declares `requires: ["network"]`.

---

## Scripting API Surface (Phase 1–2)

```typescript
// graphshell:core synthetic module surface

// Query the graph (returns copied snapshots)
declare const graph: {
    nodes(): GraphNode[];
    edges(): GraphEdge[];
    selected(): GraphNode[];
};

// Dispatch an action by name
declare function dispatch(action: string, payload?: Record<string, unknown>): void;

// Register an event listener
declare function on(event: string, callback: (event: Record<string, unknown>) => void): void;

// Register a custom named action (must use "user:" namespace prefix)
declare function registerAction(name: string, handler: () => void): void;

// Node snapshot shape
interface GraphNode {
    key: string;         // NodeKey as string
    title: string;
    url: string;
    kind: "web" | "file" | "nostr" | "rss" | "dir" | string;
    tags: string[];
    lastVisited?: number; // Unix ms
}

interface GraphEdge {
    source: string;      // NodeKey as string
    target: string;
    kind: string;
}
```

---

## Integration Points

| Graphshell concept | JS surface | Notes |
|---|---|---|
| `ActionRegistry` | `dispatch(name, payload)` | String key; `namespace:name` convention |
| `GraphSemanticEvent` | `on(event, cb)` | Serialized to plain object at dispatch boundary |
| `GraphIntent` | queued inside `dispatch()` | Scripts never call intent APIs directly |
| `Node` fields | `graph.nodes()` snapshot | Copied; no live borrow |
| Mod loader | `activate()` export | JS mod tier, alongside native/WASM |
| Diagnostics | script error → channel | `ChannelSeverity::Warn` for script runtime errors |

---

## Risks

**GC lifetime**: All Rust values exposed to JS are copies. If a `JsFunction` callback is stored,
the `Context` must outlive the callback registry. The `ScriptingContext` owns both and must be
dropped together.

**Cross-`Context` sharing**: `JsValue` and `JsObject` cannot cross `Context` boundaries. Each mod
`Context` is self-contained. Shared state between mods must go through Graphshell's own APIs
(`dispatch`, `on`), not through shared JS objects.

**Instruction budget**: Without a budget, a malicious or buggy script can spin forever. Enable
`instructions_remaining` for untrusted user scripts. Trusted built-in scripts (shipped with
Graphshell) may run without a budget.

**`boa_engine` compile time**: It is a large crate. Feature-gate it (`scripting` feature) so it
does not affect builds that do not need scripting.

---

## Acceptance Criteria

| Criterion | Verification |
|---|---|
| `graph.nodes()` returns correct count and fields | Unit test: mock graph with 3 nodes |
| `dispatch` routes to `ActionRegistry` | Integration test: dispatch calls action handler |
| Script runtime error does not panic host | Test: throw in script; host continues |
| `on(event, cb)` fires on correct event | Integration test: emit `GraphSemanticEvent`; verify callback called |
| `registerAction("user:x", fn)` appears in action registry | Integration test |
| Unknown module import errors cleanly | Test: `import "unknown:x"` → error, no panic |
| Mod `activate()` called at startup | Integration test: `mods/test.js` with side-effect via dispatch |
| `cargo build` without `scripting` feature is clean | CI compile matrix |
| Instruction budget halts infinite loop | Test: `while(true){}` → error within budget |

---

## Progress

### 2026-03-11

- Plan created. Research basis: Boa v0.21 embedding API, `js_class!` / `boa_interop`, synthetic
  module API, `boa_runtime` job executor, instruction budget via `ContextBuilder`.
- Phase 1–2 scoped as the minimal viable slice for user-authored automation.
- Phase 3 deferred but designed to slot into `SUBSYSTEM_MODS` JS mod tier.
- Phase 4 (async/runtime) deferred; requires main loop integration.
