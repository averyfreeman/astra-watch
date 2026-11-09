---
name: astra-recover
description: Recover interrupted Astra Watch mission-bound work by checking durable state, immutable artifacts, current source and failed verification before resuming native Codex execution.
---

# Recover an interrupted run

Bind to the same absolute project root/worktree, then call `watch_state`. Inspect the task's current status/version, profile freshness, latest checkpoint and unresolved controls. Read the immutable checkpoint with this package's CLI `artifact <id>` through native shell; re-query relevant source rather than trusting an old summary.

If integrity fails, stop and preserve the evidence. Do not overwrite records or delete the state database to make checks pass. Local immutable files and hash links detect accidental changes; they are not signed/WORM storage against a privileged host user.

If the accepted profile changed, cancel the old task and obtain acceptance of a new immutable contract. If the task is paused, resume using the current version. A queued/running task is durable state, not proof an autonomous model worker is still alive. Resume the native Codex thread/goal separately as appropriate.

Compare observed source against the task scope. Unexpected/concurrent edits need explicit reconciliation; never overwrite another worker or widen scope silently. Prefer serial integration or separate native worktrees. Re-run every stale check after code changes. Do not infer a pass from a checkpoint, missing error, elapsed time or a successful unrelated task.

Recover within configured slice/retry/timeout limits. Adapt implementation tactics to actual failures. If authority, a requirement decision or budget revision is required, report the precise blocker and request it. This workflow does not override native goal budget limits or authorize external writes.
