# Operator Console ADR

## Status

Accepted.

## Context

The visibility-surface plan sequenced Siftwire operator tooling: CLI commands
first, then a read-only viewer, then a configuring dashboard when a concrete
task demanded writes. The concrete task now exists: the operator wants to view
and edit sources, inspect deliveries including past ones, and review
candidate selection from a browser. Prooflane already ships a loopback
dashboard, and other personal automations could each want one; three bespoke
dashboards would make a shared layer the obvious design.

## Decisions

### Per-project console now; shared layer only on evidence

The console lives in this repository as `siftwire-console`. The candidate
shared layer would have to reconcile Prooflane's Gleam/mist dashboard with a
Rust server and two speculative consumers. That is machinery without callers.
If Firesale and another automation both commission dashboards, compare a shared
layer then; the process-boundary access pattern below is designed to survive
that comparison.

### The runner process stays the only boundary

Every request spawns the installed `siftwire` binary: reads use the
`inspect_config` action and the `runs` CLI JSON, writes use `upsert_source`,
`delete_source`, `replace_outlet_policies`, and `set_brief_options`. The
console holds no SQLite dependency and duplicates no storage code. This keeps
the AgentOps surface policy intact without a daemon inside the released
runner binary.

### Writes are limited to configuration

Sources, outlet policies, and brief options are writable because the runner
already owns those validated actions. Run execution is not exposed: a dashboard
button for `run_brief` would make its not-retry-safe latest-seen transition one
accidental click away.

### Serving model matches the house pattern

Axum serves the JSON API under `/api/v1` and static files built from
`apps/web/dist`, mirroring Sourcebound. Loopback binding is the default; any
wider exposure is an explicit bind override by an operator on a trusted
network, consistent with the homelab posture that adds TLS before non-LAN
access.

## Consequences

- The runner binary keeps its lean dependency tree; HTTP dependencies live in
  the separate console crate behind a workspace boundary.
- Console features that need new data (per-run selection evidence) required
  runner-side persistence and CLI output, which landed first.
- No authentication exists. If the console ever leaves a trusted LAN, TLS and
  authentication become prerequisites, not options.
