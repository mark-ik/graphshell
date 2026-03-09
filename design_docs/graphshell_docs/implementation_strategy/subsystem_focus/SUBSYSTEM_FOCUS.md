# SUBSYSTEM_FOCUS

**Date**: 2026-02-28  
**Status**: Architectural subsystem note  
**Priority**: Immediate architecture clarification

**Related**:
- `focus_and_region_navigation_spec.md`
- `../2026-02-28_ux_contract_register.md`
- `2026-03-08_unified_focus_architecture_plan.md`

**Policy authority**: This file is the single canonical policy authority for the Focus subsystem.
Supporting focus docs may refine contracts, interfaces, and execution details, but must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §3.5):
- **WCAG 2.2 Level AA** — focus order (SC 2.4.3), focus appearance (SC 2.4.11/2.4.12), and modal-blocking rules implement WCAG 2.2 requirements; deterministic return-path and handoff behavior are preconditions for conformance

---

## 0A. Subsystem Policies

1. **Semantic-focus policy**: Framework-local focus state may assist rendering but cannot become semantic focus authority.
2. **Deterministic-handoff policy**: Region activation, return paths, and handoff behavior must be deterministic and consistent across surfaces.
3. **Modal-integrity policy**: Blocking/modal focus rules must be explicit and enforceable, never implicit side effects.
4. **Cross-surface precedence policy**: Focus precedence order across graph/workbench/viewer/tool surfaces must be stable and documented.
5. **Regression-visibility policy**: Focus regressions and fallback paths should be diagnosable and scenario-test visible.

---

## 1. Purpose

This note defines the **Focus subsystem** as the architectural owner of focus, region activation, and handoff.

It exists to keep one boundary explicit:

- focus ownership is Graphshell-owned,
- framework-local focus state may inform rendering,
- but framework focus must not become semantic authority.

## 1A. Runtime Reality Gap

The current runtime does not yet have one unified focus-state authority object.

Today, focus behavior is split across:

- semantic region-cycle and handoff orchestration
- workbench tile/pane activation
- graph-view-scoped focus via `focused_view`
- GUI runtime hints such as focused node and graph-surface focus flags
- local widget focus in the UI framework
- embedded-content/webview preference heuristics in host code

That means the subsystem contract is directionally correct, but the implementation model is still distributed. `2026-03-08_unified_focus_architecture_plan.md` is the canonical cleanup path for closing that gap.

## 1B. Canonical Focus Tracks

The subsystem should be read as six linked tracks rather than one flat state:

1. `SemanticRegionFocus`
2. `PaneActivationFocus`
3. `GraphViewFocus`
4. `LocalWidgetFocus`
5. `EmbeddedContentFocus`
6. `FocusReturnAndCapture`

The focus router owns the first and sixth tracks directly. Workbench and graph subsystems own parts of the second and third tracks. Framework-local widget focus and embedded content focus remain subordinate to host focus policy.

---

## 2. What The Focus Subsystem Owns

- active region
- focus handoff rules
- return-path rules
- modal or blocking focus policy
- cross-surface activation precedence
- the architecture contract separating region focus, pane activation, graph-view focus, local widget focus, embedded-content focus, and capture/return state

---

## 3. Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` may affect how aggressive or inert transitions feel, but the Focus subsystem still owns deterministic focus authority and handoff rules.

---

## 4. Bridges

- Focus -> Graph: active graph pane ownership
- Focus -> Workbench: active tile/frame ownership
- Focus -> Command: which surface receives command context
- Focus -> Viewer / Control surfaces: return-path and escape behavior
- Focus -> Host/content boundary: embedded-content focus reclaim and escape behavior

---

## 5. Architectural Rule

If a behavior answers "which surface owns input right now?" it belongs to the **Focus subsystem**.

The unified architecture plan should be used whenever that question is ambiguous because multiple runtime “active” states exist at once.
