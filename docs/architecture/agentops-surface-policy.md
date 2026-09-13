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

Keep `crates/siftwire/src/runner/`, `crates/siftwire/src/engine/`, and
`crates/siftwire/src/storage/` private. A public Rust library, alternate store,
fetch plugin, daemon, or generic client crate would add lifecycle and
compatibility contracts without a real consumer. Reconsider an in-process API
only when an integration cannot reasonably use the process protocol and supplies
concrete lifecycle and error requirements.

### Share email presentation, not runtime APIs

`crates/siftwire/src/runner/email.rs` maps private delivery evidence into
the standalone `email_ui::Document`. The isolated shared crate owns the fixed HTML
components and styles; it imports no SiftWire runner, engine, or storage APIs.
SiftWire still owns edition/date/time-zone labels, counts, grouping, and empty
section policy. Its process protocol remains the only production consumer
boundary.

- **Safety:** delivery preparation already normalizes links to credential-free
  absolute HTTP(S), substitutes empty links for invalid values, and filters
  images before mapping. The shared renderer validates its presentational
  inputs and performs no fetching. Prepared-plan persistence, replay,
  confirmation, and existing Markdown/plain-text/final-answer rendering remain
  unchanged; SiftWire consumes only the shared result's HTML.
- **Capability:** the shared components replace the previous private HTML
  implementation. Exact-byte assertions use [frozen synthetic outputs](../../crates/siftwire/tests/fixtures/email/README.md)
  captured before extraction, not expected strings regenerated from the new
  renderer.
- **User experience:** this is a presentation ownership change, not a redesign.
  No new production actions, calls, or configuration are required. Byte parity
  does not establish rendering in every mail client or real delivery success.

Copying the template into each caller would leave style changes independently
maintained. Sharing the runtime would expose unrelated state and lifecycle
contracts. The presentational document is the smallest shared contract needed
by SiftWire and Mailgate's offline rendering caller. Cargo pins the shared crate
to an email-ui git revision; development-only sibling paths must not ship.

### Private modules own complete outcomes

Keep the process protocol as the reusable building block. Its private modules
own complete caller tasks:

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

These modules replace caller-owned sequencing rather than forward calls.
A public library or generic repository interface would add contracts without
helping these callers. Splitting files alone would leave the same knowledge
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

### The evaluator owns a vendor-neutral executable boundary

Replacing Codex with Pi still coupled the evaluator to an agent runtime. The
accepted boundary is now a local executable with SiftWire-defined request and
result objects. The production runner and `skills/siftwire/SKILL.md` remain
unchanged and harness-independent. The Pi SDK remains one adapter implementation,
not the evaluator's public boundary.

| Candidate | Safety | Capability | User experience | Decision |
| --- | --- | --- | --- | --- |
| Small direct runtime integration | Can preserve controlled resources and native completion, but spreads vendor auth, event, and session assumptions into the evaluator. | Covers the selected runtime; replacing it requires evaluator changes. | Initially direct, but changing vendors also changes evaluator configuration and evidence handling. | Superseded; swapping Codex for Pi did not remove coupling. |
| SiftWire-owned executable adapter | Keeps native infrastructure in trusted leaves; Rust retains normalized hygiene and exact delivery verification. Same-account execution is not an OS sandbox. | Covers fixed scenarios with complete action receipts; each leaf owns its tool loop and continuity. | Requires one explicit executable selection; wrapper or environment configuration stays adapter-local. | Selected. It owns the contract needed by this caller without owning a general agent platform. |
| Generic agent framework | Adds trust and configuration surfaces without stronger isolation or receipt guarantees for this caller. | Generalizes providers, tools, and sessions beyond the fixed scenarios. | Adds framework concepts and configuration without demonstrated caller benefit. | Rejected; no real caller justifies the larger interface. |

Rust owns the checkout runner build, synthetic fixtures, scenario prompts,
workspace and candidate resources, requests, reports, temporary resources,
databases, run-root lock, hygiene, and exact immutable-delivery verification.
It requires `--adapter executable` and launches it without shell interpolation
or executable arguments. It knows no Pi configuration, Bun, model flags, native
events, or session format.

One request contains one entire fixed scenario. The adapter owns multi-turn
continuity internally and returns one JSON result only after actual native
completion; diagnostics use stderr and nonzero exit means failure. One request
per scenario removes the need for session IDs, resume commands, and native file
references in the neutral protocol. Strict fields, protocol and turn
counts, exact final strings, nullable assistant execution counts, and complete
action receipts define the contract. Unknown execution counts are `null`, not
zero. Runtime identity is adapter-reported per scenario, not a hardcoded global
Pi model.

The adapter host may retain provider environment variables, with run-local
`TMPDIR`; shell tools receive only the supplied clean `tool_env`. Adapters must
use supplied workspace and candidate resources and truthfully report all
actions. This is configuration separation, not an OS sandbox. Hygiene checks
inspect reported execution after the fact and cannot establish that a dishonest
same-account adapter omitted nothing. Private artifacts and credentials never
belong in committed evidence.

The earlier SDK choice still applies inside the Pi leaf: explicit resources,
in-memory settings, native model/auth handling, controlled tools, and awaited
completion fit this implementation. The previously considered Pi CLI did not
provide the required resource exclusion while preserving native credential-lock
identity. That does not make the SDK the neutral boundary. The Pi adapter owns
Bun, native event parsing, personal auth and model defaults, the optional exact
`SIFTWIRE_PI_MODEL` override, and fixed `medium` reasoning without fallback.
Configuration resolves once per scenario; turns share an in-memory session.
Credentials retain their native file paths and locks without copies or symlinks;
personal resource discovery remains disabled.

A raw API adapter is not categorically excluded, but it must own any necessary
tool loop, completion validation, and complete receipts. Final-only API output
cannot claim agent-eval equivalence. Do not introduce a universal model-only
interface, harness registry, or session abstraction until a real caller needs
one. The deterministic stub is a separate, Pi-free transport smoke implementation
for `routine-agent-hygiene`, not full capability or production evidence.

The [adapter contract](../evals/agent-adapter.md) owns request/result and leaf
implementation rules. The [production evaluation guide](../evals/agent-production.md)
owns scenarios, invocation, verification, and report semantics.

This approves the boundary, not release promotion. The
[first Pi receipt](../agent-eval-results/siftwire-v0.9.0-pi-sdk-candidate.md)
remains historical: two scenarios passed before provider usage-limit failures
under the earlier Pi-specific implementation. Historical Codex and Pi reports
retain their original formats and receipts; no compatibility layer is needed
for the unpublished previous eval format. Neutral-contract tests and repository
CI passed. The [independent stub smoke](../agent-eval-results/siftwire-v0.9.0-adapter-stub.md)
passed without Pi, and the [execution-receipt Pi inventory](../agent-eval-results/siftwire-v0.9.0-adapter-pi-execution-receipts.md)
passed all 13 scenarios. Safety and capability passed within the synthetic
boundary; the evaluation guide records the separate user-experience judgment.
These receipts do not establish real delivery success or authorize a production
interface change.

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
does not by itself invalidate the need. User-experience improvements do not waive
provenance, source authority, safety, approval, or promotion evidence.

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
