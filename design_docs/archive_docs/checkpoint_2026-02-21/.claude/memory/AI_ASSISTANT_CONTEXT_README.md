# AI Assistant Context

This directory contains configuration for AI coding assistants.

## Documentation Location

**All project documentation lives in:**

```
/design_docs/
```

**Whenever this readme is edited, add its notes to the DOC_README.md FILE IN /design_docs/ (and vice versa, except for the index)**

## Essential Reading for AI Assistants

1. **[DOC_POLICY.md](../design_docs/DOC_POLICY.md)** — Documentation rules, archival strategy, planning doc conventions
2. **[DEVELOPER_GUIDE.md](../design_docs/graphshell_docs/DEVELOPER_GUIDE.md)** — Quick orientation, commands, patterns
3. **[CODEBASE_MAP.md](../design_docs/graphshell_docs/CODEBASE_MAP.md)** — Module map, test distribution, data flow
4. **[ARCHITECTURAL_OVERVIEW.md](../design_docs/graphshell_docs/ARCHITECTURAL_OVERVIEW.md)** — Implementation status
5. **[IMPLEMENTATION_ROADMAP.md](../design_docs/graphshell_docs/IMPLEMENTATION_ROADMAP.md)** — Current feature targets and status

## Current Status

- **Phase:** M1 complete (FT1-6); M2 active
- **Active work:** Workspace routing, graph UX polish, edge traversal, settings
- **Build:** `cargo build` / `cargo run` (standalone, no mach needed)
- **Servo:** pulled as git dep from `github.com/servo/servo.git` main branch

## Working Principles

- **Verify, don't assume.** When the user makes a claim, determine whether it's true rather than assuming it is. Only take claims at face value when explicitly asked to.

## Configuration

- **settings.local.json** — Allowed commands and domains for AI operations