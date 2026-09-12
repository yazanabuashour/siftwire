# AgentOps Surface Policy

## Status

Accepted.

## Context

SiftWire serves agents through an installed JSON runner and a single-file
skill. The design must keep routine production work local, auditable, and
independent of repository or SQLite inspection while avoiding new APIs without
real callers.

## Decisions

### The process protocol is the reusable boundary

The supported building block is the one-shot `siftwire config|brief` JSON
process protocol. The skill, Prooflane adapter, and production eval harness all
use this boundary. No production caller imports Rust modules.

Keep `crates/siftwire/src/runner/`, `crates/siftwire/src/engine/`, and `crates/siftwire/src/storage/` private. A public Rust
library, alternate store, fetch plugin, daemon, or generic client crate would
add lifecycle and compatibility contracts without a real
consumer. Reconsider an in-process API only when an integration cannot
reasonably use the process protocol and supplies concrete lifecycle and error
requirements.

### Private modules own complete outcomes

Keep the process protocol as the reusable building block and deepen its private
modules around existing caller tasks:

- `crates/siftwire/src/engine/process.rs` owns each source outcome: collected
  content, the optional next marker, fetch-selection evidence, suppression, and
  fetch/date warnings.
  The runner skips marker reads for current news, applies permitted writes only
  for durable runs, and uses the outcome's status for both JSON and stored fetch
  evidence. It does not reconstruct marker or count policy.
- Delivery persistence accepts a prepared plan and derives sent records inside
  its transaction. The runner must not supply independent message/item copies or
  placeholder sent timestamps.
- Preparation reads run eligibility separately from archive details. An eligible
  run's existing plan replays before loading selection inputs or rendering. New
  plans retain storage's concurrent-insert conflict protection. Archive reads keep
  their own full-detail interface.

This replaces caller-owned sequencing rather than adding forwarding modules.
A public library or generic repository interface would add contracts without
improving these callers; file splitting alone would leave the same knowledge
spread across modules.

Safety still requires exact immutable bodies, delivery evidence references,
rejection after delivery, idempotent confirmation, and unchanged current-news and
Required marker behavior. Capability and user experience stay on the existing
process protocol: this redesign adds no actions or agent calls and makes no
performance claim. Process tests remain the external contract gate; focused
engine and storage tests protect the private outcome and transaction invariants.

### Sources stay generic

RSS and Atom share one feed model. GitHub releases remain a separate source kind
because they use a different fetch interface. Provider-specific feed source
kinds do not belong in the runner when generic URL canonicalization, outlet
extraction, deduplication, priority, and reporting thresholds express the task.

### Intake separates inspection from writes

A user-provided public feed, repository, topic, or named legacy configuration
can authorize focused inspection. It does not authorize a durable write.

- Infer mechanical fields such as source kind, key, label, and neutral defaults.
- Propose fields that affect grouping, ranking, suppression, or alerting.
- Require explicit approval before writing configuration through the runner.
- Never infer credentials, private source authority, destructive changes, or
  operational state.
- Do not import delivery history, latest-seen state, or run state without an
  explicit runner-backed import path.

### The skill routes tasks; the runner owns behavior

`skills/siftwire/SKILL.md` contains activation, safety boundaries, minimal
request shapes, source-intake guidance, and final-answer choreography. Runner
JSON carries task data and safe rejection reasons. Eval docs own scenario and
promotion evidence. Maintainer docs own repository, release, and security work.
Prooflane owns its adapter invocation and recovery guidance, not the installed
SiftWire skill or a SiftWire shell wrapper.

### No discovery API yet

Current `config` and `brief` actions cover proven tasks. Compact skill guidance
is sufficient for source intake today. Do not add a capabilities command,
draft API, or handoff action until repeated eval evidence shows that agents fail
or require unreasonable ceremony with the current surface.

## Taste review

Decision reports record safety, capability, and user experience separately. A
technically passing workflow can still carry taste debt when it needs many
calls, long latency, exact prompt choreography, surprising clarification, or
brittle manual sequencing. Prefer extending a natural existing runner action
over declaring an adjacent user task unsupported. A rejected implementation
does not by itself invalidate the need. User experience does not waive provenance,
source authority, safety, approval, or promotion evidence.

Before closing a non-promotion decision, search for existing follow-up work and
record any missing follow-up in the decision. This includes `keep-as-reference`,
`defer`, `more evidence`, `candidate selected`, and `none viable yet` outcomes.
When the evaluated shape fails but a capability, usability, safety, auditability,
or workflow need remains valid, document or propose candidate-surface comparison
before handoff. Normally compare 2–3 plausible shapes; explain when only one is
viable. The follow-up must choose or combine candidates, defer or kill the track,
or record `none viable yet`.

Known debt remains around exact-message delivery recording and some config
assembly. The exact delivered message is currently load-bearing for auditability,
history, and repeat suppression, so no shortcut is promoted without evidence
that preserves those properties.

Reopen a surface decision only when new evidence identifies a repeated failure,
unsafe ambiguity, or materially excessive ceremony and compares the current
protocol, compact guidance, an extension to an existing action, and a new action
where applicable.

## Repository evidence

Public artifacts stay repo-relative and free of personal source inventories,
outlet-policy evidence, delivery logs, run history, workspace backups, and local
SQLite databases. Validate the shipped skill, release documentation, runner
behavior, and production agent path with their repository-owned gates.
