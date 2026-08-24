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
{"action":"set_brief_options","max_delivery_items":7}
```

Supported source kinds are `rss`, `atom`, and `github_release`. Supported
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
write. `max_delivery_items` defaults to 7.

## Brief Tasks

Use `siftwire brief`:

```json
{"action":"validate"}
{"action":"run_brief","dry_run":false}
{"action":"record_delivery","run_id":"run_id_from_run_brief","message":"- [Title](<https://example.com>)"}
```

If a result has `rejected: true`, answer with `rejection_reason`. Runtime
failures exit nonzero and write diagnostics to stderr.

Build the current brief from `run_brief` as follows:

- Include every `must_include` item first.
- Fill remaining slots up to `max_delivery_items` from `candidates` using brief
  judgment appropriate to each `section` and `threshold`.
- If `must_include` exceeds the limit, include all of it and no candidates.
- Format bullets as `- [Title](<https://example.com>)`.
- Append a non-empty `health_footnote` as plain text after the bullets.
- If neither bullets nor a health footnote exist, use exactly `NO_REPLY`.

`run_brief.delivery_message_scope` is `current_brief_only`. Before answering,
call `record_delivery` with the exact current brief body. When JSON string data
contains backticks or other shell metacharacters, pipe it with a single-quoted
heredoc so the shell cannot evaluate the data. Do not include `Current brief`,
`Previous brief`, or any history in `message`. If the runner
rejects a history wrapper, rebuild `message` from only the current run result and
correct `record_delivery` with the same run ID; do not rerun `run_brief`.

If `record_delivery.final_answer` exists, answer with exactly that string and
ignore `run_brief.previous_briefs`. Otherwise, answer with only the current body
when `previous_briefs` is empty. When it is not empty, render `Current brief`,
the current body, then up to two `Previous brief (<delivered_at>)` sections in
JSON order. Render each prior `message` exactly as recorded. Do not summarize,
paraphrase, strip links, or alter `NO_REPLY` or health-footnote text.
