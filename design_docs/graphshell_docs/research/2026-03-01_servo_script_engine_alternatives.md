# Servo Script Engine Alternatives — Research Note

**Date**: 2026-03-01
**Status**: Research / Long-horizon watch
**Author**: Arc
**Context**: Exploratory research into replacing SpiderMonkey (via mozjs) in Servo with a
native Rust JS engine, motivated by the same architectural pressure driving the
WebRender → wgpu migration: eliminating C++ dependencies in favor of Rust-native,
cross-platform alternatives.

**Related**: [`2026-03-01_webrender_wgpu_renderer_research.md`](2026-03-01_webrender_wgpu_renderer_research.md)

---

## 1. Current State: mozjs

Servo uses SpiderMonkey (Mozilla's C++ JS engine) via the `servo/mozjs` crate family:

- **`mozjs-sys`**: Raw bindgen-generated FFI bindings to SpiderMonkey's C++ API
- **`mozjs`**: Idiomatic Rust wrappers over `mozjs-sys`

Current tracking: SpiderMonkey esr140 branch. The binding layer exists almost entirely
to bridge the C++/Rust impedance mismatch — rooting API, compartments, GC roots,
`JSObject*` handles all require careful Rust-side management to avoid UB. Servo's
`script::dom::bindings::*` module generates the glue between Servo's Rust DOM objects
and SpiderMonkey's heap.

**The problem this creates**: SpiderMonkey is ~50 MB of C++ compiled as a Rust build
dependency. It requires `mozjs-sys` bindgen maintenance on every SpiderMonkey update,
has a rooting API that fights Rust's ownership model, and is the primary reason Servo
cannot be a pure-Rust build.

---

## 2. Nova: A Rust-Native JS Engine

**Repository**: `trynova/nova` (1.9k stars, 825 commits, 99.9% Rust)
**Status**: Work in progress — explicitly "very far from being suitable for use"
**Activity**: Active development, tracking Test262 compliance at trynova.dev

### Architecture

Nova uses **data-oriented design** rather than traditional pointer-chased OOP. ECMAScript
spec records become Rust structs stored in typed heap vectors, accessed by 32-bit index
rather than pointer. This is directly analogous to how an ECS game engine stores
components — and to how Graphshell's own intent model and node registry are structured.

Design inspiration: Kiesel and SerenityOS's LibJS. The index-based approach avoids
re-entrancy and aliasing issues that plague pointer-based JS heaps in Rust.

### Why this matters for a Servo integration

A `nova`-based `mozjs` replacement would be structurally cleaner than the current
`mozjs-sys` approach:

- No bindgen, no C++ FFI, no rooting API fighting Rust's borrow checker
- Nova's GC roots and object handles are native Rust types
- Servo's DOM binding glue (`script::dom::bindings::*`) could become direct
  Rust-to-Rust integration — typed Nova heap indices accessible directly from
  Servo's DOM code, no FFI boundary
- Pure Rust build: eliminates the C++ compiler dependency for the script layer

### Current gaps

- **No JIT compiler**: Nova is interpreter-only. Real web content requires a JIT
  for competitive performance on JS-heavy sites.
- **Test262 coverage**: Tracking compliance but not at web-compat parity with
  SpiderMonkey. Gap is significant for production use.
- **No DOM bindings**: The Servo DOM binding layer (`script::dom::bindings::*`)
  would need to be rewritten against Nova's API. This is the highest web-compat
  risk surface.

---

## 3. JIT Options for Nova

Nova's interpreter-only status is the primary practical blocker. The viable JIT
backends in the Rust ecosystem:

### Cranelift (recommended)

**Location**: Lives in the `bytecodealliance/wasmtime` repository
**Maturity**: Production-stable — used as Wasmtime's code generation backend and
as an experimental baseline JIT tier in SpiderMonkey itself
**Language**: Pure Rust
**Targets**: x86-64, aarch64, riscv64, s390x

Cranelift is the natural fit for Nova:

- Same ecosystem (Bytecode Alliance adjacent)
- Pure Rust — no C++ dep introduced
- Nova's index-based value representation maps cleanly onto Cranelift's SSA IR —
  values are already closer to SSA form than pointer-chased object graphs would be
- `cranelift-jit` crate provides runtime code generation and memory management

**What Cranelift provides vs. what still needs building**: Cranelift is a code
generation backend. The speculative optimization layer above it — type profiling,
inline caches, deoptimization bailouts back to the interpreter, GC safepoints,
on-stack replacement — is the bulk of a production JS JIT and must be built on top.
This is what V8's TurboFan, SpiderMonkey's IonMonkey, and JSC's DFG all are.
Cranelift replaces the machine-code emission tier, not the optimization pipeline.

### HolyJIT (rejected)

34 commits, last active 2018. Demonstrated Brainfuck compilation. Not viable.

### LLVM via inkwell

Works but introduces a C++ dependency (LLVM) — the same class of problem as
SpiderMonkey. Counterproductive for this goal.

---

## 4. ohim: Not a Component of This Stack

`wusyong/ohim` is a WIT-based DOM interface layer for Wasm components. It implements
a different scripting model: arbitrary languages compiled to Wasm components interact
with a DOM-like structure through WebAssembly Interface Types, rather than running JS.

ohim and Nova are **mutually exclusive design choices** for the same role:
- Nova path: JS remains the scripting language; engine is replaced
- ohim path: JS is not the scripting language; arbitrary Wasm components script the DOM

They cannot be combined. For a web browser embedding Servo with web-compat requirements,
Nova is the relevant path. ohim is relevant only for an application platform where you
control the scripting language (see §6 on Graphshell-specific applicability).

---

## 5. Servo's AI Policy

**Servo explicitly prohibits AI-generated contributions** (code, docs, PR text, issue
comments). From `book.servo.org/contributing/getting-started`:

> "Contributions must not include content generated by large language models or other
> probabilistic tools, including but not limited to Copilot or ChatGPT."

Rationale: maintainer burden from untested AI code, correctness/security concerns,
copyright issues from training data, ethical concerns.

**Nova has no stated AI policy** as of 2026-03-01. Contribution requirements focus on
spec alignment (ECMAScript spec as source of truth), Conventional Commits, and test262
coverage.

**Implication for this research avenue**: Any work intended to upstream into Servo
must be written independently without AI assistance. Work on Nova itself (which has no
policy) could proceed differently. The DOM binding layer (Servo-side) is where the
policy conflict would be sharpest.

---

## 6. Scope Assessment

### Comparison to WebRender → wgpu migration

| Work | Core challenge | Rough scale |
| --- | --- | --- |
| wgpu backend for WebRender | Reimplement GL Device/Renderer in wgpu; translate shaders | ~3–5 months (one person) |
| Nova + Cranelift baseline JIT | Type profiling, inline caches, deopt, GC safepoints on Cranelift | ~1–2 years additional |
| Nova + Servo DOM bindings | Replace `script::dom::bindings::*` glue | ~6–12 months; high web-compat risk |
| Full stack in Servo | End-to-end Nova replacing mozjs, real web content | ~2–4 years |

The script engine replacement is roughly **5–8× the scope** of the WebRender wgpu
backend work, with higher web-compat risk at every layer.

### Realistic first milestone

Rather than committing to the full stack, a bounded first milestone that produces
useful signal:

1. **Test262 gap audit**: Run Servo's test262 suite against Nova. Quantify the
   coverage gap. Identify which failing tests block the most real-world JS patterns.
2. **Cranelift baseline JIT prototype**: Implement a Cranelift-backed baseline JIT
   for a subset of Nova's bytecode opcodes (arithmetic, property access, function
   calls). Measure the speedup on a microbenchmark suite.
3. **DOM binding feasibility spike**: Write a minimal Rust binding from one Servo
   DOM interface (e.g., `Element`) to Nova's heap. Measure the ergonomic gap vs.
   the current mozjs binding approach.

This is ~2–3 months of part-time work and produces a go/no-go signal before deeper
investment.

---

## 7. Graphshell-Specific Applicability

For Graphshell's **web viewer nodes** (Servo-embedded), SpiderMonkey replacement
requires full web-compat — the full Nova + JIT + DOM bindings stack.

For Graphshell's **non-web viewer nodes and plugin model**, the ohim/Wasm-component
approach is independently interesting: a node whose behavior is a Wasm component
implementing a WIT interface (e.g., `viewer:render`, `viewer:handle-input`) is
architecturally consistent with the `ViewerRegistry` contract model. This does not
require Nova and is not blocked by SpiderMonkey.

These are two separate research avenues:
- **Nova track**: Replace SpiderMonkey for web content; long-horizon; upstream
  coordination required
- **Wasm plugin track**: Enable non-JS, non-web viewer behaviors via Wasmtime +
  WIT interfaces; independent of Nova; shorter horizon; no Servo dependency

---

## 8. Upstream Watch List

| Project | What to watch | Signal |
| --- | --- | --- |
| `trynova/nova` | Test262 pass rate progression; Cranelift JIT PRs; any Servo integration discussion | JIT PR opening = meaningful maturity signal |
| `bytecodealliance/wasmtime` | `cranelift-jit` API stability; any JS engine use cases in issues | Stable `cranelift-jit` = Nova JIT becomes more tractable |
| `wusyong/ohim` | WIT interface stabilization; multi-language guest examples; community growth | First non-toy guest language = plugin model becomes practical |
| `servo/servo` | Any issue/discussion about mozjs alternatives or script engine modularity | Opening a tracking issue = upstream intent exists |
