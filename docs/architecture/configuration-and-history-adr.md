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
source write. New writes use the narrower contract. Archive, publisher, exact
email, priority and partial-cache ownership decisions below remain in force.

## Original decision

Extend the existing runner results and operator history commands. Keep raw source
configuration, the collection-based publisher write, and immutable delivery plans.
The console presents those contracts around reading and configuration tasks.

## Why reopen this decision

The earlier field-policy and diagnostic-handoff comparison favored guidance.
The current implementation exposes contradictions that guidance cannot fix:

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
| Compute effective policy in the runner, record delivery references, and extend existing history queries | Preserves raw fields and old plans; can expose historical uncertainty | Supports accurate configuration summaries, evidence, and archive retrieval | One reporting choice, one reader, one picker, explicit saves | Adopt |

The adopted design combines runner-owned decisions with a smaller console.
It does not promote a new agent action or expand the installed skill.

## Ownership and preservation

- The domain computes effective reporting. Collection and configuration results
  use the same decision. The console translates an explicit reporting edit into
  existing fields, but does not normalize those fields during unrelated edits.
- Priority remains a signed 64-bit integer in the runner and decimal text in the
  browser. It ranks ordinary duplicate representatives, not editorial importance.
- Feed handling remains generic. Feed-only controls do not appear for releases
  or schedules. Existing RSS versus Atom spelling survives unrelated edits.
- Publisher notes remain optional metadata, not runtime instructions. Watch and
  Allow retain items and annotate matches. New writes reject ambiguous enabled
  name/alias matches; existing ambiguous configuration remains readable and keeps
  its prior matching order until the operator changes it.
- New delivery plans reference recorded items. Older evidence uses recorded
  context only where it proves a match. An unknown outcome is not a rejection.
  No historical HTML or plan is regenerated.
- Warnings and publisher annotations are separate from exclusions. New fetch
  records retain source labels. Old records never borrow today's renamed label.
- The runner filters, orders, searches, and paginates the archive. The console
  owns the selected run in the URL and fetches explicit IDs independently of the
  current page. A missing explicit record is an error, not permission to switch.
- Publisher edits remain a local collection draft until Save. Normalized partial
  mutation results update only the corresponding configuration collection.

Seen versus sent, ordinary required items versus recurring sports, and prepared
versus confirmed delivery remain separate. The change does not renumber sources,
replace sources during migration, change deduplication scope, or turn observation
into a review backlog. The console still invokes the runner and never opens SQLite.

## Validation and limits

Repository tests cover reporting precedence, preservation, archive retrieval,
delivery evidence, publisher conflicts, and picker/configuration interactions.
Browser checks use a disposable fictional database. They establish the console
workflow, not agent success rates or performance claims.

The rejected shapes do not defer the underlying need. The adopted runner and
console changes address it. Existing source-intake and agent workflow decisions
remain governed by [agentops-surface-policy.md](agentops-surface-policy.md).
