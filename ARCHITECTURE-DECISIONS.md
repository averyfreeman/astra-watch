# Astra Watch: architecture decisions

Verification date: **2026-10-02**. Native CLI checked: **0.160.0**. This is an implementation decision report, not a claim that every optional integration has been deployed or that an 80-turn autonomous mission has been proven. The original [native field guide](../project-agent/docs/research/native-codex-capability-field-guide.md) contains feature-by-feature Desktop/CLI evaluation instructions.

## Decision

Build a small Rust project-evidence companion to Codex, not another coding harness. The extra layer validates project profiles, keeps current source retrieval and immutable contracts, gates task completion on executed checks, and exposes truthful local controls. Native Codex owns model execution, planning/goals, tool invocation, permissions, delegation, worktrees, continuation and—on supported Desktop clients—scheduling. [Native long-running work](https://learn.chatgpt.com/docs/long-running-work), [subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents), [extension points](https://learn.chatgpt.com/docs/skills-and-plugins)

The architectural split is deliberate: add deterministic behavior at the evidence/contract boundary; use workflows to compose native and installed tools elsewhere. A second runtime would add model routing, auth, cost, cancellation and recovery obligations without evidence it improves ordinary development. No new communication format is adopted.

## Feature-path matrix

| Goal | Chosen implementation | Alternatives weighed / reason | Actual coverage and boundary |
|---|---|---|---|
| 1. AST → Mermaid | Tree-sitter in Rust; stable bounded `watch_graph` | Model-generated graph is cheaper to start but less repeatable; full semantic graph is expensive and language-dependent | Current syntax symbols/containment, path/line/hash/parse-error evidence. No resolved call/dependency graph |
| 2. LSP / living code | Refresh current source on retrieval; native compiler/checks; optional named MCP semantic adapter | Mandatory Helios/LSP front-loading adds language servers, startup/index costs and failure states | Current source evidence works without LSP. LSP/LST semantics remain an explicit optional integration, not a relabeled AST |
| 3. Sequential tools | Native Codex orders tools; typed state gates; checks run through native shell | A thinking/sequence MCP alone records intent; a universal tool executor would duplicate permissions and broaden side effects | Start/check/complete dependencies enforced in our ledger, not arbitrary cross-server execution order |
| 4. Sequential thoughts | Native plans/goals, concise decisions, observed tool events and checks | Requiring a separate thinking tool on every turn adds overhead; private chain-of-thought is not a supported audit contract | Inspectable work products, not hidden reasoning transcripts |
| 5. Project RAG | SQLite, incremental syntax/file chunks, BM25-style lexical retrieval with symbol/path boosts | Embeddings/vector service can help semantic paraphrases but introduces model/service configuration and evaluation burden | Project-local retrieval, no paid embedding calls. Fresh hashes, delete invalidation, coverage limits and no-answer state |
| 6. Recurring Git | Native scheduler + finite read-only cadence guard; verified-slice checkpoint proposals | Clock-driven commits reward cadence instead of evidence; a second scheduler duplicates native lifecycle | Expiry/run counts enforced for inspections, no active-checkout setup, protected branch check. Never stages/commits/pushes |
| 7. Scaffolds / ADRs | Preserve native `AGENTS.md`; typed profile + short prompt bridge; copy-on-write contract/decision/handoff artifacts | Make everything immutable: poor adaptability. Make everything mutable: weak audit. Git history alone: not append-only enforcement | Ordinary guidance/profile mutable/reviewed; accepted revisions immutable. Decision JSON canonical with Markdown projection |
| 8. Adaptive multi-turn work | Native goal/thread execution + durable CAS task state, scope leases, checks and checkpoints | Ralph/full custom worker loop adds runtime complexity; pure prompt promises do not enforce criteria | Mutable tactics/steering; mission drift invalidates tasks; real slice/retry/timeout bounds. No guarantee of intervention-free 40–80 turns |
| 9. Observability / controls | Live loopback dashboard, chained local events, bounded OTLP/HTTP JSON trace export; native telemetry separately | Static control labels falsely imply runtime power. Mandatory collector/Grafana stack raises bootstrap friction | Actual queue/steer/pause/cancel ledger controls; cancellation observed by active checks. Native turns use native interrupt. No invented flame graph |
| 10. Docs / demos / Pages | Native-shell language generators + optional installed Spit Roast / asciinema / Playwright / site workflow | Reimplementing doc generators and recorders duplicates mature tools; default publication exceeds setup authority | `watch_docs` is a truthful composition plan. Generators/recordings/deployment are optional, separately scoped and approved |
| 11. Turnkey bootstrap | Portable plugin + local marketplace, three short skills, one compiled MCP/CLI binary; idempotent root bootstrap | Global prompt replacement/config rewrites leak project preferences and weaken native evolution; mandatory hooks require separate trust | Profile/prompt/AGENTS bridge/local storage + optional native roles. No global trust/auth/model/telemetry rewrite; no install-time repo mutation |

These choices reflect native coverage, not just implementation expediency. See the official [AGENTS.md discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [MCP configuration](https://learn.chatgpt.com/docs/extend/mcp), [worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees), [scheduled tasks](https://learn.chatgpt.com/docs/automations), and [OTel reference](https://learn.chatgpt.com/docs/config-file/config-advanced#observability-and-telemetry).

## Native harness versus custom runtime

Codex can already plan, execute shell commands, invoke MCP, delegate and continue work. That does not prove every task meets a typed acceptance contract. Astra Watch therefore tracks explicit criterion IDs and executes configured checks in its CLI. MCP returns the native-shell invocation rather than running arbitrary commands behind a connector. This keeps execution inside the selected native command permission environment; it does not make an executable project check harmless. Review trusted argv and test meaning. [Native command/connector boundaries](https://learn.chatgpt.com/docs/agent-approvals-security)

Native goals are not re-created as an invisible Astra model worker. A queued or running ledger row is coordination state, not a claim a model is running. The parent still owns requirement coverage, delegation, integration and final outcome review. Native App Server control APIs are not a required transport in this version; production would acquire a substantial version/protocol support burden. [App Server maturity and experimental boundaries](https://learn.chatgpt.com/docs/app-server)

## Stable contracts and explicit project binding

The named `astra-watch` MCP server exposes ten fixed `watch_*` tools. Tool schemas/descriptions do not change with the mission. Runtime state belongs in arguments/results. Each tool accepts an absolute root; omission uses launch cwd, never a mutable global “last project.” The skill explicitly resolves the connected root to accommodate Desktop's primary-folder discovery and multiple worktrees. Native namespace wrappers may differ by client, so select the installed namespace instead of guessing flattened host tool names. [MCP](https://learn.chatgpt.com/docs/extend/mcp), [primary folder context](https://learn.chatgpt.com/docs/projects)

The plugin uses the current root `plugin.json`, fixed `skills/`, and typed `mcp.json` layout with `extensions.com.openai`. It is not a hosted ChatGPT web MCP endpoint or publicly submitted plugin. Local marketplace installation was tested without touching normal account configuration. [Official packaging](https://developers.openai.com/plugins/build/plugins), [portable manifest schema](https://agent-plugins.org/schemas/1.0.0/plugin.schema.json), [MCP package schema](https://agent-plugins.org/schemas/1.0.0/mcp.schema.json)

Skills are discoverable normally and load details progressively. Setup, delivery and recovery have separate narrow triggers; intake detail is loaded only when needed. The skill-creation and agent-writing guidance influenced this split. A generic one-file edit should not inherit a mandatory enterprise ceremony. No lifecycle hook is required: installation does not trust hooks, and an unconditional session hook would inject cost into unrelated work. [Skills](https://learn.chatgpt.com/docs/build-skills), [plugin hook trust](https://developers.openai.com/plugins/build/plugins#bundled-mcp-servers-and-lifecycle-hooks)

## Configuration and prompt decision

`project.config.toml` is custom data with a validated `[astra_watch]` namespace, not an invented native Codex config file. Seed defaults come from one reviewed central location; an existing project namespace wins. Bootstrap preserves other consumers and their comments. The config links `.astra-watch/project_prompt.md`; the effective root `AGENTS.override.md` if present, otherwise `AGENTS.md`, contains a small owned instruction to read it. Existing handwritten prompt content is preserved.

I rejected a “lightly modified replacement” of the OpenAI system prompt. The harness's evolving native instructions, safety and tool behavior should remain intact. A short project augmentation is enough to explain evidence gates and recovery. The TOML path itself does not make Codex load a prompt: the AGENTS bridge/skill provides that explicit behavior. Fallback instruction names do not supplement an existing AGENTS.md in the same directory. [Instruction discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md)

Optional `.codex` changes only enable this plugin if no policy exists and add read-only explorer/reviewer role templates. Existing `enabled=false` wins. No model is pinned or parent permission guarantee fabricated: effective subagent permissions can inherit/override role defaults. Trusted project loading and native protected paths still apply. Use `--no-native-config` or manual reviewed setup when necessary. [Trust/config precedence](https://learn.chatgpt.com/docs/config-file/config-basic), [custom agents](https://learn.chatgpt.com/docs/agent-configuration/subagents)

## Fresh retrieval before semantic infrastructure

The RAG guidance influenced syntax-aware chunks, file windows for imports/prose, bounded query results, provenance, deletion invalidation and a measured-evaluation seam. Fresh content is more valuable than an old, impressive embedding index. Queries return source hashes, path/lines, a coverage description, and source-data authority. Dedicated JavaScript and TypeScript/TSX grammars avoid treating every JS file as TypeScript.

This is local lexical retrieval used as context, not a benchmark-proven semantic RAG system. It favors exact symbols/paths; paraphrases and cross-language concepts may miss. AST containment is not a semantic LSP graph, and neither provides compiler-level type correctness. Current limits are explicit: source inventory up to configured files, per-text-file/chunk limits and bounded scan deadlines; incomplete inventory fails rather than authorizing completion. Ignored/build/runtime/record paths and symlinks are excluded or rejected; secret content is not indexed. Retrieval does not replace native search/reads when they are cheaper or more precise.

Evidence snapshots hash the inventory, including binary data and tracked ignored files where available, independently of text retrieval. Build outputs are intentionally excluded. This means generated build-only behavior needs a real check rather than a source hash alone. Hashes observe files; they do not freeze a filesystem against concurrent edits. Use isolated worktrees and serial final integration.

Promotion to semantic retrieval/LSP should require a representative local query set: lexical and paraphrase questions, negative/no-answer cases, exact source citations, revision-change cases, MRR/recall/completeness and task success. The few fixture queries prove invariants, not broad retrieval quality, token savings or superiority over stock Codex.

## Immutable intent, mutable execution

Accepted mission/task/decision/handoff snapshots are content-addressed files and append-only records. Decision revisions name `supersedes`; old records remain. Task status/version/slice and steering are mutable. Profile changes—even comments, conservatively—require a new task contract; a worker cannot silently reinterpret the old mission.

SQLite transactions and expected-version checks protect overlapping state changes. Lease conflicts are checked in the same start/resume transaction. Out-of-scope edits, including other active workers' source changes, prevent verification/completion. Paused tasks retain their lease. These are cooperative plugin-owned rules, not universal write access controls over Codex, Git, external tools or privileged host users. Worktree isolation remains the primary edit separation mechanism. [Native worktree semantics](https://learn.chatgpt.com/docs/environments/git-worktrees)

Immutable files and chained events detect ordinary edits; they are not signed, remote WORM storage. A host owner can defeat local controls. Git history/remote attestation can be added for stronger audit needs, with explicit authority. Immutable JSON is canonical; Markdown ADR projections improve human access without duplicating mutable truth.

Task completion requires the most recent executed pass for **each** assigned criterion, bound to current source/profile/mission/check. A generic successful check can still be a bad acceptance test: test quality and integration review remain human/parent responsibilities. Manual-review criteria remain pending in v0.1; no model assertion can masquerade as a human signoff.

## Bounded long work and honest controls

The default is at most 80 plugin slices and two retries beyond the first failed check per criterion/slice; a slice is a durable checkpoint boundary, **not** a Codex turn count. Check argv has a timeout. Source/check output bounds prevent unbounded reads/log growth. Timeout/pause/cancel kills Astra-owned subprocess groups on Unix; Windows descendant behavior has not been verified. Index work cooperatively observes cancellation/deadlines before transactional commit.

The live dashboard binds only loopback, uses a private bearer token and host/origin checks, escapes displayed data via text nodes, and applies the same versioned state transitions. It queues tasks and steers their next slice; it cannot independently compel a native model to read a new instruction. Pause/cancel is observed by active verification. Native turn interruption, model queueing and goal pause/resume belong to the native UI/CLI. [Native steering/queueing](https://learn.chatgpt.com/docs/prompting#steering-and-queuing), [native goal controls](https://learn.chatgpt.com/docs/long-running-work)

An absolute promise of no intervention or hallucination over 40–80 turns would be false. The architecture instead makes drift/failure observable and prevents unsupported completion within its own state machine. It stops for missing authority or budget/mission revision. Recovery reads immutable checkpoints, verifies integrity, re-queries source and reruns stale checks; it never equates a resumed chat summary with a current source snapshot.

## Cadence without milestone pressure

Native Desktop scheduling avoids a redundant scheduler, but a schedule alone does not establish finite authority or milestone evidence. The CLI cadence guard requires a named non-protected branch, no active Astra task at setup, an explicit isolated-checkout confirmation, expiry within 30 days and at most 1,000 inspections. It atomically consumes a run and refuses inspections after expiry/exhaustion. Active-task detection skips inspection rather than disturbing a worker.

It never mutates Git. A verified slice can produce a reviewed checkpoint proposal; a timer cannot produce a fake achievement. Native schedule invocations/quota continue until the native schedule is paused/removed—our inspection guard does not claim otherwise. Manually trial first, then use native scheduling only with user-selected cadence/bounds/expiry. [Testing scheduled tasks](https://learn.chatgpt.com/docs/automations#test-scheduled-tasks)

## Telemetry provenance

Native Codex OTel already exists; project config is not the authority for host export routing. Astra Watch stores local task/check/MCP events and exports a bounded **OTLP/HTTP JSON trace batch** on explicit CLI request. Trace/span IDs and nanosecond timestamps are correctly shaped; measured check/tool durations are distinct from near-instant audit events. The optional exporter is not a resident collector, complete distributed trace correlation, CPU profiler or flame-graph backend. [Native telemetry](https://learn.chatgpt.com/docs/config-file/config-advanced#observability-and-telemetry), [OTLP JSON encoding](https://opentelemetry.io/docs/specs/otlp/)

Token accounting is separate from local event timing. Import an explicitly selected native usage event with its source/schema ID and stable event ID. Example input, with absent quantities left null:

```json
{
  "source": "codex:exec-json/0.160.0",
  "event_id": "<real-session-and-turn-id>",
  "input_tokens": 1000,
  "cached_input_tokens": 800,
  "output_tokens": 200,
  "reasoning_output_tokens": null,
  "prompt_cache_write_tokens": null,
  "mcp_tool_cache_write_duration_ms": null
}
```

Run `telemetry --import /absolute/reviewed-usage.json` through native shell. This example is illustrative, not measured account usage. Imports are deduplicated; summaries expose field presence and reported-only sums. Cached input is a subset of input; reasoning output, if reported, is a subset of output. Missing cache-write counts stay unknown rather than zero. MCP cache-write duration measures metadata-cache latency, not prompt-cache tokens. Native automatic-log ingestion/cost reconstruction is not implemented. [Codex JSONL usage](https://learn.chatgpt.com/docs/non-interactive-mode#make-output-machine-readable), [native token metrics](https://learn.chatgpt.com/docs/config-file/config-advanced#turn-and-tool-activity)

API-level prompt-cache options and API cache-write billing do not establish equivalent Codex controls or native write-token telemetry. A literal “cache boundary” heading is prose, not a configured breakpoint. Stable plugin schemas/prompts may help prefix reuse, but do not guarantee native cache behavior. [API cache accounting, distinct from Codex](https://developers.openai.com/api/docs/guides/prompt-caching)

## Docs and accessibility

Use native execution to compose real installed generators and recording tools. `watch_docs` is intentionally a plan, not a claim rustdoc/go doc/asciinema/Playwright/Pages already ran. Source docstring mutation, site design and publication remain a separately approved scope; use the user's installed Spit Roast skills when available, without making them an undeclared mandatory dependency. Pages deployment must be verified at its real URL; local build success is not publication success.

Accessibility here primarily means low configuration friction: compiled local runtime, no embedding/API keys, one connected-root invocation, preserved files, short workflow instructions and truthful capability reports. The optional dashboard has labeled controls/status and text-first data. Browser accessibility has not been certified; keyboard/responsive/browser QA is part of the promotion checklist, not assumed from markup.

## When stock Codex is the better choice

Skip the plugin for a small isolated edit, a quick explanation, an unfamiliar/untrusted repo you should not scaffold, a project without meaningful acceptance checks, or when native search is already sufficient. It adds profile maintenance, source hashing and ledger ceremony. Huge monorepos, sensitive repositories needing stronger isolation, and semantic refactors needing a real LSP should not assume this baseline is enough.

Use it for long mission-bound work where current evidence, recoverable decisions, multi-worker integration discipline or explicit completion contracts are worth that cost. Measure success using accepted-criterion completeness, rework/defects, current-citation accuracy, recovery quality and total developer effort—not token savings alone. Rust improves local typing/deployment consistency; it does not prove better agent work products by itself.

## Desktop versus CLI

The same Rust server/profile works on both local surfaces. Desktop is the lower-friction human experience for connected projects, native worktrees, scheduled tasks and goal controls; CLI is stronger for reproducible script/check/MCP trials and headless JSONL capture. No separate plugin worker is needed on either. The default is native Desktop scheduling, not a CLI cron workaround. Availability remains version/account/trust/host-dependent. [Desktop environments](https://learn.chatgpt.com/docs/environments/local-environment), [CLI non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode)

The included binary targets this Mac; rebuild per host. Protected native config paths may require manual review. A local stdio package does not automatically become a web-hosted ChatGPT service. No installation, hook trust, external connector authentication, publication or model setting is silently changed to manufacture turnkey behavior.

## Promotion criteria

The implementation/package evidence is in [VERIFICATION.md](VERIFICATION.md). Before making stronger product claims: run representative retrieval and stock-versus-plugin task evaluations, a 40–80-turn controlled native-goal run, concurrent worktree integration and crash recovery, actual Desktop discovery/trust trials, real OTel collector interoperability, comprehensive browser accessibility/control tests, and one explicitly authorized doc/recording/Pages fixture. Add embeddings, semantic adapters or automatic native telemetry ingestion only if these tests demonstrate material value.
