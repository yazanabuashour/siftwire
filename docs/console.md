# Operator console

`siftwire-console` is a local web application for viewing and configuring a
SiftWire database. It is an operator tool, not part of the runner protocol:
every request spawns the installed `siftwire` binary and uses the `config`
JSON contract or the `runs` operator CLI. The console never opens
SQLite directly.

## Capabilities

- Read the latest sent brief and browse recent sent messages.
- View configured sources and publisher rules.
- Add, edit, and delete sources (writes through `upsert_source` and
  `delete_source`). The source editor supports ESPN UFC scoreboards and Riot's
  optional top-two standings filter.
- Edit publisher rules and save them as one replacement write.
- Set `max_delivery_items`, recurring sports windows, and the IANA sports time
  zone. The time-zone control can fill the browser's detected value before an
  explicit save.
- Browse runs with per-run must-include, candidate, dropped, fetch, and
  delivery evidence.

The console intentionally cannot trigger `run_brief`. A brief run mutates
latest-seen state and is not retry-safe; exposing it behind a dashboard button
would invite accidental duplicate-suppressed runs.

## App and email design

The console uses Folio's white reading column, serif headings, violet accent,
and flat lists. Less common configuration fields remain under disclosures.
The existing routes and runner-backed save operations are unchanged. The labels
Publishers, History, and Sent briefs correspond to `/outlets`, `/runs`, and
`/deliveries`.

The brief view renders the recorded message, including sports updates and health
notes. It uses saved story links only when the message is missing. Images in
recorded Markdown appear as links instead of loading remote content automatically.
Configuration and storage paths remain readable in Settings under Technical
details.

`apps/web/src/styles/globals.css` owns the app styling.
`crates/siftwire/src/runner/email.rs` owns the matching email styling. Email
changes affect newly prepared messages, not already stored delivery plans.
The production app contains no comparison gallery, alternate designs, or sample
configuration.

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
and LAN bindings. On this machine the LAN exposure is the systemd user unit
plus one UFW rule scoped to `192.168.0.0/24`.

## Configuration

| Variable                            | Default          | Purpose                                  |
| ----------------------------------- | ---------------- | ---------------------------------------- |
| `SIFTWIRE_CONSOLE_BIND`             | `127.0.0.1:8790` | Listen address.                          |
| `SIFTWIRE_CONSOLE_WEB_ROOT`         | `apps/web/dist`  | Built web assets directory.              |
| `SIFTWIRE_CONSOLE_RUNNER_BIN`       | `siftwire`       | Runner executable to spawn.              |
| `SIFTWIRE_CONSOLE_DATABASE`         | unset            | Optional `--db` override for every call. |
| `SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS` | `30`             | Per-invocation tripwire.                 |

Port 8790 avoids Prooflane's dashboard default of 8787.
