# Run and delivery history

The `runs` operator commands read saved collection and delivery evidence. They
are separate from the `config` and `brief` agent protocol. The console uses these
commands through the installed runner, never through SQLite.

## Commands

```text
siftwire runs list [--delivered] [--before run_id] [--search text] [--limit N] [--json] [--db path]
siftwire runs show <run_id> [--candidates] [--dropped] [--selected] [--json] [--db path]
```

`runs list` returns the run archive. `runs show` returns a single run. Without
`--json`, `runs show` prints its summary and fetch checks. `--candidates` adds
required items and candidates, `--dropped` adds exclusions and annotations, and
`--selected` adds sent items and the saved delivery message. These flags can be
combined. `--json` always returns the complete record, regardless of section
flags.

Both commands use the [runner's database selection](runner-contract.md#storage-selection).
They do not collect items, send email, or advance source state. Opening the
database can still initialize an empty schema. Incompatible databases are rejected.

Example queries:

```bash
siftwire runs list --json
siftwire runs list --delivered --json
siftwire runs list --delivered --search '2026-09' --json
siftwire runs list --delivered --before '<run-id-from-next_before>' --json
siftwire runs show '<run-id>' --json
```

## Pagination and search

`runs list --json` returns `runs` and nullable `next_before`. The next request's
`--before` value is that cursor, with the same filters. A null cursor means the
query has no older matches. An unknown cursor is an error. The default page size
is 20. `--limit N` changes the page size, not the total archive available.

Activity sorts by run start time descending, then run ID descending. `--delivered`
filters confirmed deliveries before applying the limit and sorts by delivery
time descending, then run ID descending. A run delivered late can therefore be
the newest brief without being the newest activity.

`--search` matches literal text in stored ISO timestamps, the run ID, or the
summary. It does not interpret wildcards or regular expressions and does not
search email bodies. Stored date fragments such as `2026-09` match. Localized
month names do not. Search applies to the full archive before pagination.

Console HTTP queries use `/api/v1/runs?delivered=true&search=2026-09` with optional
`before` and `limit`. `/api/v1/runs/<id>` fetches a record independently of list
filters and pagination. Query values and IDs require percent-encoding.

## Collection and delivery evidence

`runs show --json` includes `must_include`, `candidates`, `dropped`, `annotations`,
`fetch`, `sent_items`, and nullable `delivery_html` alongside the `run` summary.

Each evidence row has a stable decimal-string `id`.
`reporting` describes an item's effective reporting policy from its recorded
source fields, or null when those fields are incomplete. It does not consult
current configuration. `delivery_status` separates that collection decision
from delivery:

| Value | Meaning |
| --- | --- |
| `sent` | Recorded evidence proves inclusion in the confirmed delivery. |
| `not_selected` | A confirmed plan's recorded candidate selection excludes this candidate. |
| `not_delivered` | The run has no confirmed delivery. This is not an editorial rejection. |
| `unknown` | The recorded evidence cannot prove the outcome. |

Plans reference recorded items directly. The runner does not infer delivery
from matching titles or URLs.

`dropped` contains recorded exclusions before editorial selection, with
`disposition: "dropped"`. `annotations` contains retained-link warnings and
allowed publisher matches, with `disposition: "retained"`. Disposition describes
that processing step, not the final delivery outcome. A warning does not prove
that an item was dropped.

## Fetch evidence

Fetch records retain their source label. Renaming a source does not rename its
history.

`fetch[].items` is the fetched item count, or the update count for sports.
`new_items` is nullable. Required checks retain marker-selected new counts. Optional RSS checks include `current_news`
with `since`, `until`, `eligible_items`, `stale_items`, `undated_items`, and
`future_items`, and set `new_items` to null. Those date counts precede publisher
policy, duplicate checks, and recent-delivery suppression. Eligible does not
mean first seen, selected, or delivered.

Failed checks have no selection count or current-news statistics. The runner
does not infer selection mode from current source settings.

## Saved email

`delivery_html` is the exact saved HTML for a confirmed plan, or null. The runner
never regenerates it from today's settings. An unconfirmed prepared plan does
not appear as a confirmed email.
