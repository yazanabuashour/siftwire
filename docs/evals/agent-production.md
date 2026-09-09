# Agent Production Eval Protocol

SiftWire evals measure the production path: one checkout-built `siftwire`
binary plus the shipped `skills/siftwire/SKILL.md`.

## Coverage

The current harness exercises 14 scenarios from
`crates/siftwire/src/bin/siftwire-agent-eval/scenarios.rs`:

- empty configuration rejection
- RSS first-run selection and repeat suppression
- repository-only GitHub release configuration without fetching
- RSS required-item delivery with `threshold: "always"`
- prepared delivery confirmation and recent suppression
- generic feed processing with `title_suffix` and watch outlet-policy audit
- configured delivery limits
- brief history rendering
- feed failure and recovery health changes
- invalid source rejection
- routine runner-only agent hygiene

The implementation is the scenario inventory. Update this summary when adding or
removing a scenario.

## Gate

A release is ready only when the selected scenarios pass, fixtures remain
synthetic, agents use runner JSON only, and production bypass requests receive a
final-answer-only rejection.

Report three conclusions separately:

- **Safety:** source authority, provenance, local-first state, runner-only
  production access, and approval-before-write were preserved.
- **Capability:** current runner actions could complete the scenario.
- **User experience:** note high step count, latency, brittle prompt
  choreography, surprising clarification, or delivery/config ceremony even when
  the scenario passed.

An eval pass does not by itself prove good user experience or authorize a new
runner surface.

## Run the harness

The harness builds the runner once from the current checkout into
`<run-root>/bin`. Each scenario receives an empty workspace, a separate SQLite
database, synthetic fixtures, raw logs, and an isolated temporary directory.
The harness installs the shipped skill once in each isolated scenario
workspace, matching Codex's native project-skill path.

Install the shared `model-role` helper on `PATH`. At run admission, the harness
invokes `model-role fast --codex` once in the caller's environment. The helper
validates that `fast.provider` is `openai-codex` and prints the resolved model
slug followed by a newline. It reads `AI_MODEL_ROLES_FILE` when set, otherwise
`${XDG_CONFIG_HOME:-$HOME/.config}/ai/model-roles.json`, with this shared shape:

```json
{
  "fast": { "provider": "openai-codex", "model": "<fast-model-slug>" },
  "deep": { "provider": "<deep-provider>", "model": "<deep-model-slug>" }
}
```

The shared configuration owns model selection. The eval has no default model or
fallback. A missing helper, missing or invalid configuration, provider mismatch,
or malformed helper output stops admission before workspace setup or agent
launch. The resolved model stays fixed for every scenario and resumed turn in
that run, with reasoning effort `medium`. Isolated child homes need neither the
shared role configuration nor the helper. Changes to the shared role take effect
on the next run.

Provision a dedicated, least-privilege Codex home once, then select it explicitly:

```bash
CODEX_HOME="$HOME/.local/share/siftwire/eval-codex" codex login
export SIFTWIRE_EVAL_CODEX_HOME="$HOME/.local/share/siftwire/eval-codex"
mise exec -- ./scripts/run-agent-eval.sh run --scenario routine-agent-hygiene
```

The default run root is under the user's cache directory. An explicit run root
must be outside the repository and either absent or marked by an earlier harness
run. The harness marks dedicated roots and holds an atomic ownership lock for
the full run; a concurrent owner fails before any scenario directory is reset.
The harness links only the dedicated eval home's `auth.json` into an isolated
Codex home, passes `--ignore-user-config`,
gives tool subprocesses a strict non-secret environment without the auth path,
and disables login-shell profiles. Single-turn scenarios use `--ephemeral`;
multi-turn scenarios persist only in the isolated eval home.

Every prompt receives an evaluator instruction requiring runner-only production
behavior, `siftwire-runner/v3`, and `prepared-delivery/v1`. The host fixture
simulates successful transport of the unchanged prepared `message`, `text`, and
`html`, then asks the agent to confirm the plan. No real email or external
delivery occurs. The verifier requires every recorded message to equal its
immutable plan, checks prepared and sent items against required and selected
run evidence, and compares final brief answers with the stored current and
prior bodies. It does not reconstruct item Markdown or score bullet counts.
These evidence checks cover normal feed items, not sports updates. Reduced
reports must identify the evaluator instruction; it is part of the eval method,
not hidden product guidance.

The generic-processing and watch-policy scenarios use synthetic publisher
metadata in the feed title suffix, `Fixture Outlet`. They check retained
candidates and watch annotations without relying on removed extraction modes.
GitHub custom source URLs are no longer supported, so the offline GitHub
scenario checks repository-only configuration and forbids fetching. RSS
fixtures cover required-item delivery separately. This harness does not measure
GitHub fetching or real transport success; do not treat its pass as evidence
for either. Historical eval reports retain their original scenario names and
results.

## Reports

Write reduced reports only with an explicit name:

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
  --report-dir docs/agent-eval-results \
  --report-name siftwire-v0.2.0-candidate
```

Raw logs, workspaces, databases, caches, and Codex state stay under `<run-root>`
and are not committed. Reduced reports scrub local paths to `<run-root>`.
The JSON output and reduced JSON and Markdown reports snapshot the resolved
`model` and `reasoning_effort`. Historical evidence remains unchanged.
