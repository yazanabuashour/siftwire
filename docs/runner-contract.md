# Runner Contract

Siftwire is a language-neutral one-shot process building block. Consumers invoke
the installed executable; they do not import implementation packages or read its
SQLite database.

## Invocation

```text
siftwire config [--db path]
siftwire brief [--db path]
```

Human-facing operator commands also exist on the same binary: read-only
`source list` and `runs list|show`, plus a `source add` stdin write that uses
the same validation as `upsert_source`. They are operator conveniences outside
this protocol; automated consumers must use only `config` and `brief`.

The binary release version is the protocol compatibility version. There is no
separate in-band protocol version.

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

Actions:

| Action | Purpose |
| --- | --- |
| `init` | Open and initialize the selected database. |
| `inspect_config` | Return runtime configuration, sources, and outlet policies. |
| `replace_sources` | Transactionally replace all sources. An empty array clears them. |
| `upsert_source` | Add or replace one normalized source. |
| `delete_source` | Delete one source by key. |
| `replace_outlet_policies` | Transactionally replace all outlet policies. An empty array clears them. |
| `set_brief_options` | Set `max_delivery_items`. |

Config results contain `rejected`, optional `rejection_reason`, `paths`, optional
`runtime_config`, `sources`, and `outlets`, plus `summary`.

Sources support `rss`, `atom`, and `github_release`. Their stable fields are
`key`, `label`, `kind`, `url`, `repo`, `section`, `threshold`, `enabled`,
`url_canonicalization`, `outlet_extraction`, `dedup_group`, `priority_rank`, and
`always_report`. A `github_release` source fetches `url` when provided;
otherwise it derives the GitHub API endpoint from `repo`. Network sources must
resolve only to public addresses. The runner applies that policy to every DNS
resolution and redirect, disables proxy bypass, and rejects decoded response
bodies beyond the measured 16 MiB tripwire in
`receipts/http-response-size.json`. Outlet policies use `name`, `aliases`,
`policy`, `note`, and `enabled`.

## Briefs

Every brief request can contain `action`, `dry_run`, `run_id`, and `message`.

| Action | Purpose |
| --- | --- |
| `validate` | Verify that the selected runtime and database open. It does not fetch sources. |
| `run_brief` | Fetch configured sources, update permitted state, and return selection evidence. |
| `record_delivery` | Record the exact delivered body and return history-backed final output. |

Brief results contain `rejected`, optional `rejection_reason`, `paths`, optional
`run_id`, `must_include`, `candidates`, `previous_briefs`,
`delivery_message_scope`, `recent_sent`,
`suppressed`, `suppressed_recent`, `suppressed_policy`,
`suppressed_unresolved`, `fetch_status`, `health_footnote`, `health_delta`,
`max_delivery_items`, `sent_items`, `deliveries`, and `final_answer`, plus
`summary`. Empty optional collections may be omitted.

`paths.data_dir`, `recent_sent`, aggregate `suppressed`, and `previous_briefs`
remain compatibility fields. New consumers should prefer typed suppression
collections and `record_delivery.final_answer`, but must not require older
fields to disappear.

## State and retries

Durable configuration writes require operator approval.

`run_brief` can mutate run, latest-seen, fetch, and health state before a caller
observes an uncertain process outcome. Do not retry it automatically after an
interruption or transport uncertainty. Its `delivery_message_scope` is
`current_brief_only`: `record_delivery.message` must not contain rendered
`Current brief` or `Previous brief` history sections. That domain rejection
writes nothing, so the caller can correct the message for the same run ID.

`record_delivery` is retry-safe only with the same database, run ID, and exact
message. An identical retry returns the existing delivery. A different message
for an already delivered run ID returns exit 0 with `rejected: true`. Database
corruption or an incomplete idempotency record remains an exit-1 runtime error.

Result JSON can contain local paths, source URLs, brief text, and delivery
history. Do not treat raw requests or results as telemetry-safe.

## Storage selection

Path precedence is explicit `--db`, `SIFTWIRE_DATABASE_PATH`, then the Siftwire
default of `${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`.
`XDG_DATA_HOME` is honored only when absolute. Newly created database
directories and files are owner-only; existing explicit parent directories
retain their permissions.

## Minimal consumer

`examples/inspect_config.rs` is a minimal process consumer:

```bash
run_root="$(mktemp -d)"
mise exec -- cargo build --locked --bin siftwire
mise exec -- cargo run --locked --example inspect_config -- \
  --binary target/debug/siftwire \
  --db "$run_root/siftwire.sqlite"
```

It uses only this documented process interface and does not inspect storage.
