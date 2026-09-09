# Runner contract

The installed `siftwire` executable accepts one JSON request per process and
returns one JSON result. Production consumers use this interface, not Rust
implementation modules or direct SQLite access.

## Invocation

```text
siftwire config [--db path]
siftwire brief [--db path]
```

The `runs list` and `runs show` operator commands provide
[archive reads](run-history.md) for operators and the console. Source inspection
and writes use `config.inspect_config` and `config.upsert_source`. There is no
separate `source` command.

Every `config` and `brief` result identifies `runner_protocol` and
`capabilities`. Consumers must require the protocol and capabilities they use
before acting. v0.8.0 uses `siftwire-runner/v4` with `prepared-delivery/v1`,
`sports-updates/v1`, and `current-news/v1`. The binary release version does not
replace capability checks.

## Framing and exits

- Send exactly one JSON object on stdin.
- Unknown request fields, malformed JSON, and additional JSON values are
  transport failures.
- An exit-0 task result is exactly one JSON object followed by a newline on
  stdout. Exit-1 and exit-2 failures do not produce result JSON.
- Stderr contains diagnostics and migration warnings, never result JSON.
- Exit 0 means the task returned a result. `rejected` identifies domain
  validation and policy refusals within successful result transport.
- Exit 1 means decoding, runtime initialization, orchestration, storage, or
  output encoding failed. Per-source fetch failures normally appear in an
  exit-0 brief result.
- Exit 2 means command-line misuse.

The `action` field selects the operation. Known request fields that decode
successfully but do not apply to that action are currently ignored. Consumers
must ignore unknown result fields. Adding a result field is compatible.
Removing a field or changing its type or meaning is breaking. Changes to action
names, framing, exits, or retry semantics are also breaking.

## Configuration

### Request fields and actions

A `config` request accepts these fields:

- `action`
- `source` or `sources`
- `key`
- `outlets`
- `max_delivery_items`
- `sports_pre_game_days`
- `sports_post_game_days`
- `sports_timezone`

The `action` field accepts these values:

| Action                    | Purpose                                                                  |
| ------------------------- | ------------------------------------------------------------------------ |
| `init`                    | Open and initialize the selected database.                               |
| `inspect_config`          | Return runtime configuration, sources, and outlet policies.              |
| `replace_sources`         | Transactionally replace all sources. An empty array clears them.         |
| `upsert_source`           | Add or replace one normalized source.                                    |
| `delete_source`           | Delete one source by key.                                                |
| `replace_outlet_policies` | Transactionally replace all outlet policies. An empty array clears them. |
| `set_brief_options`       | Set normal-item limits, sports windows, and the sports time zone.        |

### Results

Config results contain `runner_protocol`, `capabilities`, `rejected`, `paths`,
`source_reporting`, `outlet_conflicts`, and `summary`. The runner omits
`rejection_reason`, `runtime_config`, `sources`, and `outlets` when empty.
`runtime_config` maps option names to string values.

Mutation results are partial. Source upsert returns the normalized source.
Source deletion returns the remaining source collection. Publisher replacement
returns the normalized publisher collection, and brief-option writes return
runtime configuration. A partial mutation result does not replace an entire
cached configuration.

`source_reporting` is a read-only map keyed by each returned source's key.
`outlet_conflicts` contains `{matcher, names}` records for overlapping enabled
publisher matchers in the returned policies.

### Reporting policies

`source_reporting` has these values:

- `sports`: recurring schedule updates, outside normal candidate slots.
- `required`: eligible new items are required by a release source or an `always`
  threshold.
- `observe`: retained legacy audit configuration only, not a new reporting choice.
- `major`: high-threshold candidates for caller judgment.
- `highlights`: medium-threshold candidates for caller judgment.

The runner owns this decision. Reporting metadata is not writable. New feed
thresholds are `always`, `high`, and `medium`. Optional RSS sources use the
[current-news contract](architecture/current-news.md), independent of source
markers. Unselected stories remain eligible while current, not in a durable
backlog. See [v4 migration](runner-v4-migration.md) for caller compatibility and
[v3 migration](runner-v3-migration.md) for legacy source configuration.

### Source fields

Source writes support `rss`, `github_release`, and `sports_schedule`. The `rss`
kind auto-detects RSS and Atom documents. Fields are `key`, `label`, `kind`,
`url`, `repo`, `section`, `threshold`, `enabled`, `url_canonicalization`,
`outlet_extraction`, `dedup_group`, `priority_rank`, `schedule_format`,
`schedule_filter`, and `api_key`. A `github_release` source requires `repo`
as `owner/name` and rejects a nonempty `url`. The runner derives the public
GitHub releases endpoint. Feed canonicalization accepts `none` or
`google_news_article_url`, applied only to Required feeds. Optional RSS preserves
original links and ignores the saved canonicalization choice without rewriting
it. Publisher extraction accepts `none` or `title_suffix` in either case.
A `sports_schedule` source fetches a schedule endpoint and emits recurring
upcoming fixtures and completed results. `schedule_format` selects `espn`,
`espn_scoreboard`, or `riot`. `api_key` carries Riot's public frontend key for
`riot`. See [sports schedules](architecture/schedule-source-adr.md) for the
recurring-update design.

### Sports sources

`schedule_filter` defaults to `all`. Only Riot accepts `standings_top_two`. That
filter emits matches involving teams in the first two standings positions and
includes ties at the second-position boundary. It falls back to schedule
records only when Riot returns no populated standings section. Rankings request
and decode failures fail the source. `espn_scoreboard` adds the configured
sports window to an ESPN UFC scoreboard endpoint. Upcoming events produce one
card-level update. Completed events produce one update per usable bout with the
winner, loser, records, weight class, and provider-supplied round and clock.
Every update links to an ESPN FightCenter page, never the JSON endpoint.

### Brief options

`max_delivery_items` defaults to 7. Required normal items always remain. The
limit determines how many optional candidates fit after them. Sports updates do
not use candidate slots.
`sports_pre_game_days` defaults to 7, `sports_post_game_days` defaults to 3, and
`sports_timezone` defaults to the IANA value `America/Chicago`.
`set_brief_options` updates any supplied subset in one transaction.

### Publisher policies

Outlet policies use `name`, `aliases`, `policy`, `note`, and `enabled`. Notes are
optional metadata, not instructions.
Allow and Watch retain matching items and produce annotations. Unmatched items
also remain. Block excludes matches. New replacement writes reject overlapping
enabled names or aliases under the runner's shared matcher normalization.
Existing conflicts remain readable and keep their previous first-match order
until the operator changes them. Rejection leaves the whole collection intact.

### Network constraints

Network sources must resolve only to public addresses. The runner checks every
DNS resolution and redirect, disables proxy bypass, and rejects decoded
response bodies beyond the measured 16 MiB tripwire. The measurement receipt is
`receipts/http-response-size.json`.

## Briefs

### Request fields and actions

A `brief` request accepts `action`, `dry_run`, `run_id`,
`candidate_indexes`, and `delivery_plan_id`. Caller-supplied `message` is an
unknown request field.

| Action             | Purpose                                                                          |
| ------------------ | -------------------------------------------------------------------------------- |
| `validate`         | Verify that the selected runtime and database open. It does not fetch sources.   |
| `run_brief`        | Fetch configured sources, update permitted state, and return selection evidence. |
| `prepare_delivery` | Validate candidate indexes and persist one immutable rendered delivery plan.     |
| `confirm_delivery` | Idempotently record the exact prepared plan after transport acceptance.          |

`run_brief` defaults to `dry_run: false`. With `dry_run: true`, it still fetches
sources but does not save run evidence, source markers, fetch logs, or health
changes. A successful dry run returns `run_id: "dry-run"`, which cannot be prepared for
delivery.
Database initialization and schema migration can still occur when the command
opens storage. `validate` opens the database but does not test source
compatibility or fetchability.

### Results

Every brief result includes `runner_protocol`, `capabilities`, `rejected`,
`paths`, `health_delta`, `candidate_slots`, and `summary`. The other fields depend
on the action:

| Purpose | Fields |
| --- | --- |
| Rejection and run identity | `rejection_reason`, `run_id` |
| Selection | `must_include`, `candidates`, `max_delivery_items` |
| Recent context | `previous_briefs`, `recent_sent`, `delivery_message_scope` |
| Suppression | `suppressed`, `suppressed_recent`, `suppressed_policy`, `suppressed_unresolved` |
| Fetch and health | `fetch_status`, `health_footnote` |
| Sports | `sports_section`, `sports_updates` |
| Prepared delivery | `delivery_plan_id`, `message`, `text`, `html`, `prepared_items` |
| Confirmed delivery | `sent_items`, `deliveries`, `final_answer` |

The runner omits empty strings and collections in these action-dependent fields.
It omits `max_delivery_items` when zero. `candidate_slots` remains present even
when zero.

### Fetch status

Each `fetch_status` row contains `items` and nullable `new_items`. Successful
Required checks retain marker-selected new counts. Successful optional RSS
checks set `new_items` to null and include `current_news` with `since`, `until`,
`eligible_items`, `stale_items`, `undated_items`, and `future_items`. These date
counts cover parsed entries before publisher policy, deduplication, and recent
suppression. Invalid dates join undated entries. Both window endpoints are
inclusive. A failed new check has no selection count or current-news statistics.
Archived `fetch` rows use this same shape. Older stored counts remain numeric.
The runner never infers historical freshness from current configuration.

### Sports results

`sports_section` is runner-rendered Markdown for inspection. Prepared bodies
already place it after normal items and before the health footnote. Consumers
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

### Prepared delivery

Preparation makes no network requests. It renders saved selection evidence and
run settings, not current configuration.

`candidate_slots` is the maximum number of candidate indexes accepted by
`prepare_delivery`. Candidate indexes are zero-based positions in the returned
`candidates` array. They must be unique and in range. The first successful
`prepare_delivery` call persists the run's only plan. Before confirmation, an
identical retry returns that plan, while different indexes are rejected. Its `message`, `text`, and `html` are
complete transport bodies in Markdown, plain text, and HTML, respectively.
Required normal items can exceed `max_delivery_items`. `candidate_slots` then
remains explicitly zero. Consumers must send prepared bodies unchanged. Each
`prepared_items` entry includes its source `kind`. Sports evidence remains
auditable but does not enter normal-item recent suppression. New plans also retain optional `run_item_ids` as strings
linking prepared items to immutable collection evidence. Old plans without these
references remain valid and unchanged.

### Recent context

`paths.data_dir`, `recent_sent`, aggregate `suppressed`, and `previous_briefs`
remain available. Recent brief context supports editorial selection. Aggregate
suppression includes same-run evidence. Confirmation returns `final_answer`
with the current brief and recorded prior messages.

## State and retries

Durable configuration writes require operator approval.

`run_brief` can change run, Required latest-seen, fetch, and health state before
a caller receives its result. Automatic retry after an interruption or uncertain
outcome is unsafe. The caller saves the result, chooses candidate indexes, and
calls `prepare_delivery`. The prepared plan survives interruption and supplies
complete transport bodies. Only after transport acceptance does the caller
invoke `confirm_delivery` with that plan ID. The runner does not send email.

`delivery_message_scope` is `current_brief_only`. Transport uses the immutable
prepared bodies, not the `final_answer` history wrapper. `confirm_delivery` is
retry-safe with the same database and `delivery_plan_id`. An identical retry returns the
existing delivery. It rejects an unknown plan or a supplied `run_id` that does
not match. A delivered run cannot create a new plan. `record_delivery` is no
longer an action. Old deliveries remain readable. Database corruption or an
incomplete idempotency record remains an exit-1 runtime error.

Result JSON can contain local paths, source URLs, brief text, and delivery
history. Do not treat raw requests or results as telemetry-safe.

## Storage selection

Path precedence is explicit `--db`, `SIFTWIRE_DATABASE_PATH`, then the SiftWire
default of `${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`.
`XDG_DATA_HOME` is honored only when absolute. Newly created database
directories and files are owner-only. Existing explicit parent directories
retain their permissions. All commands that open storage can initialize the
database or apply schema migrations, including inspection and archive reads.

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
