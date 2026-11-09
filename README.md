# Astra Watch

Native-first Codex project bootstrap and evidence-bound development. One Rust binary supplies ten stable MCP tools, a CLI, project-local retrieval and a live local control dashboard. Codex still owns models, plans/goals, shell execution, permissions, subagents, worktrees and scheduling.

Start with [architecture decisions and limitations](ARCHITECTURE-DECISIONS.md). The native-capability evaluation guide is [in the original research project](../project-agent/docs/research/native-codex-capability-field-guide.md). Compact-format evaluation remains deferred.

## Install this local marketplace

The included binary was built for this Mac. Rebuild on another host using Rust/Cargo and a C compiler; Python, Node and an embedding service are not plugin runtime dependencies.

For this host, `./scripts/install` registers and installs the bundled marketplace in one invocation. It builds only if the packaged launcher cannot run. For an intentional source rebuild, run `./scripts/build` first. Installation changes your native plugin settings, not project trust or global permissions.

```sh
cd /Users/avery/Documents/Codex/plugin-research/astra-watch
./scripts/build
codex plugin marketplace add /Users/avery/Documents/Codex/plugin-research/astra-watch
codex plugin add astra-watch@astra-watch-local
codex plugin list
```

`plugin add` was locally verified with Codex CLI 0.160.0; inspect your version's `codex plugin --help`. Official packaging docs describe marketplace registration and Desktop installation. Alternatively, open this directory as a project, restart the supported Desktop client, choose **Astra Watch Local** in the Plugins Directory and install **Astra Watch**. Availability and UI vary by client/account. Use the installed copy; refresh/reinstall after rebuilding a package. [Official packaging and marketplace instructions](https://developers.openai.com/plugins/build/plugins)

These commands intentionally change your Codex plugin configuration. They are not run automatically by project bootstrap. Installation testing used an isolated temporary Codex home, leaving your normal installed-plugin settings unchanged.

## Bootstrap a project

Connect the intended project directory, select the installed Astra Watch setup skill (`$astra-watch:astra-watch` in the locally verified CLI inventory), and ask:

> Use Astra Watch to bootstrap this connected project. Preserve existing guidance and ask for the missing mission parameters before starting delivery.

The skill uses the absolute connected root. The MCP fallback default is the server's launch directory; explicit roots avoid ambiguity across Desktop primary/secondary folders and separate worktrees. Installation alone does not mutate a repository. Bootstrap occurs when the workflow is invoked. Start on a disposable trusted checkout when evaluating it.

Or use the CLI directly:

```sh
/Users/avery/Documents/Codex/plugin-research/astra-watch/plugins/astra-watch/scripts/astra-watch --root /absolute/project bootstrap
```

It preserves or creates:

- `project.config.toml`, exclusively the `[astra_watch]` namespace;
- the linked `.astra-watch/project_prompt.md`, without replacing Codex's base prompt;
- one owned marker section in existing `AGENTS.override.md` if present, otherwise `AGENTS.md`;
- optional `.codex/config.toml` plugin enablement and two read-only-role templates;
- ignored `.astra-watch/runtime/state.sqlite` with the local index/task/audit ledger;
- immutable `.astra-watch/records/` mission, task, decision and handoff revisions.

Use `bootstrap --no-native-config` to omit `.codex` edits. Existing explicit plugin disablement is respected. Native configuration requires project trust and does not substitute for installing the plugin. If protected-path policy rejects `.codex` changes, do not bypass it: review/setup those files manually. [Native configuration and trust](https://learn.chatgpt.com/docs/config-file/config-basic)

Central seed preferences: set `ASTRA_WATCH_DEFAULTS` to a reviewed TOML file, or use `$XDG_CONFIG_HOME/astra-watch/defaults.toml` / `~/.config/astra-watch/defaults.toml`. See [example defaults](examples/defaults.toml). Existing project settings win and are never refreshed silently from changed central defaults. Keep project-specific missions out of shared defaults. Bootstrap detection suggests Rust/Go/Python checks; review them rather than assuming frameworks or coverage.

## Deliver and recover

Accept outcome, non-goals, constraints and criterion/check bindings in the project profile. Use the installed **Astra Watch Delivery** skill (`$astra-watch:astra-delivery`) for bounded slices and **Astra Watch Recovery** (`$astra-watch:astra-recover`) after interruption. Select the actual installed names if your client labels them differently. The plugin is optional for one-off edits; no universal “call every tool” rule applies.

Example MCP `watch_task` request, with the absolute `root` alongside `request`:

```json
{
  "root": "/absolute/project",
  "request": {
    "action": "queue",
    "id": "slice-1",
    "title": "Implement the accepted calculator behavior",
    "criteria": ["correct-sums"],
    "write_scope": ["src", "tests"]
  }
}
```

Start using the returned version, then run each `watch_verification` CLI plan through Codex's native shell. The profile specifies `program`, argv and timeout; there is no arbitrary command executor behind MCP. Check output is bounded/redacted and actual exit status recorded. Profile/source changes invalidate evidence. Manual-review criteria cannot be auto-completed in v0.1.

```sh
plugins/astra-watch/scripts/astra-watch --root /absolute/project state
plugins/astra-watch/scripts/astra-watch --root /absolute/project context 'quaternion normalization'
plugins/astra-watch/scripts/astra-watch --root /absolute/project graph --query Quaternion
plugins/astra-watch/scripts/astra-watch --root /absolute/project verify --task slice-1 --criterion correct-sums
plugins/astra-watch/scripts/astra-watch --root /absolute/project dashboard
```

The dashboard prints a private loopback URL with a bearer token. It offers live queue/start/pause/resume/cancel/steer controls and events. Pause/cancel stops active Astra verification; use native Codex controls for the model turn. Do not share that URL. Ctrl-C stops the dashboard. Worktree isolation, native permission checks and human review remain necessary.

## Guarded scheduling and telemetry

Use a separate non-protected Git checkout. Choose finite run bounds and a Unix expiry, create a guard, and manually trial its tick **before** using native Desktop Scheduled tasks:

```sh
plugins/astra-watch/scripts/astra-watch --root /absolute/isolated-project cadence create --id git-check --max-runs 8 --expires-unix <FUTURE_UNIX_SECONDS> --isolated-checkout
plugins/astra-watch/scripts/astra-watch --root /absolute/isolated-project cadence tick --id git-check
```

Ticks inspect only; they never commit/push or claim milestones. `stop: true` means end the run and pause/remove the native schedule. The guard bounds inspections, not native scheduler invocations or model quota. CLI scheduling remains an external scheduler concern. [Native scheduled tasks](https://learn.chatgpt.com/docs/automations)

For optional local OTLP/HTTP JSON trace export:

```sh
plugins/astra-watch/scripts/astra-watch --root /absolute/project telemetry --otlp
plugins/astra-watch/scripts/astra-watch --root /absolute/project telemetry --export http://127.0.0.1:4318/v1/traces --after 0
```

Advance the cursor only after acknowledgment. Remote export requires `--allow-remote`; no automatic network export runs through MCP. Native Codex token data is a separate explicit import—see [telemetry provenance](ARCHITECTURE-DECISIONS.md#telemetry-provenance). No native prompt-cache-write counts or full flame graph are fabricated. Native OTel routing belongs in user/host configuration, not project config. [Codex telemetry](https://learn.chatgpt.com/docs/config-file/config-advanced#observability-and-telemetry)

## Verify the implementation

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Tests use disposable projects: actual Rust compilation/tests, complete criterion coverage, stale evidence, mission drift, leases/CAS, source freshness/deletions, tampering, finite cadence, process timeout/cancellation, MCP JSON-RPC and cross-project binding. [Verification record](VERIFICATION.md) distinguishes tested behavior from untested client/platform/deployment integrations.
