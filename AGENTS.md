# Repository guide

## Project

Siftwire is a local-first Rust brief runtime for agents. Its supported production
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

# ADR/POC/Eval Decision Taste Review

When doing Siftwire ADR, POC, eval, promotion, or deferred-capability decision work, keep the existing evidence discipline but add a taste check before accepting a defer/reference outcome:

- Ask whether a normal user would expect a simpler Siftwire surface than the one being preserved.
- Distinguish read/fetch/inspect permission from durable configuration or write approval. User-provided public feeds, release sources, or named migration inputs can justify inspection; durable config writes and private or state imports still require explicit approval and runner support.
- Prefer extending the natural existing runner action when the input clearly belongs there, instead of declaring the adjacent UX unsupported.
- Treat "completed but ceremonial" eval passes as possible taste debt when they require high step count, long latency, exact prompt choreography, or surprising clarification turns.
- Record safety pass, capability pass, and UX quality separately when a report or decision needs to justify defer/reference.
- When taste debt, defer, keep-as-reference, or another non-promotion outcome still leaves a real capability, ergonomics, safety, auditability, or workflow need, identify whether the evaluated shape failed while the need remains valid. If it does, document or propose follow-up candidate-surface comparison work before handoff, normally with 2-3 plausible shapes unless the decision explains why only one is viable. The follow-up must compare candidates, choose the best, combine useful behaviors if appropriate, defer or kill the track, or record `none viable yet`.
- Before closing any ADR, POC, eval, promotion, or deferred-capability decision with outcome `keep-as-reference`, `defer`, `more evidence`, `candidate selected`, `none viable yet`, or another non-promotion result, search the repository for existing follow-up work. If none exists, record the needed follow-up in the decision before handing off.
- Do not use taste review to bypass safety or evidence discipline: provenance, source authority, auditability, local-first behavior, runner-only production access, private-state boundaries, approval-before-write, and ADR/POC/eval/promotion decisions still apply.
