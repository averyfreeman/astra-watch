# Mission intake

Record accepted facts, explicit unknowns and delegated choices. Resolve blockers for the next slice; defer irrelevant future choices. Evidence from the repository outranks guessed boilerplate.

| Decision group | Questions to resolve when relevant |
|---|---|
| Outcome | Who benefits? What observable behavior marks success? Which capabilities are non-goals? |
| Acceptance | Which requirements are mandatory? Which test/check proves each? Which require real human review? What negative cases must fail safely? |
| Domain and compatibility | What terms, API behavior, platforms, language/toolchain versions, migrations and compatibility constraints are authoritative? |
| Write authority | Which exact files/directories can this slice change? What must be preserved? What generated files, secrets and infrastructure are excluded? |
| External authority | Are dependency installation, network, credentials, Git writes, deployment, publishing, paid calls or destructive actions approved, gated or forbidden? |
| Quality | What performance, accessibility, security, observability, privacy and maintenance requirements need measurable checks? |
| Unknowns | Which assumptions are safe to prototype? Which missing decisions block implementation? Which experiment would settle a disputed design? |
| Execution budget | What slice, timeout and retry bounds apply? What is the stopping condition? Who can revise the mission/budget? |
| Parallelism | Which independent reads or disjoint implementations justify delegation? Who owns integration and review? Which shared paths must be serialized? |
| Recovery | What belongs in a checkpoint: observations, current revision/hashes, accepted decisions, unresolved failures, next action and authority? |
| Docs and delivery | Are source docs, ADRs, terminal/browser demos, GitHub Pages or release artifacts required? What publication authority exists? |

A criterion without an executable check remains pending human review in v0.1; the runtime will not auto-attest it. Use meaningful checks: a successful `true` command proves process plumbing, not product correctness. Do not lower the acceptance bar to force a completion event.

Profile changes invalidate existing task contracts conservatively, including comment-only changes. Cancel the old task, preserve its records, and queue a new task after the revised mission is accepted. No in-place editing of immutable records.
