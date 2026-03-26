<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Documentation Corpus Compression Plan

**Date**: 2026-03-14
**Status**: Proposed
**Scope**: All non-archived files under `design_docs/` (~230+ markdown files)
**Goal**: Produce a compressed, navigable representation of the full documentation corpus that replaces needing to hold 230 files in working memory.

---

## Motivation

The Graphshell design documentation corpus has grown organically across research, specs, plans, audits, and implementation receipts. The corpus is well-structured — documents declare their role, authority scope, status, and related docs — but the sheer volume makes it difficult to:

1. Answer "what owns X?" without scanning multiple files.
2. Verify that authority claims are consistent across subsystems.
3. Find all open work items without reading every plan.
4. Detect terminology drift, stale supersession chains, or dangling references.
5. Onboard a new contributor (human or AI) without re-reading everything.

This plan describes a procedure to extract, validate, and compress the corpus into a small number of canonical summary layers.

---

## Phase 1: Structural Inventory

**Input**: Filesystem tree under `design_docs/`.
**Output**: A table of every non-archived file with path, date, size, and declared status/role.

This phase is mechanical — walk the tree, parse front matter. The inventory has already been produced as of 2026-03-14.

**Status**: Complete.

---

## Phase 2: Bottom-Up Extraction

**Input**: Every non-archived markdown file.
**Output**: One normalized record per document.

### Record schema

| Field | What it captures |
|---|---|
| **Identity** | Path, date, status (Active / Draft / Archived / Superseded) |
| **Doc role** | Spec, plan, research, audit, receipt, index/hub |
| **Authority scope** | What this doc is the canonical policy authority for |
| **Adopted standards** | External standards cited (OSGi R8, RFC 3986, RFC 4122, OpenTelemetry, etc.) |
| **Key concepts defined** | Named types, registries, subsystems, protocols introduced here |
| **Policies declared** | Numbered normative policy statements |
| **Acceptance criteria** | Concrete done-gates |
| **Dependencies** | Other docs this one cites as prerequisites or companions |
| **Supersedes / superseded by** | Explicit replacement chains |
| **Open items** | Anything flagged as incomplete, deferred, or planned-but-not-landed |
| **Implementation state** | Stub / partial / landed / complete — as self-reported |

This phase is purely mechanical — no judgment, just faithful extraction from each file's front matter and body.

### Execution approach

- Start with the ~20 hub/index documents (`SUBSYSTEM_*.md`, `SYSTEM_REGISTER.md`, `system_architecture_spec.md`, `PLANNING_REGISTER.md`, `GRAPH.md`, `WORKBENCH.md`, `VIEWER.md`, `ASPECT_*.md`, registry development plan, etc.) since these summarize and link to their children.
- Then process leaf specs, plans, and research docs in subsystem order.
- Records are accumulated into a single structured file (JSON, TOML, or markdown table).

---

## Phase 3: Graph Construction

**Input**: Phase 2 records.
**Output**: Three typed graphs.

### 3.1 Document Dependency Graph

- **Nodes**: Documents.
- **Edges**: `cites`, `supersedes`, `is-parent-of`, `is-companion-to`.
- **Purpose**: Reveals the authority hierarchy, orphan docs, and circular reference chains.

### 3.2 Concept Ownership Graph

- **Nodes**: Named concepts (`ControlPanel`, `SignalBus`, `CoopSessionId`, `GraphViewState`, `PaneId`, `NodeId`, `GraphIntent`, `AppCommand`, etc.).
- **Edges**: `defined-by` (canonical authority doc), `referenced-by` (every other doc that mentions it).
- **Purpose**: Reveals split ownership, concepts without a canonical home, and terminology drift.

### 3.3 Subsystem Boundary Graph

- **Nodes**: Concepts (from 3.2) and subsystems (from `system_architecture_spec.md`).
- **Edges**: `owned-by-subsystem`.
- **Cross-subsystem edges**: Integration seams and potential boundary violations.
- **Purpose**: Validates the single-owner policy from `system_architecture_spec.md` and extends the Architectural Inconsistency Register.

---

## Phase 4: Consistency Audit

**Input**: Phase 3 graphs + Phase 2 records.
**Output**: A findings register.

### Audit checks

| Check | What it catches |
|---|---|
| **Authority conflicts** | Two docs both claiming canonical authority over the same concept |
| **Orphan policies** | Policy statements in a doc that isn't the declared authority for that domain |
| **Stale supersession chains** | Doc A supersedes Doc B, but Doc B still says "Active" |
| **Dangling references** | Doc A cites Doc B, but Doc B doesn't exist or has been archived |
| **Implementation claim drift** | A plan says "Phase N complete" but the corresponding spec still says "Draft" |
| **Terminology drift** | The same runtime concept called different names in different docs |
| **Standard adoption gaps** | A subsystem spec that should cite an adopted standard (per `2026-03-04_standards_alignment_report.md`) but doesn't |
| **Undeclared authority** | A doc that acts as policy authority but doesn't declare itself as such |

### Relationship to existing registers

- Extends `2026-03-12_architectural_inconsistency_register.md` with doc-level (not just code-level) findings.
- Extends `2026-03-03_spec_conflict_resolution_register.md` with cross-subsystem conflicts.

---

## Phase 5: Compressed Representation

**Input**: All previous phases.
**Output**: A single multi-layer summary document (~10–12 pages).

### Layer 1 — System Map (1 page)

A single table/diagram showing every subsystem, its canonical hub doc, its registries, and the authority boundaries between them. This is `system_architecture_spec.md` validated and corrected against the actual doc corpus.

### Layer 2 — Registry State Matrix (1 page)

The registry inventory table from `2026-03-08_registry_development_plan.md`, validated against each sector plan and extended with any registries mentioned elsewhere but missing from the master index. Columns: Registry, Kind, Struct, API, Wired, Tested, Diag, Sector, Canonical Spec.

### Layer 3 — Policy Digest (2–3 pages)

Every numbered policy statement from every spec, grouped by subsystem, deduplicated, and cross-referenced to its authority doc. This becomes the "constitutional index" — the single place to look up what rules govern any behavior.

### Layer 4 — Concept Dictionary (2–3 pages)

Every named type/concept, its canonical owner doc, its subsystem, its current implementation state, and where it's referenced. This is the compressed version of the concept ownership graph. Sorted alphabetically within subsystem groups.

### Layer 5 — Open Work Register (1–2 pages)

Every open item, deferred decision, and incomplete phase, pulled from all plans and specs, sorted by subsystem and priority. This replaces scanning 230 files to find what's still unfinished.

### Layer 6 — Inconsistency & Risk Register (1–2 pages)

The output of the Phase 4 audit — authority conflicts, stale chains, dangling refs, terminology drift. Extends the existing Architectural Inconsistency Register with doc-level findings.

### Layer 7 — Supersession & Archival Recommendations (1 page)

Which docs should be archived, which should be merged, which authority chains should be cleaned up, and which docs need status updates.

---

## Design Rationale

The documentation corpus is itself a graph — documents are nodes, citations are edges, and authority claims are typed properties on nodes. The compressed representation is a **projection** of that graph into a small number of canonical views, each answering a different class of question:

| Question class | Answered by |
|---|---|
| "What owns X?" | Layer 1 + Layer 4 |
| "What rules govern X?" | Layer 3 |
| "What's the implementation state of X?" | Layer 2 + Layer 5 |
| "What's broken or drifting?" | Layer 6 |
| "What should I clean up next?" | Layer 7 |
| "What's the full authority chain for X?" | Layer 1 → Layer 4 → source doc |

---

## Execution Notes

### Tooling requirements

This plan can be executed by:

1. **A human** reading each file and filling in the Phase 2 record schema manually.
2. **An LLM with file access** reading files in batches, extracting records, and accumulating them across sessions.
3. **A script** parsing front matter and `## Policies` / `## Acceptance Criteria` sections mechanically, with LLM-assisted judgment for ambiguous cases.

Option 2 is the most practical given the corpus size and the judgment needed for concept extraction and authority validation. The key constraint is that the work must be **resumable across sessions** — Phase 2 records should be committed incrementally so progress isn't lost.

### Suggested incremental execution order

1. Hub/index documents first (~20 files) — produces Layers 1–3 draft and partial Layer 5.
2. Registry spec files (~20 files) — completes Layer 2 and extends Layer 4.
3. Subsystem plans and specs by subsystem (~80 files) — completes Layers 4–5.
4. Research and audit docs (~40 files) — extends Layer 5 open items and Layer 6 findings.
5. Verse docs (~30 files) — extends all layers with the networking/P2P domain.
6. Remaining leaf docs — fills gaps.
7. Phase 3–4 audit pass — produces Layer 6–7.

### Output location

The compressed representation should live at:

```
design_docs/graphshell_docs/2026-03-14_documentation_corpus_compressed.md
```

Phase 2 intermediate records (if persisted) should live at:

```
design_docs/graphshell_docs/2026-03-14_documentation_extraction_records.md
```

---

## Acceptance Criteria

- [ ] Every non-archived file under `design_docs/` has a Phase 2 record.
- [ ] The compressed representation contains all seven layers.
- [ ] Layer 3 (Policy Digest) accounts for every numbered policy statement in the corpus.
- [ ] Layer 6 (Inconsistency Register) has been reviewed against the existing `2026-03-12_architectural_inconsistency_register.md` and `2026-03-03_spec_conflict_resolution_register.md`.
- [ ] The compressed representation is under 15 pages / 30KB.
- [ ] At least one "what owns X?" lookup has been validated end-to-end through the layers.
