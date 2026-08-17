# Prooflane Shadow Dogfooding

Prooflane shadow mode observes Siftwire's supported installed JSON runner
without changing brief selection, local delivery recording, or the final
answer. It does not read Siftwire's SQLite database and does not add a second
Siftwire state store.

This integration is opt-in. Routine production use remains `siftwire config`
and `siftwire brief` as documented by the Siftwire skill.

## Local-checkout Preflight

This is a local dogfood path, not a public Siftwire release or installation
contract. Before rollout, build the current Siftwire checkout into the active
local binary, install its matching `skills/siftwire/SKILL.md`, use the sibling
Prooflane checkout, and confirm:

- the active `siftwire` binary was built from this checkout and is configured
  against the existing database
- `../prooflane/bin/prooflane` is executable and includes the `dogfood
  siftwire` adapter commands
- a reviewed Prooflane Siftwire contract exists at `<prooflane-contract>`
- the normal `SIFTWIRE_DATABASE_PATH` environment, when the default Siftwire
  database is not intended, still points to the existing database
- `PROOFLANE_HOME` points to an explicit, private Siftwire-only directory that
  has never held OpenBrief state

The executable checks can be performed without running a brief:

```sh
test -x scripts/prooflane-shadow-siftwire.sh
test -x ../prooflane/bin/prooflane
test -x "$SIFTWIRE_BINARY"
"$SIFTWIRE_BINARY" --version
```

The repository helper honors an explicit `PROOFLANE_BINARY`; otherwise it uses
the executable in the sibling Prooflane checkout before falling back to
`prooflane` from `PATH`. `SIFTWIRE_BINARY` selects the existing Siftwire
binary and defaults to `siftwire` from `PATH`. Each value must name one
executable, not a shell command with arguments.

If the existing automation used `siftwire brief --db <path>`, export that same
path as `SIFTWIRE_DATABASE_PATH` before invoking the helper. The shadow wrapper
does not accept embedded binary arguments; dropping `--db` without the matching
environment variable would split run and delivery state.

## Routing And Limits

Prooflane publishes current observations under the `siftwire` identifier. Its
`openbrief` command and stored-context support remain compatibility surfaces for
historical runs; new Siftwire automation must not create OpenBrief contexts.

Only two actions pass through Prooflane shadow mode:

- `run_brief` uses the helper's `run` command.
- `record_delivery` uses the helper's `delivery` command.

Keep `validate` on `siftwire brief`, and keep every configuration action on
`siftwire config`. Prooflane's internal `inspect_config` observation has a
30-second timeout and 4 MiB output limit. The observed `run_brief` and
`record_delivery` calls each have a 120-second timeout and 16 MiB request and
output limits.

## Rollout And Retry Ownership

Replace the two direct invocations inside the existing scheduled job; do not
run a second mirrored Siftwire job. The same scheduler remains the sole owner
of the same `SIFTWIRE_DATABASE_PATH`, and the shadow helper invokes Siftwire
exactly once for each routed action.

`run_brief` advances Siftwire's latest-seen state and is not safe for an
automatic retry after an uncertain child outcome. After a successful run,
continue with its reported Siftwire and Prooflane run IDs instead of invoking
`run` again. Delivery context is claimed before `record_delivery`. Concurrent
reuse fails before a second runner call, and claims never expire by wall-clock
age. A nonzero runner exit releases its claim. After an interruption, first
confirm that the runner process has settled and determine whether its durable
side effect completed. Then release the claim explicitly:

```sh
PROOFLANE_HOME=<siftwire-prooflane-home> \
  prooflane dogfood siftwire recover-delivery --run <prooflane-run-id>
```

Retry only when Siftwire's durable idempotency record makes that safe, using the
same Prooflane run ID, Siftwire run ID, and exact message. Siftwire returns the
existing local delivery for an identical retry and rejects a different message.
A completed claim cannot be reused.

## Run A Brief

Pipe the unchanged `run_brief` request through the shadow helper:

```sh
printf '%s\n' '{"action":"run_brief","dry_run":false}' |
  scripts/prooflane-shadow-siftwire.sh run <prooflane-contract>
```

The command is a thin delegation to:

```sh
prooflane dogfood siftwire run \
  --contract <prooflane-contract> \
  --config-binary siftwire \
  --brief-binary siftwire
```

Stdout is the unchanged Siftwire JSON result. Prooflane run metadata is the
final stderr record, framed as `PROOFLANE_METADATA<TAB>{...}` so it remains
distinguishable from runner diagnostics. Retain the Prooflane run ID separately
from Siftwire's `run_brief.run_id`; the two IDs have different purposes.

Build the exact current brief from the Siftwire result using the normal skill
rules. Shadow mode must not choose items or alter the message.

## Record The Local Delivery

Pipe the unchanged `record_delivery` request through the helper, using the
Prooflane run ID reported by the first command:

```sh
printf '%s\n' \
  '{"action":"record_delivery","run_id":"<siftwire-run-id>","message":"NO_REPLY"}' |
  scripts/prooflane-shadow-siftwire.sh delivery <prooflane-run-id>
```

The command delegates to:

```sh
prooflane dogfood siftwire delivery \
  --run <prooflane-run-id> \
  --brief-binary siftwire
```

Stdout remains the Siftwire `record_delivery` JSON result and remains the only
source for the final answer. Prooflane receipt metadata uses the same final
stderr frame. The adapter atomically claims a valid run context before invoking
`record_delivery`, so a concurrent reuse fails before a duplicate local record.
Missing or unparsable Prooflane context falls back to the unchanged runner call
and reports `proof_error:true` in metadata.

## Current Proof Boundary

The adapter can currently observe and derive five assertions:

- strict runner protocol validity
- correlation of Siftwire's run and local delivery record
- complete enabled-source fetch coverage with no failed source
- conformance of the exact recorded message to selection and delivery-limit
  rules
- successful local delivery recording

When all currently available observations pass, the shadow verdict ceiling is
`unverified`, not `verified`, because two required proofs are intentionally
unavailable. An observed execution or assertion failure may still produce a
`failed` verdict. The missing proofs are:

- Siftwire does not expose digest-only stable item identities or latest-seen
  state before and after the run.
- `record_delivery` commits Siftwire's local audit record before the host
  delivers the final answer; it is not a downstream message acknowledgment.

Do not fill either gap with direct database reads, raw private state, inferred
identities, or a producer-authored delivery claim. A later runner extension may
expose digest-only identity transitions. External delivery requires an
acknowledgment from a host-controlled adapter.

## Dogfood Data Discipline

Prooflane may retain approved counts, booleans, timestamps, verdicts, blocker
IDs, and SHA-256 digests. It must not copy source URLs, source inventories,
database paths, raw runner output, brief bodies, delivery history, or private
latest-seen values into repository artifacts or telemetry.

Shadow failures must not change the Siftwire answer. Report the Prooflane
diagnostic separately and keep the Siftwire runner result as the authoritative
production result. Retry delivery only with the same IDs and exact message
under the bounded rule above; never retry `run_brief` after an uncertain result.
