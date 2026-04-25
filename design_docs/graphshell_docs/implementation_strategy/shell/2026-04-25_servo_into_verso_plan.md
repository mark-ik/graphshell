<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Servo-into-Verso Lane (2026-04-25)

**Status**: Active execution plan — sliced S1 → S5
**Lane**: Move Servo's role behind a `verso/servo-engine` feature so
graphshell can be selectively compiled with or without Servo, mirroring
the 2026-04-25 wry-into-verso refactor.

**Related**:

- [VERSO_AS_PEER.md](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md) —
  the spec verso has been working toward; calls out Servo (as
  `viewer:webview`) and wry (as `viewer:wry`) as the two engines
  verso owns.
- [Iced host migration execution plan](2026-04-14_iced_host_migration_execution_plan.md) —
  Phase A2 (wry impl → verso) landed 2026-04-25, establishing the
  pattern this plan follows for Servo.
- [Iced content-surface scoping](2026-04-24_iced_content_surface_scoping.md) —
  §0 platform-tier framing: native = Servo, mobile/web =
  middlenet/wry. Servo gating is the prerequisite for non-native
  builds.

---

## 1. Goal

graphshell main becomes selectively compileable across three
independent engine axes:

| Cargo feature | Engine | Default? |
|---|---|---|
| `verso/servo-engine` (via graphshell `servo-engine`) | `viewer:webview` (Servo, texture mode) | yes (matches today) |
| `verso/wry-engine` (via graphshell `wry`) | `viewer:wry` (system WebView, overlay mode) | yes (matches today) |
| middlenet (always-on, lightweight) | `viewer:middlenet` lanes | yes |

The three are independent; **a build without `servo-engine` should
still compile and run** — chrome + middlenet + wry only. This unlocks:

- Mobile / WASM target paths (Servo isn't viable there)
- Faster CI iteration loops for chrome-focused work
- A test surface for graceful degradation when one engine is unavailable

---

## 2. Today's Reality

**Workspace crates** ✅ — all servo-free:

- `graphshell-core`, `graphshell-runtime`, `graph-canvas`, `graph-tree`,
  `middlenet-*`, `verso`, `verso-host`, `iced-middlenet-viewer`,
  `iced-graph-canvas-viewer`, `iced-wry-viewer`. None depend on the
  `servo` crate. Only docstring mentions of `servo::WebViewId` exist
  in `graphshell-core/src/content.rs`.

**Graphshell main** ❌ — Servo is unconditional:

- [Cargo.toml:142](../../../Cargo.toml) — `servo = { path = "...", default-features = false }`
  is a non-optional direct dep.
- ~75 `.rs` files in graphshell main directly `use servo::*` types.
- Many graphshell features cascade into Servo features
  (`gamepad = ["servo/gamepad"]`, `webgpu = ["servo/webgpu"]`, lines
  70–87 of Cargo.toml).

So gating Servo is the same magnitude of refactor as the iced host
migration was — broad mechanical sweep + a handful of architectural
decisions about what graphshell-without-Servo does.

---

## 3. Sliced Execution Plan

### S1 — `verso/servo-engine` feature scaffold

Mirror the wry-engine pattern from Phase A1 (2026-04-25):

- Add `servo-engine = ["dep:servo", ...]` to verso's `[features]`.
- Add `servo` Cargo dep to verso, `optional = true`, identical
  features to graphshell main today.
- New `verso::servo_engine` module re-exports the `servo` crate so
  downstream consumers depend on `verso/servo-engine` rather than
  `servo` directly.
- Verso compiles with the feature on; nothing in graphshell main
  changes yet.

**Receipt**: `cargo check -p verso --features servo-engine` clean.

### S2 — Graphshell main gates `servo::*` behind `servo-engine`

The bulk of the mechanical work, split into S2a (Cargo wiring,
done) and S2b (file-level sweep, in progress).

#### S2a — Cargo wiring (✅ landed 2026-04-25)

- Both `servo` deps in `Cargo.toml` (cross-platform line 142, Windows
  target-conditional line 263) marked `optional = true`.
- New feature `servo-engine = ["dep:servo", "verso/servo-engine"]`.
- All previously-forwarded servo features now require `servo-engine`
  before forwarding into the `servo` crate: `gamepad`, `webgpu`,
  `webxr`, `js_jit`, `crown`, `debugmozjs`, `jitspew`,
  `js_backtrace`, `vello`, `webgl_backtrace`, `tracing`,
  `media-gstreamer`, `native-bluetooth`, `profilemozjs`,
  `refcell_backtrace`.
- `default` feature set keeps `servo-engine` on, so existing builds
  are bit-identical to pre-S2a behavior.

**Receipt (S2a)**: `cargo check` (default features) clean — verified
2026-04-25, completes in 4m 17s, no regressions, two pre-existing
egui deprecation warnings only.

#### S2b — File-level sweep (🚧 module-level pass landed; body-level cascades remain)

**Module-level pass (2026-04-25 evening)**: gated Servo-coupled
submodules at their parent `mod.rs` level. The structural cuts:

- `shell/desktop/mod.rs`: gate `host` + `render_backend` (entirely
  Servo-coupled).
- `shell/desktop/lifecycle/mod.rs`: gate `lifecycle_reconcile`,
  `semantic_event_pipeline`, `webview_backpressure`,
  `webview_controller`, `webview_status_sync` (kept
  `lifecycle_intents` open).
- `shell/desktop/workbench/mod.rs`: gate `compositor_adapter`,
  `tile_render_pass`, `tile_runtime`, `tile_view_ops`, +
  consumer cascades: `graph_tree_dual_write`, `tile_behavior`,
  `tile_compositor`, `tile_invariants`, `tile_post_render`,
  `graph_tree_projection`, `semantic_tabs`, `ux_probes`.
- `shell/desktop/runtime/protocols/mod.rs`: gate `resource`,
  `router`, `servo`, `urlinfo` (kept `registry` open).
- `shell/desktop/runtime/registries/mod.rs`: gate
  `workbench_surface` + `workflow`; `ServoUrl` aliased to
  `url::Url` when servo-engine is off.
- `shell/desktop/ui/mod.rs`: gate `dialog`, `dialog_panels`,
  `egui_host_ports`, `gui`, `gui_frame`, `gui_orchestration`,
  `host_ports`, `nav_targeting`, `persistence_ops`,
  `thumbnail_pipeline`, `toolbar`, `toolbar_routing`,
  `workbench_host`, plus the consumer cascade `gui_state`,
  `finalize_actions`, `graph_search_flow`, `graph_search_ui`,
  `overview_plane`, `shell_layout_pass`. The in-tree iced
  launch path (`iced_app` etc.) now requires both `iced-host`
  AND `servo-engine` (see "iced launch path coupling" below).
- `lib.rs`: gate `mod render` (egui rendering layer);
  `mod parser`, `mod prefs`, `mod resources` behind
  servo-engine; provide a tiny stub `mod prefs { ... }` exposing
  `FileAccessPolicy` so `graph_app.rs` still compiles.
- `shell/desktop/runtime/cli.rs`: split into `main()` (host-neutral
  prelude + iced-host branch + no-servo exit warning) and
  `run_servo_launch_path()` (Servo+egui boot, gated).
- `shell/desktop/runtime/tracing.rs`: gate `from_winit` LogTarget
  impls (use host::event_loop::AppEvent).
- `shell/desktop/runtime/diagnostics.rs`: gate
  compositor_adapter import.
- `panic_hook.rs`: gate `servo::opts` use; SIGSEGV path becomes
  no-op without servo-engine.
- `graph_app.rs`: bug fix — disambiguate `use
  ::graph_cartography::*` (workspace crate, vs. local
  `mod graph_cartography` shim).

**Receipt (module-level pass)**:

- ✅ Default build (`cargo check`, servo-engine + wry on) clean,
  no regressions, ~18s incremental.
- ⏳ `cargo check --no-default-features --features wry`:
  **down to 75 errors** from 142, all body-level cascades.

**iced launch path coupling**: the in-tree iced launch path
(`shell::desktop::ui::iced_app`, `iced_host`, `iced_host_ports`,
etc.) consumes `host_ports::*` traits, `render_backend::Backend*`
types, `compositor_adapter`-re-exported `PortableRect`, and
`servo::WebViewId` directly. Achieving a true no-Servo iced
launch path requires extracting these to host-neutral locations
(graphshell-core for vocab; graphshell-runtime for traits) which
is **S3 architectural work**, not S2b mechanical sweep. For now,
the iced-host launch path inside graphshell main is gated with
`cfg(all(feature = "iced-host", feature = "servo-engine"))`.
The standalone iced demo crates (`crates/iced-{middlenet,graph-canvas,
wry}-viewer`) are unaffected — they remain fully no-Servo and
demonstrate the Phase B portable surface.

**Remaining 75 errors (deferred to S3 / S2c body-level pass)**:

- `shell/desktop/runtime/registries/mod.rs` body (~30 refs to
  `WorkbenchSurfaceRegistry`, `WorkflowRegistry`, etc.) — needs
  per-line gating in fields, function signatures, match arms.
- `shell/desktop/runtime/diagnostics.rs` body (`CompositorReplaySample`,
  `replay_samples_snapshot`).
- `app/persistence_facade.rs`, `app/settings_persistence.rs`
  (use `prefs::read_user_stylesheet_source`, `crate::render::*`,
  `ui::persistence_ops`).
- `app/workspace_state.rs` (uses `crate::render::*`).
- `registries/atomic/viewer.rs`, `registries/viewers/{directory,
  image_viewer, middlenet, plaintext}.rs` (5 files: use
  `workbench::tile_behavior`).
- `shell/desktop/workbench/ux_tree.rs` body (gated types in
  function signatures).
- `shell/desktop/ui/tag_panel.rs` (uses `crate::render`).

These are body-level uses of gated types. Gating them per-line is
mechanical but tedious; the proper fix is the S3 trait/vocab
extraction so the consumers don't depend on Servo-coupled modules
in the first place. **Recommendation: S2c body-level pass should
follow S3a (host_ports trait extraction) so the body-level work
isn't redone.**

#### S2b — Original file-level sweep target (✅ catalogued, ⏳ deferred)

`cargo check --no-default-features --features iced-host,wry` against
post-S2a tree surfaces **141 errors across 58 unique files**
(close to the pre-survey "~75 files" estimate). Inventory below
freezes the sweep target so future sessions can resume mid-sweep
without re-running the full check.

Categories (by gating strategy):

**Cluster A — gate-as-whole-module candidates** (Servo coupling
is structural; entire file is a Servo embedder/compositor adapter):

- `shell/desktop/host/*` (~17 files): `accelerated_gl_media`,
  `embedder`, `event_loop`, `geometry`, `headed_window` (+
  `clip_extraction`, `embedder_controls`, `input_routing`, `xr`),
  `headless_window`, `host_app`, `keyutils`, `running_app_state`
  (+ `webview_delegate`), `webdriver_runtime`, `window` (+
  `projection`, `runtime`).
- `shell/desktop/lifecycle/`: `lifecycle_reconcile`,
  `webview_backpressure`, `webview_controller`,
  `webview_status_sync`, `semantic_event_pipeline`.
- `shell/desktop/render_backend/`: `mod`, `shared_wgpu_context`,
  `wgpu_backend`.
- `shell/desktop/runtime/protocols/`: `resource`, `router`, `servo`,
  `urlinfo` (Servo URL-scheme protocol handlers).
- `shell/desktop/workbench/`: `compositor_adapter`,
  `tile_render_pass`, `tile_runtime`, `tile_view_ops`.
- `shell/desktop/ui/thumbnail_pipeline.rs` (Servo screenshot pipeline).

**Cluster B — partial-gate candidates** (file is host-neutral
overall but pulls Servo types at specific boundaries):

- `shell/desktop/runtime/registries/mod.rs:99` — single import.
- `shell/desktop/runtime/cli.rs:75` — single import in startup path.
- `shell/desktop/ui/dialog.rs`, `dialog_panels.rs`.
- `shell/desktop/ui/egui_host_ports.rs` — egui-host bridge
  (egui being retired; can stay servo-coupled at file level since
  iced-host is the post-retirement path).
- `shell/desktop/ui/iced_host_ports.rs`, `iced_host.rs` — iced-host
  bridge; **must work both with and without `servo-engine`** since
  this is the litmus-test path. Partial gating required.
- `shell/desktop/ui/host_ports.rs` — generic host bridge, partial.
- `shell/desktop/ui/gui.rs` (+ `accessibility`, `gui_frame`,
  `gui_orchestration`, `pre_frame_flow`, `semantic_lifecycle_flow`,
  `toolbar_phase_flow`) — egui-host main loop, similar reasoning to
  egui_host_ports.rs.
- `shell/desktop/ui/nav_targeting.rs`, `persistence_ops.rs`.
- Crate-root: `panic_hook.rs`, `prefs.rs`, `parser.rs`,
  `graph_resources.rs`.

**Sweep approach** (recommendation for next session):

1. Start with Cluster A: gate at the `mod.rs` declaration site
   (e.g., add `#[cfg(feature = "servo-engine")] pub mod host;`
   in `shell/desktop/mod.rs`). One edit per cluster, ~7 mod-level
   edits removes ~40 of 58 file errors at once.
2. Then Cluster B: edit-by-edit gating of import lines and call
   sites, focusing on iced_host* first since those are the path
   that must stay alive without `servo-engine`.
3. Imports become `verso::servo_engine::*` rather than `servo::*`,
   so the route through verso is consistent with the wry pattern
   established in Phase A2.

**Sweep blocker** (surfaced 2026-04-25): the workspace's parallel
`webrender-wgpu` checkout has a compile error in
`webrender_build/src/compiled_artifacts.rs:23` (`unresolved import
crate::glsl`). This blocks `cargo check` from progressing far
enough to surface graphshell-main errors when the cache is cold for
a given feature combo. Next session must either fix or stash that
checkout's working tree before the sweep can iterate. The 58-file
inventory above was captured before the blocker manifested, so the
sweep can proceed against this list without re-running the check.

**Receipt (S2b, deferred)**: `cargo check --no-default-features
--features iced-host,wry` clean (servo-engine off, iced + wry only)
once the sweep lands. `cargo check` with default features
(servo-engine on) must remain clean throughout.

### S3 — `graphshell-without-Servo` runtime architecture

What does the binary actually *do* when `servo-engine` is off?

Decision matrix:

| Subsystem | With `servo-engine` | Without `servo-engine` |
|---|---|---|
| Chrome + canvas + iced/egui host | works | works |
| middlenet content rendering | works | works |
| wry overlay (fullnet) | works (when `wry` feature on) | works (when `wry` feature on) |
| `viewer:webview` (Servo) | works | unavailable; routes to wry / middlenet / unsupported |
| Servo wgpu shared device | works | not constructed |
| Servo accesskit bridge | works | stubbed |
| Webview backpressure | works | reduced to wry-only path |
| Workbench compositor `ViewerSurfaceBacking::NativeRenderingContext` | works | always `None`; callback-fallback or wry-only |

**Key code areas** that need restructuring (not just gating):

1. **`HostCapabilities` defaults**: today `HostCapabilities::default`
   has `supports_servo: false` already (per
   [verso/src/lib.rs:58](../../../crates/verso/src/lib.rs)). The
   live wiring sets it to `true` when graphshell boots Servo.
   Without `servo-engine`, the live wiring stays `false` and verso's
   dispatch routes accordingly. **No breaking change here**.
2. **`ViewerSurfaceRegistry::backing` typing**: currently can be
   `NativeRenderingContext(Rc<dyn RenderingContextCore>)` where
   `RenderingContextCore` is a trait Servo provides. Without
   Servo, the trait still exists (we own the trait? need to verify)
   but no impls are imported. The variant becomes uninhabitable —
   but the enum compiles fine; just no producers.
3. **`shared_wgpu_context.rs`**: holds `servo::wgpu::Device` +
   `servo::wgpu::Queue`. Either gate the whole file behind
   `servo-engine`, or extract the wgpu types into a `verso::wgpu`
   re-export so `servo-engine`-off builds use a stub or the
   `wgpu` crate directly.

For S3 first pass: **gate liberally, document the architectural
follow-ons, don't refactor the trait surface**. Goal is a working
no-servo build, not the cleanest possible no-servo architecture.
The trait extraction (e.g., move `RenderingContextCore` into verso
as a portable trait) is a separable later slice.

**Receipt**: with `servo-engine` off, the binary opens an iced
window with chrome + canvas + wry overlay. Submitting an `https://`
URL falls back to wry (or returns "engine not available" via
verso's existing dispatch) instead of attempting Servo.

### S4 — Startup path gating

- `cli.rs::main()` currently initializes Servo unconditionally.
  Wrap Servo init in `#[cfg(feature = "servo-engine")]`; provide a
  no-servo branch that proceeds to graphshell startup without
  Servo.
- The iced-host launch path (already gated on `iced-host` feature)
  is independent; it'll work with or without `servo-engine`.
- Egui-host launch path needs the same conditional Servo init
  treatment, since today it expects Servo to exist.

**Receipt**: `--no-default-features --features iced-host` runs
`graphshell --iced` against a Servo-free binary. `--no-default-features
--features iced-host,wry` adds wry overlays.

### S5 — Build matrix + documentation

- Add a CI build configuration (or document one if CI isn't
  automated yet): `cargo check --no-default-features --features
  iced-host,wry` should be part of the check matrix to prevent
  regressions where a non-`servo-engine` change breaks the
  no-Servo build.
- Update [PROJECT_DESCRIPTION.md](../../../PROJECT_DESCRIPTION.md)'s
  rendering-architecture paragraph to reflect tri-engine selectivity.
- Update [VERSO_AS_PEER.md](../../../verso_docs/technical_architecture/VERSO_AS_PEER.md)
  to note Servo + wry are both behind verso features, not just
  registered viewers.
- Update the iced-host migration plan with a Phase A2 sibling
  entry for Servo (S2/S3 receipts).

**Receipt**: documentation changes land in the same session; the
"three independent engine axes" picture is canonically captured.

---

## 4. Open Architectural Questions (informs S3)

1. **Does `ViewerSurfaceBacking::NativeRenderingContext` survive
   without Servo?** If we want `servo-engine`-off builds to still
   support some "native rendering context" (e.g., a future
   non-Servo wgpu producer), the `RenderingContextCore` trait
   needs to live in verso (not be re-exported from servo). For
   first-pass, gate the variant entirely — re-introduce when a
   non-Servo producer arrives.
2. **Webview lifecycle vocabulary**: `WebviewBackpressureState` and
   the `webview_backpressure` module assume Servo's webview
   creation cadence. Need to identify what's Servo-specific vs.
   what's host-neutral state machine. Probably gate the whole
   module behind `servo-engine` for first pass.
3. **Accesskit bridge**: Servo provides accesskit tree updates per
   webview. Without Servo, the bridge has no producers but still
   has consumers (chrome accesskit). Stub the producer side.
4. **Shared wgpu device acquisition**: today Servo provides the
   wgpu device that webrender + the compositor share. Without
   Servo, the chrome's iced renderer is the only wgpu consumer
   (plus future iced-graph-canvas-viewer with WebRender — but
   that's webrender-wgpu's wgpu, not Servo's). Need to identify
   who owns the device in non-Servo builds.

---

## 5. Receipts at a glance

Status as of 2026-04-25:

- ✅ `cargo check -p verso --features servo-engine` — clean (S1).
- ✅ `cargo check` (default features) — clean (S2a; servo-engine + wry
  on, no regressions, 4m 17s).
- 🚧 `cargo check --no-default-features --features wry` — 75 errors
  remaining (down from 142 after S2b module-level pass; remaining
  75 are body-level cascades pending S3a trait extraction).
- ⏳ `cargo check --no-default-features --features iced-host,wry` —
  same 75 errors plus iced launch path coupling (currently gated
  on servo-engine; decoupling is S3 work).
- ⏳ `cargo check --no-default-features --features iced-host` — not
  yet attempted (depends on S2b + S3).
- ⏳ `cargo check --no-default-features --features
  servo-engine,iced-host` — not yet attempted (depends on S2b).
- All four matrix entries to be documented post-S5.

---

## 6. Execution log

- **2026-04-25 (S1)**: Added `servo-engine` feature + optional `servo`
  dep + `verso::servo_engine` re-export module to `crates/verso`.
  Verso compiles standalone with the feature on.
- **2026-04-25 (S2a)**: Made graphshell main's `servo` deps (both
  cross-platform line 142 and Windows-target-specific line 263)
  optional. Added `servo-engine = ["dep:servo",
  "verso/servo-engine"]`. Cascaded all 16 servo-forwarded features
  to require `servo-engine` first. Default feature set keeps
  `servo-engine` on; default build verified clean (4m 17s).
- **2026-04-25 (S2b survey)**: Surveyed `cargo check
  --no-default-features --features iced-host,wry` errors;
  catalogued 141 errors across 58 files into Cluster A (whole-module
  gate candidates) and Cluster B (partial-gate candidates). See §3
  S2b for the full inventory. Discovered concurrent
  `webrender-wgpu` working-tree breakage that blocks further
  cargo-check iteration; flagged as sweep prerequisite.
- **2026-04-25 (S2b module-level pass)**: webrender-wgpu blocker
  cleared; ran the module-level gating pass across `lib.rs`,
  `shell/desktop/{mod,lifecycle/mod,workbench/mod,ui/mod,
  runtime/{mod,cli,tracing,diagnostics,registries/mod,protocols/mod}}`,
  plus `panic_hook.rs` and `graph_app.rs`. Down from 142 → 75
  errors against `cargo check --no-default-features --features
  wry`; default build (servo-engine on) remains clean. Remaining
  75 are body-level cascades that S3a (host_ports trait
  extraction) should supersede; deferred to S2c post-S3.

## 7. Bottom line

This lane lands the architectural claim that's been implicit in the
recent refactors: **graphshell is a chrome + spatial canvas; the
content engines are pluggable**. Phase A2 proved verso can own a
heavy engine (wry); this lane proves it can own all of them.

Estimated effort: 3–5 sessions of focused work. S1 + S2a landed
2026-04-25; S2b survey complete; S2b sweep + S3 + S4 + S5 remain.
S2b is the mechanical bulk (58 files, see §3 inventory), S3 is
design + impl (probably split into S3a/S3b if it grows), S4 + S5
are short.
