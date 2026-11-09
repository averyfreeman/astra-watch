---
name: astra-delivery
description: Deliver bounded development slices under an accepted Astra Watch project mission, using fresh source retrieval, native Codex execution, checkpoints, and criterion-bound actual checks. Use for multi-turn mission-bound work, not routine edits without a profile.
---

# Evidence-bound delivery

Resolve the absolute project root and call `watch_state`. If the profile is absent, bootstrap first; if the mission is draft, settle the missing intake choices. Read the linked project prompt and accepted requirements. Preserve the native Codex instruction hierarchy: the project prompt is an augmentation, not a system-prompt replacement.

Queue a task using `watch_task` with `request.action: queue`, a stable unique ID, a concise title, acceptance criterion IDs and explicit relative `write_scope` paths. Start it using the returned `version` as `expected_version`. Keep the immutable mission and task contract fixed; adapt tactics, not acceptance requirements.

For each meaningful slice:

1. Read current state and reconcile the task version/profile. Retrieve relevant current source with `watch_context`; use `watch_graph` only for structural questions. Validate important answers against cited source. Syntax graphs do not resolve call sites or replace a configured LSP adapter.
2. Plan and implement through native Codex tools. Native goals/continuation, permissions and tool ordering remain authoritative. Independent research/review may use native subagents when authorized. Prefer read-only workers; disjoint write scopes are cooperative leases, not a universal security sandbox. Use separate native worktrees for parallel edits, then integrate serially under a new/current root contract.
3. For every task criterion, call `watch_verification`, review its configured argv, and run the returned absolute CLI invocation through the native shell. Do not guess the installed cache path. Run checks sequentially after edits settle. Only the CLI records executed evidence; worker claims, plans and fabricated logs cannot mark a pass through MCP.
4. If work continues, use `watch_task` action `checkpoint` with observed results, source references, blockers and next action. Before compaction/handoff preserve this immutable checkpoint; a checkpoint advances the bounded slice count. On failures respect retry/time bounds and stop for missing authority. Do not extend budgets or loosen requirements silently.
5. Complete through `watch_task` only after all required checks pass against the current source and profile. Source changes invalidate prior passes. The native parent remains responsible for cross-slice integration, meaningful test coverage, review and whole-mission completion.

Before a Git checkpoint use `watch_git`; it produces a proposal, never an automatic commit. Timer ticks are not milestones and no-op is valid. Scheduling requires a manually successful trial, user-selected cadence/expiry/run bounds, and an isolated checkout. Use Codex's native scheduler, not a second model loop.

For non-obvious architecture choices, `watch_decision` records alternatives and `supersedes` without editing old decisions. Use concise rationale and evidence, not hidden chain-of-thought. Docs/recordings/publication are a separate approved scope; use `watch_docs` and existing language generators or installed documentation skills.

Human steering is visible in task state. Pause/cancel prevents Astra checks from continuing; use native Codex interrupt/queue/steer for the coding turn. Re-read state at slice boundaries and after a control change. No prompt or plugin can guarantee a model never ignores an instruction; deterministic gates protect the plugin's own ledger and evidence only.
