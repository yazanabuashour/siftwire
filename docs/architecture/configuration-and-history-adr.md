# Configuration and history ownership

## Protocol v5 supersession

The [v5 compatibility reset](../runner-v5-reset.md) removes legacy source-read
adapters, readable saved publisher conflicts, and historical title/URL matching.
It requires a fresh database and direct delivery evidence references. The v3
amendment and original decision below preserve historical rationale, not current
compatibility guarantees. Current contracts live in the
[runner guide](../runner-contract.md) and [run history](../run-history.md).

## Historical protocol v3 amendment

The approved removals in [runner v3 migration](../runner-v3-migration.md)
supersede this decision's writable Atom/always-report/Observe choices. Current
configuration reads adapt the equivalent legacy Atom and required-feed fields;
raw stored rows and historical evidence remain untouched until an approved
source write. New writes use the narrower contract. The archive, publisher, exact
email, and priority ownership decisions below remain in force.

## Original decision

Extend the existing runner results and operator history commands. Keep raw source
configuration, the collection-based publisher write, and immutable delivery plans.
Agents inspect configuration through `config.inspect_config`; operators read
evidence through the `runs` commands.

## Why reopen this decision

The earlier field-policy and diagnostic-handoff comparison favored guidance.
At the time of this decision, the implementation exposed contradictions that
guidance could not fix:

- A release can be required while its raw `always_report` flag is false.
- Sports delivery titles differ from compatibility candidate titles. Exact
  title-and-link comparison can report a delivered fixture as absent.
- An unresolved link can retain its original item, so its warning does not prove
  an exclusion.
- Filtering delivered runs after a limited activity query can hide the latest
  delivery. Run start time also differs from delivery time.
- Separate latest and sent-history pages discard local selection on navigation.

These are contract and ownership problems, not a reason to add a rule engine.

## Candidate comparison

| Candidate | Safety and preservation | Capability | User experience | Decision |
| --- | --- | --- | --- | --- |
| Keep raw controls and add guidance | Preserves storage, but leaves contradictory evidence | Cannot repair archive ordering or identify delivered sports | Requires users to interpret internal precedence | Reject |
| Replace source fields with provider-specific schemas and a rules language | Requires configuration migration and new compatibility rules | More expressive than the current task needs | Adds configuration concepts before removing existing ones | Reject |
| Compute effective policy in the runner, record delivery references, and extend existing history queries | Preserves raw fields and old plans; can expose historical uncertainty | Supports accurate configuration summaries, evidence, and archive retrieval | Consistent reporting and explicit writes through the existing process interface | Adopt |

The adopted design keeps decisions in the runner without adding an agent action.

## Ownership and preservation

- The domain computes effective reporting. Collection and configuration results
  use the same decision. Unrelated edits do not normalize source fields.
- Priority remains a signed 64-bit integer in the runner. It ranks ordinary
  duplicate representatives, not editorial importance.
- Feed handling remains generic rather than introducing provider-specific schemas.
- Publisher notes remain optional metadata, not runtime instructions. Watch and
  Allow retain items and annotate matches. New writes reject ambiguous enabled
  name/alias matches; existing ambiguous configuration remains readable and keeps
  its prior matching order until the operator changes it.
- New delivery plans reference recorded items. Older evidence uses recorded
  context only where it proves a match. An unknown outcome is not a rejection.
  No historical HTML or plan is regenerated.
- Warnings and publisher annotations are separate from exclusions. New fetch
  records retain source labels. Old records never borrow today's renamed label.
- The runner filters, orders, searches, and paginates the archive. A missing
  explicit record is an error, not permission to switch.
- Publisher replacements require approval for the complete collection. Partial
  mutation results describe only the corresponding configuration collection.

Seen versus sent, ordinary required items versus recurring sports, and prepared
versus confirmed delivery remain separate. The change does not renumber sources,
replace sources during migration, change deduplication scope, or turn observation
into a review backlog.

## Validation and limits

Repository tests cover reporting precedence, preservation, archive retrieval,
delivery evidence, and publisher conflicts. They do not establish agent success
rates or performance claims.

The runner changes address the underlying need; rejecting the alternatives does
not defer it. Existing source-intake and agent workflow
decisions remain governed by [agentops-surface-policy.md](agentops-surface-policy.md).
