# UX Issue Domain Map

**Date**: 2026-02-28  
**Status**: Operational mapping  
**Purpose**: Map the current open GitHub issues into the reduced six-spec UX structure so UX work can be planned without violating architectural boundaries.

**Primary reference**:

- `2026-02-28_ux_contract_register.md`

---

## 1. How To Read This Map

This document does three things:

1. maps current open issues into the six canonical UX specs,
2. distinguishes direct UX work from enabling architecture work,
3. identifies which issues are outside primary UX-contract scope.

An issue can appear in one of three statuses here:

- **Primary**: directly implements or fixes a user-facing UX contract
- **Enabling**: architecture/refactor work needed so a UX contract can be implemented correctly
- **Support**: supporting docs, diagnostics, or policy work

This is an operational planning map, not a permanent taxonomy.

---

## 2. Canonical Spec Map

### 2.1 Graph / Node / Edge

**Primary**

- `#173` graph canvas pan/wheel zoom/zoom commands no-op across contexts
- `#104` zoom-to-fit regression repro + fix + regression test
- `#101` camera commands target global pending state in multi-view
- `#185` define single-select and additive-select contract for the graph pane
- `#102` lasso selection metadata ID hardcoded after per-view metadata split

**Enabling**

- `#103` input ownership audit (graph focus vs hidden/active node-pane event consumption)
- `#105` control responsiveness diagnostics (dropped/blocked camera-input command counters)

**Deferred migration overlap**

- `#181` extract `GraphCanvasBackend` seams
- `#182` replace `egui_graphs` with a minimal Graphshell custom canvas

### 2.2 Workbench / Frame / Tile

**Primary**

- `#174` new tile/pane focus activation race leaves viewport blank until follow-up focus changes
- `#175` web content new-tile/context-menu path bypasses Graphshell node/tile creation semantics
- `#186` define deterministic open-selected-node contract for pane/tab/split routing
- `#187` define deterministic close-pane focus return contract

**Execution note (2026-03-01)**

- `#175` is implemented: content-originating child-webview opens now route through Graphshell frame routing semantics (`OpenNodeFrameRouted`) rather than reducer-side direct selection mutation.
- Diagnostics receipt: `design_docs/archive_docs/checkpoint_2026-03-01/2026-03-01_issue_175_content_open_routing_receipt.md`

**Enabling**

- `#118` split `gui.rs` responsibilities, reduce `RunningAppState` coupling
- `#119` split `gui_frame.rs` responsibilities

**Support**

- `#100` tile focus ring renders behind Servo document view during rearrange

### 2.3 Command Surfaces

**Primary**

- `#176` unify F2 command palette and right-click contextual command surface across canvas/tiles/workbench
- `#106` command palette global trigger parity (keyboard + non-node pointer path)
- `#107` command palette semantics cleanup
- `#108` unify command surface dispatch boundary (radial/palette -> one execution path)
- `#178` omnibar node-search Enter action retains input focus for result iteration

**Support**

- `#89` control-ui-settings hub

### 2.4 Focus and Region Navigation

**Primary**

- `#140` F6 region cycle and focus return-path regression tests
- `#174` new tile/pane focus activation race leaves viewport blank until follow-up focus changes
- `#187` define deterministic close-pane focus return contract
- `#189` define settings and history surface return-path contract

**Enabling**

- `#103` input ownership audit (shared with graph interaction correctness)

**Support**

- `#138` replace Accessibility Inspector tool-pane placeholder with functional scaffold
- `#139` WebView bridge health summary + diagnostics pane surfacing
- `#141` Graph Reader phase-1 scaffold entry point
- `#95` accessibility hub

### 2.5 Viewer Presentation and Fallback

**Primary**

- `#162` overlay affordance policy per `TileRenderMode`
- `#188` make viewer fallback and degraded states explicit and reasoned
- `#109` replace settings tool-pane placeholder with page-backed scaffold
- `#111` define tool-pane vs node-pane rendering contract
- `#112` viewer is selectable but non-embedded in node pane

**Support**

- `#155` viewer capability + embedding claims audit receipt
- `#159` placeholder inventory receipt
- `#92` viewer-platform hub

**Deferred migration overlap**

- `#169` viewer backend hot-swap intent and state contract
- `#182` custom canvas replacement

### 2.6 Settings and Control Surfaces

**Primary**

- `#109` replace settings tool-pane placeholder with page-backed scaffold
- `#110` settings information architecture skeleton
- `#177` add theme mode toggle
- `#189` define settings and history surface return-path contract

**Support**

- `#89` control-ui-settings hub
- `#136` orphan channel surfacing and pane visibility
- `#134` AnalyzerRegistry scaffold
- `#135` in-pane TestHarness scaffold
- `#137` startup structural analyzer integration
- `#142` persistence diagnostics channels + health summary
- `#94` diagnostics hub

---

## 3. Migration-Deferred UX Work

These issues are part of the future UX evolution but are explicitly deferred behind the current app-readiness focus and the later migration plan.

### Deferred backend/canvas migration issues

- `#179` legacy umbrella issue for the combined `wgpu` migration (superseded by the split below; do not execute as one combined slice)
- `#180` prove runtime-viewer GL -> `wgpu` bridge
- `#181` extract `GraphCanvasBackend` seams
- `#182` replace `egui_graphs` with a minimal Graphshell custom canvas (deferred unless `egui_graphs` becomes a proven bottleneck)
- `#183` replace `egui_glow` with `egui_wgpu` (blocked by `lane:embedder-debt` / `#90` until embedder decomposition makes the backend cut technically viable)
- `#184` stabilize and optimize the `egui_wgpu` backend landing after `#183` (do not couple this to custom-canvas work by default)

These matter strategically, but they are not current UX contract slices to execute before the app is usable.

---

## 4. Current Priority Lens

If Graphshell wants to improve app UX now while respecting the architecture, the highest-value current UX-contract work is:

1. **Graph / Node / Edge**
   - `#173`, `#104`, `#101`, `#103`, `#185`, `#102`
2. **Workbench / Frame / Tile**
   - `#174`, `#175`, `#186`, `#187`, `#118`, `#119`
3. **Command Surfaces**
   - `#176`, `#108`, `#106`, `#107`, `#178`
4. **Focus and Region Navigation**
   - `#140`, `#174`, `#187`, `#189`
5. **Settings and Control Surfaces**
   - `#109`, `#110`, `#177`, `#189`
6. **Viewer Presentation and Fallback**
   - `#162`, `#188`, `#111`, `#112`

This cluster best improves practical app usability without forcing premature backend migration work.

---

## 5. Issues Outside Primary UX-Contract Scope

The following open issues are important, but they are not primarily UX-contract slices. They should not be forced into the UX register as if they were direct user-behavior work.

### Architecture / renderer / platform support

- `#160`, `#166`, `#167`, `#168`, `#169`, `#171`
- `#90`, `#91`, `#92`, `#94`, `#97`, `#99`

### Security / storage / subsystem hardening

- `#96` hub
- `#142`, `#143`, `#144`, `#145`

### Test-infra scaling

- `#97` hub
- `#146`, `#147`, `#148`, `#149`

### Knowledge-capture / future feature work

- `#98` hub
- `#150`, `#151`, `#152`, `#153`, `#154`

### Verse intelligence incubation

- `#93` hub
- `#127` through `#133`

### Roadmap-only concept work

- `#19`

These may support UX over time, but they should not consume the current UX planning bandwidth as if they were immediate interaction contracts.

---

## 6. How To Use This Map

When opening or refining an issue:

1. assign it to one of the six canonical specs if it directly changes user behavior
2. mark whether it is Primary, Enabling, or Support
3. if it is not direct UX work, keep it in the architectural/support lanes instead of forcing it into UX
4. use the UX Contract Slice issue template for new behavior work

This keeps UX planning honest and prevents architecture work from masquerading as UX closure.

