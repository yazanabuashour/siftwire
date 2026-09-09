# Runner Contract

SiftWire is a language-neutral one-shot process building block. Consumers invoke
the installed executable; they do not import implementation packages or read its
SQLite database.

## Invocation

```text
siftwire config [--db path]
siftwire brief [--db path]
```

The read-only `runs list|show` operator commands also serve the console's
archive requests. Source inspection and writes use `config.inspect_config`
and `config.upsert_source`; there is no separate `source` command.

Every result identifies `runner_protocol` and `capabilities`. Consumers must
require the protocol and capabilities they use before acting. The current
protocol is `siftwire-runner/v3`; `prepared-delivery/v1` and `sports-updates/v1`
identify the contracts documented below. The binary release version remains
useful operator information but is not capability negotiation.

## Framing and exits

- Send exactly one JSON object on stdin.
- Unknown request fields, malformed JSON, and additional JSON values are
  transport failures.
- An exit-0 task result is exactly one JSON object followed by a newline on
  stdout. Exit-1 and exit-2 failures do not produce result JSON.
- Stderr contains diagnostics and migration warnings, never result JSON.
- Exit 0 means the task returned a result. Check `rejected`; domain validation
  and policy refusals are successful result transport.
- Exit 1 means decoding, runtime initialization or orchestration, storage, or output encoding failed. Per-source fetch failures are normally reported in an exit-0 brief result.
- Exit 2 means command-line misuse.

Request objects are action unions. Fields known to the command but irrelevant to
the chosen action are currently accepted and ignored. Consumers must ignore
unknown result fields. Adding a result field is compatible. Removing a field,
changing its type or meaning, changing action names, framing, exits, or retry
semantics is breaking.

## Configuration

Every config request can contain:

- `action`
- `source` or `sources`
- `key`
- `outlets`
- `max_delivery_items`
- `sports_pre_game_days`
- `sports_post_game_days`
- `sports_timezone`

Actions:

| Action                    | Purpose                                                                  |
| ------------------------- | ------------------------------------------------------------------------ |
| `init`                    | Open and initialize the selected database.                               |
| `inspect_config`          | Return runtime configuration, sources, and outlet policies.              |
| `replace_sources`         | Transactionally replace all sources. An empty array clears them.         |
| `upsert_source`           | Add or replace one normalized source.                                    |
| `delete_source`           | Delete one source by key.                                                |
| `replace_outlet_policies` | Transactionally replace all outlet policies. An empty array clears them. |
| `set_brief_options`       | Set normal-item limits, sports windows, and the sports time zone.        |

Config results contain `runner_protocol`, `capabilities`, `rejected`, optional
`rejection_reason`, `paths`, optional `runtime_config`, `sources`, and `outlets`,
plus `summary`. Source-bearing results also contain the read-only
`source_reporting` map keyed by source key. Inspection includes `outlet_conflicts`
as `{matcher, names}` records for overlapping enabled publisher matchers.
Mutation results are partial: source upsert returns the normalized source,
source deletion returns the remaining source collection, publisher replacement
returns the normalized publisher collection, and brief-option writes return
runtime configuration. Do not replace an entire cached configuration with a
partial mutation result.

`source_reporting` has these values:

- `sports`: recurring schedule updates, outside normal candidate slots.
- `required`: eligible new items are required by a release source or an `always`
  threshold.
- `observe`: retained legacy audit configuration only, not a new reporting choice.
- `major`: high-threshold candidates for caller judgment.
- `highlights`: medium-threshold candidates for caller judgment.

The runner owns this decision. Reporting metadata is not writable. New feed
thresholds are `always`, `high`, and `medium`. Unselected candidates are not a
durable backlog. See [v3 migration](runner-v3-migration.md) for legacy source
configuration and historical reporting.

Source writes support `rss`, `github_release`, and `sports_schedule`. The `rss`
kind auto-detects RSS and Atom documents. Fields are `key`, `label`, `kind`,
`url`, `repo`, `section`, `threshold`, `enabled`, `url_canonicalization`,
`outlet_extraction`, `dedup_group`, `priority_rank`, `schedule_format`,
`schedule_filter`, and `api_key`. A `github_release` source requires `repo`
as `owner/name` and rejects a nonempty `url`; the runner derives the public
GitHub releases endpoint. Feed canonicalization accepts `none` or
`google_news_article_url`; publisher extraction accepts `none` or `title_suffix`.
A `sports_schedule` source fetches a schedule endpoint and emits recurring
upcoming fixtures and completed results. `schedule_format` selects `espn`,
`espn_scoreboard`, or `riot`; `api_key` carries Riot's public frontend key for
`riot`. See `docs/architecture/schedule-source-adr.md` for sports behavior.

`schedule_filter` defaults to `all`. Only Riot accepts `standings_top_two`. That
filter emits matches involving teams in the first two standings positions and
includes ties at the second-position boundary. It falls back to schedule
records only when Riot returns no populated standings section. Rankings request
and decode failures fail the source. `espn_scoreboard` adds the configured
sports window to an ESPN UFC scoreboard endpoint. Upcoming events produce one
card-level update. Completed events produce one update per usable bout with the
winner, loser, records, weight class, and provider-supplied round and clock.
Every update links to an ESPN FightCenter page, never the JSON endpoint.

`max_delivery_items` defaults to 7. Required normal items always remain; the
limit determines how many optional candidates fit after them. Sports updates do
not use candidate slots.
`sports_pre_game_days` defaults to 7, `sports_post_game_days` defaults to 3, and
`sports_timezone` defaults to the IANA value `America/Chicago`.
`set_brief_options` updates any supplied subset in one transaction. Network
sources must resolve only to public addresses. The runner applies that policy to every DNS
resolution and redirect, disables proxy bypass, and rejects decoded response
bodies beyond the measured 16 MiB tripwire in
`receipts/http-response-size.json`. Outlet policies use `name`, `aliases`,
`policy`, `note`, and `enabled`. Notes are optional metadata, not instructions.
Allow and Watch retain matching items and produce annotations; unmatched items
also remain. Block excludes matches. New replacement writes reject overlapping
enabled names or aliases under the runner's shared matcher normalization.
Existing conflicts remain readable and keep their previous first-match order
until the operator changes them. Rejection leaves the whole collection intact.

## Briefs

Every brief request can contain `action`, `dry_run`, `run_id`,
`candidate_indexes`, and `delivery_plan_id`. Caller-supplied `message` is an
unknown request field.

| Action             | Purpose                                                                          |
| ------------------ | -------------------------------------------------------------------------------- |
| `validate`         | Verify that the selected runtime and database open. It does not fetch sources.   |
| `run_brief`        | Fetch configured sources, update permitted state, and return selection evidence. |
| `prepare_delivery` | Validate candidate indexes and persist one immutable rendered delivery plan.     |
| `confirm_delivery` | Idempotently record the exact prepared plan after transport acceptance.          |

Brief results contain `runner_protocol`, `capabilities`, `rejected`, optional
`rejection_reason`, `paths`, optional `run_id`, `must_include`, `candidates`, `previous_briefs`,
`delivery_message_scope`, `recent_sent`,
`suppressed`, `suppressed_recent`, `suppressed_policy`,
`suppressed_unresolved`, `fetch_status`, `sports_section`, `sports_updates`,
`health_footnote`, `health_delta`, `max_delivery_items`, `candidate_slots`,
`delivery_plan_id`, `message`, `text`, `html`, `prepared_items`, `sent_items`,
`deliveries`, and `final_answer`, plus `summary`. Empty optional collections may
be omitted.

`sports_section` is runner-rendered Markdown for inspection. Prepared bodies
already place it after normal items and before the health footnote; consumers
must not append it again. It does not count against `max_delivery_items`.
Each structured sports update contains
`source_key`, `source_label`, `status`, `title`, `competition`, `url`, and the
UTC RFC 3339 `starts_at` value. `status` is `upcoming` or `final`. The optional
`images` array contains provider image URLs and labels. SiftWire accepts images
only from `a.espncdn.com` and `static.lolesports.com`. Consumers must treat them
as decorative because mail clients can block remote assets.

Upcoming fixtures also remain in `must_include` for stored evidence linkage.
`prepare_delivery` removes this duplicate representation from the rendered body
and keeps sports outside `max_delivery_items` automatically.

`candidate_slots` is the maximum number of candidate indexes accepted by
`prepare_delivery`. Candidate indexes are zero-based positions in the returned
`candidates` array. They must be unique and in range. The first successful
`prepare_delivery` call persists the run's only plan; an identical retry returns
it, while different indexes are rejected. Its `message`, `text`, and `html` are
complete transport bodies. The HTML body uses a compact light layout so webmail
clients can place it on their message canvas without a competing full-width
dark background. Required normal items can exceed
`max_delivery_items`; `candidate_slots` then remains explicitly zero. Consumers
must send prepared bodies unchanged. Each `prepared_items` entry includes its
source `kind`; sports evidence remains auditable but does not enter normal-item
recent suppression. New plans also retain optional `run_item_ids` as strings
linking prepared items to immutable collection evidence. Old plans without these
references remain valid and unchanged.

`paths.data_dir`, `recent_sent`, aggregate `suppressed`, and `previous_briefs`
remain available. Recent brief context supports editorial selection; aggregate
suppression includes same-run evidence. Confirmation returns `final_answer`
with the current brief and recorded prior messages.

## State and retries

Durable configuration writes require operator approval.

`run_brief` can mutate run, latest-seen, fetch, and health state before a caller
observes an uncertain process outcome. Do not retry it automatically after an
interruption or transport uncertainty. Persist its result, obtain candidate
indexes, and call `prepare_delivery`. The prepared plan survives interruption
and supplies complete transport bodies. After transport acceptance, call
`confirm_delivery` with that plan ID.

`delivery_message_scope` is `current_brief_only`. Send the immutable prepared
bodies, not the final-answer history wrapper. `confirm_delivery` is retry-safe
with the same database and `delivery_plan_id`. An identical retry returns the
existing delivery. It rejects an unknown plan or a supplied `run_id` that does
not match. A delivered run cannot create a new plan. `record_delivery` is no
longer an action; old deliveries remain readable. Database corruption or an
incomplete idempotency record remains an exit-1 runtime error.

Result JSON can contain local paths, source URLs, brief text, and delivery
history. Do not treat raw requests or results as telemetry-safe.

## Storage selection

Path precedence is explicit `--db`, `SIFTWIRE_DATABASE_PATH`, then the SiftWire
default of `${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`.
`XDG_DATA_HOME` is honored only when absolute. Newly created database
directories and files are owner-only; existing explicit parent directories
retain their permissions.

## Minimal consumer

`crates/siftwire/examples/inspect_config.rs` is a minimal process consumer:

```bash
run_root="$(mktemp -d)"
mise exec -- cargo build --locked --bin siftwire
mise exec -- cargo run --locked --example inspect_config -- \
  --binary target/debug/siftwire \
  --db "$run_root/siftwire.sqlite"
```

It uses only this documented process interface and does not inspect storage.
