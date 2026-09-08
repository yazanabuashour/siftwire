# Read run and delivery history

These read-only operator commands are outside the `config|brief` agent protocol.
The console uses them through the installed runner, never through SQLite.

```bash
siftwire runs list --json
siftwire runs list --delivered --json
siftwire runs list --delivered --search '2026-09' --json
siftwire runs list --delivered --before '<run-id-from-next_before>' --json
siftwire runs show '<run-id>' --json
```

## Page through the archive

`runs list` returns `runs` and nullable `next_before`. Pass a returned cursor as
`--before` with the same filters. A null cursor means the query has no older
matches. An unknown cursor is an error. The default page size remains 20;
`--limit N` changes the page size, not the total archive available.

Activity sorts by run start time descending, then run ID descending. `--delivered`
filters confirmed deliveries before applying the limit and sorts by delivery
time descending, then run ID descending. A run delivered late can therefore be
the newest brief without being the newest activity.

`--search` matches literal text in stored ISO timestamps, the run ID, or the
summary. It does not interpret wildcards or regular expressions and does not
search email bodies. Search stored dates such as `2026-09`, not localized month
names. Search applies to the full archive before pagination.

Console HTTP queries use `/api/v1/runs?delivered=true&search=2026-09` with optional
`before` and `limit`. `/api/v1/runs/<id>` fetches a record independently of list
filters and pagination. Percent-encode query values and IDs.

## Interpret collection and delivery evidence

`runs show --json` returns the complete record regardless of text section flags.
It includes `must_include`, `candidates`, `dropped`, `annotations`, `fetch`,
`sent_items`, and nullable `delivery_html` alongside the run summary.

Each evidence row has a stable decimal-string `id`, including older stored rows.
`reporting` describes an item's effective reporting policy from its recorded
source fields, or null when those fields are incomplete. It does not consult
current configuration. `delivery_status`
separates that collection decision from delivery:

| Value | Meaning |
| --- | --- |
| `sent` | Recorded evidence proves inclusion in the confirmed delivery. |
| `sent_as_sports` | The recorded fixture appears through its sports representation. |
| `not_selected` | A confirmed plan's recorded candidate selection excludes this candidate. |
| `not_delivered` | The run has no confirmed delivery. This is not an editorial rejection. |
| `unknown` | The old or incomplete evidence cannot prove the outcome. |

The legacy `selected` boolean remains for compatibility. Prefer `delivery_status`
when explaining an outcome. Do not interpret false as proof of exclusion.

New plans reference recorded items. Older sports records require a unique match
through their recorded source, compatibility title, URL, start time, and delivery
context. A shared event URL alone cannot identify a fixture or bout. The runner
leaves uncertain outcomes unknown rather than altering a historical plan.

`dropped` contains recorded exclusions before editorial selection. `annotations`
contains retained-link warnings, allowed publisher matches, and ambiguous older
link-resolution diagnostics. Their `disposition` is `retained`, `dropped`, or
`unknown` at that processing step, not the final delivery outcome. A warning
does not prove that an item was dropped.
New fetch records retain their source label; older records may expose only the
source key. Renaming a source does not rename its history.

`delivery_html` is the exact saved email for a confirmed plan, or null. It is never
regenerated from today's settings. Reading history does not send email or advance
source state.
