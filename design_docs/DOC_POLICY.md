# Documentation Policy

There are two halves of this project, and an archive. Those are the three directories:

graphshell_docs\
    Graphshell: force directed graph web browser and desktop application.
verse_docs\
    Verse: a decentralized, peer network of tokenized browsing data, search indices, and storage.
archive_docs\

## Core Principles

### 1. Control Documentation Growth

We prioritize keeping the number of docs manageable. Add information to existing docs unless the material is substantial (>500 words, trying to keep words to a minimum), covers multiple sub-topics such as to constitute a new category, and unrelated to any current documentation. When considering whether to create a new file, prioritize organization and keep things consolidated. Do not create files for one-time analyses or specific details; add to existing files when appropriate. If a more sensible organizational structure emerges, suggest it in discussion.

### 2. Eliminate Redundancy

Documentation should be organized and edited to prevent duplicate information across files. Conduct periodic (before any commit, or upon a substantial change with confirmation) audits to ensure all documentation is properly organized and non-redundant. Archival docs do not need to be kept up-to-date with the project unless directly relevant developments have occurred; if so, add notes as opposed to editing/removing archival content. Only add to archival docs when necessary for context in the future (e.g., when a related feature is deprecated). If a more realistic redundancy elimination plan emerges, suggest it in discussion. Newer documents should be considered more authoritative, generally.

### 3. (No Legacy Friction)

When a new framework or architecture path is chosen, optimize for a clean fit with that path rather than preserving legacy implementations by default.

- Replace unique behavior/semantics that are at odds with planned architecture; keep if these patterns still align with planned architecture, unless it would compromise the intention of the plan (no half-migrations)
- Don't preserve legacy subsystems when preservation adds complexity or friction to a plan. No duplicate, redundnant systems.
- Preserve fallback mechanisms only when they provide necessary architectural/technical safety (for example, preventing crashes on empty runtime state), not to keep obsolete parallel systems alive.
- Do not add migration branches for historical formats unless explicitly requested and justified.
- Utilize current dependencies before adding new dependencies. If a new dependency can replace old dependencies while reducing project complexity and/or improving project reliability, do so upon explanation and confirmation.
- Keep tests focused on current semantics and current persistence schema. Don't maintain obviated tests, remove them.
- This default remains in force until an explicit product release introduces real-user migration requirements.

### 4. Documentation Location and Archival Strategy

- **Active project docs**: Store in `graphshell_docs/` or `verse_docs/`, or in subdirectories of either. Create, delete, and edit files in `design_docs/` to eliminate redundancy, prevent stale files, and make it easier to understand large-scale project changes. Edit files primarily with discussion and confirmation, unless explicitly granted permission.
- **Archive structure**: `archive_docs/` contains notes and information no longer actively relevant to the project, organized in checkpoint folders (each dated to last edit of any file in the checkpoint folder). If archiving, check if there's a current checkpoint folder; if not, create one to store the archival document(s) in.
- **Lifecycle**: When a feature or angle is deprecated as we develop, create a doc for it in `archive_docs/`. If a feature is adopted again, move the relevant documentation back into the appropriate active doc files/folders. Delete files only upon providing a rationale and asking for confirmation.
- **Cross-referencing conventions**: Reference other docs using relative links; for cross-folder references (graphshell_docs ↔ verse_docs), include the full path in link text for clarity.

### 5. Category Organization

Subdirectories are categories. Start with these core categories, and only add, delete, merge, or alter categories upon confirmation. In all docs, link other docs, even across category, for context:

1. **Research**: Briefs, reports, critiques, reviews, notes, etc. on architecture, design, testing, or implementation. Holistic (considering all relevant parts of the codebase) implementation guidance (code patterns, integration points, best practices, etc.), feature/dependency suggestions, strategies for ensuring a reliable, nonredundant, efficient project and codebase. Intended to be general and technical architecture resources.
2. **Technical Architecture**: Core app architecture, including descriptions of core components (including boundaries and interfaces), dependencies, cross-component interactions, Servo integration, IPC patterns, architectural decisions, feature targets, and their respective rationales. Sync with codebase changes to keep these docs current. Intended to be general and implementation strategy resources.
3. **Implementation Strategy**: Plans, development approaches, feature-gated roadmaps (in order of necessity and prerequisites), technical details of features for reference, implementation status assessments. Intended to be general and design resources.
4. **Design**: UI and UX documentation, interaction design, user workflows, design principles and rationale, accessibility targets and implementations, visual hierarchy and information design. Intended to be general and testing resources.
5. **Testing**: documentation for the repo's automated tests, reported and inferred bugs, feature target gaps, performance targets/profiling/benchmarks, checklists and process guidance for manual validation testing. Intended to be the final check before archiving completed feature plans.

### 6. README Requirements

The design_docs directory must contain a unique `DOC_README.md` file at the root of that directory (not in subdirectories). Each README should include:

- AI assistant instructions for the project and documentation
- Working principles and notes for reference
- Index: links, current status for all documents in the directory (links only + minimal context). Point to root README for project status.

### 6.1 DOC_README Canonical Authority Rule

`DOC_README.md` is the sole canonical index and first-reference document for `design_docs/`.

It must satisfy all three goals at all times:

1. Capture AI-agent notes that provide project insight and convert durable insights into documented **Working Principles**.
2. Provide the only canonical index for all active `design_docs` documentation and current project guidance, with references to `DOC_POLICY.md`, `PROJECT_DESCRIPTION.md`, and current authoritative docs.
3. Serve as the authoritative, first document of reference for current project documentation state, kept synchronized with the real contents of `design_docs/`.

Operational requirements:
- When AI assistant context/memory/instruction notes change in meaningful ways, update `DOC_README.md` Working Principles in the same session.
- If any index conflict exists between `DOC_README.md` and any other README/index file, `DOC_README.md` is authoritative and other files must be aligned to it.
- Any doc move/add/remove in active docs must include a same-session `DOC_README.md` index update.

### 7. PROJECT_DESCRIPTION.md Ownership

`PROJECT_DESCRIPTION.md` is reserved for the maintainer. Do not edit this file unless explicitly instructed. Consider this document authoritative, but if there are contradictions with the rest of the docs, surface them for discussion.

The root README.md is derived from PROJECT_DESCRIPTION.md, BUILD.md, current authoritative docs, and project consensus, providing current project state, build instructions, and planned features. Speculative features with no associated plans yet are only for PROJECT_DESCRIPTION.md.

### 8. Upstream Dependency Maintenance: BUILD.md

The `BUILD.md` document provides cross-platform build instructions for Graphshell. Servo is a git dependency (no local Servo checkout or `mach` needed). As Servo and graphshell evolve, `BUILD.md` must be kept current:

- **Trigger**: When updating the Servo git dependency (`cargo update -p servo`):
  - Check if Rust version requirements changed (see `rust-toolchain.toml`)
  - Verify `cargo build` still works end-to-end
  - Test on at least one platform (preferably the maintainer's native platform, Windows)
  - Document any new system library requirements or tool versions

- **Update process**:
  1. Run `cargo build` and note any new system dependency errors or prompts
  2. Record timing (first build and incremental) if significantly changed
  3. Add notes in BUILD.md like: "Updated Feb 2026: Rust 1.92.0 → 1.95.0"
  4. Test platform-specific instructions if you modified them
  5. Commit BUILD.md updates together with Servo version bumps in `Cargo.lock`

- **Deprecation**: If a platform becomes unsupported by Servo, mark instructions as "Deprecated" with date and move detailed instructions to `archive_docs/` while keeping a reference in BUILD.md

### 9. Implementation Planning Documents: Feature-Driven Organization

Checklists, task lists, and implementation planning documents should be organized by **feature targets and validation tests**, not by calendar time (days, weeks, or dates). This approach provides flexibility in actual development while maintaining clear acceptance criteria.

**Structure for task documents:**

- **Feature Target**: Name the capability (e.g., "Feature Target 1: Understand Architecture Foundation")
- **Context**: Brief explanation of why this target matters
- **Tasks**: Step-by-step work items (without time estimates or "Day 1/Monday" labels)
- **Validation Tests**: Specific, measurable criteria that prove the feature target is complete
- **Outputs**: Deliverables (documents, code, diagrams) that result from completing this target
- **Success Criteria**: Knowledge validation questions that confirm deep understanding

### Workflow Documentation Rule

For AI assistants: store documentation (including memories, instructions, plans, etc.) in DOC_README.md per this file's rules. Refer to DOC_README.md first, then this file, for context when needed. This also means that the content of agent-specific instruction folders should exist in design_docs (such as .claude/ readme sending its updates to DOC_README.md, but also memory files that are useful for context should be copied and archived).

When asked to do a project (like implementing a feature, or trying to accomplish something documented as a planned task in design_docs) that requires changing the codebase (not the docs), create a markdown file in the relevant design_docs directory with the current date, a keyword related to the task (such as the feature being implemented), and the suffix _plan.

Planning file metastructure example:

Include in a markdown file three sections:

- (keyword) Plan: track phases and progress
- Findings: store research and findings
- Progress: session log and test results

Suggest alternate structures if relevant and useful for the document's purpose.

Update the _plan file every two prompts related to the project, or every two tasks you complete related to the project. Update the file upon completing the project, and move it to archive docs (creating a timestamped folder if none has been made for the relevant day). Reread the relevant file before working on the same project, if the _plan file exists.
