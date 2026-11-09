---
name: astra-watch
description: Bootstrap the connected project with Astra Watch's native-first Codex workflow, namespaced project profile, fresh code context, and mission intake. Use for initial setup or profile discovery, not ordinary one-off edits.
---

# Bootstrap the connected project

Resolve the connected working directory to an absolute root. If several directories are connected and the target is ambiguous, ask which project; do not pick a parent folder. Use the installed `astra-watch` MCP namespace, keeping this same explicit `root` on every call.

Call `watch_bootstrap` for that root. Installation alone does not authorize bootstrap; invoking this workflow does. It preserves user-owned content, adds an `AGENTS.md` bridge and optional trusted-project Codex roles, and initializes local retrieval. It does not change global trust, models, permissions, telemetry routing, or the native base prompt. Read its created/preserved list and report partial failure honestly; rerunning is safe after fixing the reported condition.

If `mission_ready` is false, use [references/intake.md](references/intake.md) to gather the missing decisions. Ask a small relevant batch at a time; do not demand answers already established by current source or authoritative docs. Draft the `[astra_watch]` mission, non-goals, constraints and criterion-to-check bindings in `project.config.toml` through the native file editor. Have the user accept consequential scope and authority choices before queueing delivery. Inferred language test commands are suggestions, not proof of test coverage.

Then call `watch_state`, and inspect relevant source using `watch_context`. Use `watch_graph` only when syntax structure helps. For bounded delivery invoke the installed `astra-delivery` skill; for an interrupted run use `astra-recover`. Do not require every tool on every turn. Keep tool definitions stable and put changing project data in results.

Central preferences are opt-in via `ASTRA_WATCH_DEFAULTS`, or read from the standard user config location if present. A project's existing namespace overrides those seed defaults. Never copy another project's mission into a new project without confirmation.

If MCP is unavailable, run this package's `../../scripts/astra-watch --root /absolute/project bootstrap` through native shell. The package binary must already be built; do not silently install a runtime or bypass project trust. Restart/refresh the installed plugin after changing package files.
