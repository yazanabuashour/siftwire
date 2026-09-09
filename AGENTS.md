# Repository guide

## Project

SiftWire is a local-first Rust brief runtime for agents. Its supported production
boundary is the installed, one-shot `siftwire config|brief` JSON process plus
`skills/siftwire/SKILL.md`. Operator configuration and mutable state live in an
external SQLite database; they never belong in this repository.

## Code and architecture

- `crates/siftwire/src/main.rs` and `crates/siftwire/src/runner/` own command
  parsing, strict JSON framing, operator CLI subcommands, action orchestration,
  version output, and process exits.
- `crates/siftwire/src/paths.rs` selects the database path;
  `crates/siftwire/src/domain.rs` normalizes and validates sources and outlet
  policies.
- `crates/siftwire/src/engine/` owns feed fetching, canonicalization, selection, suppression,
  and health reporting.
- `crates/siftwire/src/storage/` owns SQLite schema migration and durable configuration, run,
  source, health, and delivery state.
- `skills/siftwire/` defines the production agent policy; `docs/runner-contract.md`
  defines the public process contract.
- `crates/siftwire/tests/` and colocated unit tests cover the process and implementation
  behavior. `scripts/ci.sh` is the main gate.
- `crates/siftwire/src/bin/siftwire-agent-eval/` and `docs/agent-eval-results/` own agent
  evaluation; release, security, and architecture guidance lives under `docs/`.
- `crates/siftwire-console/` is an optional local web console that spawns the
  runner per request over the process protocol; it must never open SQLite or
  import runner internals. Its TypeScript app lives in `apps/web`.
- Frontend work follows `apps/web`: React 19, Vite, Tailwind v4, TanStack
  Query, zod-decoded API contracts, oxlint/oxfmt through the shared policies in
  `tools/`, and Bun as the package manager.

## Working contracts

- For all committed docs, reports, and artifact references, use repo-relative paths or neutral repo-relative placeholders. Never use machine-absolute filesystem paths.
- Do work on the current branch. Do not create or switch to another branch unless explicitly instructed.
- For repo-pinned developer tools declared in `mise.toml`, run commands through `mise exec -- ...` so agents use the same tool versions as local docs and CI.

## Architecture and evaluation decisions

For architecture, proof-of-concept, evaluation, promotion, or deferred-capability
decisions, follow [AgentOps Surface Policy](docs/architecture/agentops-surface-policy.md).
It owns the inspection/write boundary and separate safety, capability, and user
experience judgments.
