# Planning Register Lane Sequence Receipt (2026-02-25)

**Receipt type**: Timestamped sequencing snapshot
**Source control-plane doc**: `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`
**Purpose**: Preserve the merge-conflict-aware lane sequencing decision so active planning sections can stay concise.

## Snapshot Context

- Merge-churn hotspots: `app.rs`, `render/mod.rs`, workbench/gui integration files, and high-traffic planning docs.
- Operating rule source: `CONTRIBUTING.md` lane workflow (one active mergeable PR per lane for shared hotspots, stacked PRs for dependencies).
- Maintainer objective: minimize merge conflicts while preserving forward progress across `p6`, `p7`, `p10`, runtime, and quickwins lanes.

## Recommended Execution Sequence (Captured)

1. **lane:p6 (pane/workbench architecture)**
   - `#76` Workbench pane architecture follow-up (tool-pane intents + legacy panel bridge)
   - `#77` Tool-pane render/title/focus dispatch by `ToolPaneState`

2. **lane:p7 (viewer/content/registry alignment)**
   - `#78` Split node-pane semantics from webview-runtime helpers
   - `#68` P7.a node MIME/address fields + WAL intents
   - `#69` P7.b MIME detection pipeline
   - `#70` P7.c lifecycle integration for `ViewerRegistry::select_for`
   - `#71` P7.d plaintext viewer baseline renderer + tests
   - `#80` Fold subsystem capability/conformance declarations into descriptors
   - `#82` `RegistryRuntime` integration (replace remaining legacy desktop dispatch)

3. **Return to lane:p6 for graph multi-view stack**
   - `#63` P6.a state model + focused-view wiring
   - `#64` P6.b graph-pane payload integration across open/split/focus paths
   - `#65` P6.c render path accepts `GraphViewId`
   - `#66` P6.d split graph view + per-pane lens selector UI
   - `#67` P6.e Canonical/Divergent controls + commit stub

4. **lane:p6 / storage overlap follow-up**
   - `#79` Workspace persistence schema alignment (pane-model tool/node semantics)

5. **lane:p10 baseline completion**
   - `#74` Complete WebView a11y bridge graft
   - `#75` Accessibility validation harness + manual checks
   - `#73` Culling validation + benchmark instrumentation
   - close `#10` after `#73/#74/#75`

6. **lane:runtime (gui churn low)**
   - `#81` ControlPanel cleanup (globals + SignalBus reconciliation)

7. **lane:quickwins (opportunistic)**
   - `#21` extract radial menu module
   - `#22` extract command palette module
   - `#27` semantic tab labels
   - `#28` zoom-adaptive LOD thresholds

8. **lane:roadmap (planning/adoption)**
   - `#11`, `#12`, `#13`, `#14`, `#18`, `#19`

## Near-Term PR Stack (Captured)

- `lane:p6`: `#76` → `#77`
- `lane:p7`: `#78` → `#68` → `#69` → `#70`
- `lane:p10`: `#74`
- `lane:p7`: `#71` → `#80` → `#82`
- `lane:p6`: `#63` → `#64` → `#65` → `#66` → `#67`
- `lane:p6`: `#79`
- `lane:p10`: `#75` → `#73` → close `#10`
- `lane:runtime`: `#81`

## Conflict-Avoidance Assumptions

- One active mergeable PR per lane when touching hotspot files.
- Cross-lane sequencing avoids simultaneous changes to workbench/gui hotspots.
- PR stacks are merged bottom-up to reduce repeated rebases.

## Update / Closure Criteria

Update this receipt only when sequencing assumptions materially change (lane order, stack order, or hotspot constraints). Otherwise, update only the active control-plane section in the planning register.
