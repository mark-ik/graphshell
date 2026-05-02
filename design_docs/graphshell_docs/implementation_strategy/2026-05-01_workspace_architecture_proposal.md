# Workspace Architecture Proposal — decompose the root crate

**Status**: proposal, awaiting approval before extraction work begins.
**Author**: Slice 49 follow-on after the gophermap proof-of-concept.
**Audience**: anyone touching `Cargo.toml`, root-level Rust files, or the
`shell::desktop::*` tree.

---

## 1. Why this exists

The Graphshell workspace has 17 published member crates today, but the
root `graphshell` crate is itself a workspace-scale god object: 11
top-level subsystem directories (`app/`, `domain/`, `graph/`, `input/`,
`model/`, `mods/`, `platform/`, `registries/`, `render/`, `services/`,
`tests/`) plus the entire `shell/desktop/*` tree all compile as part of
one Cargo target. The §5 anti-patterns in
[`shell/2026-04-28_iced_jump_ship_plan.md`](shell/2026-04-28_iced_jump_ship_plan.md)
forbid new ≥600 LOC files and ≥6-responsibility structs, but they do not
forbid a 200K+ LOC workspace member, and the result is exactly the
file-level discipline being undone at workspace level.

The cost is concrete:

- **Build coupling.** Any change to any subsystem in the root crate
  rebuilds every other subsystem that compiles into the same target.
- **No conceptual boundary enforcement.** A `registries/` file can pull
  freely on `services/` internals, on `app/` private state, on
  `shell/desktop/runtime/` internals, etc. There is no compiler-enforced
  separation between layers.
- **Test isolation impossible.** A test that needs a registry cannot
  import only the registry — it must drag in the entire root crate.
- **Two registries directories.** `registries/` (top-level, 30 files,
  10,695 LOC) and `shell/desktop/runtime/registries/` (37 files, 32,538
  LOC) both exist. They overlap conceptually but are separated only by
  directory placement; the code review burden of reconciling them is
  invisible because they're in the same crate.

The Slice 49 gophermap-extraction proof-of-concept showed that
per-protocol crate extraction is mechanical when the protocol surface is
well-defined. Per-subsystem crate extraction at the root-crate level is
the same shape of work, scaled up.

---

## 2. Inventory — current sprawl by LOC

Captured 2026-05-01.

### 2.1 Root crate top-level directories

| Directory | LOC | Files | Conceptual content |
|---|---:|---:|---|
| `app/` | 25,852 | 35 | `GraphBrowserApp` god object + workspace routing + persistence facade + intent dispatch + agents |
| `registries/` (top-level) | 10,695 | 30 | atomic / domain / infrastructure / viewers — registry primitives + viewer-content registries |
| `render/` | 19,391 | 13 | Renderer host, surface composition, paint extraction (vendored from servoshell era) |
| `services/` | 6,664 | 11 | facts / import / persistence / query / search — app-level service implementations |
| `mods/` | 4,774 | 21 | native + wasm — mod runtime |
| `graph/` | 3,473 | 5 | frame_affinity / graphlet / physics / scene_runtime — running graph state in the binary |
| `model/` | 1,442 | 5 | archive + graph — domain model types |
| `input/` | 1,351 | 1 | Input dispatch |
| `domain/` | 31 | 1 | (essentially empty stub) |
| `platform/` | 45 | 1 | (essentially empty stub) |

### 2.2 `shell/desktop/` sub-tree

| Directory | LOC | Files | Conceptual content |
|---|---:|---:|---|
| `shell/desktop/ui/` | 53,777 | 99 | iced + egui hosts, both presents; widgets; panes; modals |
| `shell/desktop/runtime/` | 32,538 | 37 | iced-host runtime composition (registries, protocols, diagnostics, signal routing, snapshots) |
| `shell/desktop/workbench/` | 24,909 | 32 | egui workbench glue (frozen per S1) |
| `shell/desktop/host/` | 6,918 | 19 | egui-host residue (S6 deletion target) |
| `shell/desktop/lifecycle/` | 3,720 | 10 | webview lifecycle |
| `shell/desktop/tests/` | 5,406 | 20 | desktop integration tests |
| `shell/desktop/render_backend/` | 536 | 4 | wgpu renderer adapter |

### 2.3 Two duplicate `registries/` trees

- `registries/atomic/` — diagnostics, knowledge, lens, protocol,
  protocol_provider, viewer, viewer_provider
- `registries/domain/` — layout, presentation
- `registries/infrastructure/` — mod_activation, mod_loader
- `registries/viewers/` — audio, directory, image, middlenet, pdf,
  plaintext

vs.

- `shell/desktop/runtime/registries/` — action, agent, canvas, identity,
  index, input, knowledge, layout, lens, nostr_core, physics_profile,
  protocol, renderer, signal_routing, theme, workbench_surface, workflow

`knowledge`, `lens`, `protocol`, `layout` appear in both. The boundary
between them is not principled.

---

## 3. Conceptual architecture

The proposal organises Graphshell into five conceptual layers, each
with its own crate group. Numbers in parentheses are best-guess LOC for
the resulting workspace members.

### Layer A — Portable primitives (already extracted)

These crates already exist and stay as-is. They define the boundary
between graph truth and host:

- `crates/graphshell-core` — graph truth, ids, portable shell state,
  geometry, events, sanctioned writes, ux_observability, ux_probes
- `crates/graphshell-runtime` — host-neutral `runtime.tick()`,
  command dispatch, workbench/viewer/navigator services
- `crates/graph-canvas`, `crates/graph-tree`, `crates/graph-cartography`,
  `crates/graph-memory` — graph domain crates
- `crates/middlenet-core`, `crates/middlenet-render` — middlenet
  primitives + renderer
- `crates/middlenet-gopher` (Slice 49b) — protocol crates split per
  the per-protocol plan
- `crates/graphshell-comms` — comms primitives (newly added to
  workspace.members per Slice 49a)

### Layer B — Register layer (NEW conceptual home)

Every registry — action, agent, canvas, identity, knowledge, layout,
lens, nostr_core, physics_profile, protocol, renderer, theme, workflow,
viewer, etc. — moves into individual crates under `crates/register/`.

Why per-registry crates instead of one `graphshell-registries`:

- Each registry has a stable conceptual surface (a key namespace, an
  entry trait, a lookup API). The trait surface fits naturally as a
  crate boundary.
- Per-registry crates make it explicit which registry depends on which.
  Today, every registry can pull on every other; with crate borders, a
  cycle becomes a visible error.
- A slim Graphshell build can omit registries it doesn't need (e.g.,
  the `nostr_core` registry only matters when nostr is active).
- The duplication between top-level `registries/` and
  `shell/desktop/runtime/registries/` becomes unsupportable: each
  registry has exactly one crate.

Proposed extraction list (one crate per registry):

| Crate | Source today |
|---|---|
| `crates/register/register-action` | `shell/desktop/runtime/registries/action.rs` |
| `crates/register/register-agent` | `shell/desktop/runtime/registries/agent.rs` |
| `crates/register/register-canvas` | `shell/desktop/runtime/registries/canvas.rs` |
| `crates/register/register-identity` | `shell/desktop/runtime/registries/identity.rs` |
| `crates/register/register-knowledge` | `shell/desktop/runtime/registries/knowledge.rs` + `registries/atomic/knowledge.rs` (merged) |
| `crates/register/register-layout` | `registries/domain/layout/` + `shell/desktop/runtime/registries/layout.rs` (merged) |
| `crates/register/register-lens` | `registries/atomic/lens/` + `shell/desktop/runtime/registries/lens.rs` (merged) |
| `crates/register/register-nostr` | `shell/desktop/runtime/registries/nostr_core.rs` |
| `crates/register/register-physics-profile` | `shell/desktop/runtime/registries/physics_profile.rs` |
| `crates/register/register-protocol` | `registries/atomic/protocol*.rs` + `shell/desktop/runtime/registries/protocol.rs` (merged) |
| `crates/register/register-renderer` | `shell/desktop/runtime/registries/renderer.rs` |
| `crates/register/register-theme` | `shell/desktop/runtime/registries/theme.rs` |
| `crates/register/register-viewer` | `registries/viewers/*` + viewer registry primitives |
| `crates/register/register-workbench-surface` | `shell/desktop/runtime/registries/workbench_surface*` |
| `crates/register/register-workflow` | `shell/desktop/runtime/registries/workflow.rs` |
| `crates/register/register-input` | `shell/desktop/runtime/registries/input.rs` |
| `crates/register/register-index` | `shell/desktop/runtime/registries/index.rs` |
| `crates/register/register-signal-routing` | `shell/desktop/runtime/registries/signal_routing.rs` (the signal-bus seam) |
| `crates/register/register-mod-loader` | `registries/infrastructure/mod_*.rs` |
| `crates/register/register-diagnostics` | `registries/atomic/diagnostics.rs` |

Open question: directory naming. `register/` (singular, per the user's
phrasing) versus `registers/` versus `registry/`. Recommend `register/`
to match the conversational shorthand.

### Layer C — App services (NEW)

The current `services/` directory and parts of `app/` collapse into a
small family of service crates:

| Crate | Source today |
|---|---|
| `crates/graphshell-services-facts` | `services/facts/` |
| `crates/graphshell-services-import` | `services/import/` |
| `crates/graphshell-services-persistence` | `services/persistence/` |
| `crates/graphshell-services-query` | `services/query/` |
| `crates/graphshell-services-search` | `services/search.rs` |

Or, if these turn out to share enough surface, a single
`crates/graphshell-services` with one module per service. Decide once
the first one has been extracted and the import fan-out is visible.

### Layer D — Signal bus (NEW, called out as its own thing)

The user explicitly named this. There are at least two signal-routing
seams in the tree today:

- `shell/desktop/runtime/registry_signal_router.rs`
- `shell/desktop/runtime/registries/signal_routing.rs`

These collapse into `crates/graphshell-signal-bus` (or
`crates/register/register-signal-routing` per Layer B), which both the
runtime and the host depend on. Pulling it out makes the
publish-subscribe contract a real interface instead of a convention.

### Layer E — Hosts and host-bundled glue

The iced-host runtime composition that lives under
`shell/desktop/runtime/` today is *not* the portable runtime — it's the
iced bring-up of the runtime. It moves into `crates/hosts/iced-host`
(with `crates/hosts/` as the new directory home for all host crates):

| Crate | Source today |
|---|---|
| `crates/hosts/iced-host` | `shell/desktop/ui/iced_*` + `shell/desktop/ui/iced_host_ports.rs` + `shell/desktop/ui/iced_host.rs` |
| `crates/hosts/iced-graph-canvas-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-middlenet-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-wry-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-widgets` | rename of `crates/graphshell-iced-widgets` to fit the `hosts/` family naming |
| `crates/hosts/egui-host` | (S6 deletion target — the existing `shell::desktop::ui::gui*` + `shell::desktop::workbench` + `shell::desktop::host`) |

The `shell::desktop::lifecycle/` (webview lifecycle) folds into either
`crates/hosts/iced-host` or — if it turns out to be host-neutral — into
a new `crates/graphshell-webview-lifecycle`.

### Layer F — Verso / Servo fork (already extracted)

`crates/verso` and `crates/verso-host` stay as-is. middlenet does NOT
move under verso — middlenet's whole point is portability outside
Servo, and grouping its directory under `verso/` would send the wrong
signal.

### What stays in the binary root

After all extractions, the root `graphshell` crate contains only what a
Cargo binary needs:

- `main.rs`, `build.rs`, `lib.rs` (composition root only)
- `panic_hook.rs`, `crash_handler.rs`, `backtrace.rs` (process-startup
  glue)
- `parser.rs`, `graph_app.rs`, `graph_resources.rs`, `graph_app_tests.rs`
  (composition glue — likely thinned dramatically once dependencies
  move out)

Everything else moves to one of the layers above. The binary's job
becomes: select default host, wire features, run.

---

## 4. Why per-registry crates (the headline reorg)

The biggest single move in this proposal is splitting the registries.
There are two principled options:

**Option A**: one crate, one file per registry (today's directory
layout, just promoted to its own crate). Cheap, low risk, but does not
solve the duplication-between-trees problem and does not let downstream
consumers depend on individual registries.

**Option B (recommended)**: one crate per registry, organised under
`crates/register/`. Every registry's trait + impl + tests live together;
inter-registry dependencies become explicit Cargo edges; each registry
is independently testable; slim Graphshell builds can omit registries
they don't use; the two-trees duplication is forced into reconciliation
by the act of extracting (you cannot have two crates with the same
name).

The cost of Option B is per-crate boilerplate (one Cargo.toml per
registry) and the upfront work of resolving the two-trees overlap. Both
are one-time costs.

Recommendation: Option B.

---

## 5. Sequencing

Big-bang extractions are bad. The proposed order:

### Phase 1 — Trivial wins (Slice 50)

- Reconcile the two `registries/` trees as a *documentation* exercise.
  Identify which registry concept lives in which file across both
  trees; update the doc with a definitive list.
- Pick one isolated registry (probably `register-theme` or
  `register-physics-profile` — both are small and have few external
  callers) and extract it as the proof-of-concept. The middlenet-gopher
  extraction (Slice 49b) is the template.

### Phase 2 — Signal bus extraction (Slice 51)

- Pull the signal-routing surface out as `crates/graphshell-signal-bus`
  (or `crates/register/register-signal-routing` if it fits cleanly
  there). This is high leverage because the rest of the registry
  extractions can depend on it cleanly instead of reaching into
  internals.

### Phase 3 — Service extraction (Slices 52-53)

- Extract `crates/graphshell-services-search` first (the smallest single
  file). Then `import`, `persistence`, `query`, `facts` as appetite
  allows.

### Phase 4 — Registry sweep (Slices 54-60+)

- Extract the remaining registries one at a time, using whatever
  per-registry crate naming the Phase 1 proof-of-concept settled on.
  Order: smallest first, dependency leaves before dependency roots.

### Phase 5 — `app/` decomposition (Slices 61+)

- Decompose `app/` (the largest remaining root-crate subsystem) into
  conceptual modules:
  - `crates/graphshell-graph-app` — graph_*.rs, arrangement_graph_bridge,
    canvas_scene
  - `crates/graphshell-workspace-routing` — routing, workspace_*.rs,
    workbench_*.rs
  - `crates/graphshell-persistence` — persistence_facade,
    settings_persistence, startup_persistence, storage_interop
  - `crates/graphshell-intent-system` — intent_phases, intents,
    focus_selection, selection
  - The rest stays in the binary as composition glue, or folds into
    existing crates.

### Phase 6 — Host moves (Slices 70+)

- Move `crates/iced-*-viewer` and `crates/graphshell-iced-widgets` into
  `crates/hosts/`. Cosmetic but clarifies the Cargo.toml at a glance.
- Extract `shell/desktop/ui/iced_*` into `crates/hosts/iced-host`.
- The egui-host residue (`shell/desktop/ui/gui*`,
  `shell/desktop/workbench/`, `shell/desktop/host/`) is the S6 deletion
  target; do not extract it as its own crate.

### Phase 7 — `render/` decision (defer)

The 19,391-LOC `render/` directory is mostly servoshell/Servo-renderer
glue that's slated for either retirement (S6) or significant rework as
part of the renderer-and-host-refactor plan
([`aspect_render/2026-04-30_renderer_and_host_refactor_plan.md`](aspect_render/2026-04-30_renderer_and_host_refactor_plan.md)).
Defer extraction decisions for `render/` until the refactor plan picks
its target shape.

---

## 6. Risks and tradeoffs

- **Cargo build-graph churn.** Every extraction churns
  `Cargo.toml` + `Cargo.lock`, requires `cargo build` to repopulate
  target/, and may surface latent visibility issues. Mitigation:
  one extraction per slice; verify build + tests after each.
- **IDE re-indexing.** rust-analyzer re-indexes on workspace changes.
  Annoying for the reviewer, not blocking.
- **Per-crate boilerplate.** Each new crate adds one Cargo.toml,
  one src/lib.rs (often just re-exports), and an entry in
  workspace.members. Worth it for the ones that earn a crate; not
  worth it for tiny ad-hoc helpers.
- **Public API surface accidentally widens.** Code that was
  `pub(crate)` in the root crate becomes `pub` in the extracted crate
  unless deliberately gated. Mitigation: extraction PRs explicitly call
  out which items become public; add `#[doc(hidden)]` for anything that
  shouldn't be a stable API.
- **Naming scheme bikeshed.** `register/` vs `registers/` vs
  `registry/`; `graphshell-services-*` vs single
  `graphshell-services` with modules; `hosts/` vs no nesting. Decide
  once at Phase 1 and stay consistent.

---

## 7. Decision points the user should weigh in on

Before extraction work starts, three questions need answers:

1. **Directory naming.** `register/` (singular, conversational) or
   `registers/` (plural, conventional Rust pluralization) or
   `registry/` (consistent with the existing
   `RegistrationRegister` / `register*.rs` filenames inside the
   runtime)? The proposal recommends `register/`.
2. **One service crate or many?** `graphshell-services` with one module
   per service, or `graphshell-services-{facts,import,persistence,
   query,search}` as separate crates? The proposal recommends starting
   with one crate (search) extracted as its own, decide after.
3. **Target shape for `app/`.** The decomposition in Phase 5 lists
   five candidate crates from one directory. Is that the right cut?
   Or should `app/` collapse into fewer (e.g., one
   `graphshell-app` crate with strict module discipline)? The
   proposal recommends the multi-crate cut because it forces the
   conceptual boundaries to be real, but reasonable people can prefer
   the one-crate version.

---

## 8. What this document is not

- Not a slice plan with acceptance criteria. The slice plan lives in
  the relevant per-slice plan documents per
  [DOC_POLICY §9](../DOC_POLICY.md).
- Not authoritative on the `render/` directory's future — that lives
  with the renderer-and-host-refactor plan.
- Not a refactor of the iced jump-ship plan. The `app/` decomposition
  in §5 Phase 5 is parallel to S5 / S6 of that plan, not a
  replacement.
