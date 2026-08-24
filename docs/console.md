# Operator Console

`siftwire-console` is a local web application for viewing and configuring a
Siftwire database. It is an operator tool, not part of the runner protocol:
every request spawns the installed `siftwire` binary and uses the `config`
JSON contract or the `runs` operator CLI. The console never opens
SQLite directly.

## Capabilities

- View configured sources and outlet policies.
- Add, edit, and delete sources (writes through `upsert_source` and
  `delete_source`).
- Edit outlet policies and save them as one replacement write.
- Set `max_delivery_items`.
- Browse runs with per-run must-include, candidate, dropped, fetch, and
  delivery evidence.

The console intentionally cannot trigger `run_brief`. A brief run mutates
latest-seen state and is not retry-safe; exposing it behind a dashboard button
would invite accidental duplicate-suppressed runs.

## Running

An installed `siftwire` runner must be on `PATH` (or named through
`SIFTWIRE_CONSOLE_RUNNER_BIN`). Build the console and web assets from a
checkout:

```bash
mise exec -- cargo build --locked --release -p siftwire-console
mise exec -- bun --cwd apps/web build
```

Start the server against the default database:

```bash
SIFTWIRE_CONSOLE_WEB_ROOT=apps/web/dist \
  target/release/siftwire-console
```

The server binds loopback only by default. Sharing beyond loopback is an
explicit operator decision; there is no authentication, so expose it only on a
trusted network. A Host-header check rejects requests addressed through foreign
domain names, which blocks browser-based DNS rebinding against both loopback
and LAN bindings.

```bash
SIFTWIRE_CONSOLE_BIND=192.168.x.x:8790 SIFTWIRE_CONSOLE_WEB_ROOT=apps/web/dist \
  target/release/siftwire-console
```

## Configuration

| Variable                            | Default          | Purpose                                  |
| ----------------------------------- | ---------------- | ---------------------------------------- |
| `SIFTWIRE_CONSOLE_BIND`             | `127.0.0.1:8790` | Listen address.                          |
| `SIFTWIRE_CONSOLE_WEB_ROOT`         | `apps/web/dist`  | Built web assets directory.              |
| `SIFTWIRE_CONSOLE_RUNNER_BIN`       | `siftwire`       | Runner executable to spawn.              |
| `SIFTWIRE_CONSOLE_DATABASE`         | unset            | Optional `--db` override for every call. |
| `SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS` | `30`             | Per-invocation tripwire.                 |

Port 8790 avoids Prooflane's dashboard default of 8787.
