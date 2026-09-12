# ADR: DB-Backed Configuration And State

## Status

Accepted.

## Context

SiftWire is intended to be open sourced. Personal source inventories, paywall
policy, delivery history, and latest-seen state must not be committed to the
repository or encoded in the skill.

Agents also need a narrow production interface that does not require source
inspection, workspace reads, direct SQLite queries, or legacy scripts for
routine runtime tasks. Repository development, docs review, tests, release
verification, security review, and migration design can still inspect public
repository files.

## Decision

SiftWire stores runtime configuration and mutable state in SQLite. The
database path is the storage anchor.

The canonical app-specific environment variable is `SIFTWIRE_DATABASE_PATH`.
The runner also accepts `--db` for explicit datasets and tests. If neither is
provided, it uses `${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`.

The repository seeds no personal sources, outlet policies, latest-seen state,
delivery records, or run history. A fresh database contains only schema and
runtime defaults. Operators configure sources through `siftwire config` or by
preparing a host database outside this repository.

Private historical artifacts are not automatically authoritative production
configuration. When an operator explicitly points to legacy automation or config
input, agents may inspect only the named input, draft sources and outlet
policies for review, and apply approved changes through `siftwire config`.
Delivery history, latest-seen state, and run state remain unsupported until the
runner provides an explicit import path.

Sources contain generic feed-processing settings: URL canonicalization, outlet
extraction, dedup group, and priority rank. Reporting thresholds determine
required items. These settings do not embed any operator feed inventory.

The current runner initializes an empty database and rejects incompatible
schemas. It does not migrate earlier databases or normalize retired source
settings on read.

## Consequences

- The shipped artifact can be public without private brief data.
- Routine production agents use runner JSON results instead of reading files.
- Local operators can keep private configuration in host storage.
- Repo development and migration design can inspect public repository files.
- User-directed legacy migration can draft reviewed sources and outlet policies.
- Operational state import remains outside the repository until it is
  implemented as a runner-backed feature.
