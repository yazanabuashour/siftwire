# Use the local console

Use `siftwire-console` to read confirmed briefs, edit sources and publisher
rules, and inspect recorded runs. The console calls the installed `siftwire`
process for each request. It never opens SQLite directly.

The console cannot trigger `run_brief`. Collection can change latest-seen state
before the caller receives a result, so it is not safe to retry as a dashboard
request. Use your agent and the [runner workflow](runner-contract.md#state-and-retries)
for collection and delivery.

## Install and start the console

1. Install the matching runner and put `siftwire` on `PATH`. To use another
   executable, set `SIFTWIRE_CONSOLE_RUNNER_BIN`.
2. From a matching checkout, install the pinned tools and frontend dependencies,
   then build and install the console:

   ```bash
   mise install
   mise exec -- bun install
   mise exec -- ./scripts/install-console.sh
   ```

   The script installs `~/.local/bin/siftwire-console` and replaces the web assets
   under `~/.local/share/siftwire-console/web`. Set `SIFTWIRE_CONSOLE_INSTALL_DIR`
   or `SIFTWIRE_CONSOLE_WEB_INSTALL_DIR` before installation to change those paths.
3. Start the server against the default database:

   ```bash
   SIFTWIRE_CONSOLE_WEB_ROOT="$HOME/.local/share/siftwire-console/web" ~/.local/bin/siftwire-console
   ```

4. Open `http://127.0.0.1:8790`.

When upgrading, update the runner, console, and web assets together. Re-run the
install script after pulling changes, then restart your console process or
existing user service. Refresh open browser tabs to load the matching client.
See [v4 migration](runner-v4-migration.md) for existing installations.

## Read a confirmed brief

Open **Brief** to read the latest confirmed delivery. Use the picker to search
or select an older brief. Typing filters the list without changing the current
selection. Press Enter or click an option to select it. Press Escape to cancel.
Use **Load older** to retrieve another page.

Search matches stored ISO dates, run IDs, and summaries, not email bodies. For
example, search `2026-09`, not a localized month name. The selected run stays in
`?run=<id>` across reloads and browser navigation. An explicit ID that does not
exist produces an error instead of showing another brief.

The reader displays the exact saved `delivery_html` from a confirmed plan. It
does not rebuild old email from current settings. Older deliveries without HTML
show labelled recorded text, including sports and health notes. If the saved
message is also missing, the reader falls back to saved story links.

The email frame blocks scripts, forms, embedded pages, and external stylesheets.
Logos load from their original HTTP or HTTPS hosts without a referrer. Story
links open in new tabs without opener access. Markdown fallback images remain
opt-in links.

## Configure sources

Open **Sources** to add, edit, or delete a source. These actions call
`upsert_source` and `delete_source` through the runner. Choose reporting by the
kind of update you want:

- **Major** and **Highlights** make normal feed stories available for agent
  selection within the rolling 24-hour publication window. Undated, invalid,
  stale, and future publication dates are excluded. Unsent stories can recur
  while current.
- **Required** keeps normal feed latest-seen handling without a publication-age
  filter.
- Releases include eligible new GitHub releases. Supply a repository, not a
  custom endpoint URL.
- Schedules produce recurring sports updates. ESPN UFC scoreboards and Riot's
  optional top-two standings filter are supported.

The `rss` feed kind reads both RSS and Atom documents. Feed processing offers
Google News link resolution and title-suffix publisher identification. Only
Required feeds apply saved Google News resolution. Optional feeds retain their
original links. Publisher identification still applies.

In **Selection**, set source preference and title matching. Newer publication
wins duplicate representation before source preference. Lower preference values
break ties. Title groups limit exact normalized headline matching, not identical
absolute URL matching or recent-delivery suppression. These checks do not
establish that two headlines describe the same event.

Source priorities accept the runner's full signed 64-bit integer range. Enter a
decimal integer, not a fraction or exponent. The console preserves priorities as
text, including values beyond JavaScript's exact integer range. Its HTTP API
uses decimal strings for `priority_rank`. The runner protocol remains numeric.
Older clients can still send JSON integers within JavaScript's exact range.
Larger numeric writes are rejected instead of rounded. An omitted priority means
zero.

Old Atom and `always_report` configuration appears through effective current
fields without rewriting stored rows. To save a disabled legacy Observe source,
choose a supported reporting policy explicitly. Historical evidence retains its
original reporting. For other retired settings, follow
[v3 migration](runner-v3-migration.md).

Use **Close**, Escape, or an outside click to discard a dialog's unapplied
changes. The close and save actions stay visible while fields scroll. The
console disables dismissal while a source write is pending.

## Edit publisher rules

Under **Sources**, change a publisher's rule or **Active** checkbox in its row.
Use **Edit** to change its name, aliases, or optional note. Notes are metadata,
not instructions to the runner.

**Allow** and **Watch** both retain matching items and record annotations.
Watch does not create a review queue. **Block** excludes matches. New writes
reject overlapping enabled names or aliases. Existing conflicts remain visible
and keep their old matching order until you resolve them.

Use **Save publishers** to replace the stored collection with your draft.
Use **Discard** to leave stored rules unchanged. All publisher edits belong to
that draft, and rule edits preserve hidden notes. The console uses the normalized
configuration returned by each write rather than guessing what the runner stored.

## Set brief options

Open **Settings** to set `max_delivery_items`, the recurring sports windows, and
the IANA sports time zone. The time-zone control can fill your browser's detected
value, but it does not save until you confirm the change. Configuration and
storage paths appear under **Technical details**.

## Inspect collection and delivery evidence

Open **Activity** to browse required items, candidates, exclusions, annotations,
source checks, and confirmed delivery evidence. Its picker uses the same search
and keyboard controls as Brief. Read [run history](run-history.md) for the
meaning of `delivery_status`, unknown historical outcomes, and fetch counts.

Existing links remain valid: `/deliveries` opens Brief, `/runs` opens Activity,
and `/outlets` opens publisher rules.

## Change server settings

Set environment variables before starting the console:

| Variable | Default | Purpose |
| --- | --- | --- |
| `SIFTWIRE_CONSOLE_BIND` | `127.0.0.1:8790` | Listen address. |
| `SIFTWIRE_CONSOLE_WEB_ROOT` | `apps/web/dist` | Built web assets directory. |
| `SIFTWIRE_CONSOLE_RUNNER_BIN` | `siftwire` | Runner executable to spawn. |
| `SIFTWIRE_CONSOLE_DATABASE` | unset | Optional `--db` override for every call. |
| `SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS` | `30` | Per-invocation tripwire. |

The console has no authentication. Keep the default loopback binding unless you
intend to grant access to a trusted network. For LAN access, configure both the
bind address and host firewall. The Host-header check rejects foreign domain
names to block browser-based DNS rebinding on loopback and LAN bindings. It does
not authenticate users.
