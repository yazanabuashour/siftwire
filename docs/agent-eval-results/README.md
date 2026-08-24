# Agent Eval Results

Committed files in this directory are reduced release evidence. They are not a
substitute for running the current harness against a candidate release.

## Historical evidence

- `docs/agent-eval-results/siftwire-v0.1.0-final.md`
- `docs/agent-eval-results/siftwire-v0.1.0-final.json`

These immutable reports cover the selected v0.1.0 runner and skill. They do not
prove current HEAD or later releases. Their method predates the current report
requirement to identify the runner-only evaluator instruction.

## Siftwire v0.2.0 evidence

- `docs/agent-eval-results/siftwire-v0.2.0-final.md`
- `docs/agent-eval-results/siftwire-v0.2.0-final.json`

All 13 scenarios passed safety and capability checks. The run took 444.77
seconds and 100 command executions. Manual taste review accepts the workflow for
the rename release but retains the known configuration and exact-delivery
ceremony in `docs/architecture/agentops-surface-policy.md`; the evaluated shape
did not fail, so this result does not promote another runner surface.

## Current method

See `docs/evals/agent-production.md`. New reports must use an explicit report
name, synthetic fixtures, neutral `<run-root>` placeholders, and separate
safety, capability, and user-experience conclusions.

Raw Codex logs, workspaces, local databases, caches, and isolated Codex homes
must not be committed.
