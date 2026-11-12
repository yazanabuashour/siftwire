---
name: siftwire
description: Use SiftWire through the installed JSON runner for local brief and configuration tasks. Reject direct SQLite, HTTP, MCP, source-built runner, and unreviewed private-state import substitutes. Inspect only user-named legacy inputs, draft configuration for review, and write it only after approval.
license: MIT
compatibility: Requires local filesystem access and an installed SiftWire binary on PATH with siftwire-runner/v6, prepared-delivery/v1, and current-news/v1.
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
Require `runner_protocol: "siftwire-runner/v6"` and `current-news/v1` in
`capabilities` in every config and brief result. Stop and report incompatibility
when either is absent or different.

## Production Boundary

Reject a production request before using tools when it asks you to:

- read or write SQLite directly as a substitute for runner JSON
- use HTTP, MCP, source-built commands, repo inspection, or ad hoc scripts
  instead of the installed runner
- import delivery history, latest-seen state, run state, or inferred private
  configuration without a runner-backed workflow

Report unsupported workflows without substituting tool discovery or repository
inspection. Repository development, testing, and migration design are outside
this production boundary.

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
changes, or operational state. Draft legacy sources and outlet policies only
from the named input.

## Config Tasks

Before a configuration write, call `inspect_config` with the same binary and
dataset. The compatibility check above prevents retired fields from silently
changing an older runner's behavior.

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

Supported source kinds for writes are `rss`, `github_release`, and
`sports_schedule`. Use `rss` for both RSS and Atom feeds. Schedule endpoints
accept `schedule_format` values `espn`, `espn_scoreboard`, or `riot`; `riot`
requires the public Riot frontend key in `api_key`. `schedule_filter` defaults
to `all`; Riot also accepts `standings_top_two` for matches involving the first
two standings positions, including boundary ties. Supported thresholds are
`always`, `medium`, and `high`. A fresh database has no sources. For
`github_release`, supply `repo`, not a custom `url`; the runner generates the
GitHub API endpoint.

These feed-processing fields are optional:

- `outlet_extraction`: `none` or `title_suffix`
- `dedup_group`: same-topic grouping across sources
- `priority_rank`: lower values win duplicate selection after publication recency
- `release_url_prefixes`: RSS-only absolute HTTP(S) directory URLs ending in `/`,
  without credentials, queries, or fragments; matching items use Releases while
  others keep the source section

Require `rss-release-routing/v1` before configuring `release_url_prefixes` or an
RSS source with section `releases`.

Use `threshold: "always"` for required items. Replacement actions replace the
entire corresponding set; an empty array clears it. `max_delivery_items`
defaults to 7 optional items shared across all delivery sections. Required items
and sports do not consume that allowance. Sports fixtures repeat for 7 days before
kickoff and final results repeat for 3 days after kickoff by default.
`sports_timezone` is an IANA time zone and defaults to `America/Chicago`.

## Brief Tasks

Validate, then run the brief only with compatible runner metadata:

```json
{"action":"validate"}
{"action":"run_brief","dry_run":false}
```

If either result has `rejected: true`, answer with `rejection_reason`. Runtime
failures exit nonzero with diagnostics on stderr. Require `prepared-delivery/v1`
in every brief response, starting with `validate`; stop if absent.

Choose optional candidates by zero-based index from `run_brief.candidates`.
Select at most `candidate_slots`, using each candidate's section, threshold,
title, summary, source, and recent brief context. Optional RSS candidates are
current stories published within the preceding 24 hours, not proven first-seen
items. Unsent stories can recur while current. The runner excludes items with
missing, invalid, stale, or future publication dates and reports their fetch
counts. Required feeds keep latest-seen handling and have no publication-age
filter.

News links remain the original feed links. Exact duplicate suppression does not
establish story identity or prove that a headline changed because facts changed.
Sports fixtures and scores come from `sports_updates`; skip routine team news,
previews, recaps, lineups, odds, watch guides, and transfer rumors. Select sports
news only for a confirmed major development, such as a championship, major
sanction, season-changing injury or transaction, or head-coach departure. Reject
livestream promotions; source labels and headline claims do not verify authority.
Compare recent brief context and select meaningful developments rather than
cosmetic rewrites or repeated coverage. Do not claim article-body verification
from feed metadata alone.

Prepare the delivery with `prepare_delivery`. After transport acceptance,
confirm it with `confirm_delivery`:

```json
{"action":"prepare_delivery","run_id":"run_id_from_run_brief","candidate_indexes":[0,3]}
{"action":"confirm_delivery","run_id":"run_id_from_run_brief","delivery_plan_id":"plan_id_from_prepare_delivery"}
```

The runner keeps every required item and limits optional candidates. Required
items and sports do not use candidate slots. The result includes complete
`message`, `text`, and `html` bodies. Deliver those bodies unchanged before calling `confirm_delivery`.
If confirmation returns `final_answer`, answer with exactly that string;
otherwise answer with exactly the prepared `message`.

Preserve the run result and prepared plan. Do not automatically retry an
uncertain `run_brief`; it may have changed latest-seen state. Before confirmation,
retry `prepare_delivery` with the same indexes to recover its immutable plan.
After transport acceptance, retry interrupted confirmation with the same plan
ID without resending. Never confirm failed or uncertain transport.

`final_answer` includes recent briefs when available. Do not reconstruct history
or alter prior messages, links, `NO_REPLY`, or health text.
