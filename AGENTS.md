# Repository guide

## Project

SiftWire is a local-first Rust brief runtime for agents. Production tasks use
the installed, one-shot `siftwire config` or `siftwire brief` JSON process with
`skills/siftwire/SKILL.md`. Operator configuration and mutable state live in an
external SQLite database, never in this repository.

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
- `crates/siftwire/src/bin/siftwire-agent-eval/` owns the vendor-neutral evaluator;
  `tools/agent-eval/` owns its executable adapters and shared contract decoding.
  `docs/evals/agent-adapter.md` defines that boundary; `docs/agent-eval-results/`
  holds evaluation receipts. Pi is one development-only adapter implementation;
  release, security, and architecture guidance lives under `docs/`.
- `crates/siftwire-console/` is an optional local web console that spawns the
  runner per request over the process protocol; it must never open SQLite or
  import runner internals. Its TypeScript app lives in `apps/web`.
- Frontend work follows `apps/web`: React 19, Vite, Tailwind v4, TanStack
  Query, zod-decoded API contracts, oxlint/oxfmt through the shared policies in
  `tools/`, and Bun as the package manager.

## Working contracts

- Use repo-relative paths or neutral repo-relative placeholders in committed docs, reports, and artifact references. Never use machine-absolute filesystem paths.
- Work on the current branch. Do not create or switch branches unless explicitly instructed.
- Run tools pinned in `mise.toml` through `mise exec -- ...` to use the same versions as local docs and CI.

## Architecture and evaluation decisions

For architecture, proof-of-concept, evaluation, promotion, or deferred-capability
decisions, follow [AgentOps Surface Policy](docs/architecture/agentops-surface-policy.md).
It owns the inspection/write boundary and separate safety, capability, and user
experience judgments.
