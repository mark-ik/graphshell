<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Agent Pane Spec

**Date**: 2026-04-30
**Status**: Canonical / Active
**Scope**: An **agent pane** — a tool-pane variant that hosts conversational
interaction with an `AgentRegistry` agent and serves as the surface for
**AI-enabled graph scripting**: an agent can read the graph, propose
mutations as `GraphReducerIntent` sequences, and execute those sequences
under the user's confirmation per the canonical sanctioned-writes contract.
The agent pane is also the canonical inspector for agent-emitted intent
provenance (per `subsystem_ux_semantics`).

**Code-sample mode**: **Illustrative signatures**. Concrete S3/S4 code lives
in the implementation, not this spec.

**Related**:

- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — `AgentRegistry`, atomic registry pattern, Tool Pane
- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — tool-pane integration in Frame split tree
- [`iced_command_palette_spec.md`](iced_command_palette_spec.md) — sibling command surface; agent pane is a *runtime* surface, not a command surface
- [`../system/register/SYSTEM_REGISTER.md`](../system/register/SYSTEM_REGISTER.md) — register layer where AgentRegistry lives
- [`../../../graphshell_docs/implementation_strategy/subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — provenance / UxTree contract; agent intents land here
- [`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) — agent permission grants per the settings spine
- [`../aspect_control/settings_and_permissions_spine_spec.md`](../aspect_control/settings_and_permissions_spine_spec.md) — agent permissions follow the same hierarchy
- [`2026-04-28_iced_jump_ship_plan.md` §4.10](2026-04-28_iced_jump_ship_plan.md) — coherence guarantee for tool panes (agent pane is one)

---

## 1. Intent

`AgentRegistry` (per TERMINOLOGY.md) holds autonomous agents that observe
app state, connect to external AI/inference providers, and emit
`GraphReducerIntent` streams. Until 2026-04-30 there was no canonical
**UI surface** for an agent — agents emitted intents but the user had no
direct way to converse with one or to author/edit/replay agent
instructions inside Graphshell.

The agent pane closes that gap. It is:

- A tool pane (`verso://tool/agent/<agent_id>` or `verso://tool/agent` for
  the default conversational agent) hosting:
  - a conversation transcript,
  - an input field for the user's turn,
  - a **proposed-intent inspector** showing the agent's pending
    `GraphReducerIntent` queue with confirm / edit / reject affordances,
  - a **graph-context display** showing what the agent currently has
    access to (selection, focused tile, recent activity).
- A **graph scripting surface**: the user can ask the agent to perform
  graph mutations through natural-language requests; the agent emits
  Intent sequences; the user reviews and confirms (or auto-confirms
  for sandbox-scoped agents under explicit grants).
- An **agent-provenance inspector**: every agent-emitted Intent
  surfaces in the Activity Log with full provenance (which agent,
  which prompt-turn, which model, which time); the agent pane is the
  canonical place to drill into that provenance.

The agent pane is **not** a command surface (commands go through the
[Command Palette](iced_command_palette_spec.md)). Agents may *invoke*
commands via the same `ActionRegistry` path, but the conversational
loop with the user lives in the agent pane.

---

## 2. Pane Shape

The agent pane is a **tool pane** (per TERMINOLOGY.md). Its `verso://`
addresses:

- `verso://tool/agent` — default agent (one persona-scope default).
- `verso://tool/agent/<agent_id>` — specific agent instance from
  `AgentRegistry`.

Multiple agent panes can be open simultaneously, each pinned to a
different agent. Agent identity is preserved across Frame Snapshot
restore.

### 2.1 Layout

```text
┌────────────────────────────────────────────────────────────┐
│  Agent: <agent display name>      [model: <model>]   ⚙   ⓘ │  Pane chrome
├────────────────────────────────────────────────────────────┤
│  Conversation transcript (scrollable, virtualized)         │
│                                                            │
│    user: "Tag every node with #archived if it hasn't       │
│           been activated in the last 30 days"              │
│                                                            │
│    agent: "Found 47 matching nodes. Proposed:              │
│            — TagNodes with #archived: 47 nodes             │
│            [Review] [Confirm All] [Reject]"                │
│                                                            │
│  ...                                                       │
│                                                            │
├────────────────────────────────────────────────────────────┤
│  Pending Intents (3)  [▶ expand to inspect]                │  Proposed-intent inspector
├────────────────────────────────────────────────────────────┤
│  Context: 47 selected, focused tile = "research/2024…"     │  Graph-context display
├────────────────────────────────────────────────────────────┤
│  > _                                              [Send]   │  Input field
└────────────────────────────────────────────────────────────┘
```

The four regions (transcript / proposed-intents / context / input) are
the canonical layout; visual styling per the
[theme tokens spec](../aspect_render/theme_and_tokens_spec.md).

---

## 3. Conversation Transcript

A virtualized scrollable list of conversation turns. Each turn is one
of:

| Turn kind | Source | Visual |
|---|---|---|
| `user` | user typed input | right-aligned, `text_secondary` background, `font_family_ui` |
| `agent` | agent response | left-aligned, `surface_raised` background, `font_family_content` |
| `tool_call` | agent invoked a tool / read graph state | inline collapsed; expand to see args + result |
| `intent_proposal` | agent proposed Intents | inline expandable; lists pending intents with action buttons |
| `intent_applied` | sanctioned write completed | brief inline confirmation; click to jump to Activity Log |
| `intent_rejected` | user or system rejected an intent | brief inline explanation |
| `system` | system-injected info (rate limit, model switch, etc.) | dim, italic |

Turns are ordered chronologically. The transcript is **append-only**
within a session; editing a past turn forks the conversation (creating
a new branch from that point). Editing UX is deferred (open item §10).

### 3.1 Persistence

Transcripts persist as a graph node of `address_kind = ToolPane` with
content stored adjacent to the pane state. The transcript becomes
graph-addressable: a user can pin a useful conversation, share it
across personas, or reference it from another node. Addressed as
`verso://tool/agent/<agent_id>/conversations/<conversation_id>`.

### 3.2 Virtualization

`scrollable` + virtualized `column!` rendering only the visible turns.
Long agent responses (>2000 tokens) collapse to a summary by default
with an "Expand" affordance.

---

## 4. Proposed-Intent Inspector

When an agent emits a `GraphReducerIntent` sequence, those intents land
in the **proposed-intent queue** for the conversation. They do **not**
auto-execute; user review is the default.

### 4.1 Queue display

The inspector region above the input shows:

- count of pending intents
- one-line summary per intent (action verb + target count)
- collapse/expand affordance for full inspection

Expanding shows the full intent diff:

```text
TagNodes:
  Action: add tag #archived
  Target: 47 nodes
    - https://example.com/page-a (last active: 45 days ago)
    - https://example.com/page-b (last active: 38 days ago)
    - ...
  Reversible: yes (untag action available)
  Idempotent: yes (per Intent Idempotence + Replay Contract)
```

### 4.2 Actions

Per-intent and queue-level actions:

| Action | Effect |
|---|---|
| **Confirm** (per intent) | Applies that intent; surfaces in Activity Log |
| **Confirm All** | Applies all pending intents in order |
| **Reject** (per intent) | Drops that intent from the queue; tells the agent |
| **Reject All** | Drops the queue; tells the agent |
| **Edit** (per intent) | Opens an inline editor for the intent payload (advanced; deferred — see §10) |
| **Why?** (per intent) | Asks the agent to explain why it proposed this; appends an agent turn |

Confirmation routes through the existing sanctioned-writes contract
([iced jump-ship plan §5](2026-04-28_iced_jump_ship_plan.md)); each
confirmed intent is recorded with provenance:

```rust
ActionOrigin::Agent {
    agent_id: AgentId,
    conversation_id: ConversationId,
    turn_id: TurnId,
}
```

This origin is preserved in the Activity Log entry and the WAL record,
so a replay or audit can attribute every mutation to its source agent
+ conversation.

### 4.3 Auto-confirmation grants

For trusted scenarios (sandbox graphs, read-only agents, agents the user
has explicitly granted auto-confirm), the proposed-intent inspector can
auto-apply intents without per-intent user confirmation. Auto-confirm
is a **per-agent-and-scope permission** that follows the [settings spine
permission hierarchy](../aspect_control/settings_and_permissions_spine_spec.md):

```text
default:    auto_confirm = false   (always require user confirm)
persona:    auto_confirm = false
graph:      auto_confirm.scratch = true   (this scratch graph: yes)
view:       —
```

The agent pane chrome shows an explicit "auto-confirm: ON" indicator
when the active scope grants it; the user can revoke per-intent or
per-conversation at any time.

---

## 5. Graph-Context Display

A persistent strip showing what graph state the agent has access to:

```text
Context: <selection summary>
         <focused tile/pane>
         <recent activity window>
```

The agent's access to graph state is **explicit and visible**: the
context display shows what the agent can read, and a "📎 Add to
context" affordance lets the user push specific nodes / graphlets / log
entries into the agent's working set. The agent does not silently read
the entire graph; access is grant-shaped.

### 5.1 Context grants

Context grants follow the same scope spine:

| Grant | Effect |
|---|---|
| `selection` | agent reads current SelectionSet |
| `focused_tile` | agent reads focused tile's content snapshot |
| `graphlet:<id>` | agent reads all nodes in a graphlet |
| `frame` | agent reads all open Panes' content |
| `graph` | agent reads all nodes (large; explicit grant required) |
| `activity_log:<window>` | agent reads recent activity events |

Grants are explicit; the agent pane chrome shows the active grant set;
the user can revoke any grant at any time. Default grant for a fresh
agent pane: `selection` only.

### 5.2 Privacy boundary

Context grants are a **privacy boundary**: the agent never reads
content outside its grant set. For an external-LLM-backed agent (most
common case), this means the LLM only sees what the user has explicitly
granted; the agent pane logs every outbound LLM request with the
context payload for audit.

For local / offline agents, the same grant boundary applies — the
agent's *code* respects the grant.

---

## 6. Input Field

A `text_input` at the bottom of the pane. Submission emits the user
turn into the conversation and triggers an agent-side LLM (or local)
inference invocation.

Multi-line input via Shift+Enter; Enter submits.

`text_input` is IME-aware (per the
[omnibar spec §8](iced_omnibar_spec.md), same iced 0.14+ guarantees).

### 6.1 Inline graph references

The input field supports `@`-prefixed graph references:

- `@node:<title|address>` references a specific node
- `@graphlet:<name>` references a graphlet
- `@selection` references the current SelectionSet

When the user types `@`, an autocomplete dropdown (using nucleo per the
[search providers + fuzzy spec](search_providers_and_fuzzy_spec.md))
suggests matching graph entities. References are resolved before the
turn is sent, expanding into context grants for that turn.

### 6.2 Slash-commands

A small set of `/`-prefixed commands work in the input:

- `/clear` — clear the conversation
- `/reset` — reset conversation + revoke all temporary context grants
- `/grant <scope>` — explicitly grant a context scope
- `/revoke <scope>` — revoke a context scope
- `/model <name>` — switch the agent's underlying model (if the
  agent supports multi-model; agent-provider-defined)
- `/save` — pin the conversation as a graph node

These are agent-pane-local; they do not collide with the Command
Palette's command set.

---

## 7. Message Contract

Agent pane Messages are local to the pane:

```rust
pub enum Message {
    AgentInput(String),                    // text_input on_input
    AgentSubmit,                            // Enter / Send button
    AgentMultilineInput(String),

    // Inline references
    AgentInputAtTrigger,
    AgentReferenceSelected(AgentReference),

    // Streaming agent response (Subscription-driven)
    AgentTokenReceived { conversation_id: ConversationId, token: String },
    AgentTurnComplete { conversation_id: ConversationId, turn_id: TurnId },

    // Proposed intents
    AgentProposedIntents(Vec<GraphReducerIntent>),
    AgentIntentConfirm { intent_id: PendingIntentId },
    AgentIntentReject { intent_id: PendingIntentId },
    AgentIntentConfirmAll,
    AgentIntentRejectAll,
    AgentIntentExplain { intent_id: PendingIntentId },

    // Context grants
    AgentGrantContext(ContextScope),
    AgentRevokeContext(ContextScope),

    // Slash-commands
    AgentSlashCommand(SlashCommand),

    // Lifecycle
    AgentPaneClose,
    AgentPaneSettings,                      // open agent settings sub-pane
}
```

---

## 8. Subscription Streams

The agent pane consumes three Subscriptions:

1. **Agent token stream** — partial responses from the LLM/local agent
   stream into `AgentTokenReceived`; the transcript renders a
   live-updating "agent typing" turn until `AgentTurnComplete`.
2. **AgentRegistry intent stream** — when an agent emits intents, the
   pane receives them as `AgentProposedIntents`. The same stream is
   consumed by `graphshell-runtime` for the actual queueing; the pane
   just observes.
3. **Activity Log filter** — agent-attributed entries from the global
   Activity Log appear as an "intent_applied" turn in the transcript
   for context.

Per the [iced jump-ship plan §12.6 anti-pattern](2026-04-28_iced_jump_ship_plan.md),
the pane does not poll runtime state; everything flows through
Subscriptions.

---

## 9. Coherence Guarantee

Per the
[tool-pane coherence guarantee](2026-04-28_iced_jump_ship_plan.md):

> Tool panes are observers, not authorities. They surface state from
> their owning subsystems and emit intents to those subsystems'
> authorities; they never bypass the uphill rule, never bypass the
> sanctioned-writes contract, and never silently mutate graph truth.

The agent pane preserves and tightens this guarantee:

- The pane never directly mutates graph state. Even when the user
  clicks "Confirm All" on a queue of agent-proposed intents, the pane
  emits the intents through `runtime.emit(...)`; the receiving
  authority validates and applies.
- Auto-confirm is a permission grant, not a bypass. The intent still
  flows through `apply_reducer_intents()`, just without per-intent
  user prompt — the *grant* is the user's authorization, recorded once
  per conversation+scope.
- Every applied intent records its `ActionOrigin::Agent { ... }`
  provenance; the Activity Log carries it; the WAL carries it; replay
  preserves it.
- Context grants are explicit, visible, and revocable. The agent never
  reads beyond its grant set; outbound LLM requests log their context
  payload.
- The pane is read-only over `AgentRegistry` agent state itself — it
  does not mutate the registry; agent enable/disable lives in
  `verso://settings/agents`.

---

## 10. Open Items

- **Intent editing UX**: §4.2 lists "Edit" as deferred. Editing a
  proposed intent requires a structured intent editor (per intent
  variant); meaningful effort. Deferred to a later sub-slice.
- **Conversation forking** (§3): editing a past turn forks the
  conversation. UX for navigating forks is a Stage F polish.
- **Multi-agent collaboration**: multiple agents working on the same
  graph in parallel. Currently each agent pane is single-agent; multi-
  agent orchestration is research-stage.
- **Agent authoring**: how do users author or import agent definitions
  (system prompt, tools, models)? Lives under `verso://settings/agents`
  + a future "agent authoring spec" — not in the agent pane itself.
- **Cost / token tracking**: external-LLM agents incur API costs. A
  per-conversation cost meter is a useful chrome addition; deferred.
- **Offline / local-model agents**: some agents may run locally
  (smaller models, no network). Permission grants and UX same shape;
  performance and capability differ. Tracked under the agent provider
  catalog.
- **Conversation export**: Markdown / JSON export for sharing
  conversations outside Graphshell. Stage F.
- **AT support**: AccessKit role mapping for transcript turns,
  pending-intent-inspector buttons. Stage E sweep applies; agent pane
  doesn't introduce new AT requirements beyond the standard tool-pane
  accessibility.

---

## 11. Bottom Line

The agent pane is the canonical UI surface for `AgentRegistry` agents:
a tool pane hosting a conversation transcript, a proposed-intent
inspector with explicit user confirmation, an agent-context display
showing what graph state the agent has read access to, and an input
field with `@`-references and `/`-slash-commands. Agents propose
`GraphReducerIntent` sequences; users review and confirm (or grant
auto-confirm per scope); every applied intent records
`ActionOrigin::Agent { ... }` provenance through the Activity Log and
WAL. The pane is an observer of `AgentRegistry`, not an authority over
it; intents flow through the standard sanctioned-writes path. Context
grants are explicit, visible, and revocable — the agent never reads
beyond its grant set.

This is the surface that closes the AI-enabled-graph-scripting use
case: the user can ask the agent to do graph things in natural
language, see exactly what the agent proposes, and confirm with full
provenance and reversibility.
