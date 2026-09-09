# Operator console

`siftwire-console` is a local web application for viewing and configuring a
SiftWire database. It is an operator tool, not part of the runner protocol:
every request spawns the installed `siftwire` binary and uses the `config`
JSON contract or the `runs` operator CLI. The console never opens
SQLite directly.

## Capabilities

- Read the latest confirmed brief and search or page through the delivery archive.
- View configured sources and publisher rules.
- Add, edit, and delete sources (writes through `upsert_source` and
  `delete_source`). The source editor supports ESPN UFC scoreboards and Riot's
  optional top-two standings filter.
- Add, rename, edit, or remove publisher rules in a local draft. Save replaces
  the collection; Discard leaves stored rules unchanged.
- Set `max_delivery_items`, recurring sports windows, and the IANA sports time
  zone. The time-zone control can fill the browser's detected value before an
  explicit save.
- Browse Activity with required items, candidates, exclusions, annotations,
  source checks, and confirmed delivery evidence.

The console intentionally cannot trigger `run_brief`. A brief run mutates
latest-seen state and is not retry-safe; exposing it behind a dashboard button
would invite accidental duplicate-suppressed runs.

## App and email design

All tabs share one application frame; brief content keeps Folio's narrower
reading column. Navigation follows tasks: Brief, Sources, Activity, Settings.
Sources groups feeds and publisher rules. `/deliveries` remains a Brief alias;
`/runs` opens Activity and `/outlets` opens publisher rules.

The Brief and Activity pickers share keyboard navigation and archive search.
Typing filters without selecting; Enter or an option click commits a choice,
and Escape cancels. Search matches stored ISO dates, run IDs, and summaries.
Load older retrieves another page, not a separate fixed-size history window.
The selected run lives in `?run=<id>`, so reloads, links, and back/forward keep
it. A missing explicit ID shows an error rather than another brief.

The Brief view displays the immutable HTML saved for a confirmed
email delivery. This preserves the email's sports cards, team and event logos,
date columns, story layout, and health notes. The console reads the nullable
`delivery_html` field from `siftwire runs show --json`; it never regenerates a
historical email from current configuration. Update the runner as well as the
console together for the current configuration and history contracts. The
runner adds a source-label column to fetch history on open. It does not rewrite
old labels, source state, delivery plans, or emails.

Email HTML appears in a sandboxed frame whose height follows its content, so the
page scrolls as one document. Scripts, forms, embedded pages, and external
stylesheets are blocked. Logos load from their original HTTP or HTTPS image
hosts without a referrer. Story links open in new tabs without opener access.

Legacy deliveries without saved HTML show a labelled recorded-text fallback,
including sports updates and health notes. Saved story links appear only when
the message is also missing. Images in the Markdown fallback remain opt-in
links. Configuration and storage paths remain readable in Settings under
Technical details.

`apps/web/src/styles/globals.css` owns shared styling; configuration, navigation,
and picker styles live with those components.
`crates/siftwire/src/runner/email.rs` owns the matching email styling. Email
changes affect newly prepared messages, not already stored delivery plans.
The production app contains no comparison gallery, alternate designs, or sample
configuration.

## Configure sources and publishers

Normal feeds have one Reporting choice: Required, Major, or Highlights.
Releases always include eligible new releases; schedules produce recurring sports
updates. The feed kind reads both RSS and Atom documents. Releases require a
repository, not a custom endpoint URL. Feed processing offers Google News link
resolution and title-suffix publisher identification.

The runner adapts old Atom/always-report configuration to effective current
fields without rewriting stored rows. Disabled legacy Observe sources remain
visible, but saving them requires an explicit current Reporting choice.
Historical evidence keeps its original reporting. See
[runner v3 migration](runner-v3-migration.md) before updating an existing setup.

Source options appear directly in the editor, without an extra disclosure.
Feed link resolution and publisher extraction appear only for feeds. The
Selection section contains source preference and title matching. Lower preference
wins ordinary duplicate representation, not editorial importance. Title groups
do not isolate identical URLs or recent-delivery suppression.

Edit dialogs keep Close and their save actions visible while fields scroll.
Close, Escape, or an outside click discards that dialog's unapplied changes.
Dismissal is disabled while a source write is pending.

Change a publisher's rule or Active checkbox directly in its row. Edit opens
its name, aliases, and optional note. Notes are metadata, not runner instructions.
Allow and Watch both retain matching items; Watch records an annotation rather
than a review queue.
Block excludes matches. New writes reject overlapping enabled names or aliases.
Existing conflicts remain visible and keep their old matching order until an
operator resolves them.

All publisher edits belong to one draft until Save publishers. Hidden notes
survive rule edits. The console merges each normalized mutation response into
its own configuration collection rather than guessing what the runner stored.

## Large priority values

Source priorities use the runner's full signed 64-bit integer range. The console
keeps them as decimal text in the editor and in historical evidence. Its HTTP
API encodes `priority_rank` as a decimal string, such as
`"9223372036854775807"`; an omitted priority still means zero.

Source writes accept decimal strings from `"-9223372036854775808"` through
`"9223372036854775807"`. The console converts them to exact integers before
calling the runner. Fractions, exponent notation, and values outside that range
are rejected. Older clients may still send JSON integers within JavaScript's
exact range, but larger numeric writes are rejected rather than silently rounded.
Refresh console tabs after updating so they use the string contract.

This changes only the console's HTTP representation. The runner JSON protocol
and SQLite storage remain numeric and require no migration.

## Running

An installed `siftwire` runner must be on `PATH` (or named through
`SIFTWIRE_CONSOLE_RUNNER_BIN`). Build and install the console and web assets
from a checkout:

```bash
mise exec -- ./scripts/install-console.sh
```

This installs `~/.local/bin/siftwire-console` and the web assets under
`~/.local/share/siftwire-console/web`. Re-run it after pulling changes; a
systemd user service can then restart against the new files.

Start the server against the default database:

```bash
SIFTWIRE_CONSOLE_WEB_ROOT="$HOME/.local/share/siftwire-console/web" \
  ~/.local/bin/siftwire-console
```

The server binds loopback only by default. Sharing beyond loopback is an
explicit operator decision; there is no authentication, so expose it only on a
trusted network. A Host-header check rejects requests addressed through foreign
domain names, which blocks browser-based DNS rebinding against both loopback
and LAN bindings. For LAN access, explicitly configure both the bind address
and host firewall for the trusted network.

## Configuration

| Variable                            | Default          | Purpose                                  |
| ----------------------------------- | ---------------- | ---------------------------------------- |
| `SIFTWIRE_CONSOLE_BIND`             | `127.0.0.1:8790` | Listen address.                          |
| `SIFTWIRE_CONSOLE_WEB_ROOT`         | `apps/web/dist`  | Built web assets directory.              |
| `SIFTWIRE_CONSOLE_RUNNER_BIN`       | `siftwire`       | Runner executable to spawn.              |
| `SIFTWIRE_CONSOLE_DATABASE`         | unset            | Optional `--db` override for every call. |
| `SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS` | `30`             | Per-invocation tripwire.                 |

Port 8790 avoids Prooflane's dashboard default of 8787.
