# SiftWire

SiftWire is a local-first brief runtime for agents. Its supported production
surface is one installed `siftwire` JSON runner plus one single-file skill.
Runtime configuration and mutable state stay in the operator's SQLite database,
not in this repository.

## Install

Tell your agent:

```text
Install SiftWire from https://github.com/yazanabuashour/siftwire.
Complete both steps before reporting success:
1. Install and verify the runner with `siftwire --version`.
2. Register skills/siftwire/SKILL.md with the agent's native skill system.
```

Install the latest runner:

```bash
sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/latest/download/install.sh)"
```

Install a pinned runner:

```bash
SIFTWIRE_VERSION=v0.2.0 sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/download/v0.2.0/install.sh)"
```

Set `SIFTWIRE_INSTALL_DIR` to choose another executable directory. The installer
accepts no arguments; these two environment variables are its configuration.

The skill must come from the same tag as the runner. Register it from
`skills/siftwire/SKILL.md`, the matching repository tag, or the release's
`siftwire_<version>_skill.tar.gz`. SiftWire does not require a particular
agent implementation or skill directory.

## Upgrade

Upgrade the runner with the latest installer, verify it, then re-register the
matching skill:

```bash
sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/latest/download/install.sh)"
siftwire --version
```

The unreleased v3 protocol removes legacy writes and unused source options.
Before upgrading a v2 installation, follow [v3 migration](docs/runner-v3-migration.md)
and update process consumers together. Existing public release tags keep their
original contracts.

## First use

The runner reads exactly one JSON request from stdin and writes one JSON result
to stdout:

```bash
printf '%s\n' '{"action":"init"}' | siftwire config
printf '%s\n' '{"action":"inspect_config"}' | siftwire config
printf '%s\n' '{"action":"run_brief","dry_run":true}' | siftwire brief
```

Configuration actions manage RSS/Atom feeds, GitHub releases, sports schedules,
feed processing, publisher policies, and brief options. Brief actions validate
the runtime, collect items, prepare immutable email bodies, and confirm their
accepted delivery for history and repeat suppression. See [`skills/siftwire/SKILL.md`](skills/siftwire/SKILL.md)
for the installed agent policy.

## Operator commands

The installed runner also ships read-only archive commands:

```bash
siftwire runs list [--delivered] [--before run_id] [--search text] [--limit N] [--json] [--db path]
siftwire runs show <run_id> [--candidates --dropped --selected] [--json] [--db path]
```

`runs list` and `runs show` are read-only. Each run persists its required items,
candidates, exclusions, and annotations. `runs show` reports confirmed delivery
outcomes separately from collection decisions and labels older outcomes unknown
when the saved evidence cannot prove them. Use `config.inspect_config` and
`config.upsert_source` for source inspection and approved writes.
Each successfully completed non-dry run persists selection evidence. `--json`
switches to machine-readable output; `runs show --json` returns the complete
run detail regardless of section flags. Its nullable `delivery_html` field
contains the exact saved HTML for a confirmed delivery, or `null` when no
confirmed HTML is available. Reading history never regenerates an email.
See [run history](docs/run-history.md) for archive cursors, search, and evidence.

## Console

The optional local web console has one brief reader with a searchable archive,
source and publisher configuration, and a separate Activity view for recorded
runs. It reads and writes through the runner process contract.
See [`docs/console.md`](docs/console.md).

## Storage

The default database is
`${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`. `XDG_DATA_HOME`
is used only when it is absolute. Select another database with:

- `SIFTWIRE_DATABASE_PATH`
- `--db` for explicit datasets and tests

SiftWire does not support a data-directory variable, workspace state, or
repo-local runtime files. This repository must not contain personal source
inventories, outlet policies, delivery logs, `.openclaw` content, workspace
backups, run history, or local SQLite databases.

## Develop

Install the pinned Rust toolchain and run the repository gate:

```bash
mise install
mise exec -- ./scripts/ci.sh
```

The gate enforces formatting, the repository's strict Rust and Clippy lint
policy, tests, shell checks, skill validation, installer behavior, and a release
build.

Release-document changes also require:

```bash
mise exec -- ./scripts/validate-release-docs.sh <tag>
```

## Documentation

- [`docs/README.md`](docs/README.md): task-oriented documentation index
- [`docs/evals/agent-production.md`](docs/evals/agent-production.md): production
  agent eval protocol
- [`docs/release-verification.md`](docs/release-verification.md): release
  verification
- [`CONTRIBUTING.md`](CONTRIBUTING.md): contribution expectations
- [`SECURITY.md`](SECURITY.md): private vulnerability reporting

Published release assets are immutable. Fix an artifact with a new patch release
rather than replacing an existing tag or asset.
