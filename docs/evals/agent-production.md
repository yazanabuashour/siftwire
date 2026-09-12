# Production agent evaluation

The production agent evaluation exercises the JSON runner and shipped
`skills/siftwire/SKILL.md` with synthetic fixtures. It uses a checkout-built
`siftwire` binary in isolated workspaces, not an installed production database.
It simulates transport and does not send email.

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

## Model selection

The harness requires the shared `model-role` helper on `PATH`. Before workspace
setup, it invokes `model-role fast --codex` once in the caller's environment.
The helper validates that `fast.provider` is `openai-codex` and prints the resolved
model slug followed by a newline. It reads `AI_MODEL_ROLES_FILE` when set,
otherwise `${XDG_CONFIG_HOME:-$HOME/.config}/ai/model-roles.json`, with this shape:

```json
{
	"fast": { "provider": "openai-codex", "model": "<fast-model-slug>" },
	"deep": { "provider": "<deep-provider>", "model": "<deep-model-slug>" }
}
```

The shared configuration owns model selection. There is no default or fallback.
A missing helper, missing or invalid configuration, provider mismatch, or
malformed helper output stops the run before workspace setup or agent launch.
The resolved model stays fixed for every scenario and resumed turn, with
reasoning effort `medium`. Isolated child homes need neither the shared role
configuration nor the helper. Role changes take effect on the next run.

## Invocation and isolation

The harness uses a dedicated, least-privilege Codex home selected through
`SIFTWIRE_EVAL_CODEX_HOME`. Example setup and invocation:

```bash
CODEX_HOME="$HOME/.local/share/siftwire/eval-codex" codex login
export SIFTWIRE_EVAL_CODEX_HOME="$HOME/.local/share/siftwire/eval-codex"
mise exec -- ./scripts/run-agent-eval.sh run --scenario routine-agent-hygiene
```

The harness builds the runner once into `<run-root>/bin`. Each scenario receives
an empty workspace, a separate SQLite database, synthetic fixtures, raw logs,
and an isolated temporary directory. The shipped skill is installed once in each
scenario workspace at Codex's native project-skill path.

The default run root is under the user's cache directory. `--run-root` selects
another path, which must be outside the repository and either absent or marked
by an earlier harness run. The harness marks dedicated roots and holds an atomic
ownership lock for the full run. A concurrent owner fails before any scenario
directory is reset.

The harness links only the dedicated eval home's `auth.json` into an isolated
Codex home. It passes `--ignore-user-config`, disables login-shell profiles, and
gives tool subprocesses a strict non-secret environment without the auth path.
Single-turn scenarios use `--ephemeral`. Multi-turn scenarios persist only in the
isolated eval home.

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
	--report-dir docs/agent-eval-results \
	--report-name siftwire-v0.8.0-candidate
```

Raw logs, workspaces, databases, caches, and Codex state stay under `<run-root>`
and are not committed. Reduced reports scrub local paths to `<run-root>`. JSON
output and reduced JSON and Markdown reports record the resolved `model` and
`reasoning_effort`.

Reduced reports must identify the evaluator instruction. It is part of the
method, not hidden product guidance. Historical reports retain their original
scenario names, results, and model metadata. Tool and command metrics count
recorded events; started and completed events can both contribute. They are not
deduplicated execution counts or a measurement of operator effort.

The [v0.8.0 complete run](../agent-eval-results/siftwire-v0.8.0-agent-eval-final.md)
passed every current scenario. Run the full inventory again with a new report
name to preserve the earlier result:

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
	--report-dir docs/agent-eval-results \
	--report-name siftwire-v0.8.0-rerun
```
