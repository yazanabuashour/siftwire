# Apply the v3 migration from a v2 installation

Use this guide when upgrading v0.6.1 or another `siftwire-runner/v2` installation.
The v3 changes remove unused source options and manual delivery writes. They
also apply when upgrading directly to v0.8.0 and `siftwire-runner/v4`. Complete
the [v4 migration](runner-v4-migration.md) as well. No intermediate runner
installation is required.

Update the runner, console, skill, and automated consumers together. A binary
version alone does not establish protocol compatibility. Published v2 tags keep
their original contracts.

## Prepare existing configuration

Inspect configuration with the installed runner before upgrading. Back up the
external database through your normal backup procedure. Keep private inventories
and backups outside the repository.

Use individual `upsert_source` requests for approved changes. Do not use
`replace_sources` as a migration shortcut. Replacement deletes sources and can
also delete their latest-seen state.

| v2 choice | Supported choice or required decision |
| --- | --- |
| `kind: atom` | Use `rss`. The feed parser still auto-detects Atom documents. |
| `always_report: true` | Use `threshold: always` for normal feeds, then omit the flag. Releases remain required by kind. |
| `threshold: audit` | Disable the watch with v2 before upgrading, or explicitly choose `high`, `medium`, or `always` if it should produce items. |
| `schedule_format: espn_core` | Choose an equivalent ESPN schedule or scoreboard endpoint, or disable the source with v2. Compare coverage before changing it. |
| `feedburner_redirect` | Choose a direct feed or explicitly accept unresolved links with `none`. |
| `outlet_extraction: url_host` or `rss_source` | Use `title_suffix` only when feed titles actually supply publisher names, or accept no extraction with `none`. Recheck publisher policy coverage. |
| GitHub release `url` override | Supply `repo: owner/name` and remove `url`. Only the derived public releases endpoint is supported. |

Reads adapt stored Atom kind to `rss` and a stored true `always_report` flag to
an effective `always` threshold. They do not rewrite source rows, identities,
state, or timestamps. An approved upsert stores the effective current fields and
clears the legacy flag. Historical item rows retain their original kind,
threshold, flag, and reporting interpretation.

Other unsupported stored choices remain inspectable, including disabled audit
watches. They are not accepted by new writes. Choose supported settings before
saving such a source. An enabled unsupported source stops `run_brief` before
fetching or creating a run, rather than silently changing its behavior. The
`validate` action checks runtime and database initialization, not source
compatibility.

## Update callers

For v0.8.0, require `siftwire-runner/v4` and the capabilities in the
[v4 migration guide](runner-v4-migration.md). If maintaining a historical v3
consumer, require `siftwire-runner/v3` and its task capabilities explicitly.
Test each supported protocol. Do not accept arbitrary future protocol strings.

- Replace `source list` with `config.inspect_config`. Filter enabled sources in
  the caller when needed.
- Replace `source add` with `config.upsert_source`. The latter returns a config
  envelope and can report domain rejection at exit zero. Check `rejected`.
- Remove `always_report` from source requests, including `false` and `null` values.
  Unknown request fields fail decoding.
- Replace manual `record_delivery` with `prepare_delivery`, transport of the
  exact returned bodies, and `confirm_delivery` after transport acceptance.
  `message` is no longer a brief request field.
- Keep recent context for editorial selection. Prepared delivery does not make
  `previous_briefs`, suppression evidence, or archive reads obsolete.

The SiftWire-owned Prooflane shell wrapper and mock forwarding test are retired.
Prooflane owns adapter invocation, compatibility, and recovery guidance. Before
upgrading, verify that the deployed adapter and launcher support the target
protocol. A source checkout update does not update a separately pinned runtime.

## Preserve deliveries and recovery

Do not rerun a brief with an uncertain outcome. Keep its durable host markers
and resolve the original outcome. Do not discard pending transport or delivery
claims as part of migration.

Prepared plans still contain immutable Markdown, plain text, and HTML. Confirming
the same plan remains retry-safe. The runner's shared delivery store, sent-item
evidence, and local idempotency remain in place.

Completed manual deliveries remain readable through the archive. Old plans and
emails are never regenerated or rewritten. Recorded Markdown remains the reader
fallback when old deliveries lack saved HTML. Missing historical evidence stays
unknown.

An unfinished old run without saved delivery context cannot be prepared by v3
or v4. If an active workflow still needs that run, reconcile it using the
matching older runner and its established recovery process before upgrading. An inactive
historical row does not require replay merely to upgrade. Do not invent a modern
plan for it or collect a replacement run automatically.

## Verify the change

For checkout validation, run `mise exec -- ./scripts/ci.sh` and the
[production-agent scenarios](evals/agent-production.md). Test each actual
consumer against its target protocol, including transport ordering and recovery,
before deployment.

Compare configuration, latest-seen state, and retained exact delivery bodies on
a separate upgrade fixture. Never use a production brief as an upgrade smoke
test.
