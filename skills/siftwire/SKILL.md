---
name: siftwire
description: Use SiftWire through the installed JSON runner for local brief and configuration tasks. Reject direct SQLite, HTTP, MCP, source-built runner, and unreviewed private-state import substitutes. Inspect only user-named legacy inputs, draft configuration for review, and write it only after approval.
license: MIT
compatibility: Requires local filesystem access and an installed SiftWire binary on PATH.
---

# SiftWire

Use the installed runner for routine production work:

```bash
siftwire config
siftwire brief
```

Pipe exactly one JSON request to one command and answer only from its JSON
result. The runner honors `SIFTWIRE_DATABASE_PATH`; do not pass `--db` unless
the user names a specific dataset. Do not maintain repo-local state files.

## Production Boundary

Reject a production request before using tools when it asks you to:

- read or write SQLite directly as a substitute for runner JSON
- use HTTP, MCP, source-built commands, repo inspection, or ad hoc scripts
  instead of the installed runner
- import delivery history, latest-seen state, run state, or inferred private
  configuration without a runner-backed workflow

For an unsupported workflow, say the production SiftWire runner does not
support it. Do not use `siftwire --help`, command discovery, broad file search,
or source inspection for routine tasks. Repository development, tests, docs,
release verification, security review, and migration design may inspect
repository files.

## Source Intake

When the user provides a public feed, GitHub repository, topic input, or named
legacy configuration:

1. Infer mechanical fields from the supplied value without fetching it when possible.
2. Inspect only a user-provided public destination when metadata is necessary.
   Reject credential-bearing, loopback, private, link-local, or metadata-service
   destinations, and revalidate every redirect before following it.
3. Propose fields that affect grouping, ranking, deduplication, alerting, or
   outlet policy.
4. Apply durable configuration only after explicit approval through
   `siftwire config`.

Never infer source authority, credentials, private configuration, destructive
changes, or operational state. Legacy Migration may draft SiftWire sources and
outlet policies from an input the user explicitly points to; it does not permit
operational-state import.

## Config Tasks

Use `siftwire config` with one of these request shapes:

```json
{"action":"init"}
{"action":"inspect_config"}
{"action":"replace_sources","sources":[{"key":"example","label":"Example","kind":"rss","url":"https://example.com/feed.xml","section":"technology","threshold":"medium","enabled":true}]}
{"action":"upsert_source","source":{"key":"example","label":"Example","kind":"rss","url":"https://example.com/feed.xml","section":"technology","threshold":"medium","enabled":true}}
{"action":"upsert_source","source":{"key":"tool-releases","label":"Tool Releases","kind":"github_release","repo":"owner/name","section":"releases","threshold":"always","enabled":true}}
{"action":"delete_source","key":"example"}
{"action":"replace_outlet_policies","outlets":[{"name":"Example Outlet","aliases":[],"policy":"watch","note":"Review coverage","enabled":true}]}
{"action":"set_brief_options","max_delivery_items":7,"sports_pre_game_days":7,"sports_post_game_days":3,"sports_timezone":"America/Chicago"}
```

Supported source kinds are `rss`, `atom`, `github_release`, and
`sports_schedule` (schedule endpoints; `schedule_format` is `espn`,
`espn_scoreboard`, `espn_core`, or `riot`, and `riot` requires the public Riot
frontend key in `api_key`). `schedule_filter` defaults to `all`; Riot also
accepts `standings_top_two` for matches involving the first two standings
positions, including boundary ties. Supported
thresholds are `always`, `medium`, `high`, and `audit`. A fresh database has no
sources. For `github_release`, include the user-provided `url` when present; it
overrides the generated GitHub API endpoint while `repo` keeps the source
identity.

Optional feed-processing fields:

- `url_canonicalization`: `none`, `feedburner_redirect`, or
  `google_news_article_url`
- `outlet_extraction`: `none`, `title_suffix`, `url_host`, or `rss_source`
- `dedup_group`: same-topic grouping across sources
- `priority_rank`: lower values win duplicate selection
- `always_report`: makes new feed items `must_include`

Replacement actions replace the entire corresponding set; an empty array clears
it. Feed URLs, thresholds, priorities, always-report choices, outlet policies,
and brief options are durable operator configuration and require approval before
write. `max_delivery_items` defaults to 7. Sports fixtures repeat for 7 days
before kickoff and final results repeat for 3 days after kickoff by default.
`sports_timezone` is an IANA time zone and defaults to `America/Chicago`.

## Brief Tasks

First invoke only the common actions:

```json
{"action":"validate"}
{"action":"run_brief","dry_run":false}
```

If either result has `rejected: true`, answer with `rejection_reason`. Runtime
failures exit nonzero and write diagnostics to stderr. After `run_brief`, branch
on its `capabilities`; never mix prepared and legacy delivery in one run.

When capabilities contain `prepared-delivery/v1`, choose optional candidates by
zero-based index from `run_brief.candidates`. Select at most `candidate_slots`,
use brief judgment appropriate to each `section` and `threshold`, then invoke:

```json
{"action":"prepare_delivery","run_id":"run_id_from_run_brief","candidate_indexes":[0,3]}
{"action":"confirm_delivery","run_id":"run_id_from_run_brief","delivery_plan_id":"plan_id_from_prepare_delivery"}
```

The runner keeps every required item, limits optional candidates, places sports
outside those candidate slots, removes compatibility duplicates, and returns
complete `message`, `text`, and `html` bodies. Deliver those bodies unchanged
before calling `confirm_delivery`. If confirmation returns `final_answer`,
answer with exactly that string; otherwise answer with exactly the prepared
`message`.

Without `prepared-delivery/v1`, build the legacy current body as follows:

- When `sports_section` is non-empty, exclude `must_include` items whose `kind`
  is `sports_schedule`. Otherwise retain them for compatibility.
- Include every remaining `must_include` item. If they reach or exceed
  `max_delivery_items`, include no candidates.
- Fill remaining slots from `candidates` using their section, threshold, title,
  summary, source, and recent brief context.
- Format normal items as `- [Title](<https://example.com>)`, append a non-empty
  `sports_section`, then append a non-empty `health_footnote`, with one blank
  line between parts. Use exactly `NO_REPLY` when every part is empty.
- Call `record_delivery` with only that exact current body:

```json
{"action":"record_delivery","run_id":"run_id_from_run_brief","message":"exact current brief"}
```

Do not include `Current brief`, `Previous brief`, or history in `message`. If
`record_delivery.final_answer` exists, answer with exactly that string and
ignore `run_brief.previous_briefs`. Otherwise answer with only the current body
when no previous briefs exist; when they do, render `Current brief`, the current
body, then up to two `Previous brief (<delivered_at>)` sections in JSON order.
Render each prior `message` exactly as recorded. Never summarize, paraphrase,
strip links, or alter `NO_REPLY` or health text.
