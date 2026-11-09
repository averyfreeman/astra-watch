# Verification record

Date: **2026-10-02**. Host: this macOS machine. Rust/Cargo **1.98.1**; native Codex CLI **0.160.0**. Tests below use disposable directories and do not scaffold the research workspace or modify normal Codex plugin configuration.

## Confirmed

| Trial | Evidence and result |
|---|---|
| Rust validation | `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets`. 20 unit tests and 5 integration tests pass |
| Actual project delivery | Disposable Rust calculator compiles/tests with Cargo; profile binds criterion to the real test command; task queues/starts, CLI verification records a pass, completion succeeds, reopened state verifies the audit chain |
| No asserted completion | Completion rejects missing criterion evidence, a single pass for a two-criterion task, stale source hashes and changed project profiles |
| Coordination | Expected-version rejection, overlapping write-scope rejection, out-of-scope change detection and enforced slice bound |
| Fresh local retrieval | Source edits change search results; deleted source disappears; secret-file content is excluded; binary-source changes affect evidence snapshots; dedicated JavaScript parser yields syntax evidence |
| Immutable records | Existing identical creation is idempotent; changed immutable bytes and canonical artifact tampering are detected; SQLite append-only triggers prevent event mutation/deletion |
| Bootstrap preservation | Existing prose/config/disabled plugin policy retained. Existing `AGENTS.override.md` receives the bridge instead of editing a shadowed AGENTS.md |
| Native schedule guard | Requires isolated-checkout confirmation and a named non-protected branch; one-run fixture reaches `stop=true`, then refuses further inspection. No Git mutation |
| Verification timeout | Actual sleeping subprocess times out, fails evidence and cannot authorize completion |
| Live HTTP control | Real loopback dashboard rejects missing bearer authorization and foreign Origin. Authorized state read and cancel request stop an actual sleeping verification process within the test deadline; evidence reports interruption and no pass |
| MCP wire protocol | Real stdio child negotiates MCP `2025-06-18`, lists ten fixed tools, bootstraps an explicitly selected second project, leaves launch project untouched, returns current state and rejects a relative root. Repeated tools/list remains identical |
| OTLP delivery | Actual HTTP receiver fixture reads posted trace JSON; validates trace/span ID lengths and numeric timestamp strings; acknowledges `{}`; exporter returns an advancing cursor. Remote export without opt-in is rejected |
| Token telemetry | Cache input distinct from total input; counts deduplicated by source/event ID; missing native prompt-cache-write tokens remain null, not inferred from cache read or MCP timing |
| Skill packaging | All three skills pass the skill-creator validator. Validator-only PyYAML was installed in an isolated temporary environment; Python is not a plugin runtime requirement |
| Portable schemas | `plugin.json` and `mcp.json` pass Draft 2020-12 validation against their current canonical `agent-plugins.org` schemas |
| Native marketplace installation | In an isolated temporary Codex home, `codex plugin marketplace add <astra-watch-root> --json`, `codex plugin add astra-watch@astra-watch-local --json`, and `codex plugin list --json` succeed; package/skills/launcher/runtime are copied into native cache |
| Native skill discovery | `codex debug prompt-input` discovers `astra-watch:astra-watch`, `astra-watch:astra-delivery` and `astra-watch:astra-recover` in the installed cache. No model turn or account usage comparison was run |
| Compiled package | `scripts/build` runs locked tests and release build, packages the executable, and launcher reports `astra-watch 0.1.0`. Final native-cached release package also negotiates MCP, lists ten tools, bootstraps a disposable project and returns audited state. This host's release binary is approximately 19 MB |
| Browser smoke test | Local dashboard loads in the in-app browser, shows `Live · audit verified`, draft mission and explicit null telemetry, and has no captured console errors/warnings. Visual rendering inspected; this is not a comprehensive accessibility/responsive certification |

The native `codex mcp list --json` trial returns no manually configured servers in the isolated home; it is not used as proof of plugin-bundled MCP activation. Package discovery, direct packaged-server launch and MCP wire behavior are separate evidence from a complete live native model/tool invocation.

The optional `scripts/install` helper performs these same native marketplace/add operations in one invocation; shell syntax and an actual isolated-home install/list trial pass. It does not bootstrap a repository merely by installing the package.

## Deliberately not claimed

- No proven intervention-free 40–80-turn native-goal run or improvement over stock Codex.
- No semantic LSP/LST service, resolved call graph, embeddings, global project-search quality benchmark, or measured token-saving claim.
- No automatic native model-runtime worker or universal control over native turns/external processes.
- No hostile-host-proof immutability or OS sandbox from scope leases. Stronger audit/signing and native isolation remain separate.
- No native Desktop marketplace/trust/subagent/live-MCP model trial. UI paths are grounded in official docs; actual availability is client/account/version-dependent.
- No deployed OTel collector/backend/flame graph, full native distributed-trace correlation, or automatic Codex usage-log ingestion. HTTP receiver fixture proves transport plumbing, not every collector's interoperability.
- No inferred prompt-cache-write counts, missing usage treated as zero, cost reconstruction or per-project billing allocation.
- No installed native recurring automation: cadence and expiry are user choices; read-only guard bounds inspections, not scheduler invocations/quota.
- No Git commits/pushes/merges/releases, docstring changes to user repositories, terminal/browser recording, GitHub Pages publication or paid/API-key integration.
- No Linux/Windows package verification or Windows descendant-process cancellation guarantee. Rebuild/test for the target host.

## Reproduce

```sh
cd /Users/avery/Documents/Codex/plugin-research/astra-watch
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
./scripts/build
```

Use the [README](README.md) for intentional installation/bootstrap trials and the [decision report](ARCHITECTURE-DECISIONS.md) for optional integration/promotion criteria. Compact-format evaluation is out of scope.
