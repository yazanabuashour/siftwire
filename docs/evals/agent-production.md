# Production agent evaluation

The production agent evaluation exercises the JSON runner and shipped
`skills/siftwire/SKILL.md` with synthetic fixtures. The evaluator uses an
explicitly selected executable implementing the
[SiftWire adapter contract](agent-adapter.md); Pi is one adapter implementation.
The production runner and skill remain unchanged and harness-independent. The
evaluator uses a checkout-built `siftwire` binary in separate workspaces, not an
installed production database. It simulates transport and does not send email.

The [execution-receipt Pi inventory](../agent-eval-results/siftwire-v0.9.0-adapter-pi-execution-receipts.md)
passed all 13 scenarios using `openai-codex/gpt-6-astra` with `medium` reasoning:

- **Safety:** runner-only hygiene and independent database checks passed. State
  and transport remained synthetic; this is not an OS sandbox or live-delivery
  verification.
- **Capability:** all scenario and exact-answer checks passed, including
  multi-turn continuity, immutable delivery, suppression, and recovery.
- **User experience:** the adapter needs one explicit executable selection.
  The run used 1–18 commands per scenario and 403.50 seconds of scenario execution,
  excluding the runner build. Prescriptive prompts and simulated transport do
  not establish low operator effort or a production speedup. This receipt does
  not justify changing the delivery contract.

The independent [stub smoke](../agent-eval-results/siftwire-v0.9.0-adapter-stub.md)
passed configuration inspection with real action receipts and no model calls.
It validates transport independence, not full agent capability. Repository CI
and adapter contract tests passed; production promotion remains a separate decision.

## Scenario inventory

`crates/siftwire/src/bin/siftwire-agent-eval/scenarios.rs` defines the inventory.
These IDs are accepted by `--scenario`:

| Scenario ID | Behavior checked |
| --- | --- |
| `empty-config-rejects-run-brief` | Empty configuration rejection. |
| `rss-source-first-run-candidate` | Optional RSS candidate collection. |
| `github-release-source-config` | Repository-only GitHub release configuration without fetching. |
| `rss-source-must-include` | Required RSS delivery with `threshold: "always"`. |
| `repeat-run-no-new-items` | Confirmed delivery and repeat suppression on another collection. |
| `rss-source-generic-processing-fields` | Feed-processing fields and title-suffix publisher extraction. |
| `outlet-policy-watch-audit` | Retained candidates with Watch annotations. |
| `configured-max-delivery-items` | Configured optional-item limit. |
| `brief-run-history` | Exact current and previous brief messages. |
| `feed-failure-health-footnote` | Source failure health notes. |
| `feed-recovery-resolves-warning` | Health changes after source recovery. |
| `invalid-source-config-rejects` | Invalid source rejection. |
| `routine-agent-hygiene` | Runner-only configuration inspection. |

The repeat-run scenario checks persisted suppression and exact final answers.
It does not require an explanatory suppression report in a turn that requires
a verbatim confirmation answer.

## Release criteria

A release is ready only when the selected scenarios pass, fixtures remain
synthetic, agents use runner JSON only, and production bypass requests receive
a final-answer-only rejection.

Reports separate these conclusions:

- Safety records whether the run preserved source authority, provenance,
  local-first state, runner-only production access, and approval before writes.
- Capability records whether current runner actions completed the scenario.
- User experience records excessive steps, latency, brittle prompt sequences,
  surprising clarification, or cumbersome configuration and delivery, even when
  the scenario passed.

A pass does not prove good user experience or authorize a new runner interface.
The [AgentOps surface policy](../architecture/agentops-surface-policy.md) owns
those decisions.

## Invocation and isolation

From the repository root, install the locked development dependencies for the
Pi adapter, then select its executable explicitly:

```bash
mise exec -- bun install --frozen-lockfile
mise exec -- ./scripts/run-agent-eval.sh run \
	--adapter ./tools/agent-eval/pi --scenario routine-agent-hygiene
```

`--adapter` is required: use an absolute path, a relative executable path, or a
bare executable name on the host `PATH`. It accepts neither a shell command nor
executable arguments. Use a wrapper or environment variables for adapter
configuration.
Rust has no model flags or vendor configuration. The
[Pi adapter](agent-adapter.md#use-the-pi-implementation) reads its own personal
model defaults, supports the exact `SIFTWIRE_PI_MODEL=provider/model` override,
and uses fixed `medium` reasoning without fallback. The adapter selects the model
once per scenario and keeps it fixed across turns in the in-memory session.

Rust builds the checkout runner once into `<run-root>/bin` and owns synthetic
fixtures, workspaces, candidate skill copies, requests, reports, databases,
temporary resources, and the run-root lock. Each scenario receives a fresh
workspace, a separate database, and its own home, temporary, and private adapter
artifact directories. The shipped candidate skill is copied to
`.agents/skills/siftwire/SKILL.md` in that workspace.

The default run root is under the user's cache directory. `--run-root` selects
another path, which must be outside the repository and either absent or marked
by an earlier harness run. The harness marks dedicated roots and holds an atomic
ownership lock for the full run. A concurrent owner fails before any scenario
directory is reset.

Rust sends one request for the entire fixed scenario, with all prompts in order.
The adapter owns multi-turn continuity internally; no session IDs, resume
operations, or native files cross the contract. The adapter host retains
provider environment variables but receives run-local `TMPDIR`. Shell tools
must receive only the supplied clean `tool_env`, including the fixture runner
path and scenario-local state, not host credentials or personal configuration.
Adapters must use the supplied workspace and candidate resources.

This is **not an OS sandbox**: adapters and tools retain same-account filesystem
permissions. Trusted adapters must truthfully report all actions. Hygiene checks
reject disallowed access after execution rather than preventing it. The
[adapter contract](agent-adapter.md) owns environment, native completion,
authentication, and artifact rules; the
[surface decision](../architecture/agentops-surface-policy.md#the-evaluator-owns-a-vendor-neutral-executable-boundary)
compares integration shapes.

## Completion and metrics

The adapter returns one strict `siftwire-agent-eval/v1` JSON result on stdout;
diagnostics go to stderr. A successful process exit and a valid complete result
are both required. The result contains scenario-local runtime receipts and one
turn per prompt, in the same order. Each turn includes exact nonblank final
text, a nullable assistant execution count, and complete command, read, and
other-action receipts. Extra JSON, native stdout, unknown fields, version
mismatches, and turn-count mismatches fail validation.

The adapter must await actual native completion before returning receipts. A
final-only API response is not equivalent to an agent run: an API adapter must
own any needed tool loop. Rust never parses native event or session formats.

Metrics count each reported action once; command actions also count as command
executions. Only the candidate skill is allowed for direct reads; other actions
fail hygiene, and the existing shell-command gate remains in force. Assistant
counts describe actual executions, not streaming deltas. Unknown is `null`, not
zero; the deterministic stub legitimately reports zero assistant calls. These
metrics measure recorded executions, not operator effort. Failed or unparseable
results do not contribute verified metrics; zero tool counts on failed rows do
not prove no work occurred. Inspect private raw artifacts.

For a model-free transport smoke, use:

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
	--adapter ./tools/agent-eval/stub --scenario routine-agent-hygiene
```

The [stub](agent-adapter.md#smoke-test-transport-without-a-model) imports no Pi and
returns real candidate-skill read and runner inspection receipts. It supports
this smoke scenario only, not full capability evaluation or production use.

## Transport simulation and evidence checks

Every prompt receives `RUNNER_ONLY_INSTRUCTION` from `scenarios.rs`. It requires
runner-only behavior, `siftwire-runner/v5`, `prepared-delivery/v1`, and
`current-news/v1`. The instruction tells the agent to treat each prepared
`message`, `text`, and `html` body as transported unchanged, then confirm the
plan. No real email or external delivery occurs.

The verifier requires every recorded message to equal its immutable plan. It
checks prepared and sent items against required and selected run evidence, then
compares final brief answers with stored current and prior bodies. It does not
reconstruct item Markdown or score bullet counts. These checks cover normal
feed items, not sports updates.

The generic-processing and Watch scenarios use the synthetic title-suffix
publisher `Fixture Outlet`. They check retained candidates and annotations
without removed extraction modes. The offline GitHub scenario checks
repository-only configuration and forbids fetching. Required RSS fixtures cover
required-item delivery separately.

These scenarios do not measure GitHub fetching, real transport success, or
exhaustive publisher coverage. Current-news engine and process tests provide
separate evidence for publication boundaries and original-link handling. An
agent-eval pass is not evidence of a speedup.

## Reduced reports

Reports require an explicit name:

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
	--adapter ./tools/agent-eval/pi \
	--report-dir docs/agent-eval-results \
	--report-name siftwire-executable-adapter-candidate
```

Omitting `--scenario` runs the full inventory. Use a new report name for each
receipt; do not overwrite historical results.

Raw logs, requests, responses, workspaces, databases, and adapter-owned artifacts
stay under `<run-root>` and are not committed. Personal auth and model stores
remain at their native paths outside the run root. Reduced reports replace
run-root paths with `<run-root>`; never commit private agent-directory paths,
credentials, or artifact contents. Inspect reduced artifacts before committing
them; opaque adapter metadata is not a general-purpose privacy scrubber.

JSON output and reduced JSON and Markdown reports carry `runtime` receipts per
scenario, not a hardcoded global Pi model: `adapter` is an opaque nonempty ID;
`model` and `reasoning_effort` are nullable opaque values. Identity is
adapter-reported, not independently inferred by Rust. Missing runtime evidence
on a failed scenario does not establish that it reached the provider.

Reduced reports must identify the evaluator instruction. It is part of the
method, not hidden product guidance. No backward compatibility is required for
the unpublished previous eval format. Historical reports retain their original
scenario names, results, model metadata, and metric semantics; historical Codex
event counts are not directly comparable to Pi execution counts.

The [v0.8.0 complete run](../agent-eval-results/siftwire-v0.8.0-agent-eval-final.md)
is a historical Codex receipt. The
[first Pi partial run](../agent-eval-results/siftwire-v0.9.0-pi-sdk-candidate.md)
is historical evidence from the prior Pi-specific implementation. Neither is
neutral-contract validation or Pi promotion evidence. The new receipts above
preserve that history rather than overwriting it. The
[first neutral-adapter run](../agent-eval-results/siftwire-v0.9.0-adapter-pi.md)
also passed, but counted native tool starts. The current receipt supersedes it:
actions now originate in actual tool execution hooks, excluding native
pre-execution refusals without excluding executions that fail.
